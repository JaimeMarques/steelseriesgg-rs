#![cfg(unix)]
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use steelseries_gg::desktop::hardware::{Controller, HardwareDevice};
use steelseries_gg::desktop::{Backend, Service, Sink, Snapshot, Stream};
use steelseries_gg::devices::headsets::nova7_gen2::{ChatMixSample, Report, Status};

fn stream(id: u32, key: &str, v: f64) -> Stream {
    Stream {
        id,
        app_key: key.into(),
        volume: v,
        effective_volume: v,
        group: "unmanaged".into(),
        ..Default::default()
    }
}
#[derive(Default)]
struct Fake {
    snapshot: Snapshot,
    routes: u32,
    fail_hid_on_next_snapshot: Option<Arc<AtomicBool>>,
    fail_hid_after_next_route: Option<Arc<AtomicBool>>,
    hid_failed: Arc<AtomicBool>,
    volume_writes: u32,
    fail_on_volume_write: Option<u32>,
}
impl Backend for Fake {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        if let Some(unplug) = self.fail_hid_on_next_snapshot.take() {
            unplug.store(true, Ordering::Release);
            let until = Instant::now() + Duration::from_secs(2);
            while !self.hid_failed.load(Ordering::Acquire) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                self.hid_failed.load(Ordering::Acquire),
                "injected HID owner did not fail"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        Ok(self.snapshot.clone())
    }
    fn set_stream(&mut self, id: u32, v: Option<f64>, m: Option<bool>, sink: Option<u32>) -> Result<(), String> {
        if v.is_some() {
            self.volume_writes += 1;
            if self.fail_on_volume_write == Some(self.volume_writes) {
                return Err("injected second volume write failure".into());
            }
        }
        let s = self.snapshot.streams.iter_mut().find(|s| s.id == id).unwrap();
        if let Some(v) = v {
            s.volume = v;
            s.effective_volume = v;
        }
        if let Some(m) = m {
            s.muted = m;
            s.effective_muted = m;
        }
        if let Some(sink) = sink {
            self.routes += 1;
            s.sink_id = sink;
            if let Some(unplug) = self.fail_hid_after_next_route.take() {
                unplug.store(true, Ordering::Release);
                let until = Instant::now() + Duration::from_secs(2);
                while !self.hid_failed.load(Ordering::Acquire) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(
                    self.hid_failed.load(Ordering::Acquire),
                    "injected HID owner did not fail"
                );
            }
        }
        Ok(())
    }
}
fn call(s: &mut Service<Fake>, method: &str, params: Value) -> Value {
    let r = s.request(json!({"id":1,"method":method,"params":params}));
    assert!(r.get("error").is_none(), "{r}");
    r["result"].clone()
}
#[test]
fn anonymous_streams_must_not_share_unrelated_app_assignment() {
    let raw = |id| json!({"index":id,"sink":10,"mute":false,"volume":{"mono":{"value":65536}},"properties":{}});
    let snap = steelseries_gg::desktop::pulse::parse_snapshot(
        json!([raw(1), raw(2)]),
        json!([{"index":10,"name":"sink","mute":false,"volume":{"mono":{"value":65536}}}]),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut s = Service::new(
        Fake {
            snapshot: snap,
            ..Default::default()
        },
        dir.path().join("state.json"),
    )
    .unwrap();
    let response = s.request(json!({"method":"stream.set","params":{"id":1,"group":"game"}}));
    assert!(
        response.get("error").is_some(),
        "an identity-less stream cannot be assigned durably: {response}"
    );
    let state = call(&mut s, "state.get", json!({}));
    assert_eq!(state["streams"][1]["group"], "unmanaged");
}
#[test]
fn reused_anonymous_stream_id_cannot_inherit_old_saved_assignment() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let raw = json!([{"index":1,"sink":10,"mute":false,"volume":{"mono":{"value":65536}},"properties":{}}]);
    let sinks = json!([{"index":10,"name":"sink","mute":false,"volume":{"mono":{"value":65536}}}]);
    let snap = steelseries_gg::desktop::pulse::parse_snapshot(raw.clone(), sinks.clone()).unwrap();
    let mut s = Service::new(
        Fake {
            snapshot: snap,
            ..Default::default()
        },
        path.clone(),
    )
    .unwrap();
    let response = s.request(json!({"method":"stream.set","params":{"id":1,"group":"game"}}));
    assert!(
        response.get("error").is_some(),
        "anonymous assignments must not survive ID reuse: {response}"
    );
    drop(s);
    let reused = steelseries_gg::desktop::pulse::parse_snapshot(raw, sinks).unwrap();
    let mut next = Service::new(
        Fake {
            snapshot: reused,
            ..Default::default()
        },
        path,
    )
    .unwrap();
    assert_eq!(
        call(&mut next, "state.get", json!({}))["streams"][0]["group"],
        "unmanaged"
    );
}
#[test]
fn external_amplified_base_survives_a_mix_update() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Service::new(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "game", 1.5)],
                ..Default::default()
            },
            ..Default::default()
        },
        dir.path().join("state.json"),
    )
    .unwrap();
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    call(&mut s, "chatmix.set", json!({"enabled":true,"balance":0.5}));
    s.backend.snapshot.streams[0].volume = 0.6;
    s.backend.snapshot.streams[0].effective_volume = 0.6;
    let state = call(&mut s, "state.get", json!({}));
    assert!(
        (state["streams"][0]["volume"].as_f64().unwrap() - 1.2).abs() < 0.0001,
        "external edit to effective 0.6 at factor 0.5 should yield base 1.2: {state}"
    );
    call(&mut s, "chatmix.set", json!({"enabled":false}));
    assert!((s.backend.snapshot.streams[0].effective_volume - 1.2).abs() < 0.0001);
}
struct Hid {
    unplug: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}
