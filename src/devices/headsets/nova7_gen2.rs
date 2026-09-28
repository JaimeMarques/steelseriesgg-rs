//! Dedicated Arctis Nova 7 Gen 2 (1038:227e) protocol.
//!
//! Source-implemented, NOT yet validated against this application's hardware.
//! References:
//! - https://github.com/Sapd/HeadsetControl/blob/86dfc452b63aea63dfa8e0ae2ce2f53548e7073b/lib/devices/steelseries_arctis_nova_7.hpp
//! - https://github.com/Sapd/HeadsetControl/blob/86dfc452b63aea63dfa8e0ae2ce2f53548e7073b/lib/devices/protocols/steelseries_protocol.hpp
//! - https://github.com/elegos/Linux-Arctis-Manager/blob/9d877ef1c852fa714c489e64cc2f98e4c1c8a8bc/src/linux_arctis_manager/devices/nova_7_wireless_perc_battery.yaml
//! - https://downloads.steelseriescdn.com/guides/HS_arctis_nova_7_wl_gen_2_pig_pc_web.pdf
//!
//! The first two sources establish direct 0–100 battery and the settings commands.
//! The third documents unsolicited 0x45 wheel events and power byte 1. Its battery
//! scale differs (128); we deliberately follow the PID-specific HeadsetControl code.
//! Never use GenericHeadset commands: in particular 0x39 is SIDETONE, not mute.
//! The official manual requires Sonar for PC ChatMix: Linux must apply these
//! wheel gains to separate host game/chat buses. The dongle does not do that mix.

use crate::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChatMixSample {
    pub game_percent: u8,
    pub chat_percent: u8,
}

impl ChatMixSample {
    /// -1 = game, 0 = center (both full), +1 = chat. Apply the two gains
    /// directly for physical ChatMix; this scalar is only a UI representation.
    pub fn balance(self) -> f32 {
        (f32::from(self.chat_percent) - f32::from(self.game_percent)) / 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    /// None for disconnected/unknown power state; never reuse stale battery.
    pub battery_percent: Option<u8>,
    /// Unknown wire values are not silently interpreted as connected.
    pub connected: Option<bool>,
    pub charging: Option<bool>,
    pub chatmix: ChatMixSample,
    pub raw_power: u8,
    pub raw_charge: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Report {
    Status(Status),
    ChatMix(ChatMixSample),
    Sidetone(u8),
    Unknown(u8),
}

fn protocol_error(message: &str) -> Error {
    Error::DeviceCommunication(format!("Nova 7 Gen 2: {message}"))
}

fn percent(value: u8) -> Result<u8> {
    if value > 100 {
        return Err(protocol_error("percentage outside 0..=100"));
    }
    Ok(value)
}

/// Decode precisely the bytes returned by hidapi (no synthetic report-ID prefix).
/// Unknown opcodes are ignorable; truncated known reports and invalid fields fail.
pub fn decode_report(data: &[u8]) -> Result<Report> {
    if data.len() > 128 {
        return Err(protocol_error("oversized report"));
    }
    let Some(&opcode) = data.first() else {
        return Err(protocol_error("empty report"));
    };
    match opcode {
        0x20 => {
            if data.len() < 4 || data[2] > 3 {
                return Err(protocol_error("invalid audio settings report"));
            }
            Ok(Report::Sidetone(data[2]))
        }
        0x45 => {
            if data.len() < 3 {
                return Err(protocol_error("truncated wheel report"));
            }
            Ok(Report::ChatMix(ChatMixSample {
                game_percent: percent(data[1])?,
                chat_percent: percent(data[2])?,
            }))
        }
        0xb0 => {
            if data.len() < 6 {
                return Err(protocol_error("truncated status report"));
            }
            let connected = match (data[1], data[3]) {
                (_, 0) | (2, _) => Some(false),
                (3, _) => Some(true),
                _ => None,
            };
            let battery_percent = if connected == Some(true) {
                Some(percent(data[2])?)
            } else {
                None
            };
            let charging = match data[3] {
                1 | 2 => Some(true),
                3 => Some(false),
                _ => None,
            };
            Ok(Report::Status(Status {
                battery_percent,
                connected,
                charging,
                chatmix: ChatMixSample {
                    game_percent: percent(data[4])?,
                    chat_percent: percent(data[5])?,
                },
                raw_power: data[1],
                raw_charge: data[3],
            }))
        }
        _ => Ok(Report::Unknown(opcode)),
    }
}

use crate::devices::hid_reports::Nova7Gen2Command;
use crate::devices::{DeviceInfo, HidDeviceType, HidReportBuilder, product_ids::ARCTIS_NOVA_7_GEN2};
use hidapi::HidDevice;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// Injectable transport: tests never enumerate/open a real HID device.
pub trait Transport: Send {
    fn write(&mut self, data: &[u8]) -> Result<usize>;
    fn read_timeout(&mut self, data: &mut [u8], timeout_ms: i32) -> Result<usize>;
}
impl Transport for HidDevice {
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        Ok(HidDevice::write(self, data)?)
    }
    fn read_timeout(&mut self, data: &mut [u8], timeout_ms: i32) -> Result<usize> {
        Ok(HidDevice::read_timeout(self, data, timeout_ms)?)
    }
}

/// Exact control interface only; other collections must not receive settings.
pub fn is_control_interface(info: &DeviceInfo) -> bool {
    info.vendor_id == 0x1038
        && info.product_id == ARCTIS_NOVA_7_GEN2
        && info.interface_number == 3
        && info.usage_page == 0xffc0
        && info.usage == 1
}

/// Single owner for command responses and unsolicited wheel reports.
/// Opening/constructing does not write, read, reset, or apply settings.
pub struct Nova7Gen2<T: Transport = HidDevice> {
    info: DeviceInfo,
    transport: Mutex<Option<T>>,
    last_chatmix: Option<ChatMixSample>,
}

impl<T: Transport> Nova7Gen2<T> {
    pub fn new(info: DeviceInfo, transport: T) -> Result<Self> {
        if !is_control_interface(&info) {
            return Err(Error::InvalidConfig(
                "Nova 7 Gen 2 requires 1038:227e interface 3 usage ffc0:1".into(),
            ));
        }
        Ok(Self {
            info,
            transport: Mutex::new(Some(transport)),
            last_chatmix: None,
        })
    }

