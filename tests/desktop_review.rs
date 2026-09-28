#![cfg(unix)]
use serde_json::{Value, json};
use steelseries_gg::desktop::{Backend, Service, Snapshot, Stream};

#[derive(Default)]
struct MemoryBackend {
    snapshot: Snapshot,
    writes: usize,
    fail_snapshot: bool,
    fail_write_after_volume: Option<bool>,
    fail_route: bool,
}
impl Backend for MemoryBackend {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        if std::mem::take(&mut self.fail_snapshot) {
            return Err("transient inventory failure".into());
        }
        Ok(self.snapshot.clone())
    }
    fn set_stream(
        &mut self,
        id: u32,
        volume: Option<f64>,
        muted: Option<bool>,
        sink: Option<u32>,
    ) -> Result<(), String> {
        if volume.is_some_and(|v| !v.is_finite() || !(0.0..=f64::from(0x7fff_ffff_u32) / 65536.0).contains(&v)) {
            return Err("invalid stream gain".into());
        }
        self.writes += 1;
        if sink.is_some() && std::mem::take(&mut self.fail_route) {
            return Err("route failed".into());
        }
        let failure = self.fail_write_after_volume.take();
        if failure == Some(false) {
            return Err("volume command failed".into());
        }
        let s = self.snapshot.streams.iter_mut().find(|s| s.id == id).ok_or("missing")?;
        if let Some(v) = volume {
            s.volume = v;
            s.effective_volume = v;
        }
        if failure == Some(true) {
            return Err("mute or readback failed after setting volume".into());
        }
        if let Some(m) = muted {
            s.muted = m;
            s.effective_muted = m;
        }
        if let Some(sink) = sink {
            s.sink_id = sink;
        }
        Ok(())
    }
}
fn stream(id: u32, volume: f64) -> Stream {
    Stream {
        id,
        app_key: "game".into(),
        volume,
        effective_volume: volume,
        group: "unmanaged".into(),
        ..Default::default()
    }
}
fn setup() -> (tempfile::TempDir, Service<MemoryBackend>) {
    let dir = tempfile::tempdir().unwrap();
    let s = Service::new(
        MemoryBackend {
            snapshot: Snapshot {
                streams: vec![stream(1, 0.8)],
                ..Default::default()
            },
            ..Default::default()
        },
        dir.path().join("state.json"),
    )
    .unwrap();
    (dir, s)
}
fn call(s: &mut Service<MemoryBackend>, method: &str, params: Value) -> Value {
    let r = s.request(json!({"id":1,"method":method,"params":params}));
    assert!(r.get("error").is_none(), "{r}");
    r["result"].clone()
}
fn mix(s: &mut Service<MemoryBackend>) {
    call(s, "chatmix.set", json!({"enabled":true,"balance":0.5}));
}
fn actual(s: &Service<MemoryBackend>) -> f64 {
    s.backend.snapshot.streams[0].effective_volume
}

#[test]
fn shared_app_reads_and_mix_keep_latest_explicit_restart_default() {
    for reverse in [false, true] {
        let (dir, mut s) = setup();
        s.backend.snapshot.streams.push(stream(2, 0.8));
        call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
        s.tick().unwrap();
        call(&mut s, "stream.set", json!({"id":1,"volume":0.3}));
        assert_eq!(s.backend.snapshot.streams[0].volume, 0.3);
        assert_eq!(s.backend.snapshot.streams[1].volume, 0.8);
        if reverse {
            s.backend.snapshot.streams.reverse();
        }
        call(&mut s, "streams.list", json!({}));
        mix(&mut s);
        let stored: Value = serde_json::from_slice(&std::fs::read(dir.path().join("state.json")).unwrap()).unwrap();
        assert_eq!(stored["current"]["assignments"]["game"]["volume"], 0.3);
        s.backend.snapshot.streams = vec![stream(33, 1.0)];
        s.tick().unwrap();
        assert_eq!(actual(&s), 0.15);
    }
}

#[test]
fn one_observed_external_edit_updates_default_but_conflicting_edits_do_not() {
    for reverse in [false, true] {
        let (_dir, mut s) = setup();
        s.backend.snapshot.streams.push(stream(2, 0.8));
        call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
        s.tick().unwrap();
        // Both volumes changed between observations: no knowable ordering.
        s.backend.snapshot.streams[0] = stream(1, 0.4);
        s.backend.snapshot.streams[1] = stream(2, 0.7);
        if reverse {
            s.backend.snapshot.streams.reverse();
        }
        let r = call(&mut s, "streams.list", json!({}));
        assert!(r.as_array().unwrap().iter().any(|v| v["id"] == 1 && v["volume"] == 0.4));
        s.backend.snapshot.streams.push(stream(33, 1.0));
        s.tick().unwrap();
        assert_eq!(
            s.backend.snapshot.streams[2].volume, 0.8,
            "conflict retains prior app default, not inventory order"
        );
        // A subsequent unambiguous external edit is new intent for that app.
        s.backend.snapshot.streams[0] = stream(if reverse { 2 } else { 1 }, 0.6);
        call(&mut s, "streams.list", json!({}));
        s.backend.snapshot.streams = vec![stream(44, 1.0)];
        s.tick().unwrap();
        assert_eq!(actual(&s), 0.6, "a real later external edit must not be discarded");
    }
}

