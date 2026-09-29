//! Native PipeWire graph client. The worker owns all non-Send proxies and its event loop.
//! An explicit runtime socket is opened as an FD; no host/default remote is considered.
use super::{Backend, Sink, Snapshot, Stream};
use libspa::{
    param::ParamType,
    pod::{Object, Property, Value, ValueArray, deserialize::PodDeserializer, serialize::PodSerializer},
};
use pipewire as pw;
use pw::types::ObjectType;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

struct Request {
    action: Action,
    expires: Instant,
    reply: SyncSender<Result<Snapshot, String>>,
}
enum Action {
    Snapshot,
    Set {
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
        allowed: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    },
}

pub struct PipeWireBackend {
    sender: SyncSender<Request>,
    timeout: Duration,
}
impl PipeWireBackend {
    pub fn connect_at(runtime: &Path, remote: &str, timeout: Duration) -> Result<Self, String> {
        if !runtime.is_absolute()
            || remote.is_empty()
            || remote.contains('/')
            || remote == "."
            || remote == ".."
            || timeout.is_zero()
        {
            return Err("PipeWire requires an absolute explicit runtime, socket name and positive timeout".into());
        }
        let socket = runtime.join(remote);
        let (sender, receiver) = mpsc::sync_channel(8);
        thread::Builder::new()
            .name("ssgg-pipewire-graph".into())
            .spawn(move || graph_worker(socket, receiver))
            .map_err(|e| e.to_string())?;
        Ok(Self { sender, timeout })
    }

    pub fn from_environment() -> Result<Self, String> {
        let runtime = std::env::var_os("PIPEWIRE_RUNTIME_DIR")
            .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
            .ok_or("PIPEWIRE_RUNTIME_DIR or XDG_RUNTIME_DIR required")?;
        let remote = std::env::var("PIPEWIRE_REMOTE").unwrap_or_else(|_| "pipewire-0".into());
        Self::connect_at(Path::new(&runtime), &remote, Duration::from_secs(3))
    }
    fn request(&self, action: Action) -> Result<Snapshot, String> {
        let expires = Instant::now() + self.timeout;
        let (reply, received) = mpsc::sync_channel(1);
        self.sender
            .try_send(Request { action, expires, reply })
            .map_err(|e| format!("PipeWire graph queue busy: {e}"))?;
        received
            .recv_timeout(expires.saturating_duration_since(Instant::now()))
            .map_err(|e| format!("PipeWire graph deadline: {e}"))?
    }
}
impl Backend for PipeWireBackend {
    fn backend_name(&self) -> &'static str {
        "Native PipeWire"
    }
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        self.request(Action::Snapshot)
    }
    fn set_stream(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
    ) -> Result<(), String> {
        self.request(Action::Set {
            id,
            volume,
            muted,
            sink,
            allowed: None,
        })
        .map(|_| ())
    }
    fn set_stream_guarded(
        &mut self,
        _id: u32,
        _volume: Option<f64>,
        _muted: Option<bool>,
        _sink: Option<u32>,
        _allowed: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        Err("Native PipeWire async writes require an owned audio guard".into())
    }
    fn set_stream_guarded_owned(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
        allowed: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<(), String> {
        if !allowed() {
            return Err("Physical headset unavailable; audio write cancelled".into());
        }
        self.request(Action::Set {
            id,
            volume,
            muted,
            sink,
            allowed: Some(allowed),
        })
        .map(|_| ())
    }
}

