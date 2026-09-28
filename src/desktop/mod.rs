//! Desktop-only audio policy. Construction and inventory are strictly read-only.
pub mod devices;
pub mod hardware;
pub mod lighting;
pub mod pulse;
pub mod rpc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    pub id: u32,
    pub app_key: String,
    pub name: String,
    pub app_name: String,
    pub volume: f64,
    pub effective_volume: f64,
    pub effective_muted: bool,
    pub muted: bool,
    pub group: String,
    pub sink_id: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sink {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub volume: f64,
    pub muted: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub streams: Vec<Stream>,
    pub sinks: Vec<Sink>,
}
pub trait Backend {
    fn begin_cycle(&mut self) {}
    fn snapshot(&mut self) -> Result<Snapshot, String>;
    fn set_stream(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
    ) -> Result<(), String>;
    fn set_stream_guarded(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
        allowed: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        if !allowed() {
            return Err("Physical headset unavailable; audio write cancelled".into());
        }
        self.set_stream(id, volume, muted, sink)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub volume: f64,
    pub muted: bool,
    pub wheel_side: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mixer {
    pub balance: f64,
    pub enabled: bool,
    #[serde(default)]
    pub input_mode: hardware::InputMode,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub start_minimized: bool,
    pub close_to_tray: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            start_minimized: false,
            close_to_tray: true,
        }
    }
}
fn groups() -> Vec<Group> {
    [("game", "Game", "a"), ("chat", "Chat", "b"), ("media", "Media", "none")]
        .into_iter()
        .map(|(id, name, side)| Group {
            id: id.into(),
            name: name.into(),
            volume: 1.0,
            muted: false,
            wheel_side: side.into(),
        })
        .collect()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Assignment {
    group: String,
    volume: f64,
    muted: bool,
    sink: Option<String>,
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct RpcError {
    code: &'static str,
    message: String,
}
impl RpcError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new("INVALID_PARAMS", message)
    }
    fn backend(message: String) -> Self {
        Self::new("BACKEND_UNAVAILABLE", message)
    }
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, RpcError> {
    serde_json::from_value(v).map_err(|e| RpcError::invalid(e.to_string()))
}
fn gain(v: Option<f64>, min: f64) -> Result<(), RpcError> {
    if v.is_some_and(|v| !v.is_finite() || !(min..=1.0).contains(&v)) {
        return Err(RpcError::invalid("gain outside allowed range"));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StreamSet {
    id: u32,
    volume: Option<f64>,
    muted: Option<bool>,
    group: Option<String>,
    sink_id: Option<u32>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Profile {
    groups: Vec<Group>,
    mixer: Mixer,
    assignments: std::collections::BTreeMap<String, Assignment>,
}
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    settings: Settings,
    current: Profile,
    profiles: std::collections::BTreeMap<String, Profile>,
    #[serde(default)]
    applied: std::collections::BTreeMap<u32, Applied>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Applied {
    key: String,
    base: f64,
    base_mute: bool,
    expected: f64,
    expected_mute: bool,
    factor: f64,
    // Err may follow a successful volume write. Until retry, actual state is
    // not an external edit and must not replace the requested base intent.
    #[serde(default)]
    pending: bool,
}
pub struct Service<B: Backend> {
    pub backend: B,
    snapshot: Snapshot,
    groups: Vec<Group>,
    mixer: Mixer,
    settings: Settings,
    physical: hardware::Physical,
    hardware: hardware::Controller,
    lighting: lighting::AsyncController,
    hardware_dirty: bool,
    assignments: std::collections::BTreeMap<String, Assignment>,
    path: PathBuf,
    profiles: std::collections::BTreeMap<String, Profile>,
    armed: bool,
    read_only: bool,
    _lock: std::fs::File,
    applied: std::collections::BTreeMap<u32, Applied>,
}
impl<B: Backend> Service<B> {
    pub fn new(backend: B, path: PathBuf) -> Result<Self, String> {
        Self::with_hardware(backend, path, hardware::Controller::default())
    }
    pub fn with_hardware(backend: B, path: PathBuf, hardware: hardware::Controller) -> Result<Self, String> {
        Self::with_controllers(backend, path, hardware, lighting::Controller::default())
    }
    pub fn with_controllers(
        backend: B,
        path: PathBuf,
        hardware: hardware::Controller,
        lighting: lighting::Controller,
    ) -> Result<Self, String> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        let parent = path.parent().ok_or("config path has no parent")?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|e| e.to_string())?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("another desktop service owns config: {e}"))?;
        let stored = match std::fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 1024 * 1024 {
                    return Err("desktop config exceeds 1 MiB".into());
                }
                Some(serde_json::from_slice::<Stored>(&bytes).map_err(|e| format!("invalid desktop config: {e}"))?)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let mut service = Self {
            backend,
            snapshot: Snapshot::default(),
            groups: groups(),
            mixer: Mixer::default(),
            settings: Settings::default(),
            physical: hardware::Physical::default(),
            hardware,
            lighting: lighting::AsyncController::new(lighting),
            hardware_dirty: false,
            assignments: Default::default(),
            applied: Default::default(),
            path,
            profiles: Default::default(),
            armed: false,
            read_only: false,
            _lock: lock,
        };
        if let Some(stored) = stored {
            if stored.version != 1 {
                return Err("unsupported desktop config version".into());
            }
            service.settings = stored.settings;
            service.groups = stored.current.groups;
            service.mixer = stored.current.mixer;
            service.assignments = stored.current.assignments;
            service.profiles = stored.profiles;
            service.applied = stored.applied;
        }
        // Resuming a GUI or service must never automatically enable attenuation.
        service.mixer.enabled = false;
        Ok(service)
    }
    /// Reconcile restarted opted-in apps. Never arms itself on service startup.
    pub fn tick(&mut self) -> Result<(), String> {
        self.lighting.poll();
        self.backend.begin_cycle();
        self.sync_hardware();
        self.refresh().map_err(|e| e.message)?;
        self.sync_hardware();
        if self.hardware_dirty
            && self.mixer.enabled
            && self.mixer.input_mode == hardware::InputMode::Hardware
            && !self.read_only
        {
            self.apply(None).map_err(|e| e.message)?;
            self.persist().map_err(|e| e.message)?;
            self.hardware_dirty = false;
        }
        if !self.armed {
            return Ok(());
        }
        let mut changed = false;
        for i in 0..self.snapshot.streams.len() {
            let stream = &self.snapshot.streams[i];
            if self.applied.get(&stream.id).is_some_and(|a| a.pending) {
                self.apply(Some(stream.id)).map_err(|e| e.message)?;
                changed = true;
                continue;
            }
            if self.applied.contains_key(&stream.id) {
                continue;
            }
            if self.assignments.contains_key(&stream.app_key) {
                let id = stream.id;
                self.apply(Some(id)).map_err(|e| e.message)?;
                changed = true;
            }
        }
        if changed {
            self.persist().map_err(|e| e.message)?;
        }
        Ok(())
    }
    fn profile(&self) -> Profile {
        Profile {
            groups: self.groups.clone(),
            mixer: self.mixer.clone(),
            assignments: self.assignments.clone(),
        }
    }
    fn profile_names(&self) -> Vec<Value> {
        self.profiles.keys().map(|name| json!({"name":name})).collect()
    }
    fn persist(&self) -> Result<(), RpcError> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let io = |e: std::io::Error| RpcError::new("IO_ERROR", e.to_string());
        let bytes = serde_json::to_vec_pretty(&Stored {
            version: 1,
            settings: self.settings.clone(),
            current: self.profile(),
            profiles: self.profiles.clone(),
            applied: self.applied.clone(),
        })
        .map_err(|e| RpcError::new("IO_ERROR", e.to_string()))?;
        let temp = self.path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temp)
            .map_err(io)?;
        file.write_all(&bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        std::fs::rename(&temp, &self.path).map_err(io)?;
        Ok(())
    }
    pub fn set_read_only(&mut self, value: bool) {
        self.read_only = value;
        if value {
            self.lighting.cancel();
            self.hardware.stop();
            self.armed = false;
            self.mixer.enabled = false;
            self.sync_hardware();
        }
    }
    fn sync_hardware(&mut self) {
        let physical = self.hardware.snapshot();
        let gains = |sample: Option<hardware::Sample>| sample.map(|s| (s.game_percent, s.chat_percent));
        if gains(physical.sample) != gains(self.physical.sample) {
            self.hardware_dirty = true;
        }
        if physical.sample.is_none() && self.mixer.input_mode == hardware::InputMode::Hardware {
            self.mixer.enabled = false;
            self.armed = false;
        }
        self.physical = physical;
    }
    pub fn request(&mut self, request: Value) -> Value {
        self.backend.begin_cycle();
        self.sync_hardware();
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let result = match request["method"].as_str() {
            Some(method) => self.dispatch(method, request.get("params").cloned().unwrap_or(json!({}))),
            None => Err(RpcError::new("INVALID_REQUEST", "method must be a string")),
        };
        match result {
            Ok(result) => json!({"id":id,"result":result}),
            Err(e) => json!({"id":id,"error":{"code":e.code,"message":e.message}}),
        }
    }
    fn refresh(&mut self) -> Result<(), RpcError> {
        self.snapshot = match self.backend.snapshot() {
            Ok(snapshot) => snapshot,
            Err(e) => {
                self.snapshot = Snapshot::default();
                // A failed inventory is not evidence that our effective gains disappeared.
                // Retire tracking only after a successful snapshot proves the stream is gone.
                self.armed = false;
                self.mixer.enabled = false;
                return Err(RpcError::backend(e));
            }
        };
        self.applied
            .retain(|id, a| self.snapshot.streams.iter().any(|s| s.id == *id && s.app_key == a.key));
        let mut external_edits = Vec::new();
        for stream in &mut self.snapshot.streams {
            if let Some(a) = self.assignments.get(&stream.app_key) {
                stream.group = a.group.clone();
            }
            if let Some(a) = self.applied.get_mut(&stream.id) {
                if a.key == stream.app_key {
                    let volume_changed = !a.pending && (stream.effective_volume - a.expected).abs() >= 0.0001;
                    let mute_changed = !a.pending && stream.muted != a.expected_mute;
                    if volume_changed {
                        a.base = if a.factor > 0.0 {
                            (stream.effective_volume / a.factor).clamp(0.0, f64::from(0x7fff_ffff_u32) / 65536.0)
                        } else {
                            stream.effective_volume
                        };
                        a.expected = stream.effective_volume;
                    }
                    if mute_changed {
                        a.base_mute = stream.muted;
                        a.expected_mute = stream.muted;
                    }
                    stream.volume = a.base;
                    stream.muted = a.base_mute;
                    if volume_changed || mute_changed {
                        external_edits.push((
                            stream.app_key.clone(),
                            volume_changed.then_some(a.base),
                            mute_changed.then_some(a.base_mute),
                        ));
                    }
                }
            }
        }
        // Explicit stream.set owns the app default. Only a newly observed,
        // unambiguous external edit can supersede it, never unchanged siblings
        // or mixing. Conflicting simultaneous edits keep the previous default.
        for (key, assignment) in &mut self.assignments {
            let mut volumes = external_edits.iter().filter(|e| &e.0 == key).filter_map(|e| e.1);
            if let Some(v) = volumes.next() {
                if volumes.all(|other| other == v) {
                    assignment.volume = v;
                }
            }
            let mut mutes = external_edits.iter().filter(|e| &e.0 == key).filter_map(|e| e.2);
            if let Some(m) = mutes.next() {
                if mutes.all(|other| other == m) {
                    assignment.muted = m;
                }
            }
        }
        for stream in &mut self.snapshot.streams {
            if !self.applied.contains_key(&stream.id) {
                if let Some(a) = self.assignments.get(&stream.app_key) {
                    stream.volume = a.volume;
                    stream.muted = a.muted;
                }
            }
        }
        Ok(())
    }
    fn factor(&self, group: &str) -> f64 {
        let Some(g) = self.groups.iter().find(|g| g.id == group) else {
            return 1.0;
        };
        let wheel = if !self.mixer.enabled {
            1.0
        } else if self.mixer.input_mode == hardware::InputMode::Hardware {
            match (g.wheel_side.as_str(), self.physical.sample) {
                ("a", Some(s)) => f64::from(s.game_percent) / 100.0,
                ("b", Some(s)) => f64::from(s.chat_percent) / 100.0,
                _ => 1.0,
            }
        } else {
            match g.wheel_side.as_str() {
                "a" => 1.0 - self.mixer.balance.max(0.0),
                "b" => 1.0 + self.mixer.balance.min(0.0),
                _ => 1.0,
            }
        };
        g.volume * wheel
    }
    fn apply(&mut self, only: Option<u32>) -> Result<(), RpcError> {
        let hardware_mix = self.mixer.enabled && self.mixer.input_mode == hardware::InputMode::Hardware;
        let guard: std::sync::Arc<dyn Fn() -> bool + Send + Sync> = if hardware_mix {
            self.hardware.audio_write_guard()
        } else {
            std::sync::Arc::new(|| true)
        };
        self.armed = true;
        for stream in self.snapshot.streams.clone() {
            if !guard() {
                self.sync_hardware();
                return Err(RpcError::backend(
                    "Physical headset unavailable; remaining audio writes cancelled".into(),
                ));
            }
            if only.is_some_and(|id| id != stream.id)
                || (only.is_none() && stream.group == "unmanaged" && !self.applied.contains_key(&stream.id))
            {
                continue;
            }
            // New stream restoration is shared by tick and explicit mutations.
            // Leave it untracked on a route failure so either path can retry.
            if !self.applied.contains_key(&stream.id) {
                if let Some(name) = self.assignments.get(&stream.app_key).and_then(|a| a.sink.as_ref()) {
                    if let Some(sink) = self.snapshot.sinks.iter().find(|s| &s.name == name) {
                        if sink.id != stream.sink_id {
                            self.backend
                                .set_stream_guarded(stream.id, None, None, Some(sink.id), &*guard)
                                .map_err(RpcError::backend)?;
                        }
                    }
                }
            }
            let volume = stream.volume * self.factor(&stream.group);
            let muted = stream.muted || self.groups.iter().any(|g| g.id == stream.group && g.muted);
            let gain = if (volume - stream.effective_volume).abs() >= 0.0001 {
                Some(volume)
            } else {
                None
            };
            let mute = if muted != stream.effective_muted {
                Some(muted)
            } else {
                None
            };
            self.applied.insert(
                stream.id,
                Applied {
                    factor: self.factor(&stream.group),
                    key: stream.app_key,
                    base: stream.volume,
                    base_mute: stream.muted,
                    expected: volume,
                    expected_mute: muted,
                    pending: true,
                },
            );
            if gain.is_some() || mute.is_some() {
                self.backend
                    .set_stream_guarded(stream.id, gain, mute, None, &*guard)
                    .map_err(RpcError::backend)?;
            }
            if let Some(a) = self.applied.get_mut(&stream.id) {
                a.pending = false;
            }
        }
        Ok(())
    }
    fn state(&mut self) -> Value {
        let error = self.refresh().err().map(|e| e.message);
        let (mut devices, device_error) = match devices::inventory() {
            Ok(d) => (devices::with_physical(d, &self.physical), None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        self.lighting.decorate(&mut devices);
        json!({"streams":self.snapshot.streams,"sinks":self.snapshot.sinks,"groups":self.groups,"mixer":self.mixer,"physical":self.physical,"settings":self.settings,"profiles":self.profile_names(),"devices":devices,"deviceError":device_error,"backend":{"name":"PulseAudio / PipeWire-Pulse","connected":error.is_none(),"error":error}})
    }
    fn dispatch(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        if self.read_only
            && [
                "stream.set",
                "group.set",
                "chatmix.set",
                "profiles.apply",
                "device.set",
                "lighting.apply",
            ]
            .contains(&method)
        {
            return Err(RpcError::new(
                "UNSUPPORTED",
                "Audio/HID mutations disabled by --safe-mode",
            ));
        }
        match method {
            "state.get" => Ok(self.state()),
            "lighting.apply" => {
                let p: lighting::Apply = decode(params)?;
                p.validate().map_err(RpcError::invalid)?;
                self.lighting
                    .queue(p)
                    .map(|()| json!({"pending":true}))
                    .map_err(|e| RpcError::new("HARDWARE_UNAVAILABLE", e))
            }
            "stream.set" => {
                let p: StreamSet = decode(params)?;
                gain(p.volume, 0.0)?;
                if p.group
                    .as_ref()
                    .is_some_and(|g| !["game", "chat", "media", "unmanaged"].contains(&g.as_str()))
                {
                    return Err(RpcError::invalid("unknown group"));
                }
                let hardware_mix = self.mixer.enabled && self.mixer.input_mode == hardware::InputMode::Hardware;
                let guard = hardware_mix.then(|| self.hardware.audio_write_guard());
                self.refresh()?;
                if guard.as_ref().is_some_and(|g| !g()) {
                    self.sync_hardware();
                    return Err(RpcError::backend(
                        "Physical headset unavailable; audio write cancelled".into(),
                    ));
                }
                let stream = self
                    .snapshot
                    .streams
                    .iter()
                    .find(|s| s.id == p.id)
                    .ok_or_else(|| RpcError::new("NOT_FOUND", "stream disappeared"))?
                    .clone();
                let sink = match p.sink_id {
                    Some(id) => Some(
                        self.snapshot
                            .sinks
                            .iter()
                            .find(|s| s.id == id)
                            .ok_or_else(|| RpcError::new("NOT_FOUND", "sink disappeared"))?
                            .name
                            .clone(),
                    ),
                    None => self.assignments.get(&stream.app_key).and_then(|a| a.sink.clone()),
                };
                if p.sink_id.is_some() {
                    self.backend
                        .set_stream_guarded(p.id, None, None, p.sink_id, &|| guard.as_ref().is_none_or(|g| g()))
                        .map_err(RpcError::backend)?;
                }
                let assignment = Assignment {
                    group: p.group.unwrap_or(stream.group),
                    volume: p.volume.unwrap_or(stream.volume),
                    muted: p.muted.unwrap_or(stream.muted),
                    sink,
                };
                for s in &mut self.snapshot.streams {
                    if s.id == p.id {
                        s.volume = assignment.volume;
                        s.muted = assignment.muted;
                        s.group = assignment.group.clone();
                    }
                }
                self.assignments.insert(stream.app_key, assignment);
                self.apply(Some(p.id))?;
                self.persist()?;
                Ok(self.state())
            }
            "group.set" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct P {
                    id: String,
                    volume: Option<f64>,
                    muted: Option<bool>,
                    wheel_side: Option<String>,
                }
                let p: P = decode(params)?;
                gain(p.volume, 0.0)?;
                if p.wheel_side
                    .as_ref()
                    .is_some_and(|s| !["a", "b", "none"].contains(&s.as_str()))
                {
                    return Err(RpcError::invalid("unknown wheel side"));
                }
                self.refresh()?;
                let g = self
                    .groups
                    .iter_mut()
                    .find(|g| g.id == p.id)
                    .ok_or_else(|| RpcError::invalid("unknown group"))?;
                if let Some(v) = p.volume {
                    g.volume = v;
                }
                if let Some(v) = p.muted {
                    g.muted = v;
                }
                if let Some(v) = p.wheel_side {
                    g.wheel_side = v;
                }
                self.apply(None)?;
                self.persist()?;
                Ok(self.state())
            }
            "chatmix.set" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct P {
                    balance: Option<f64>,
                    enabled: Option<bool>,
                    input_mode: Option<hardware::InputMode>,
                }
                let p: P = decode(params)?;
                gain(p.balance, -1.0)?;
                let mode = p.input_mode.unwrap_or(self.mixer.input_mode);
                if mode == hardware::InputMode::Hardware
                    && p.enabled.unwrap_or(self.mixer.enabled)
                    && self.physical.sample.is_none()
                {
                    return Err(RpcError::new(
                        "HARDWARE_UNAVAILABLE",
                        "Acquire hardware and wait for a fresh connected sample first",
                    ));
                }
                self.refresh()?;
                self.mixer.input_mode = mode;
                if let Some(v) = p.balance {
                    self.mixer.balance = v;
                }
                if let Some(v) = p.enabled {
                    self.mixer.enabled = v;
                }
                self.apply(None)?;
                self.persist()?;
                Ok(self.state())
            }
            "devices.list" => {
                let mut entries = devices::with_physical(
                    devices::inventory().map_err(|e| RpcError::new("IO_ERROR", e.to_string()))?,
                    &self.physical,
                );
                self.lighting.decorate(&mut entries);
                Ok(json!(entries))
            }
            "device.set" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct P {
                    id: String,
                    hardware_enabled: Option<bool>,
                    sidetone: Option<u8>,
                    auto_off_minutes: Option<u8>,
                    status_refresh: Option<bool>,
                }
                let p: P = decode(params)?;
                let command = hardware::SettingsCommand {
                    sidetone: p.sidetone,
                    auto_off_minutes: p.auto_off_minutes,
                    status_refresh: p.status_refresh.unwrap_or(false),
                };
                if p.sidetone.is_some_and(|v| v > 3) || (p.hardware_enabled == Some(false) && !command.is_empty()) {
                    return Err(RpcError::invalid(
                        "Sidetone must be 0..3; release cannot be combined with commands",
                    ));
                }
                if !p.id.starts_with("1038:227e:") {
                    return Err(RpcError::new("UNSUPPORTED", "Unverified model; no commands sent"));
                }
                if p.hardware_enabled != Some(true) && self.physical.device_id.as_deref() != Some(&p.id) {
                    return Err(RpcError::new(
                        "HARDWARE_UNAVAILABLE",
                        "Acquire this receiver explicitly first",
                    ));
                }
                match p.hardware_enabled {
                    Some(true) => self
                        .hardware
                        .enable(&p.id)
                        .map_err(|e| RpcError::new("HARDWARE_UNAVAILABLE", e))?,
                    Some(false) => self.hardware.stop(),
                    None if command.is_empty() => return Err(RpcError::invalid("No hardware operation requested")),
                    None => {}
                }
                if !command.is_empty() {
                    self.hardware
                        .queue(&p.id, command)
                        .map_err(|e| RpcError::new("HARDWARE_UNAVAILABLE", e))?;
                }
                self.sync_hardware();
                Ok(self.state())
            }
            "streams.list" => {
                self.refresh()?;
                Ok(json!(self.snapshot.streams))
            }
            "settings.get" => Ok(json!(self.settings)),
            "settings.set" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct P {
                    start_minimized: Option<bool>,
                    close_to_tray: Option<bool>,
                }
                let p: P = decode(params)?;
                if let Some(v) = p.start_minimized {
                    self.settings.start_minimized = v;
                }
                if let Some(v) = p.close_to_tray {
                    self.settings.close_to_tray = v;
                }
                self.persist()?;
                Ok(json!(self.settings))
            }
            "profiles.list" => Ok(json!(self.profile_names())),
            "profiles.save" | "profiles.apply" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct P {
                    name: String,
                }
                let p: P = decode(params)?;
                if p.name.trim().is_empty() || p.name.len() > 80 || p.name.chars().any(char::is_control) {
                    return Err(RpcError::invalid(
                        "profile name must be 1..80 bytes without control characters",
                    ));
                }
                if method == "profiles.save" {
                    self.profiles.insert(p.name, self.profile());
                    self.persist()?;
                    Ok(json!(self.profile_names()))
                } else {
                    let profile = self
                        .profiles
                        .get(&p.name)
                        .ok_or_else(|| RpcError::new("NOT_FOUND", "unknown profile"))?
                        .clone();
                    if profile.mixer.input_mode == hardware::InputMode::Hardware
                        && profile.mixer.enabled
                        && self.physical.sample.is_none()
                    {
                        return Err(RpcError::new(
                            "HARDWARE_UNAVAILABLE",
                            "Acquire a fresh connected receiver before applying an enabled hardware profile",
                        ));
                    }
                    let guard = (profile.mixer.enabled && profile.mixer.input_mode == hardware::InputMode::Hardware)
                        .then(|| self.hardware.audio_write_guard());
                    self.refresh()?;
                    if guard.as_ref().is_some_and(|g| !g()) {
                        self.sync_hardware();
                        return Err(RpcError::backend(
                            "Physical headset unavailable; audio write cancelled".into(),
                        ));
                    }
                    self.groups = profile.groups;
                    self.mixer = profile.mixer;
                    self.assignments = profile.assignments;
                    for stream in &mut self.snapshot.streams {
                        stream.group = "unmanaged".into();
                        if let Some(a) = self.assignments.get(&stream.app_key) {
                            stream.group = a.group.clone();
                            stream.volume = a.volume;
                            stream.muted = a.muted;
                            if let Some(name) = &a.sink {
                                if let Some(sink) = self.snapshot.sinks.iter().find(|s| &s.name == name) {
                                    self.backend
                                        .set_stream_guarded(stream.id, None, None, Some(sink.id), &|| {
                                            guard.as_ref().is_none_or(|g| g())
                                        })
                                        .map_err(RpcError::backend)?;
                                }
                            }
                        }
                    }
                    self.apply(None)?;
                    self.persist()?;
                    Ok(self.state())
                }
            }
            _ => Err(RpcError::new("INVALID_REQUEST", "Unknown method")),
        }
    }
}