#[test]
fn native_amplification_is_visible_and_setting_unity_writes() {
    let inputs = json!([{"index":1,"sink":11,"mute":false,"volume":{"mono":{"value":98304}},"properties":{"application.name":"Amplified Test Player"}}]);
    let sinks = json!([{"index":11,"name":"test","mute":false,"volume":{"mono":{"value":98304}}}]);
    let snapshot = steelseries_gg::desktop::pulse::parse_snapshot(inputs, sinks).unwrap();
    assert_eq!(snapshot.streams[0].effective_volume, 1.5);
    assert_eq!(snapshot.sinks[0].volume, 1.5);
    let (_dir, mut s) = setup();
    s.backend.snapshot = snapshot;
    call(&mut s, "stream.set", json!({"id":1,"volume":1.0}));
    assert_eq!(s.backend.writes, 1, "unity must reduce an amplified native stream");
    assert_eq!(actual(&s), 1.0);
    assert_eq!(
        s.request(json!({"method":"stream.set","params":{"id":1,"volume":1.5}}))["error"]["code"],
        "INVALID_PARAMS"
    );
}

#[test]
fn native_amplification_group_only_mix_roundtrip_restores_base() {
    let (_dir, mut s) = setup();
    s.backend.snapshot.streams = vec![stream(1, 1.5)];
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    assert_eq!(actual(&s), 1.5);
    mix(&mut s);
    assert_eq!(actual(&s), 0.75);
    let managed = call(&mut s, "streams.list", json!({}));
    assert_eq!(managed[0]["volume"], 1.5);
    assert_eq!(managed[0]["effectiveVolume"], 0.75);
    call(&mut s, "chatmix.set", json!({"enabled":false}));
    assert_eq!(actual(&s), 1.5);
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 1.5);
}

#[test]
fn rpc_amplified_volume_is_invalid_without_changing_native_base() {
    let (_dir, mut s) = setup();
    s.backend.snapshot.streams = vec![stream(1, 1.5)];
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    mix(&mut s);
    let writes = s.backend.writes;
    assert_eq!(
        s.request(json!({"method":"stream.set","params":{"id":1,"volume":1.5}}))["error"]["code"],
        "INVALID_PARAMS"
    );
    assert_eq!(s.backend.writes, writes);
    assert_eq!(actual(&s), 0.75);
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 1.5);
}

#[test]
fn restarted_stream_restores_saved_base_before_explicit_mix_not_only_tick() {
    let (_dir, mut s) = setup();
    call(&mut s, "stream.set", json!({"id":1,"group":"game","volume":0.8}));
    mix(&mut s);
    s.backend.snapshot.streams = vec![stream(22, 1.0)];
    mix(&mut s); // GUI request wins the race against the periodic tick.
    assert_eq!(actual(&s), 0.4);
    s.tick().unwrap();
    assert_eq!(actual(&s), 0.4);
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 0.8);
}

#[test]
fn restarted_stream_route_failure_can_retry_explicit_restore() {
    let (_dir, mut s) = setup();
    s.backend.snapshot.sinks.push(steelseries_gg::desktop::Sink {
        id: 9,
        name: "other".into(),
        ..Default::default()
    });
    call(
        &mut s,
        "stream.set",
        json!({"id":1,"group":"game","volume":0.8,"sinkId":9}),
    );
    mix(&mut s);
    s.backend.snapshot.streams = vec![stream(22, 1.0)];
    s.backend.fail_route = true;
    assert!(s.tick().is_err());
    mix(&mut s);
    assert_eq!(actual(&s), 0.4);
    assert_eq!(s.backend.snapshot.streams[0].sink_id, 9);
}

#[test]
fn transient_snapshot_failure_preserves_base_on_reactivation() {
    let (_dir, mut s) = setup();
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    mix(&mut s);
    assert_eq!(actual(&s), 0.4);
    s.backend.fail_snapshot = true;
    assert!(s.tick().is_err());
    let writes = s.backend.writes;
    s.tick().unwrap();
    assert_eq!(s.backend.writes, writes, "recovery alone must not mutate audio");
    mix(&mut s);
    assert_eq!(actual(&s), 0.4, "the surviving attenuation must not become the base");
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 0.8);
}

#[test]
fn partial_volume_failure_and_route_retry_do_not_compound_gain() {
    let (_dir, mut s) = setup();
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    s.backend.fail_write_after_volume = Some(true);
    let r = s.request(json!({"method":"chatmix.set","params":{"enabled":true,"balance":0.5}}));
    assert_eq!(r["error"]["code"], "BACKEND_UNAVAILABLE");
    assert_eq!(actual(&s), 0.4, "volume really changed before the backend returned Err");
    s.backend.snapshot.sinks.push(steelseries_gg::desktop::Sink {
        id: 9,
        name: "other".into(),
        ..Default::default()
    });
    s.backend.fail_route = true;
    assert_eq!(
        s.request(json!({"method":"stream.set","params":{"id":1,"sinkId":9}}))["error"]["code"],
        "BACKEND_UNAVAILABLE"
    );
    call(&mut s, "stream.set", json!({"id":1,"sinkId":9}));
    mix(&mut s);
    assert_eq!(actual(&s), 0.4);
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 0.8);
    assert_eq!(s.backend.snapshot.streams[0].sink_id, 9);
}

#[test]
fn partial_failure_retains_explicit_base_and_retries_missing_mute() {
    let (_dir, mut s) = setup();
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    mix(&mut s);
    s.backend.fail_write_after_volume = Some(true);
    let r = s.request(json!({"method":"stream.set","params":{"id":1,"volume":0.6,"muted":true}}));
    assert_eq!(r["error"]["code"], "BACKEND_UNAVAILABLE");
    assert_eq!(actual(&s), 0.3);
    assert!(!s.backend.snapshot.streams[0].muted);
    s.tick().unwrap();
    assert!(
        s.backend.snapshot.streams[0].muted,
        "tick must finish a partially applied intent"
    );
    assert_eq!(actual(&s), 0.3);
    mix(&mut s);
    assert_eq!(call(&mut s, "streams.list", json!({}))[0]["volume"], 0.6);
}
