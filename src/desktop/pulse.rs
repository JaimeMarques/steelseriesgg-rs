//! PulseAudio and PipeWire-Pulse JSON inventory. No capture/source API is exposed.
use super::{Sink, Snapshot, Stream};
use serde_json::Value;
fn index(v: &Value) -> Result<u32, String> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| "missing or invalid Pulse index".into())
}
fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}
fn volume(v: &Value) -> Result<f64, String> {
    let channels = v.as_object().ok_or("missing Pulse channel volumes")?;
    if channels.is_empty() {
        return Err("empty Pulse channel volumes".into());
    }
    let mut sum = 0.0;
    for channel in channels.values() {
        sum += channel["value"].as_f64().ok_or("invalid Pulse volume")? / 65536.0;
    }
    // Pulse permits amplification above unity; clipping inventory would hide
    // a required write when the user explicitly requests 100%.
    Ok(sum / channels.len() as f64)
}
pub fn parse_snapshot(inputs: Value, sinks: Value) -> Result<Snapshot, String> {
    let streams = inputs
        .as_array()
        .ok_or("Pulse stream list is not an array")?
        .iter()
        .map(|v| {
            let props = &v["properties"];
            let app_name = text(&props["application.name"]);
            let binary = text(&props["application.process.binary"]);
            let role = text(&props["media.role"]);
            let id = index(&v["index"])?;
            // An absent identity must not make unrelated sink-inputs share an assignment.
            let app_key = if binary.is_empty() && app_name.is_empty() && role.is_empty() {
                format!("anonymous:{id}")
            } else {
                serde_json::to_string(&[binary, app_name.clone(), role]).map_err(|e| e.to_string())?
            };
            let volume = volume(&v["volume"])?;
            Ok(Stream {
                id,
                app_key,
                generation: None,
                app_name,
                name: text(&props["media.name"]),
                volume,
                effective_volume: volume,
                effective_muted: v["mute"].as_bool().ok_or("missing Pulse mute state")?,
                muted: v["mute"].as_bool().ok_or("missing Pulse mute state")?,
                group: "unmanaged".into(),
                sink_id: index(&v["sink"])?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let sinks = sinks
        .as_array()
        .ok_or("Pulse sink list is not an array")?
        .iter()
        .map(|v| {
            let mut description = text(&v["description"]);
            if description.is_empty() || description == "(null)" {
                description = text(&v["properties"]["node.nick"]);
            }
            Ok(Sink {
                id: index(&v["index"])?,
                name: text(&v["name"]),
                description,
                volume: volume(&v["volume"])?,
                muted: v["mute"].as_bool().ok_or("missing Pulse mute state")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Snapshot { streams, sinks })
}

use std::{
    io::Read,
    os::fd::AsRawFd,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                if let Err(e) = self.0.kill() {
                    eprintln!("desktop child cleanup: {e}");
                }
                if let Err(e) = self.0.wait() {
                    eprintln!("desktop child wait: {e}");
                }
            }
        }
    }
}
fn nonblocking(fd: &impl AsRawFd) -> Result<(), String> {
    // SAFETY: fd is borrowed from a live owned pipe; fcntl only changes its flags.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: the same live pipe descriptor remains owned by the caller.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
fn drain(reader: &mut impl Read, output: &mut Vec<u8>) -> Result<(), String> {
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                if output.len() + n > OUTPUT_LIMIT {
                    return Err("pactl output limit exceeded".into());
                }
                output.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}
/// Shell-free, bounded child process. Every error path kills and reaps it.
pub fn run_command(executable: &str, args: &[&str], server: Option<&str>, timeout: Duration) -> Result<String, String> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(server) = server {
        command.env("PULSE_SERVER", server);
    }
    let mut child = OwnedChild(
        command
            .spawn()
            .map_err(|e| format!("cannot execute {executable}: {e}; install pulseaudio-utils (pactl)"))?,
    );
    let mut stdout = child.0.stdout.take().ok_or("missing child stdout")?;
    let mut stderr = child.0.stderr.take().ok_or("missing child stderr")?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut out = Vec::new();
    let mut err = Vec::new();
    let start = Instant::now();
    loop {
        drain(&mut stdout, &mut out)?;
        drain(&mut stderr, &mut err)?;
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            drain(&mut stdout, &mut out)?;
            drain(&mut stderr, &mut err)?;
            if !status.success() {
                return Err(format!("pactl failed ({status}): {}", String::from_utf8_lossy(&err)));
            }
            return String::from_utf8(out).map_err(|e| e.to_string());
        }
        if start.elapsed() >= timeout {
            return Err("pactl timeout".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
/// Fresh short-lived connections naturally recover from Pulse daemon restarts.
pub struct PulseBackend {
    pub executable: String,
    pub server: Option<String>,
    cycle_timeout: Duration,
    deadline: Option<Instant>,
}
impl Default for PulseBackend {
    fn default() -> Self {
        Self {
            executable: std::env::var("SSGG_PACTL").unwrap_or_else(|_| "pactl".into()),
            server: None,
            cycle_timeout: Duration::from_secs(8),
            deadline: None,
        }
    }
}
impl PulseBackend {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            cycle_timeout: timeout,
            ..Self::default()
        }
    }
    fn command(&self, args: &[&str]) -> Result<String, String> {
        let timeout = self
            .deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3))
            .min(Duration::from_secs(3));
        if timeout.is_zero() {
            return Err("pactl cycle timeout".into());
        }
        run_command(&self.executable, args, self.server.as_deref(), timeout)
    }
}
impl super::Backend for PulseBackend {
    fn begin_cycle(&mut self) {
        self.deadline = Some(Instant::now() + self.cycle_timeout);
    }
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        let inputs = serde_json::from_str(&self.command(&["-f", "json", "list", "sink-inputs"])?)
            .map_err(|e| format!("invalid pactl stream JSON: {e}"))?;
        let sinks = serde_json::from_str(&self.command(&["-f", "json", "list", "sinks"])?)
            .map_err(|e| format!("invalid pactl sink JSON: {e}"))?;
        parse_snapshot(inputs, sinks)
    }
    fn set_stream(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
    ) -> Result<(), String> {
        self.set_stream_guarded(id, volume, muted, sink, &|| true)
    }
    fn set_stream_guarded(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
        allowed: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        // Internal restoration may exceed unity; PA_VOLUME_MAX is 0x7fff_ffff.
        if volume.is_some_and(|v| !v.is_finite() || !(0.0..=f64::from(0x7fff_ffff_u32) / 65536.0).contains(&v)) {
            return Err("invalid stream gain".into());
        }
        let index = id;
        let id = id.to_string();
        if let Some(sink) = sink {
            if !allowed() {
                return Err("Physical headset unavailable; audio write cancelled".into());
            }
            self.command(&["move-sink-input", &id, &sink.to_string()])?;
        }
        if let Some(volume) = volume {
            if !allowed() {
                return Err("Physical headset unavailable; audio write cancelled".into());
            }
            self.command(&["set-sink-input-volume", &id, &format!("{:.0}", volume * 65536.0)])?;
        }
        if let Some(muted) = muted {
            if !allowed() {
                return Err("Physical headset unavailable; audio write cancelled".into());
            }
            self.command(&["set-sink-input-mute", &id, if muted { "1" } else { "0" }])?;
        }
        let until = Instant::now() + Duration::from_millis(500);
        loop {
            let snapshot = self.snapshot()?;
            let actual = snapshot
                .streams
                .iter()
                .find(|s| s.id == index)
                .ok_or("stream disappeared during readback")?;
            if volume.is_none_or(|v| (actual.effective_volume - v).abs() < 0.0001)
                && muted.is_none_or(|m| actual.muted == m)
                && sink.is_none_or(|id| actual.sink_id == id)
            {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err(
                    "stream change was not confirmed by Pulse readback (another controller may have changed it)".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