fn graph_worker(socket: PathBuf, receiver: Receiver<Request>) {
    // The FD is connected to precisely the caller's socket; connect_fd_rc cannot
    // silently choose a host runtime or a different remote on failure.
    let graph = (|| -> Result<Graph, String> {
        let stream = UnixStream::connect(&socket).map_err(|e| format!("PipeWire {}: {e}", socket.display()))?;
        pw::init();
        let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|e| e.to_string())?;
        let context = pw::context::ContextRc::new(&mainloop, None).map_err(|e| e.to_string())?;
        let core = context
            .connect_fd_rc(OwnedFd::from(stream), None)
            .map_err(|e| e.to_string())?;
        let server_version = Rc::new(RefCell::new(None::<String>));
        let reported_version = server_version.clone();
        let core_info_listener = core
            .add_listener_local()
            .info(move |info| *reported_version.borrow_mut() = Some(info.version().to_owned()))
            .register();
        Ok(Graph {
            mainloop,
            _context: context,
            _core_info_listener: core_info_listener,
            core,
            server_version,
            known: BTreeMap::new(),
            sink_serials: BTreeMap::new(),
        })
    })();
    match graph {
        Ok(mut graph) => {
            while let Ok(request) = receiver.recv() {
                if Instant::now() >= request.expires {
                    let _ = request
                        .reply
                        .send(Err("PipeWire request expired before execution".into()));
                    continue;
                }
                let result = graph.execute(request.action, request.expires);
                let _ = request.reply.send(result);
            }
        }
        Err(error) => {
            while let Ok(request) = receiver.recv() {
                let _ = request.reply.send(Err(error.clone()));
            }
        }
    }
}
struct Graph {
    mainloop: pw::main_loop::MainLoopRc,
    _context: pw::context::ContextRc,
    _core_info_listener: pw::core::Listener,
    core: pw::core::CoreRc,
    server_version: Rc<RefCell<Option<String>>>,
    known: BTreeMap<u32, (String, Vec<f32>, f64)>,
    sink_serials: BTreeMap<u32, String>,
}
fn gain_from_props(scalar: f64, channels: &[f32]) -> Result<f64, String> {
    if !scalar.is_finite() || scalar < 0.0 || channels.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err("invalid PipeWire playback gain Props".into());
    }
    let raw = if channels.is_empty() {
        scalar
    } else {
        scalar * channels.iter().map(|v| f64::from(*v)).sum::<f64>() / channels.len() as f64
    };
    Ok(raw.cbrt())
}
fn scalar_target_for_gain(requested: f64) -> Result<f64, String> {
    let raw = requested.powi(3);
    if !requested.is_finite() || requested < 0.0 || !raw.is_finite() || !(0.0..=10.0).contains(&raw) {
        return Err("PipeWire scalar playback gain outside native bounds".into());
    }
    Ok(raw)
}
fn channel_targets_for_gain(scalar: f64, channels: &[f32], requested: f64) -> Result<Vec<f32>, String> {
    if !requested.is_finite()
        || !(0.0..=f64::from(0x7fff_ffff_u32) / 65536.0).contains(&requested)
        || !scalar.is_finite()
        || scalar <= 0.0
        || channels.is_empty()
        || channels.iter().any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err("invalid or non-writable PipeWire playback gain".into());
    }
    let mean = channels.iter().map(|v| f64::from(*v)).sum::<f64>() / channels.len() as f64;
    let desired_raw = requested.powi(3) / scalar;
    let targets = if mean > 0.0 {
        channels
            .iter()
            .map(|v| f64::from(*v) * desired_raw / mean)
            .collect::<Vec<_>>()
    } else {
        vec![desired_raw; channels.len()]
    };
    if targets.iter().any(|v| !v.is_finite() || !(0.0..=10.0).contains(v)) {
        return Err("PipeWire channel gain outside native bounds".into());
    }
    Ok(targets.into_iter().map(|v| v as f32).collect())
}
fn confirmed_channel_gain(channels: &[f32], expected: &[f32]) -> bool {
    !channels.is_empty()
        && channels.len() == expected.len()
        && channels.iter().zip(expected).all(|(v, e)| (v - e).abs() < 0.0005)
}
fn link_is_routed(state: pw::link::LinkState<'_>) -> bool {
    matches!(state, pw::link::LinkState::Active | pw::link::LinkState::Paused)
}
fn linked_sink(sinks: &[u32]) -> Result<u32, String> {
    let first = sinks
        .first()
        .copied()
        .ok_or("PipeWire playback stream has no active Link")?;
    if sinks.iter().any(|&sink| sink != first) {
        return Err("PipeWire playback stream has conflicting active Links".into());
    }
    Ok(first)
}
fn routable_sink(linked: &[u32], available: &[u32]) -> Option<u32> {
    linked_sink(linked).ok().filter(|id| available.contains(id))
}
fn supports_route_generation_fence(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    let Some(patch) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    parts.next().is_none() && (major, minor, patch) >= (1, 0, 5)
}
fn same_generation(expected: &str, current: &str) -> Result<(), String> {
    if expected.is_empty() || current != expected {
        return Err("PipeWire stream generation changed; nothing applied".into());
    }
    Ok(())
}