impl HardwareDevice for Hid {
    fn read_event(&mut self, _: i32) -> steelseries_gg::Result<Option<Report>> {
        if self.unplug.load(Ordering::Acquire) {
            self.failed.store(true, Ordering::Release);
            return Err(steelseries_gg::Error::DeviceCommunication("injected unplug".into()));
        }
        std::thread::sleep(Duration::from_millis(1));
        Ok(None)
    }
    fn request_status(&mut self) -> steelseries_gg::Result<Status> {
        Ok(Status {
            battery_percent: Some(75),
            connected: Some(true),
            charging: Some(false),
            chatmix: ChatMixSample {
                game_percent: 50,
                chat_percent: 100,
            },
            raw_power: 3,
            raw_charge: 3,
        })
    }
    fn set_sidetone(
        &mut self,
        _: u8,
        _: &mut dyn FnMut(Report) -> steelseries_gg::Result<()>,
    ) -> steelseries_gg::Result<u8> {
        Ok(2)
    }
    fn set_auto_off(&mut self, _: u8) -> steelseries_gg::Result<()> {
        Ok(())
    }
}
#[test]
fn explicit_stream_route_must_recheck_offline_hid_owner_after_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let unplug = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let u = unplug.clone();
    let f = failed.clone();
    let owner = Controller::with_factory(move |_| {
        Ok(Box::new(Hid {
            unplug: u.clone(),
            failed: f.clone(),
        }))
    });
    let mut s = Service::with_hardware(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "game", 0.8)],
                sinks: vec![Sink {
                    id: 9,
                    name: "other".into(),
                    ..Default::default()
                }],
            },
            hid_failed: failed,
            ..Default::default()
        },
        dir.path().join("state.json"),
        owner,
    )
    .unwrap();
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    call(
        &mut s,
        "device.set",
        json!({"id":"1038:227e:test","hardwareEnabled":true}),
    );
    let until = Instant::now() + Duration::from_secs(2);
    while call(&mut s, "state.get", json!({}))["physical"]["sample"].is_null() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    call(&mut s, "chatmix.set", json!({"inputMode":"hardware","enabled":true}));
    s.backend.fail_hid_on_next_snapshot = Some(unplug);
    let _ = s.request(json!({"method":"stream.set","params":{"id":1,"sinkId":9}}));
    assert_eq!(s.backend.routes, 0, "offline owner still permitted explicit sink move");
}
#[test]
fn already_offline_owner_cannot_be_bypassed_by_request_disarming_mixer() {
    let dir = tempfile::tempdir().unwrap();
    let unplug = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let u = unplug.clone();
    let f = failed.clone();
    let owner = Controller::with_factory(move |_| {
        Ok(Box::new(Hid {
            unplug: u.clone(),
            failed: f.clone(),
        }))
    });
    let mut s = Service::with_hardware(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "game", 0.8)],
                sinks: vec![Sink {
                    id: 9,
                    name: "other".into(),
                    ..Default::default()
                }],
            },
            ..Default::default()
        },
        dir.path().join("state.json"),
        owner,
    )
    .unwrap();
    call(
        &mut s,
        "device.set",
        json!({"id":"1038:227e:test","hardwareEnabled":true}),
    );
    let until = Instant::now() + Duration::from_secs(2);
    while call(&mut s, "state.get", json!({}))["physical"]["sample"].is_null() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    call(&mut s, "chatmix.set", json!({"inputMode":"hardware","enabled":true}));
    unplug.store(true, Ordering::Release);
    while !failed.load(Ordering::Acquire) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    let response = s.request(json!({"method":"stream.set","params":{"id":1,"sinkId":9}}));
    assert!(
        response.get("error").is_some(),
        "disarming must not authorize this route: {response}"
    );
    assert_eq!(s.backend.routes, 0);
}

