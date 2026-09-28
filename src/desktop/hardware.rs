//! Opt-in single-owner HID worker. Audio/RPC never wait on HID reads or queries.
use crate::devices::headsets::Headset;
use crate::devices::{
    discovery::DeviceManager,
    headsets::nova7_gen2::{ChatMixSample, Nova7Gen2, Report, Status, Transport, is_control_interface},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, SyncSender};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// Local Gen 2 captures reported wheel positions only in status replies, not
// unsolicited events. Keep that fallback interactive while hardware is opted in.
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    #[default]
    Software,
    Hardware,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Physical {
    pub device_id: Option<String>,
    pub hardware_enabled: bool,
    pub hardware_acquired: bool,
    pub connected: Option<bool>,
    pub battery: Option<u8>,
    pub charging: Option<bool>,
    pub sample: Option<Sample>,
    pub status_at_ms: Option<u64>,
    pub stale: bool,
    pub pending: bool,
    pub error: Option<String>,
    pub sidetone: Option<u8>,
    pub auto_off_minutes_sent: Option<u8>,
    pub last_command: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub game_percent: u8,
    pub chat_percent: u8,
    pub balance: f64,
    pub received_at_ms: u64,
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
impl From<ChatMixSample> for Sample {
    fn from(v: ChatMixSample) -> Self {
        Self {
            game_percent: v.game_percent,
            chat_percent: v.chat_percent,
            balance: (f64::from(v.chat_percent) - f64::from(v.game_percent)) / 100.0,
            received_at_ms: now_ms(),
        }
    }
}
/// Test seam below the real model driver, not a simulated service/event mapper.
pub trait HardwareDevice: Send {
    fn read_event(&mut self, timeout_ms: i32) -> crate::Result<Option<Report>>;
    fn request_status(&mut self) -> crate::Result<Status>;
    fn request_status_cancellable(&mut self, is_cancelled: &dyn Fn() -> bool) -> crate::Result<Status> {
        if is_cancelled() {
            return Err(crate::Error::DeviceCommunication("Status query cancelled".into()));
        }
        self.request_status()
    }
    fn set_sidetone(&mut self, level: u8, observe: &mut dyn FnMut(Report) -> crate::Result<()>) -> crate::Result<u8>;
    fn set_sidetone_cancellable(
        &mut self,
        level: u8,
        observe: &mut dyn FnMut(Report) -> crate::Result<()>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> crate::Result<u8> {
        if is_cancelled() {
            return Err(crate::Error::DeviceCommunication("Sidetone cancelled".into()));
        }
        self.set_sidetone(level, observe)
    }
    fn set_auto_off(&mut self, minutes: u8) -> crate::Result<()>;
}
impl<T: Transport> HardwareDevice for Nova7Gen2<T> {
    fn read_event(&mut self, timeout_ms: i32) -> crate::Result<Option<Report>> {
        Nova7Gen2::read_event(self, timeout_ms)
    }
    fn request_status(&mut self) -> crate::Result<Status> {
        Nova7Gen2::request_status(self)
    }
    fn request_status_cancellable(&mut self, is_cancelled: &dyn Fn() -> bool) -> crate::Result<Status> {
        Nova7Gen2::request_status_cancellable(self, is_cancelled)
    }
    fn set_sidetone(&mut self, level: u8, observe: &mut dyn FnMut(Report) -> crate::Result<()>) -> crate::Result<u8> {
        self.set_sidetone_level(level)?;
        self.sidetone_level_observed(observe)
    }
    fn set_sidetone_cancellable(
        &mut self,
        level: u8,
        observe: &mut dyn FnMut(Report) -> crate::Result<()>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> crate::Result<u8> {
        self.set_sidetone_level_cancellable(level, is_cancelled)?;
        if is_cancelled() {
            return Err(crate::Error::DeviceCommunication("Sidetone readback cancelled".into()));
        }
        self.sidetone_level_observed_cancellable(is_cancelled, observe)
    }
    fn set_auto_off(&mut self, minutes: u8) -> crate::Result<()> {
        Headset::set_auto_off(self, minutes)
    }
}
#[derive(Clone, Copy, Default)]
pub struct SettingsCommand {
    pub sidetone: Option<u8>,
    pub auto_off_minutes: Option<u8>,
    pub status_refresh: bool,
}
impl SettingsCommand {
    pub fn is_empty(&self) -> bool {
        self.sidetone.is_none() && self.auto_off_minutes.is_none() && !self.status_refresh
    }
}
type Factory = dyn Fn(&str) -> Result<Box<dyn HardwareDevice>, String> + Send + Sync;
#[derive(Default)]
struct Shared {
    state: Physical,
    status_seen: Option<Instant>,
    outstanding: usize,
}
struct Owner {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
    commands: SyncSender<SettingsCommand>,
}
pub struct Controller {
    factory: Arc<Factory>,
    shared: Arc<Mutex<Shared>>,
    owner: Option<Owner>,
}
impl Default for Controller {
    fn default() -> Self {
        Self::with_factory(open)
    }
}
fn open(id: &str) -> Result<Box<dyn HardwareDevice>, String> {
    let manager = DeviceManager::new().map_err(|e| e.to_string())?;
    let controls: Vec<_> = manager
        .devices()
        .into_iter()
        .filter(|d| is_control_interface(d))
        .collect();
    // The manager resolves by VID/PID, not serial. Refuse ambiguity rather than
    // risking a command to a different receiver with the same product ID.
    if controls.len() != 1 {
        return Err("Need exactly one Nova 7 Gen 2 control receiver; absent or ambiguous".into());
    }
    let info = controls[0];
    let actual = format!("1038:227e:{}", info.serial_number.as_deref().unwrap_or("usb"));
    if actual != id {
        return Err("Selected receiver disappeared or identity changed".into());
    }
    Ok(Box::new(manager.open_nova7_gen2(info).map_err(|e| e.to_string())?))
}
impl Controller {
    pub fn with_factory(
        factory: impl Fn(&str) -> Result<Box<dyn HardwareDevice>, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            factory: Arc::new(factory),
            shared: Arc::new(Mutex::new(Shared::default())),
            owner: None,
        }
    }
    /// Recheck the live owner at audio command boundaries, not a cached GUI sample.
    pub fn audio_write_guard(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let shared = self.shared.clone();
        Arc::new(move || {
            let shared = shared.lock();
            shared.state.hardware_enabled
                && shared.state.hardware_acquired
                && shared.state.connected == Some(true)
                && shared.state.sample.is_some()
                && shared
                    .status_seen
                    .is_some_and(|t| t.elapsed() <= Duration::from_secs(15))
        })
    }

    pub fn snapshot(&mut self) -> Physical {
        let mut shared = self.shared.lock();
        // Expire only an active acquisition, atomically with worker updates. A
        // failed owner may retain its last status time; preserve its actual error.
        if shared.state.hardware_enabled
            && shared
                .status_seen
                .is_some_and(|t| t.elapsed() > Duration::from_secs(15))
        {
            if let Some(owner) = &self.owner {
                owner.stop.store(true, Ordering::Release);
            }
            invalidate(&mut shared.state);
            shared.status_seen = None;
            shared.state.stale = true;
            shared.state.error = Some("Hardware status expired; explicitly reacquire".into());
        }
        shared.state.clone()
    }
    pub fn enable(&mut self, id: &str) -> Result<(), String> {
        if !id.starts_with("1038:227e:") {
            return Err("Unsupported receiver identity".into());
        }
        let state = self.snapshot();
        if state.hardware_enabled {
            if state.device_id.as_deref() == Some(id) {
                return Ok(());
            }
            return Err("Release the current receiver first".into());
        }
        if let Some(owner) = self.owner.take() {
            if !owner.join.is_finished() {
                self.owner = Some(owner);
                return Err("Previous HID owner is stopping; retry shortly".into());
            }
            owner.join.join().map_err(|_| "HID owner panicked".to_string())?;
        }
        *self.shared.lock() = Shared {
            state: Physical {
                device_id: Some(id.into()),
                hardware_enabled: true,
                pending: true,
                ..Default::default()
            },
            status_seen: None,
            outstanding: 1,
        };
        let (commands, rx) = mpsc::sync_channel::<SettingsCommand>(8);
        let shared = self.shared.clone();
        let factory = self.factory.clone();
        let id = id.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let join = thread::spawn(move || {
            let run = || -> Result<(), String> {
                if cancelled.load(Ordering::Acquire) {
                    return Ok(());
                }
                let mut device = factory(&id)?;
                if cancelled.load(Ordering::Acquire) {
                    return Ok(());
                }
                shared.lock().state.hardware_acquired = true;
                let is_cancelled = || cancelled.load(Ordering::Acquire);
                let status = device
                    .request_status_cancellable(&is_cancelled)
                    .map_err(|e| e.to_string())?;
                accept(&shared, Report::Status(status))?;
                complete(&shared);
                let mut next_status = Instant::now() + STATUS_POLL_INTERVAL;
                while !cancelled.load(Ordering::Acquire) {
                    if let Ok(command) = rx.try_recv() {
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        if let Some(level) = command.sidetone {
                            let actual = device
                                .set_sidetone_cancellable(
                                    level,
                                    &mut |report| accept(&shared, report).map_err(crate::Error::DeviceCommunication),
                                    &is_cancelled,
                                )
                                .map_err(|e| e.to_string())?;
                            shared.lock().state.sidetone = Some(actual);
                            if actual != level {
                                return Err("Sidetone readback did not match requested level".into());
                            }
                        }
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        if let Some(minutes) = command.auto_off_minutes {
                            device.set_auto_off(minutes).map_err(|e| e.to_string())?;
                            shared.lock().state.auto_off_minutes_sent = Some(minutes);
                        }
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        // Recover wheel packets consumed by the settings query.
                        accept(
                            &shared,
                            Report::Status(
                                device
                                    .request_status_cancellable(&is_cancelled)
                                    .map_err(|e| e.to_string())?,
                            ),
                        )?;
                        shared.lock().state.last_command = Some("completed".into());
                        complete(&shared);
                        next_status = Instant::now() + STATUS_POLL_INTERVAL;
                    } else if Instant::now() >= next_status {
                        accept(
                            &shared,
                            Report::Status(
                                device
                                    .request_status_cancellable(&is_cancelled)
                                    .map_err(|e| e.to_string())?,
                            ),
                        )?;
                        next_status = Instant::now() + STATUS_POLL_INTERVAL;
                    } else if let Some(report) = device.read_event(50).map_err(|e| e.to_string())? {
                        accept(&shared, report)?;
                    }
                }
                Ok(())
            };
            let result = run();
            let mut shared = shared.lock();
            invalidate(&mut shared.state);
            if let Err(e) = result
                && !cancelled.load(Ordering::Acquire)
            {
                shared.state.error = Some(e);
                shared.state.last_command = Some("failed".into());
            }
        });
        self.owner = Some(Owner { stop, join, commands });
        Ok(())
    }
    pub fn queue(&mut self, id: &str, command: SettingsCommand) -> Result<(), String> {
        if command.sidetone.is_some_and(|v| v > 3) {
            return Err("Sidetone outside 0..3".into());
        }
        let mut shared = self.shared.lock();
        if !shared.state.hardware_enabled || shared.state.device_id.as_deref() != Some(id) {
            return Err("Explicit acquisition required for this receiver".into());
        }
        let owner = self.owner.as_ref().ok_or("No HID owner")?;
        owner.commands.try_send(command).map_err(|e| e.to_string())?;
        shared.outstanding += 1;
        shared.state.pending = true;
        shared.state.last_command = Some("queued".into());
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(owner) = &self.owner {
            owner.stop.store(true, Ordering::Release);
        }
        let mut shared = self.shared.lock();
        invalidate(&mut shared.state);
        shared.status_seen = None;
    }
}
fn complete(shared: &Mutex<Shared>) {
    let mut shared = shared.lock();
    shared.outstanding = shared.outstanding.saturating_sub(1);
    shared.state.pending = shared.state.hardware_enabled && shared.outstanding > 0;
}
fn invalidate(state: &mut Physical) {
    if state.connected == Some(true) {
        state.connected = None;
    }
    state.stale = state.status_at_ms.is_some();
    state.hardware_enabled = false;
    state.hardware_acquired = false;
    state.sample = None;
    state.battery = None;
    state.charging = None;
    state.pending = false;
}
fn accept(shared: &Mutex<Shared>, report: Report) -> Result<(), String> {
    let mut s = shared.lock();
    if !s.state.hardware_enabled {
        return Ok(());
    }
    match report {
        Report::Status(status) => {
            s.state.connected = status.connected;
            s.state.status_at_ms = Some(now_ms());
            s.status_seen = Some(Instant::now());
            if status.connected != Some(true) {
                invalidate(&mut s.state);
                return Err("Headset offline or power unknown; explicitly reacquire".into());
            }
            s.state.battery = status.battery_percent;
            s.state.charging = status.charging;
            s.state.sample = Some(status.chatmix.into());
        }
        Report::ChatMix(sample) if s.state.connected == Some(true) => s.state.sample = Some(sample.into()),
        _ => {}
    }
    Ok(())
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop();
        if let Some(owner) = self.owner.take() {
            if owner.join.join().is_err() {
                eprintln!("desktop HID owner panicked during shutdown");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ScriptedTransport {
        reports: std::collections::VecDeque<crate::Result<Vec<u8>>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }
    impl Transport for ScriptedTransport {
        fn write(&mut self, data: &[u8]) -> crate::Result<usize> {
            self.writes.lock().push(data.to_vec());
            Ok(data.len())
        }
        fn read_timeout(&mut self, data: &mut [u8], _: i32) -> crate::Result<usize> {
            let report = self.reports.pop_front().expect("unexpected HID read")?;
            data[..report.len()].copy_from_slice(&report);
            Ok(report.len())
        }
    }

    #[test]
    fn expired_snapshot_preserves_original_hid_failure() {
        assert_failure_survives_expiry(
            Err(crate::Error::DeviceCommunication("injected unplug".into())),
            "Device communication error: injected unplug",
        );
    }

    #[test]
    fn expired_snapshot_preserves_offline_failure() {
        assert_failure_survives_expiry(
            Ok(vec![0xb0, 2, 73, 0, 100, 100]),
            "Headset offline or power unknown; explicitly reacquire",
        );
    }

    #[test]
    fn expired_snapshot_preserves_malformed_report_failure() {
        assert_failure_survives_expiry(
            Ok(vec![0x45, 101, 70]),
            "Device communication error: Nova 7 Gen 2: percentage outside 0..=100",
        );
    }

    fn assert_failure_survives_expiry(failure: crate::Result<Vec<u8>>, expected: &str) {
        let failure = Mutex::new(Some(failure));
        let writes = Arc::new(Mutex::new(Vec::new()));
        let recorded = writes.clone();
        let mut controller = Controller::with_factory(move |_| {
            assert!(recorded.lock().is_empty(), "must not automatically reacquire");
            Ok(Box::new(
                Nova7Gen2::new(
                    crate::devices::DeviceInfo {
                        name: "Nova7 fixture".into(),
                        device_type: crate::devices::DeviceType::Headset,
                        vendor_id: 0x1038,
                        product_id: 0x227e,
                        interface_number: 3,
                        usage_page: 0xffc0,
                        usage: 1,
                        serial_number: Some("test".into()),
                        manufacturer: None,
                        path: "injected".into(),
                    },
                    ScriptedTransport {
                        reports: [Ok(vec![0xb0, 3, 73, 3, 40, 70]), failure.lock().take().unwrap()].into(),
                        writes: recorded.clone(),
                    },
                )
                .unwrap(),
            ))
        });
        controller.enable("1038:227e:test").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !controller.owner.as_ref().unwrap().join.is_finished() {
            assert!(Instant::now() < deadline, "HID worker did not finish");
            thread::sleep(Duration::from_millis(1));
        }
        let original = controller.snapshot();
        assert_eq!(original.error.as_deref(), Some(expected));
        assert!(original.status_at_ms.is_some(), "a status must precede the failure");
        controller.shared.lock().status_seen = Some(Instant::now() - Duration::from_secs(16));
        for _ in 0..2 {
            let state = controller.snapshot();
            assert_eq!(state.error, original.error, "expiry must not mask the HID failure");
            assert_eq!(state.last_command.as_deref(), Some("failed"));
            assert!(!state.hardware_enabled);
            assert!(!state.hardware_acquired);
            assert!(!state.pending);
            assert!(state.stale);
            assert!(state.sample.is_none());
            assert!(!controller.audio_write_guard()());
            assert!(controller.queue("1038:227e:test", SettingsCommand::default()).is_err());
        }
        assert_eq!(
            *writes.lock(),
            vec![vec![0x00, 0xb0]],
            "no settings or reacquisition writes"
        );
    }

    #[test]
    fn completion_after_cancel_does_not_republish_pending() {
        let shared = Mutex::new(Shared {
            outstanding: 2,
            ..Default::default()
        });
        complete(&shared);
        assert!(!shared.lock().state.pending);
    }
    #[test]
    fn fresh_wheel_cannot_refresh_expired_connection() {
        let mut controller = Controller::with_factory(|_| panic!("must not open"));
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let (commands, _rx) = mpsc::sync_channel(1);
        controller.owner = Some(Owner {
            stop: stop.clone(),
            join: thread::spawn(move || {
                while !cancelled.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(1));
                }
            }),
            commands,
        });
        {
            let mut shared = controller.shared.lock();
            shared.state.hardware_enabled = true;
            shared.state.hardware_acquired = true;
            shared.state.pending = true;
            shared.state.connected = Some(true);
            shared.status_seen = Some(Instant::now() - Duration::from_secs(16));
            shared.state.status_at_ms = Some(now_ms());
        }
        accept(
            &controller.shared,
            Report::ChatMix(ChatMixSample {
                game_percent: 40,
                chat_percent: 70,
            }),
        )
        .unwrap();
        let state = controller.snapshot();
        assert!(state.sample.is_none());
        assert!(state.stale);
        assert!(!state.hardware_enabled);
        assert!(!state.hardware_acquired);
        assert!(!state.pending);
        assert!(state.connected.is_none());
        assert_eq!(
            state.error.as_deref(),
            Some("Hardware status expired; explicitly reacquire")
        );
        assert!(stop.load(Ordering::Acquire), "expired owner must be cancelled");
        assert!(!controller.audio_write_guard()());
        assert_eq!(controller.snapshot().error, state.error);
        accept(
            &controller.shared,
            Report::Status(Status {
                battery_percent: Some(73),
                connected: Some(true),
                charging: Some(false),
                chatmix: ChatMixSample {
                    game_percent: 40,
                    chat_percent: 70,
                },
                raw_power: 3,
                raw_charge: 3,
            }),
        )
        .unwrap();
        assert!(
            controller.snapshot().sample.is_none(),
            "late reports must not restore gains"
        );
        assert!(!controller.audio_write_guard()());
    }
}
