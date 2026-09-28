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
    call(&mut s, "stream.set", json!({"id":1,"group":"game"}));
    let state = call(&mut s, "state.get", json!({}));
    assert_eq!(
        state["streams"][1]["group"], "unmanaged",
        "anonymous unrelated sink-input inherited app 1 group"
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
        call(&mut s, "stream.set", json!({"id":id,"group":"game","sinkId":9}));
    }
    call(&mut s, "profiles.save", json!({"name":"Saved"}));
    for id in [1, 2] {
        call(&mut s, "stream.set", json!({"id":id,"sinkId":10}));
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
}