#[test]
fn profile_routes_must_recheck_offline_hid_owner_after_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let unplug = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let u = unplug.clone();
    let f = failed.clone();
    let owner = Controller::with_factory(move |_| {
        Ok(Box::new(Hid {
            unplug: u.clone(),
            failed: f.clone(),
        }))
    });
    let mut s = Service::with_hardware(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "game", 0.8)],
                sinks: vec![
                    Sink {
                        id: 9,
                        name: "saved".into(),
                        ..Default::default()
                    },
                    Sink {
                        id: 10,
                        name: "current".into(),
                        ..Default::default()
                    },
                ],
            },
            hid_failed: failed,
            ..Default::default()
        },
        dir.path().join("state.json"),
        owner,
    )
    .unwrap();
    call(
        &mut s,
        "device.set",
        json!({"id":"1038:227e:test","hardwareEnabled":true}),
    );
    let until = Instant::now() + Duration::from_secs(2);
    while call(&mut s, "state.get", json!({}))["physical"]["sample"].is_null() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    call(&mut s, "chatmix.set", json!({"inputMode":"hardware","enabled":true}));
    call(&mut s, "stream.set", json!({"id":1,"group":"game","sinkId":9}));
    call(&mut s, "profiles.save", json!({"name":"Saved"}));
    call(&mut s, "stream.set", json!({"id":1,"sinkId":10}));
    let before = s.backend.routes;
    s.backend.fail_hid_on_next_snapshot = Some(unplug);
    let response = s.request(json!({"method":"profiles.apply","params":{"name":"Saved"}}));
    assert!(
        response.get("error").is_some(),
        "offline profile apply should fail: {response}"
    );
    assert_eq!(
        s.backend.routes, before,
        "offline owner still permitted profile sink move"
    );
}

#[test]
fn profile_stops_between_routes_when_hid_disconnects_on_first_move() {
    let dir = tempfile::tempdir().unwrap();
    let unplug = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let u = unplug.clone();
    let f = failed.clone();
    let owner = Controller::with_factory(move |_| {
        Ok(Box::new(Hid {
            unplug: u.clone(),
            failed: f.clone(),
        }))
    });
    let mut s = Service::with_hardware(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "first", 0.8), stream(2, "second", 0.8)],
                sinks: vec![
                    Sink {
                        id: 9,
                        name: "saved".into(),
                        ..Default::default()
                    },
                    Sink {
                        id: 10,
                        name: "current".into(),
                        ..Default::default()
                    },
                ],
            },
            hid_failed: failed,
            ..Default::default()
        },
        dir.path().join("state.json"),
        owner,
    )
    .unwrap();
    call(
        &mut s,
        "device.set",
        json!({"id":"1038:227e:test","hardwareEnabled":true}),
    );
    let until = Instant::now() + Duration::from_secs(2);
    while call(&mut s, "state.get", json!({}))["physical"]["sample"].is_null() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    call(&mut s, "chatmix.set", json!({"inputMode":"hardware","enabled":true}));
    for id in [1, 2] {
        call(&mut s, "stream.set", json!({"id":id,"group":"chat","sinkId":9}));
    }
    call(&mut s, "profiles.save", json!({"name":"Saved"}));
    for id in [1, 2] {
        call(&mut s, "stream.set", json!({"id":id,"group":"game","sinkId":10}));
    }
    let before = s.backend.routes;
    s.backend.fail_hid_after_next_route = Some(unplug);
    let response = s.request(json!({"method":"profiles.apply","params":{"name":"Saved"}}));
    assert!(response.get("error").is_some(), "remaining moves must stop: {response}");
    assert_eq!(
        s.backend.routes,
        before + 1,
        "only the already-issued first move can complete"
    );
    assert_eq!(s.backend.snapshot.streams[1].sink_id, 10);
    let state = call(&mut s, "state.get", json!({}));
    assert_eq!(
        state["streams"][0]["group"], "game",
        "failed apply changed the in-memory policy"
    );
    assert_eq!(
        state["streams"][1]["group"], "game",
        "failed apply changed a later stream"
    );
    let routes = s.backend.routes;
    s.tick().unwrap();
    assert_eq!(
        s.backend.routes, routes,
        "failed apply must not replay saved route later"
    );
}

#[test]
fn failed_second_profile_gain_write_preserves_old_base_intent() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Service::new(
        Fake {
            snapshot: Snapshot {
                streams: vec![stream(1, "first", 0.8), stream(2, "second", 0.8)],
                ..Default::default()
            },
            ..Default::default()
        },
        dir.path().join("state.json"),
    )
    .unwrap();
    for id in [1, 2] {
        call(&mut s, "stream.set", json!({"id":id,"group":"game"}));
    }
    call(&mut s, "chatmix.set", json!({"enabled":true,"balance":0.5}));
    call(&mut s, "profiles.save", json!({"name":"Attenuated"}));
    call(&mut s, "chatmix.set", json!({"enabled":false}));
    assert_eq!(s.backend.snapshot.streams[0].effective_volume, 0.8);
    s.backend.fail_on_volume_write = Some(s.backend.volume_writes + 2);
    let response = s.request(json!({"method":"profiles.apply","params":{"name":"Attenuated"}}));
    assert!(response.get("error").is_some(), "expected partial failure: {response}");
    assert_eq!(
        s.backend.snapshot.streams[0].effective_volume, 0.4,
        "first write really happened"
    );
    let state = call(&mut s, "state.get", json!({}));
    assert_eq!(
        state["streams"][0]["volume"], 0.8,
        "a partial profile write was mistaken for an external base edit"
    );
    assert_eq!(state["mixer"]["enabled"], false, "failed profile must be disarmed");
    assert_eq!(
        s.backend.snapshot.streams[1].effective_volume, 0.8,
        "second write must not happen"
    );
}