    fn send_command(&mut self, command: Nova7Gen2Command) -> Result<()> {
        let mut buffer = [0; 64];
        let length = HidReportBuilder::new(HidDeviceType::Headset).build_nova7_gen2(command, &mut buffer)?;
        let transport = self
            .transport
            .get_mut()
            .as_mut()
            .ok_or_else(|| protocol_error("device closed"))?;
        // No global deduplication: each request needs its own reply, and a failed
        // write must never cause a later retry to be skipped.
        if transport.write(&buffer[..length])? != length {
            return Err(protocol_error("short HID write"));
        }
        Ok(())
    }

    /// Passive read: sends no request. 0 is nonblocking; negative/infinite waits
    /// are forbidden. Caller owns pacing, event freshness and disconnect policy.
    pub fn read_event(&mut self, timeout_ms: i32) -> Result<Option<Report>> {
        if !(0..=1000).contains(&timeout_ms) {
            return Err(Error::InvalidConfig("Event timeout must be 0..=1000 ms".into()));
        }
        let mut buffer = [0; 128];
        let transport = self
            .transport
            .get_mut()
            .as_mut()
            .ok_or_else(|| protocol_error("device closed"))?;
        let length = transport.read_timeout(&mut buffer, timeout_ms)?;
        if length == 0 {
            return Ok(None);
        }
        if length > buffer.len() {
            return Err(protocol_error("oversized HID read"));
        }
        let report = decode_report(&buffer[..length])?;
        match report {
            Report::ChatMix(mix) => self.last_chatmix = Some(mix),
            Report::Status(status) => self.last_chatmix = Some(status.chatmix),
            Report::Unknown(_) | Report::Sidetone(_) => {}
        }
        Ok(Some(report))
    }

