//! Model-specific, one-shot lighting writes. Never persisted, acquired at startup or replayed.
//! Allowlist evidence (model + interface):
//! https://github.com/CalcProgrammer1/OpenRGB/blob/19112cdeff94086fa994c4a9cf45ec5786a599bf/Controllers/SteelSeriesController/SteelSeriesControllerDetect.cpp
//! 1628 is intentionally excluded: its firmware-dependent protocol disagrees with the
//! existing experimental 645-byte implementation. Do not bypass that feature gate.
use crate::devices::hid_reports::{
    HidDeviceType, HidReportBuilder,
    apex_gen3::{ApexGen3Command, REPORT_SIZE},
};
use crate::rgb::Color;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::{collections::BTreeMap, path::PathBuf};

pub fn capability(vendor: u16, product: u16, interface: i32) -> serde_json::Value {
    let supported = vendor == 0x1038 && product == 0x1642 && interface == 1;
    let applicable = vendor == 0x1038 && [0x1642, 0x1628].contains(&product);
    let reason = if supported {
        "Solid color only; source-derived Apex Pro TKL Gen 3 wired protocol. Local RGB hardware validation pending. Explicit Apply required; no readback or on-board save."
    } else if vendor == 0x1038 && product == 0x227e {
        "This headset has no documented RGB lighting. No RGB commands will be sent."
    } else if vendor == 0x1038 && product == 0x1628 {
        "RGB unavailable: the Apex Pro TKL 2023 protocol is firmware-dependent and experimental; its existing feature gate is not bypassed."
    } else {
        "RGB unavailable: no source-verified implementation for this exact model and interface. No RGB commands will be sent."
    };
    serde_json::json!({"supported":supported,"applicable":applicable,"locallyValidated":false,"reason":reason})
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Apply {
    pub id: String,
    pub allow_hardware: bool,
    pub color: [u8; 3],
    pub brightness: u8,
}
impl Apply {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.len() > 256 || self.brightness > 100 {
            return Err("Invalid device identity or brightness (0..100)".into());
        }
        if !self.allow_hardware {
            return Err("Explicit per-Apply hardware permission required".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    pub color: [u8; 3],
    pub brightness: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub id: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: i32,
    pub path: PathBuf,
}
impl Endpoint {
    pub fn supported(&self) -> bool {
        self.vendor_id == 0x1038 && self.product_id == 0x1642 && self.interface == 1
    }
    pub fn from_info(info: &hidapi::DeviceInfo) -> Result<Self, String> {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(info.path().to_bytes()));
        let meta = path.metadata().map_err(|e| e.to_string())?;
        // Node generation prevents stale UI authorization from following a replug at
        // the same hidraw path. No HID open/query is needed to build this identity.
        let id = format!(
            "{:04x}:{:04x}:{}:{}:{}:{}",
            info.vendor_id(),
            info.product_id(),
            meta.dev(),
            meta.ino(),
            meta.ctime(),
            meta.ctime_nsec()
        );
        Ok(Self {
            id,
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            interface: info.interface_number(),
            path,
        })
    }
}
/// Injection below the actual feature-report writer; tests never open a real HID device.
pub trait Transport: Send {
    fn send_feature(&mut self, bytes: &[u8]) -> Result<(), String>;
}
pub trait Access: Send {
    fn inventory(&mut self) -> Result<Vec<Endpoint>, String>;
    fn open(&mut self, endpoint: &Endpoint) -> Result<Box<dyn Transport>, String>;
}
fn lock_interface(path: &std::path::Path) -> Result<std::fs::File, String> {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|e| format!("Lighting interface busy: {e}"))?;
    Ok(lock)
}
struct HidAccess;
struct LockedHid {
    device: hidapi::HidDevice,
    _lock: std::fs::File,
}
impl Transport for LockedHid {
    fn send_feature(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.device.send_feature_report(bytes).map_err(|e| e.to_string())
    }
}
impl Access for HidAccess {
    fn inventory(&mut self) -> Result<Vec<Endpoint>, String> {
        let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
        api.device_list()
            .filter(|d| d.vendor_id() == 0x1038 && d.product_id() == 0x1642 && d.interface_number() == 1)
            .map(Endpoint::from_info)
            .collect()
    }
    fn open(&mut self, selected: &Endpoint) -> Result<Box<dyn Transport>, String> {
        // Advisory lock on the device node, not a config-directory lock: separate
        // SSGG services cannot overlap writes. Other RGB software must be closed.
        let lock = lock_interface(&selected.path)?;
        let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
        let matches: Vec<_> = api
            .device_list()
            .filter(|d| Endpoint::from_info(d).as_ref() == Ok(selected))
            .collect();
        if matches.len() != 1 {
            return Err("Lighting interface disappeared or is ambiguous".into());
        }
        let device = matches[0].open_device(&api).map_err(|e| e.to_string())?;
        let actual = Endpoint::from_info(&device.get_device_info().map_err(|e| e.to_string())?)?;
        if actual != *selected {
            return Err("Opened lighting interface identity changed".into());
        }
        Ok(Box::new(LockedHid { device, _lock: lock }))
    }
}
pub struct Controller {
    access: Box<dyn Access>,
    last_sent: BTreeMap<String, Sent>,
}
impl Default for Controller {
    fn default() -> Self {
        Self::with_access(Box::new(HidAccess))
    }
}
impl Controller {
    pub fn with_access(access: Box<dyn Access>) -> Self {
        Self {
            access,
            last_sent: BTreeMap::new(),
        }
    }
    fn validate_request(request: &Apply) -> Result<(), String> {
        request.validate()?;
        if !request.id.starts_with("1038:1642:") {
            return Err("RGB is implemented only for Apex Pro TKL Gen 3 wired (1038:1642)".into());
        }
        Ok(())
    }
    fn select(&mut self, request: &Apply) -> Result<Endpoint, String> {
        Self::validate_request(request)?;
        let inventory = self.access.inventory()?;
        let candidates: Vec<_> = inventory.iter().filter(|d| d.id == request.id).collect();
        if candidates.len() != 1 || !candidates[0].supported() {
            return Err("Exact lighting interface absent, unsupported or ambiguous; refresh devices".into());
        }
        Ok(candidates[0].clone())
    }
    /// Synchronous protocol test seam. The live service uses AsyncController below.
    pub fn apply(&mut self, request: Apply) -> Result<Sent, String> {
        self.apply_cancelled(request, &AtomicBool::new(false))
    }
    fn apply_cancelled(&mut self, request: Apply, cancelled: &AtomicBool) -> Result<Sent, String> {
        let check = || {
            if cancelled.load(Ordering::Acquire) {
                Err("Lighting write cancelled".to_string())
            } else {
                Ok(())
            }
        };
        check()?;
        let selected = self.select(&request)?;
        check()?;
        let mut device = self.access.open(&selected)?;
        check()?;
        self.last_sent.remove(&request.id);
        let scale = |v: u8| ((u16::from(v) * u16::from(request.brightness) + 50) / 100) as u8;
        let color = Color::new(
            scale(request.color[0]),
            scale(request.color[1]),
            scale(request.color[2]),
        );
        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0; REPORT_SIZE];
        let size = builder
            .build_report(ApexGen3Command::Initialize, &mut buffer)
            .map_err(|e| e.to_string())?;
        check()?;
        device.send_feature(&buffer[..size])?;
        // Same settling allowance as the existing Gen 3 driver; headset owner runs independently.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let size = builder
            .build_report(ApexGen3Command::Solid(color), &mut buffer)
            .map_err(|e| e.to_string())?;
        check()?;
        device.send_feature(&buffer[..size])?;
        let sent = Sent {
            color: request.color,
            brightness: request.brightness,
        };
        self.last_sent.insert(request.id, sent.clone());
        // Drop releases only our handle/lock. No destructor writes or on-board save.
        Ok(sent)
    }
    pub fn decorate(&mut self, entries: &mut [serde_json::Value]) {
        self.last_sent.retain(|id, _| entries.iter().any(|d| d["id"] == *id));
        for entry in entries {
            if let Some(id) = entry["id"].as_str() {
                if let Some(sent) = self.last_sent.get(id) {
                    entry["lighting"] = serde_json::json!({"lastSent":sent});
                }
            }
        }
    }
}
struct Job {
    id: String,
    cancelled: Arc<AtomicBool>,
    completion: Receiver<(Controller, Result<Sent, String>)>,
}
/// A stalled HID open/ioctl never blocks service ticks or the independent headset owner.
/// Only one job is permitted; cancellation is checked at every report boundary.
pub struct AsyncController {
    idle: Option<Controller>,
    job: Option<Job>,
    errors: BTreeMap<String, String>,
}
impl AsyncController {
    pub fn new(controller: Controller) -> Self {
        Self {
            idle: Some(controller),
            job: None,
            errors: BTreeMap::new(),
        }
    }
    pub fn queue(&mut self, request: Apply) -> Result<(), String> {
        self.poll();
        // Validate the explicit permission and model cheaply on this thread. The
        // exact-node inventory is checked in the one-shot worker before open.
        // Enumeration can block on a stalled keyboard and must not hold RPC/audio.
        Controller::validate_request(&request)?;
        if self.idle.is_none() {
            return Err("Lighting is busy or its worker unavailable; wait or restart the service".into());
        }
        let mut controller = self.idle.take().ok_or("Lighting worker unavailable")?;
        let id = request.id.clone();
        self.errors.remove(&id);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let (tx, completion) = mpsc::channel();
        std::thread::Builder::new()
            .name("ssgg-rgb-apply".into())
            .spawn(move || {
                let result = controller.apply_cancelled(request, &worker_cancelled);
                if tx.send((controller, result)).is_err() {
                    tracing::debug!("Lighting service dropped; worker resources released");
                }
            })
            .map_err(|e| format!("Cannot start lighting worker; restart service: {e}"))?;
        self.job = Some(Job {
            id,
            cancelled,
            completion,
        });
        Ok(())
    }
    pub fn poll(&mut self) {
        let Some(job) = self.job.as_ref() else {
            return;
        };
        match job.completion.try_recv() {
            Ok((controller, result)) => {
                if let Err(error) = result {
                    self.errors.insert(job.id.clone(), error);
                }
                self.idle = Some(controller);
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.errors
                    .insert(job.id.clone(), "Lighting worker stopped; restart service".into());
                self.job = None;
            }
        }
    }
    pub fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.cancelled.store(true, Ordering::Release);
        }
    }
    pub fn decorate(&mut self, entries: &mut [serde_json::Value]) {
        self.poll();
        if let Some(controller) = &mut self.idle {
            controller.decorate(entries);
        }
        self.errors.retain(|id, _| entries.iter().any(|d| d["id"] == *id));
        for entry in entries {
            let Some(id) = entry["id"].as_str() else {
                continue;
            };
            let pending = self.job.as_ref().is_some_and(|job| job.id == id);
            let error = self.errors.get(id).cloned();
            if pending || error.is_some() {
                entry["lighting"] = serde_json::json!({"pending":pending,"error":error,"lastSent":null});
            }
        }
    }
}
impl Drop for AsyncController {
    fn drop(&mut self) {
        self.cancel();
    } // Never join a stalled ioctl or send a destructor report.
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_node_lock_excludes_concurrent_services_and_releases_on_drop() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let first = super::lock_interface(file.path()).unwrap();
        assert!(super::lock_interface(file.path()).is_err());
        drop(first);
        assert!(super::lock_interface(file.path()).is_ok());
    }
}
