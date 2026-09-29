#![cfg(target_os = "linux")]
use std::process::Command;
use std::time::{Duration, Instant};
use steelseries_gg::desktop::{Backend, pipewire::PipeWireBackend};

#[test]
fn backend_selection_supports_explicit_overrides_without_forcing_pulse_default() {
    let help = Command::new(env!("CARGO_BIN_EXE_ssgg-desktop"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("--audio-backend"));
    assert!(text.contains("pipewire"));
    assert!(!text.contains("default: pulse"));
}

fn backend_name(
    args: &[&str],
    runtime: &std::path::Path,
    remote: &str,
    pipewire_runtime: Option<&std::path::Path>,
) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let config = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssgg-desktop"));
    if let Some(private) = pipewire_runtime {
        command.env("PIPEWIRE_RUNTIME_DIR", private);
    } else {
        command.env_remove("PIPEWIRE_RUNTIME_DIR");
    }
    let mut child = command
        .args(args)
        .args(["--stdio", "--config"])
        .arg(config.path().join("state.json"))
        .env("XDG_RUNTIME_DIR", runtime)
        .env("PIPEWIRE_REMOTE", remote)
        .env("PULSE_SERVER", "unix:/nonexistent/ssgg-pulse")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"id\":1,\"method\":\"state.get\",\"params\":{}}\n")
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn absent_native_socket_defaults_to_pulse_over_rpc() {
    let runtime = tempfile::tempdir().unwrap();
    let output = backend_name(&[], runtime.path(), "pipewire-0", None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let response: serde_json::Value =
        serde_json::from_slice(output.stdout.split(|&c| c == b'\n').next().unwrap()).unwrap();
    assert_eq!(response["result"]["backend"]["name"], "PulseAudio / PipeWire-Pulse");
}

#[test]
fn detected_unresponsive_native_socket_fails_without_pulse_fallback() {
    let runtime = tempfile::tempdir().unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(runtime.path().join("pipewire-0")).unwrap();
    let output = backend_name(&[], runtime.path(), "pipewire-0", None);
    assert!(!output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(String::from_utf8_lossy(&output.stderr).contains("native PipeWire unavailable"));
}

#[test]
fn explicit_pulse_bypasses_detected_native_socket() {
    let runtime = tempfile::tempdir().unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(runtime.path().join("pipewire-0")).unwrap();
    let output = backend_name(&["--audio-backend", "pulse"], runtime.path(), "pipewire-0", None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let response: serde_json::Value =
        serde_json::from_slice(output.stdout.split(|&c| c == b'\n').next().unwrap()).unwrap();
    assert_eq!(response["result"]["backend"]["name"], "PulseAudio / PipeWire-Pulse");
}

#[test]
fn detected_custom_remote_fails_closed() {
    let runtime = tempfile::tempdir().unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(runtime.path().join("named-core")).unwrap();
    let output = backend_name(&[], runtime.path(), "named-core", None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("native PipeWire unavailable"));
}

#[test]
fn pipewire_runtime_dir_and_custom_remote_take_precedence_over_xdg_runtime() {
    let xdg_runtime = tempfile::tempdir().unwrap();
    let native_runtime = tempfile::tempdir().unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(native_runtime.path().join("named-core")).unwrap();
    let output = backend_name(&[], xdg_runtime.path(), "named-core", Some(native_runtime.path()));
    assert!(!output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(String::from_utf8_lossy(&output.stderr).contains("native PipeWire unavailable"));
}

#[test]
fn selected_missing_private_socket_exits_without_pulse_fallback() {
    let runtime = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ssgg-desktop"))
        .args(["--audio-backend", "pipewire", "--pipewire-runtime"])
        .arg(runtime.path())
        .arg("--stdio")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("native PipeWire unavailable"), "{stderr}");
}

#[test]
fn explicit_missing_private_runtime_never_falls_back_to_host() {
    let temp = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let mut backend = PipeWireBackend::connect_at(temp.path(), "pipewire-0", Duration::from_millis(300)).unwrap();
    assert!(backend.snapshot().is_err());
    assert_eq!(backend.backend_name(), "Native PipeWire");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn rejected_write_to_missing_private_runtime_is_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let mut backend = PipeWireBackend::connect_at(temp.path(), "pipewire-0", Duration::from_millis(300)).unwrap();
    let started = Instant::now();
    assert!(backend.set_stream(77, Some(0.5), None, None).is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn borrowed_guard_is_rejected_for_async_native_writes() {
    let runtime = tempfile::tempdir().unwrap();
    let mut backend = PipeWireBackend::connect_at(runtime.path(), "pipewire-0", Duration::from_millis(300)).unwrap();
    let error = backend
        .set_stream_guarded(77, Some(0.5), None, None, &|| true)
        .unwrap_err();
    assert!(error.contains("owned audio guard"), "{error}");
}

#[test]
fn private_graph_enumerates_real_null_sinks_and_playback() {
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let mut backend =
        PipeWireBackend::connect_at(std::path::Path::new(&runtime), "pipewire-0", Duration::from_secs(3)).unwrap();
    let inventory = backend.snapshot().unwrap();
    assert_eq!(
        inventory
            .sinks
            .iter()
            .filter(|s| s.name == "test_a" || s.name == "test_b")
            .count(),
        2
    );
    assert!(
        inventory
            .streams
            .iter()
            .any(|s| s.sink_id == inventory.sinks.iter().find(|sink| sink.name == "test_a").unwrap().id)
    );
}

#[test]
fn private_stream_gain_mutation_has_native_readback() {
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let mut backend =
        PipeWireBackend::connect_at(std::path::Path::new(&runtime), "pipewire-0", Duration::from_secs(3)).unwrap();
    let before = backend.snapshot().unwrap();
    let stream = before.streams.iter().find(|s| s.app_name == "SSGG-Probe").unwrap();
    backend.set_stream(stream.id, Some(0.4), None, None).unwrap();
    let after = backend.snapshot().unwrap();
    assert!(
        (after
            .streams
            .iter()
            .find(|s| s.id == stream.id)
            .unwrap()
            .effective_volume
            - 0.4)
            .abs()
            < 0.01
    );
}

#[test]
fn private_stream_mute_mutation_has_native_readback() {
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let mut backend =
        PipeWireBackend::connect_at(std::path::Path::new(&runtime), "pipewire-0", Duration::from_secs(3)).unwrap();
    let stream = backend
        .snapshot()
        .unwrap()
        .streams
        .into_iter()
        .find(|s| s.app_name == "SSGG-Probe")
        .unwrap();
    backend.set_stream(stream.id, None, Some(true), None).unwrap();
    let after = backend.snapshot().unwrap();
    assert!(
        after
            .streams
            .iter()
            .find(|s| s.id == stream.id)
            .unwrap()
            .effective_muted
    );
}

#[test]
fn private_guard_revoked_while_queued_prevents_native_write() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let mut backend =
        PipeWireBackend::connect_at(std::path::Path::new(&runtime), "pipewire-0", Duration::from_secs(3)).unwrap();
    let stream = backend
        .snapshot()
        .unwrap()
        .streams
        .into_iter()
        .find(|s| s.app_name == "SSGG-Probe")
        .unwrap();
    let previous = stream.effective_volume;
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let guard: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || counter.fetch_add(1, Ordering::SeqCst) == 0);
    assert!(
        backend
            .set_stream_guarded_owned(stream.id, Some(0.3), None, None, guard)
            .is_err()
    );
    assert!(calls.load(Ordering::SeqCst) >= 2);
    let after = backend.snapshot().unwrap();
    assert!(
        (after
            .streams
            .iter()
            .find(|s| s.id == stream.id)
            .unwrap()
            .effective_volume
            - previous)
            .abs()
            < 0.01
    );
}

#[test]
fn private_service_defaults_to_native_backend_over_rpc() {
    use std::io::Write;
    use std::process::Stdio;
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let config = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ssgg-desktop"))
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("PIPEWIRE_RUNTIME_DIR", &runtime)
        .env("PIPEWIRE_REMOTE", "pipewire-0")
        .arg("--config")
        .arg(config.path().join("state.json"))
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(b"{\"id\":1,\"method\":\"state.get\",\"params\":{}}\n")
        .unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let response: serde_json::Value =
        serde_json::from_slice(output.stdout.split(|&c| c == b'\n').next().unwrap()).unwrap();
    assert_eq!(response["result"]["backend"]["name"], "Native PipeWire");
}

#[test]
fn private_stream_move_confirms_native_active_link() {
    let Some(runtime) = std::env::var_os("SSGG_PRIVATE_PW_RUNTIME") else {
        return;
    };
    let mut backend =
        PipeWireBackend::connect_at(std::path::Path::new(&runtime), "pipewire-0", Duration::from_secs(3)).unwrap();
    let before = backend.snapshot().unwrap();
    let stream = before.streams.iter().find(|s| s.app_name == "SSGG-Probe").unwrap();
    let destination = before.sinks.iter().find(|s| s.name == "test_b").unwrap().id;
    backend.set_stream(stream.id, None, None, Some(destination)).unwrap();
    let after = backend.snapshot().unwrap();
    assert_eq!(
        after.streams.iter().find(|s| s.id == stream.id).unwrap().sink_id,
        destination
    );
}
