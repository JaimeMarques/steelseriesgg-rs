#![cfg(unix)]
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use steelseries_gg::{
    desktop::{
        hardware::{Controller as HardwareController, SettingsCommand},
        lighting::{
            Access, Apply, AsyncController, Controller as LightingController, Endpoint, Transport as LightTransport,
        },
    },
    devices::{
        DeviceInfo, DeviceType,
        headsets::nova7_gen2::{Nova7Gen2, Transport},
    },
};

#[derive(Default)]
struct Wire {
    reports: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
}
struct GateTransport {
    wire: Arc<Mutex<Wire>>,
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    block_on: u8,
    exited: Arc<AtomicBool>,
}
impl Drop for GateTransport {
    fn drop(&mut self) {
        self.exited.store(true, Ordering::Release);
    }
}
impl Transport for GateTransport {
    fn write(&mut self, data: &[u8]) -> steelseries_gg::Result<usize> {
        {
            let mut w = self.wire.lock().unwrap();
            w.writes.push(data.to_vec());
            match data[1] {
                0xb0 => w.reports.push_back(vec![0xb0, 3, 73, 3, 100, 100]),
                0x20 => w.reports.push_back(vec![0x20, 0, 2, 0]),
                _ => {}
            }
        }
        if data[1] == self.block_on {
            self.entered.store(true, Ordering::Release);
            let until = Instant::now() + Duration::from_secs(2);
            while !self.release.load(Ordering::Acquire) && Instant::now() < until {
                thread::sleep(Duration::from_millis(1));
            }
            assert!(self.release.load(Ordering::Acquire));
        }
        Ok(data.len())
    }
    fn read_timeout(&mut self, data: &mut [u8], _: i32) -> steelseries_gg::Result<usize> {
        if let Some(v) = self.wire.lock().unwrap().reports.pop_front() {
            data[..v.len()].copy_from_slice(&v);
            return Ok(v.len());
        }
        thread::sleep(Duration::from_millis(1));
        Ok(0)
    }
}
#[test]
fn cancelled_owner_does_not_issue_sidetone_save_or_readback() {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let (w, e, r) = (wire.clone(), entered.clone(), release.clone());
    let mut owner = HardwareController::with_factory(move |_| {
        let info = DeviceInfo {
            name: "fixture".into(),
            device_type: DeviceType::Headset,
            vendor_id: 0x1038,
            product_id: 0x227e,
            interface_number: 3,
            usage_page: 0xffc0,
            usage: 1,
            serial_number: Some("fixture".into()),
            manufacturer: None,
            path: "injected".into(),
        };
        Ok(Box::new(
            Nova7Gen2::new(
                info,
                GateTransport {
                    wire: w.clone(),
                    entered: e.clone(),
                    release: r.clone(),
                    block_on: 0x39,
                    exited: Arc::new(AtomicBool::new(false)),
                },
            )
            .unwrap(),
        ))
    });
    owner.enable("1038:227e:fixture").unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while owner.snapshot().sample.is_none() {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    owner
        .queue(
            "1038:227e:fixture",
            SettingsCommand {
                sidetone: Some(2),
                auto_off_minutes: Some(30),
                ..Default::default()
            },
        )
        .unwrap();
    while !entered.load(Ordering::Acquire) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    owner.stop();
    release.store(true, Ordering::Release);
    drop(owner);
    let writes = &wire.lock().unwrap().writes;
    assert_eq!(writes.iter().filter(|w| w.get(1) == Some(&0x39)).count(), 1);
    assert!(
        !writes.iter().any(|w| matches!(w.get(1), Some(0x09 | 0x20 | 0x37))),
        "commands after stop: {writes:?}"
    );
}

#[test]
fn cancelled_owner_does_not_complete_blocked_sidetone_readback() {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let exited = Arc::new(AtomicBool::new(false));
    let done = exited.clone();
    let (w, e, r) = (wire.clone(), entered.clone(), release.clone());
    let mut owner = HardwareController::with_factory(move |_| {
        let info = DeviceInfo {
            name: "fixture".into(),
            device_type: DeviceType::Headset,
            vendor_id: 0x1038,
            product_id: 0x227e,
            interface_number: 3,
            usage_page: 0xffc0,
            usage: 1,
            serial_number: Some("fixture".into()),
            manufacturer: None,
            path: "injected".into(),
        };
        Ok(Box::new(
            Nova7Gen2::new(
                info,
                GateTransport {
                    wire: w.clone(),
                    entered: e.clone(),
                    release: r.clone(),
                    block_on: 0x20,
                    exited: done.clone(),
                },
            )
            .unwrap(),
        ))
    });
    owner.enable("1038:227e:fixture").unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while owner.snapshot().sample.is_none() {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    owner
        .queue(
            "1038:227e:fixture",
            SettingsCommand {
                sidetone: Some(2),
                auto_off_minutes: Some(30),
                ..Default::default()
            },
        )
        .unwrap();
    while !entered.load(Ordering::Acquire) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(2));
    }
    owner.stop();
    release.store(true, Ordering::Release);
    while !exited.load(Ordering::Acquire) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(
        owner.snapshot().last_command.as_deref(),
        Some("completed"),
        "cancelled readback reported success"
    );
    assert_eq!(owner.snapshot().sidetone, None, "cancelled readback was accepted");
    drop(owner);
    let writes = &wire.lock().unwrap().writes;
    assert!(
        !writes.iter().any(|w| w.get(1) == Some(&0x37)),
        "auto-off after stop: {writes:?}"
    );
    // The readback reply is buffered, but must not be committed as success.
    assert_eq!(writes.iter().filter(|w| w.get(1) == Some(&0x20)).count(), 1);
}