    /// Last received gains, not a guarantee of freshness or headset connection.
    pub fn last_chatmix(&self) -> Option<ChatMixSample> {
        self.last_chatmix
    }

    /// Actively query battery/power/wheel. This DOES write a query to HID.
    /// Bound both elapsed time and report count when unsolicited traffic interleaves.
    pub fn request_status(&mut self) -> Result<Status> {
        self.request_status_cancellable(|| false)
    }

    /// Single-owner cancellation is checked at every read/re-query boundary.
    pub fn request_status_cancellable(&mut self, is_cancelled: impl Fn() -> bool) -> Result<Status> {
        const REQUERY_INTERVAL: Duration = Duration::from_millis(100);
        const MAX_QUERIES: usize = 10;
        // Allow a buffered burst (hidraw can queue 64 reports) plus its reply,
        // but keep a hard total cap even if unrelated traffic never stops.
        const MAX_REPORTS: usize = 128;
        let deadline = Instant::now() + Duration::from_millis(1000);
        if is_cancelled() {
            return Err(protocol_error("status query cancelled"));
        }
        self.send_command(Nova7Gen2Command::Status)?;
        let mut next_query = Instant::now() + REQUERY_INTERVAL;
        let mut queries = 1;
        let mut reports = 0;
        loop {
            if is_cancelled() {
                return Err(protocol_error("status query cancelled"));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(protocol_error("timed out waiting for status"));
            }
            // A status reply can be lost while wheel events interleave. Re-query
            // only read-only status, never settings, within the original deadline.
            if queries < MAX_QUERIES && now >= next_query {
                self.send_command(Nova7Gen2Command::Status)?;
                queries += 1;
                next_query = Instant::now() + REQUERY_INTERVAL;
            }
            let wake = if queries < MAX_QUERIES {
                next_query.min(deadline)
            } else {
                deadline
            };
            let remaining = wake.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                continue;
            }
            let timeout_ms = remaining.as_nanos().div_ceil(1_000_000).min(1000) as i32;
            let report = self.read_event(timeout_ms)?;
            if is_cancelled() {
                return Err(protocol_error("status query cancelled"));
            }
            if Instant::now() >= deadline {
                return Err(protocol_error("timed out waiting for status"));
            }
            match report {
                Some(Report::Status(status)) => return Ok(status),
                Some(_) => {
                    reports += 1;
                    if reports >= MAX_REPORTS {
                        return Err(protocol_error("status report budget exhausted"));
                    }
                }
                // A transport may return early. Do not turn an empty read into a
                // busy loop or an unpaced write; the next query still has its slot.
                None => std::thread::sleep(wake.saturating_duration_since(Instant::now())),
            }
        }
    }
}

use super::{EqPreset, Headset};
use crate::devices::{Device, DeviceType};

/// Implemented from the cited sources; these flags do not mean local hardware
/// validation. EQ is disabled: upstream advertises 0.5 dB steps but truncates
/// baseline + gain into bytes; this is insufficient to promise accurate EQ.
/// RGB is not documented on this model (status/mute LEDs are not RGB zones).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Capabilities {
    pub battery: bool,
    pub physical_chatmix: bool,
    pub sidetone: bool,
    pub auto_off: bool,
    pub hardware_eq: bool,
    pub rgb: bool,
    pub mic_mute: bool,
    pub mic_volume: bool,
}