impl Graph {
    fn execute(&mut self, action: Action, expires: Instant) -> Result<Snapshot, String> {
        match action {
            Action::Snapshot => self.inventory(expires),
            Action::Set {
                id,
                volume,
                muted,
                sink,
                allowed,
            } => self.set(id, volume, muted, sink, allowed, expires),
        }
    }

    fn inventory(&mut self, expires: Instant) -> Result<Snapshot, String> {
        let nodes = Rc::new(RefCell::new(BTreeMap::<u32, NodeView>::new()));
        let links = Rc::new(RefCell::new(HashMap::<u32, (u32, u32, bool)>::new()));
        let proxies = Rc::new(RefCell::new(Vec::<(pw::node::Node, pw::node::NodeListener)>::new()));
        let link_proxies = Rc::new(RefCell::new(Vec::<(pw::link::Link, pw::link::LinkListener)>::new()));
        let registry = self.core.get_registry_rc().map_err(|e| e.to_string())?;
        let weak = registry.downgrade();
        let node_views = nodes.clone();
        let link_views = links.clone();
        let node_store = proxies.clone();
        let link_store = link_proxies.clone();
        let removed_links = links.clone();
        let removed_nodes = nodes.clone();
        let _registry_listener = registry
            .add_listener_local()
            .global(move |global| {
                let Some(registry) = weak.upgrade() else { return };
                match global.type_ {
                    ObjectType::Node => {
                        let class = global.props.and_then(|p| p.get("media.class"));
                        if class != Some("Stream/Output/Audio") && class != Some("Audio/Sink") {
                            return;
                        }
                        let Ok(node) = registry.bind::<pw::node::Node, _>(global) else {
                            return;
                        };
                        let id = global.id;
                        let info_views = node_views.clone();
                        let param_views = node_views.clone();
                        let listener = node
                            .add_listener_local()
                            .info(move |info| {
                                let mut views = info_views.borrow_mut();
                                let view = views.entry(id).or_default();
                                if let Some(props) = info.props() {
                                    view.props
                                        .extend(props.iter().map(|(key, value)| (key.to_owned(), value.to_owned())));
                                }
                            })
                            .param(move |_seq, kind, _index, _next, pod| {
                                if kind != ParamType::Props {
                                    return;
                                }
                                if let Some(pod) = pod {
                                    if let Ok((_, Value::Object(object))) =
                                        PodDeserializer::deserialize_from::<Value>(pod.as_bytes())
                                    {
                                        let mut views = param_views.borrow_mut();
                                        if let Some(view) = views.get_mut(&id) {
                                            for property in object.properties {
                                                match property.key {
                                                    libspa::sys::SPA_PROP_volume => {
                                                        if let Value::Float(v) = property.value {
                                                            view.scalar = Some(f64::from(v));
                                                        }
                                                    }
                                                    libspa::sys::SPA_PROP_mute => {
                                                        if let Value::Bool(v) = property.value {
                                                            view.muted = Some(v);
                                                        }
                                                    }
                                                    libspa::sys::SPA_PROP_channelVolumes => {
                                                        if let Value::ValueArray(ValueArray::Float(values)) =
                                                            property.value
                                                        {
                                                            if !values.is_empty() {
                                                                view.channels = values.clone();
                                                            }
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                            })
                            .register();
                        node_store.borrow_mut().push((node, listener));
                    }
                    ObjectType::Link => {
                        let Ok(link) = registry.bind::<pw::link::Link, _>(global) else {
                            return;
                        };
                        let link_id = global.id;
                        let views = link_views.clone();
                        let listener = link
                            .add_listener_local()
                            .info(move |info| {
                                views.borrow_mut().insert(
                                    link_id,
                                    (
                                        info.output_node_id(),
                                        info.input_node_id(),
                                        link_is_routed(info.state()),
                                    ),
                                );
                            })
                            .register();
                        link_store.borrow_mut().push((link, listener));
                    }
                    _ => {}
                }
            })
            .global_remove(move |id| {
                removed_links.borrow_mut().remove(&id);
                removed_nodes.borrow_mut().remove(&id);
            })
            .register();
        self.roundtrip(expires)?;
        for (node, _) in proxies.borrow().iter() {
            node.enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        }
        self.roundtrip(expires)?;
        let views = nodes.borrow();
        let mut sinks = Vec::new();
        let mut sink_serials = BTreeMap::new();
        for (&id, view) in views.iter().filter(|(_, v)| v.get("media.class") == "Audio/Sink") {
            let serial = view.get("object.serial");
            if serial.is_empty() {
                return Err(format!("PipeWire sink {id} has no object.serial"));
            }
            sink_serials.insert(id, serial.to_owned());
            sinks.push(Sink {
                id,
                name: view.get("node.name").into(),
                description: view.get("node.description").into(),
                volume: gain_from_props(
                    view.scalar
                        .ok_or_else(|| format!("PipeWire sink {id} has no Props volume"))?,
                    &view.channels,
                )?,
                muted: view
                    .muted
                    .ok_or_else(|| format!("PipeWire sink {id} has no Props mute"))?,
            });
        }
        let mut streams = Vec::new();
        let mut known = BTreeMap::new();
        for (&id, view) in views
            .iter()
            .filter(|(_, v)| v.get("media.class") == "Stream/Output/Audio")
        {
            let sink_ids: Vec<u32> = links
                .borrow()
                .values()
                .filter(|&&(output, _, routed)| output == id && routed)
                .map(|&(_, sink, _)| sink)
                .collect();
            let available: Vec<u32> = sinks.iter().map(|sink| sink.id).collect();
            let Some(sink_id) = routable_sink(&sink_ids, &available) else {
                // Stream startup and policy moves may briefly have no unique
                // active Link; do not invalidate other apps or disarm ChatMix.
                continue;
            };
            let binary = view.get("application.process.binary");
            let app_name = view.get("application.name");
            let role = view.get("media.role");
            let app_key = if binary.is_empty() && app_name.is_empty() && role.is_empty() {
                format!("anonymous:{}", view.get("object.serial"))
            } else {
                serde_json::to_string(&[binary, app_name, role]).map_err(|e| e.to_string())?
            };
            let scalar = view
                .scalar
                .ok_or_else(|| format!("PipeWire stream {id} has no Props volume"))?;
            let volume = gain_from_props(scalar, &view.channels)?;
            let muted = view
                .muted
                .ok_or_else(|| format!("PipeWire stream {id} has no Props mute"))?;
            let serial = view.get("object.serial");
            if serial.is_empty() {
                return Err(format!("PipeWire stream {id} has no object.serial"));
            }
            known.insert(id, (serial.to_owned(), view.channels.clone(), scalar));
            streams.push(Stream {
                id,
                app_key,
                generation: Some(serial.to_owned()),
                name: view.get("media.name").into(),
                app_name: app_name.into(),
                volume,
                effective_volume: volume,
                muted,
                effective_muted: muted,
                group: "unmanaged".into(),
                sink_id,
            });
        }
        self.known = known;
        self.sink_serials = sink_serials;
        Ok(Snapshot { streams, sinks })
    }

    fn set(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
        allowed: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
        expires: Instant,
    ) -> Result<Snapshot, String> {
        if volume.is_some_and(|v| !v.is_finite() || !(0.0..=f64::from(0x7fff_ffff_u32) / 65536.0).contains(&v)) {
            return Err("invalid native PipeWire gain".into());
        }
        let expected = self.known.get(&id).map(|(serial, _, _)| serial.clone());
        let before = self.inventory(expires)?;
        let previous = before
            .streams
            .iter()
            .find(|s| s.id == id)
            .ok_or("PipeWire playback stream disappeared")?;
        let (serial, channels, scalar) = self
            .known
            .get(&id)
            .cloned()
            .ok_or("PipeWire stream identity unavailable")?;
        same_generation(
            expected.as_deref().ok_or("PipeWire stream had no prior snapshot")?,
            &serial,
        )?;
        if let Some(sink_id) = sink
            && sink_id != previous.sink_id
        {
            let sink_serial = self
                .sink_serials
                .get(&sink_id)
                .ok_or("PipeWire destination sink disappeared")?
                .clone();
            self.move_stream(id, &serial, sink_id, &sink_serial, allowed.as_ref(), expires)?;
        }
        if volume.is_none() && muted.is_none() {
            return self.inventory(expires);
        }

        let registry = self.core.get_registry_rc().map_err(|e| e.to_string())?;
        let weak = registry.downgrade();
        let node = Rc::new(RefCell::new(None::<pw::node::Node>));
        let found = node.clone();
        let bound_serial = serial.clone();
        let _listener = registry
            .add_listener_local()
            .global(move |global| {
                if global.id == id
                    && global.type_ == ObjectType::Node
                    && global.props.and_then(|p| p.get("media.class")) == Some("Stream/Output/Audio")
                    && global.props.and_then(|p| p.get("object.serial")) == Some(bound_serial.as_str())
                {
                    if let Some(registry) = weak.upgrade() {
                        *found.borrow_mut() = registry.bind(global).ok();
                    }
                }
            })
            .register();
        self.roundtrip(expires)?;
        let node = node
            .borrow_mut()
            .take()
            .ok_or("PipeWire stream generation changed; nothing applied")?;
        let target_channels = if channels.is_empty() {
            None
        } else {
            volume
                .map(|value| channel_targets_for_gain(scalar, &channels, value))
                .transpose()?
        };
        let target_scalar = if channels.is_empty() {
            volume.map(scalar_target_for_gain).transpose()?
        } else {
            None
        };
        let mut properties = Vec::new();
        if let Some(value) = target_scalar {
            properties.push(Property::new(libspa::sys::SPA_PROP_volume, Value::Float(value as f32)));
        }
        if let Some(values) = &target_channels {
            properties.push(Property::new(
                libspa::sys::SPA_PROP_channelVolumes,
                Value::ValueArray(ValueArray::Float(values.clone())),
            ));
        }
        if let Some(value) = muted {
            properties.push(Property::new(libspa::sys::SPA_PROP_mute, Value::Bool(value)));
        }
        if Instant::now() >= expires {
            return Err("PipeWire write expired before execution".into());
        }
        if !properties.is_empty() {
            let value = Value::Object(Object {
                type_: libspa::sys::SPA_TYPE_OBJECT_Props,
                id: libspa::sys::SPA_PARAM_Props,
                properties,
            });
            let (bytes, _) = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &value)
                .map_err(|e| format!("PipeWire Props encode: {e}"))?;
            let bytes = bytes.into_inner();
            let pod = libspa::pod::Pod::from_bytes(&bytes).ok_or("PipeWire Props pod malformed")?;
            if allowed.as_ref().is_some_and(|guard| !guard()) {
                return Err("Physical headset unavailable; native audio write cancelled".into());
            }
            if Instant::now() >= expires {
                return Err("PipeWire write expired before execution".into());
            }
            node.set_param(ParamType::Props, 0, pod);
        }
        self.roundtrip(expires)
            .map_err(|e| format!("PipeWire post-write barrier: {e}"))?;
        loop {
            let after = self
                .inventory(expires)
                .map_err(|e| format!("PipeWire post-write inventory: {e}"))?;
            if self.known.get(&id).map(|(s, _, _)| s.as_str()) != Some(serial.as_str()) {
                return Err("PipeWire stream generation changed before readback".into());
            }
            let current = after
                .streams
                .iter()
                .find(|s| s.id == id)
                .ok_or("PipeWire stream disappeared after write")?;
            let channels_ok = target_channels.as_ref().is_none_or(|targets| {
                self.known.get(&id).is_some_and(|(_, channel_values, actual_scalar)| {
                    (actual_scalar - scalar).abs() < 0.001 && confirmed_channel_gain(channel_values, targets)
                })
            });
            let scalar_ok = target_scalar.is_none_or(|target| {
                self.known.get(&id).is_some_and(|(_, channel_values, actual)| {
                    channel_values.is_empty() && (actual - target).abs() < 0.001
                })
            });
            let mute_ok = muted.is_none_or(|v| current.effective_muted == v);
            if channels_ok && scalar_ok && mute_ok {
                return Ok(after);
            }
            if Instant::now() >= expires {
                return Err(format!(
                    "PipeWire stream readback disagreed with request (prior gain {}, prior mute {})",
                    previous.effective_volume, previous.effective_muted
                ));
            }
        }
    }
    fn move_stream(
        &mut self,
        id: u32,
        serial: &str,
        target: u32,
        sink_serial: &str,
        allowed: Option<&Arc<dyn Fn() -> bool + Send + Sync>>,
        expires: Instant,
    ) -> Result<(), String> {
        let registry = self.core.get_registry_rc().map_err(|e| e.to_string())?;
        let weak = registry.downgrade();
        let default = Rc::new(RefCell::new(None::<pw::metadata::Metadata>));
        let found = default.clone();
        let ambiguous = Rc::new(Cell::new(false));
        let duplicate = ambiguous.clone();
        let _listener = registry
            .add_listener_local()
            .global(move |global| {
                if global.type_ == ObjectType::Metadata
                    && global.props.and_then(|p| p.get("metadata.name")) == Some("default")
                {
                    if found.borrow().is_some() {
                        duplicate.set(true);
                    } else if let Some(registry) = weak.upgrade() {
                        *found.borrow_mut() = registry.bind(global).ok();
                    }
                }
            })
            .register();
        self.roundtrip(expires)?;
        if ambiguous.get() {
            return Err("PipeWire has multiple default metadata owners; routing refused".into());
        }
        let metadata = default
            .borrow_mut()
            .take()
            .ok_or("PipeWire default metadata unavailable; no routing command sent")?;
        // Drain graph events again at the final command boundary. A queued
        // move must not act on an ID whose stream or sink serial was replaced.
        self.inventory(expires)?;
        if self.known.get(&id).map(|(current, _, _)| current.as_str()) != Some(serial)
            || self.sink_serials.get(&target).map(String::as_str) != Some(sink_serial)
        {
            return Err("PipeWire stream or sink generation changed; no routing command sent".into());
        }
        if !self
            .server_version
            .borrow()
            .as_deref()
            .is_some_and(supports_route_generation_fence)
        {
            return Err("Native routing requires a verified PipeWire registry-generation fence (1.0.5+)".into());
        }
        // Our final registry roundtrip observed both globals. In PipeWire 1.0.5+
        // metadata_set_property checks the subject through the client's received
        // registry generation (impl-client.c); a newer recycled global is ESTALE.
        // This is not an atomic object.serial compare-and-set on all servers.
        if allowed.is_some_and(|guard| !guard()) || Instant::now() >= expires {
            return Err(
                "Physical headset unavailable or native route deadline expired; no routing command sent".into(),
            );
        }
        metadata.set_property(id, "target.object", Some("Spa:String"), Some(sink_serial));
        self.roundtrip(expires)
            .map_err(|e| format!("PipeWire route barrier: {e}"))?;
        loop {
            if Instant::now() >= expires {
                return Err("PipeWire native route not confirmed by active Link before deadline".into());
            }
            match self.inventory(expires) {
                Ok(after) => {
                    if self.known.get(&id).is_some_and(|(current, _, _)| current != serial) {
                        return Err("PipeWire stream generation changed during route".into());
                    }
                    if after
                        .streams
                        .iter()
                        .find(|s| s.id == id)
                        .is_some_and(|s| s.sink_id == target)
                    {
                        return Ok(());
                    }
                }
                Err(error) if error.contains("no active Link") || error.contains("conflicting active Links") => {
                    // WirePlumber removes old links before it creates the new ones.
                }
                Err(error) => return Err(error),
            }
            self.mainloop
                .loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
        }
    }
    fn roundtrip(&self, expires: Instant) -> Result<(), String> {
        let done = Rc::new(Cell::new(false));
        let ready = done.clone();
        let error = Rc::new(RefCell::new(None::<String>));
        let failed = error.clone();
        let sequence = self.core.sync(0).map_err(|e| e.to_string())?;
        let _listener = self
            .core
            .add_listener_local()
            .done(move |id, seq| {
                if id == 0 && seq == sequence {
                    ready.set(true)
                }
            })
            .error(move |_, _, _, message| *failed.borrow_mut() = Some(message.into()))
            .register();
        while !done.get() {
            if let Some(reason) = error.borrow_mut().take() {
                return Err(format!("PipeWire core: {reason}"));
            }
            if Instant::now() >= expires {
                return Err("PipeWire graph roundtrip deadline".into());
            }
            self.mainloop
                .loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
        }
        Ok(())
    }
}
#[derive(Default)]
struct NodeView {
    props: BTreeMap<String, String>,
    scalar: Option<f64>,
    muted: Option<bool>,
    channels: Vec<f32>,
}
impl NodeView {
    fn get(&self, key: &str) -> &str {
        self.props.get(key).map(String::as_str).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_metadata_routing_requires_verified_registry_generation_fence() {
        assert!(super::supports_route_generation_fence("1.0.5"));
        assert!(super::supports_route_generation_fence("1.6.2"));
        assert!(!super::supports_route_generation_fence("1.0.4"));
        assert!(!super::supports_route_generation_fence("0.3.80"));
        assert!(!super::supports_route_generation_fence("unknown"));
    }

    #[test]
    fn scalar_only_playback_props_are_valid_and_writable() {
        assert!((super::gain_from_props(0.125, &[]).unwrap() - 0.5).abs() < 0.0001);
        assert!((super::scalar_target_for_gain(0.8).unwrap() - 0.512).abs() < 0.0001);
        assert!(super::scalar_target_for_gain(3.0).is_err());
    }

    #[test]
    fn pulse_compatible_gain_is_cubic_root_of_native_channel_product() {
        assert!((super::gain_from_props(1.0, &[0.125, 0.125]).unwrap() - 0.5).abs() < 0.0001);
        assert!((super::gain_from_props(1.5, &[0.5, 0.5]).unwrap() - 0.75_f64.cbrt()).abs() < 0.0001);
    }

    #[test]
    fn native_gain_write_preserves_channel_ratio_and_handles_zero() {
        let changed = super::channel_targets_for_gain(1.0, &[0.125, 0.25], 0.8).unwrap();
        assert!((f64::from(changed[1]) / f64::from(changed[0]) - 2.0).abs() < 0.001);
        assert!((super::gain_from_props(1.0, &changed).unwrap() - 0.8).abs() < 0.001);
        let restored = super::channel_targets_for_gain(1.0, &[0.0, 0.0], 1.0).unwrap();
        assert_eq!(restored, vec![1.0, 1.0]);
        assert!(super::channel_targets_for_gain(0.0, &[0.0, 0.0], 1.0).is_err());
    }

    #[test]
    fn refusing_a_recycled_node_id_preserves_original_stream_generation() {
        assert!(super::same_generation("71", "72").is_err());
        assert!(super::same_generation("71", "71").is_ok());
    }

    #[test]
    fn uneven_channels_require_matching_individual_readback() {
        assert!(!super::confirmed_channel_gain(&[0.2, 0.6], &[0.4, 0.4]));
        assert!(super::confirmed_channel_gain(&[0.2, 0.6], &[0.2, 0.6]));
    }

    #[test]
    fn unlinked_or_ambiguous_playback_is_omitted_without_poisoning_other_streams() {
        assert_eq!(super::routable_sink(&[], &[9, 10]), None);
        assert_eq!(super::routable_sink(&[9, 10], &[9, 10]), None);
        assert_eq!(super::routable_sink(&[8], &[9, 10]), None);
        assert_eq!(super::routable_sink(&[9, 9], &[9, 10]), Some(9));
    }

    #[test]
    fn route_without_exactly_one_link_is_not_reported_as_effective() {
        assert!(super::linked_sink(&[]).is_err());
        assert!(super::linked_sink(&[19, 21]).is_err());
        assert_eq!(super::linked_sink(&[19, 19]).unwrap(), 19);
    }

    #[test]
    fn unlinked_and_errored_links_cannot_claim_effective_routing() {
        use pipewire::link::LinkState;
        assert!(!super::link_is_routed(LinkState::Unlinked));
        assert!(!super::link_is_routed(LinkState::Error("disconnected")));
        assert!(super::link_is_routed(LinkState::Active));
        assert!(super::link_is_routed(LinkState::Paused));
    }
}