struct BlockingInventory {
    entered: Arc<AtomicBool>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    opens: Arc<AtomicBool>,
}
impl Access for BlockingInventory {
    fn inventory(&mut self) -> Result<Vec<Endpoint>, String> {
        self.entered.store(true, Ordering::Release);
        let (ready, changed) = &*self.gate;
        let mut ready = ready.lock().unwrap();
        while !*ready {
            ready = changed.wait(ready).unwrap();
        }
        Ok(vec![Endpoint {
            id: "1038:1642:fixture".into(),
            vendor_id: 0x1038,
            product_id: 0x1642,
            interface: 1,
            path: "/private/fixture".into(),
        }])
    }
    fn open(&mut self, _: &Endpoint) -> Result<Box<dyn LightTransport>, String> {
        self.opens.store(true, Ordering::Release);
        Ok(Box::new(MemoryLight))
    }
}
struct MemoryLight;
impl LightTransport for MemoryLight {
    fn send_feature(&mut self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
}
#[test]
fn rgb_inventory_wait_is_off_rpc_thread_and_preserves_exact_selection() {
    let entered = Arc::new(AtomicBool::new(false));
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let opens = Arc::new(AtomicBool::new(false));
    let mut controller = AsyncController::new(LightingController::with_access(Box::new(BlockingInventory {
        entered: entered.clone(),
        gate: gate.clone(),
        opens: opens.clone(),
    })));
    let (tx, rx) = std::sync::mpsc::channel();
    let caller = thread::spawn(move || {
        let result = controller.queue(Apply {
            id: "1038:1642:stale".into(),
            allow_hardware: true,
            color: [1, 2, 3],
            brightness: 100,
        });
        tx.send((result, controller)).unwrap();
    });
    let until = Instant::now() + Duration::from_secs(2);
    while !entered.load(Ordering::Acquire) {
        assert!(Instant::now() < until, "inventory was not entered");
        thread::sleep(Duration::from_millis(1));
    }
    let response = rx.recv_timeout(Duration::from_millis(200));
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    caller.join().unwrap();
    let (accepted, mut controller) = response.expect("blocking inventory stalled caller");
    assert!(
        accepted.is_ok(),
        "valid shape should be accepted pending exact-node check: {accepted:?}"
    );
    let mut entries = vec![serde_json::json!({"id":"1038:1642:stale"})];
    for _ in 0..200 {
        controller.decorate(&mut entries);
        if entries[0]["lighting"]["pending"] == false {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    assert!(!opens.load(Ordering::Acquire), "stale identity must never open");
    assert!(
        entries[0]["lighting"]["error"]
            .as_str()
            .unwrap()
            .contains("Exact lighting interface")
    );
}