impl<T: Transport> Nova7Gen2<T> {
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            battery: true,
            physical_chatmix: true,
            sidetone: true,
            auto_off: true,
            hardware_eq: false,
            rgb: false,
            mic_mute: false,
            mic_volume: false,
        }
    }

    /// Raw four-level sidetone (0 off, 1 low, 2 medium, 3 high). Success means
    /// both writes completed, not that hardware readback has verified persistence.
    pub fn set_sidetone_level(&mut self, level: u8) -> Result<()> {
        self.set_sidetone_level_cancellable(level, &|| false)
    }

    /// A stopped owner must not send Save after a blocked sidetone write returns.
    pub fn set_sidetone_level_cancellable(&mut self, level: u8, is_cancelled: &dyn Fn() -> bool) -> Result<()> {
        if is_cancelled() {
            return Err(protocol_error("Sidetone write cancelled"));
        }
        self.send_command(Nova7Gen2Command::Sidetone(level))?;
        if is_cancelled() {
            return Err(protocol_error("Sidetone save cancelled"));
        }
        self.send_command(Nova7Gen2Command::Save)
    }

    /// Query settings; asynchronous status/wheel packets are decoded, not confused
    /// with the requested settings response. Returns raw level 0..=3.
    pub fn sidetone_level(&mut self) -> Result<u8> {
        self.sidetone_level_observed(|_| Ok(()))
    }

    /// Preserve interleaved power/wheel events for a single-owner service.
    /// The observer can abort on disconnect before any subsequent setting write.
    pub fn sidetone_level_observed(&mut self, observe: impl FnMut(Report) -> Result<()>) -> Result<u8> {
        self.sidetone_level_observed_cancellable(&|| false, observe)
    }

    pub fn sidetone_level_observed_cancellable(
        &mut self,
        is_cancelled: &dyn Fn() -> bool,
        mut observe: impl FnMut(Report) -> Result<()>,
    ) -> Result<u8> {
        if is_cancelled() {
            return Err(protocol_error("Sidetone readback cancelled"));
        }
        self.send_command(Nova7Gen2Command::AudioSettings)?;
        let deadline = Instant::now() + Duration::from_millis(1000);
        for _ in 0..16 {
            if is_cancelled() {
                return Err(protocol_error("Sidetone readback cancelled"));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let report = self.read_event(remaining.as_millis().clamp(1, 1000) as i32)?;
            if is_cancelled() {
                return Err(protocol_error("Sidetone readback cancelled"));
            }
            match report {
                Some(Report::Sidetone(level)) => return Ok(level),
                None => break,
                Some(report) => observe(report)?,
            }
        }
        Err(protocol_error("timed out waiting for sidetone settings"))
    }
}

impl<T: Transport> Device for Nova7Gen2<T> {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }
    fn device_type(&self) -> DeviceType {
        DeviceType::Headset
    }
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        *self.transport.get_mut() = None;
        self.last_chatmix = None;
        Ok(())
    }
    /// Transport handle state only; wireless headset power comes from Status.
    fn is_connected(&self) -> bool {
        self.transport.lock().is_some()
    }
    fn send_raw(&mut self, _data: &[u8]) -> Result<()> {
        Err(protocol_error("raw writes disabled; use model-specific typed commands"))
    }
    fn receive_raw(&mut self, _buffer: &mut [u8]) -> Result<usize> {
        Err(protocol_error(
            "raw reads disabled; use read_event to preserve demultiplexing",
        ))
    }
}

impl<T: Transport> Headset for Nova7Gen2<T> {
    fn battery_level(&mut self) -> Result<Option<u8>> {
        Ok(self.request_status()?.battery_percent)
    }
    fn chat_mix(&mut self) -> Result<f32> {
        Ok(self.request_status()?.chatmix.balance())
    }
    fn set_sidetone(&mut self, level: u8) -> Result<()> {
        if level > 100 {
            return Err(Error::InvalidConfig("Sidetone must be 0..=100".into()));
        }
        self.set_sidetone_level(((u16::from(level) * 3 + 50) / 100) as u8)
    }
    fn set_auto_off(&mut self, minutes: u8) -> Result<()> {
        self.send_command(Nova7Gen2Command::AutoOff(minutes))
    }
    fn set_mic_mute(&mut self, _muted: bool) -> Result<()> {
        Err(protocol_error("microphone mute control is not verified"))
    }
    fn set_mic_volume(&mut self, _volume: u8) -> Result<()> {
        Err(protocol_error(
            "microphone gain writes are not enabled pending device validation",
        ))
    }
    fn set_eq_preset(&mut self, _preset: EqPreset) -> Result<()> {
        Err(protocol_error("hardware EQ encoding is not verified; use host DSP EQ"))
    }
}
