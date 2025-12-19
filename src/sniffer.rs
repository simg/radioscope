use anyhow::{Context, Result};
use pcap::{Capture, Error as PcapError};
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc::UnboundedSender;

use crate::devices::{DeviceRole, DeviceTracker};
use crate::events::{EventKind, PacketEvent, RateKey};

pub fn spawn_sniffer(
    interface: String,
    tx: UnboundedSender<PacketEvent>,
    devices: Arc<DeviceTracker>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let tx_clone = tx.clone();
        match run(&interface, tx, Arc::clone(&devices)) {
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    "Primary sniffer setup failed on {interface}: {err:?}, retrying without rfmon flag"
                );
                if let Err(err2) = run_without_rfmon(&interface, tx_clone, devices) {
                    tracing::error!("Sniffer error on {interface}: {err2:?}");
                }
            }
        }
    })
}

fn run(
    interface: &str,
    tx: UnboundedSender<PacketEvent>,
    devices: Arc<DeviceTracker>,
) -> Result<()> {
    let mut cap = Capture::from_device(interface)
        .with_context(|| format!("Unable to open device {interface}"))?
        .rfmon(false)
        .promisc(true)
        .immediate_mode(true)
        .timeout(1_000)
        .open()
        .with_context(|| format!("Failed to start capture on {interface}"))?;

    // No filter yet; we want all management/control/data frames.
    loop {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(frame) = parse_radiotap_and_frame(packet.data) {
                    observe_device(&devices, &frame);
                    if let Some(evt) = classify_frame(&frame) {
                        let _ = tx.send(evt);
                    }
                }
            }
            Err(PcapError::TimeoutExpired) => continue,
            Err(err) => {
                tracing::warn!("pcap error on {interface}: {err:?}");
                continue;
            }
        }
    }
}

fn run_without_rfmon(
    interface: &str,
    tx: UnboundedSender<PacketEvent>,
    devices: Arc<DeviceTracker>,
) -> Result<()> {
    let mut cap = Capture::from_device(interface)
        .with_context(|| format!("Unable to open device {interface} (fallback)"))?
        .rfmon(false)
        .promisc(true)
        .immediate_mode(true)
        .timeout(1_000)
        .open()
        .with_context(|| format!("Failed to start capture on {interface} (fallback)"))?;

    loop {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(frame) = parse_radiotap_and_frame(packet.data) {
                    observe_device(&devices, &frame);
                    if let Some(evt) = classify_frame(&frame) {
                        let _ = tx.send(evt);
                    }
                }
            }
            Err(PcapError::TimeoutExpired) => continue,
            Err(err) => {
                tracing::warn!("pcap error on {interface} (fallback): {err:?}");
                continue;
            }
        }
    }
}

#[derive(Debug)]
struct ParsedFrame<'a> {
    fc: u16,
    _header_len: usize,
    payload: &'a [u8],
    _addr1: Option<[u8; 6]>,
    addr2: Option<[u8; 6]>,
    addr3: Option<[u8; 6]>,
    bssid: Option<[u8; 6]>,
    signal_gain: f32,
    signal_dbm: Option<i8>,
    ssid: Option<String>,
    channel: Option<u16>,
}

fn classify_frame(parsed: &ParsedFrame) -> Option<PacketEvent> {
    let kind_bits = (parsed.fc >> 2) & 0x3;
    let subtype = (parsed.fc >> 4) & 0xF;
    let retry = parsed.fc & 0x0800 != 0;

    match kind_bits {
        0 => classify_mgmt(subtype, retry, parsed),
        1 => classify_ctrl(subtype, retry, parsed),
        2 => classify_data(subtype, retry, parsed),
        _ => None,
    }
}

fn classify_mgmt(subtype: u16, retry: bool, frame: &ParsedFrame) -> Option<PacketEvent> {
    let amplitude = frame.signal_gain;
    match subtype {
        8 => {
            let key = frame
                .bssid
                .or(frame.addr3)
                .map(RateKey::Bssid)
                .unwrap_or(RateKey::none());
            Some(PacketEvent {
                kind: EventKind::Beacon,
                rate_key: key,
                retry,
                amplitude,
                src: frame.addr2,
                bssid: frame.bssid,
            })
        }
        4 => {
            let key = frame.addr2.map(RateKey::Tx).unwrap_or(RateKey::none());
            Some(PacketEvent {
                kind: EventKind::ProbeReq,
                rate_key: key,
                retry,
                amplitude,
                src: frame.addr2,
                bssid: frame.bssid,
            })
        }
        5 => {
            let key = frame
                .bssid
                .or(frame.addr3)
                .map(RateKey::Bssid)
                .unwrap_or(RateKey::none());
            Some(PacketEvent {
                kind: EventKind::ProbeResp,
                rate_key: key,
                retry,
                amplitude,
                src: frame.addr2,
                bssid: frame.bssid,
            })
        }
        0 | 1 | 2 | 3 => {
            let bssid = frame.bssid.or(frame.addr3);
            let sta = frame.addr2;
            let key = match (sta, bssid) {
                (Some(s), Some(b)) => RateKey::Pair(s, b),
                _ => RateKey::none(),
            };
            Some(PacketEvent {
                kind: EventKind::Assoc,
                rate_key: key,
                retry,
                amplitude,
                src: frame.addr2,
                bssid,
            })
        }
        10 | 12 => {
            let bssid = frame.bssid.or(frame.addr3);
            let sta = frame.addr2;
            let key = match (sta, bssid) {
                (Some(s), Some(b)) => RateKey::Pair(s, b),
                _ => RateKey::none(),
            };
            Some(PacketEvent {
                kind: EventKind::Deauth,
                rate_key: key,
                retry,
                amplitude,
                src: frame.addr2,
                bssid,
            })
        }
        _ => None,
    }
}

fn classify_ctrl(subtype: u16, retry: bool, frame: &ParsedFrame) -> Option<PacketEvent> {
    let amplitude = frame.signal_gain;
    match subtype {
        11 => Some(PacketEvent {
            kind: EventKind::Rts,
            rate_key: RateKey::none(),
            retry,
            amplitude,
            src: frame.addr2,
            bssid: frame.bssid,
        }),
        12 => Some(PacketEvent {
            kind: EventKind::Cts,
            rate_key: RateKey::none(),
            retry,
            amplitude,
            src: frame.addr2,
            bssid: frame.bssid,
        }),
        13 | 9 => Some(PacketEvent {
            kind: EventKind::Ack,
            rate_key: RateKey::none(),
            retry,
            amplitude,
            src: frame.addr2,
            bssid: frame.bssid,
        }),
        _ => None,
    }
}

fn classify_data(subtype: u16, retry: bool, frame: &ParsedFrame) -> Option<PacketEvent> {
    let amplitude = frame.signal_gain;
    if is_eapol(subtype, frame.payload) {
        let bssid = frame.bssid.or(frame.addr3);
        let sta = frame.addr2;
        let key = match (sta, bssid) {
            (Some(s), Some(b)) => RateKey::Pair(s, b),
            _ => RateKey::none(),
        };
        return Some(PacketEvent {
            kind: EventKind::Eapol,
            rate_key: key,
            retry,
            amplitude,
            src: frame.addr2,
            bssid,
        });
    }

    Some(PacketEvent {
        kind: EventKind::DataTick,
        rate_key: RateKey::none(),
        retry,
        amplitude,
        src: frame.addr2,
        bssid: frame.bssid,
    })
}

fn observe_device(tracker: &DeviceTracker, frame: &ParsedFrame) {
    if let Some(mac) = frame.addr2 {
        tracker.observe(
            mac,
            frame.bssid,
            role_for_frame(frame),
            frame.signal_dbm,
            frame.ssid.clone(),
            frame.channel,
        );
    }
}

fn role_for_frame(frame: &ParsedFrame) -> DeviceRole {
    match (frame.addr2, frame.bssid) {
        (Some(tx), Some(bssid)) if tx == bssid => DeviceRole::Ap,
        (Some(_), Some(_)) => DeviceRole::Client,
        _ => DeviceRole::Unknown,
    }
}

fn parse_ssid(kind: u16, subtype: u16, payload: &[u8]) -> Option<String> {
    if kind != 0 {
        return None;
    }
    let start = mgmt_ie_start(subtype, payload)?;
    if payload.len() < start + 2 {
        return None;
    }
    let mut idx = start;
    while idx + 2 <= payload.len() {
        let id = payload[idx];
        let len = payload[idx + 1] as usize;
        idx += 2;
        if idx + len > payload.len() {
            break;
        }
        if id == 0 {
            let raw = &payload[idx..idx + len];
            if raw.is_empty() {
                return Some("<hidden>".to_string());
            }
            return decode_ssid(raw);
        }
        idx += len;
    }
    None
}

fn decode_ssid(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return Some("<hidden>".to_string());
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Some(s.to_string()),
        Err(_) => {
            let mut hex = String::with_capacity(bytes.len() * 2 + 4);
            hex.push_str("0x");
            for b in bytes {
                hex.push_str(&format!("{:02X}", b));
            }
            Some(hex)
        }
    }
}

fn parse_ds_channel(subtype: u16, payload: &[u8]) -> Option<u16> {
    let start = mgmt_ie_start(subtype, payload)?;
    let mut idx = start;
    while idx + 2 <= payload.len() {
        let id = payload[idx];
        let len = payload[idx + 1] as usize;
        idx += 2;
        if idx + len > payload.len() {
            break;
        }
        if id == 3 && len >= 1 {
            return Some(payload[idx] as u16);
        }
        idx += len;
    }
    None
}

fn mgmt_ie_start(subtype: u16, payload: &[u8]) -> Option<usize> {
    match subtype {
        8 | 5 => {
            if payload.len() < 12 {
                None
            } else {
                Some(12)
            }
        }
        4 => Some(0), // probe request starts tagging immediately
        _ => None,
    }
}

fn is_eapol(subtype: u16, payload: &[u8]) -> bool {
    let hdr_len = if subtype & 0x08 != 0 { 26 } else { 24 };
    if payload.len() < hdr_len + 8 {
        return false;
    }
    let llc = &payload[hdr_len..];
    if llc.len() < 8 {
        return false;
    }
    if llc[0] != 0xAA || llc[1] != 0xAA || llc[2] != 0x03 {
        return false;
    }
    if llc[3] != 0x00 || llc[4] != 0x00 || llc[5] != 0x00 {
        return false;
    }
    let eth_type = u16::from_be_bytes([llc[6], llc[7]]);
    eth_type == 0x888E
}

fn parse_radiotap_and_frame(data: &[u8]) -> Option<ParsedFrame<'_>> {
    if data.len() < 4 {
        return None;
    }
    let rt_len = u16::from_le_bytes([data[2], data[3]]) as usize;
    if data.len() < rt_len + 10 {
        return None;
    }
    let frame = &data[rt_len..];
    if frame.len() < 10 {
        return None;
    }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    let kind_bits = (fc >> 2) & 0x3;
    let subtype = (fc >> 4) & 0xF;

    let has_qos = kind_bits == 2 && (subtype & 0x08 != 0);
    let base_hdr_len = if kind_bits == 1 {
        10
    } else if has_qos {
        26
    } else {
        24
    };
    if frame.len() < base_hdr_len {
        return None;
    }

    let addr1 = frame.get(4..10).map(to_mac);
    let addr2 = frame.get(10..16).map(to_mac);
    let addr3 = if base_hdr_len >= 16 {
        frame.get(16..22).map(to_mac)
    } else {
        None
    };
    let bssid = match kind_bits {
        0 => addr3,
        2 => {
            let tods = fc & 0x0100 != 0;
            let fromds = fc & 0x0200 != 0;
            match (tods, fromds) {
                (false, false) => addr3,
                (false, true) => addr2,
                (true, false) => addr1,
                (true, true) => None,
            }
        }
        _ => None,
    };

    let payload = if frame.len() > base_hdr_len {
        &frame[base_hdr_len..]
    } else {
        &[]
    };

    let signal = radiotap_signal(data);
    let signal_gain = signal.as_ref().map(|s| s.gain).unwrap_or(1.0);
    let signal_dbm = signal.as_ref().and_then(|s| s.dbm);
    let mut channel = signal.as_ref().and_then(|s| s.channel);
    let ssid = parse_ssid(kind_bits, subtype, payload);
    if kind_bits == 0 {
        if let Some(ds) = parse_ds_channel(subtype, payload) {
            channel = Some(ds);
        }
    }

    Some(ParsedFrame {
        fc,
        _header_len: base_hdr_len,
        payload,
        _addr1: addr1,
        addr2,
        addr3,
        bssid,
        signal_gain,
        signal_dbm,
        ssid,
        channel,
    })
}

fn to_mac(slice: &[u8]) -> [u8; 6] {
    let mut arr = [0u8; 6];
    arr.copy_from_slice(&slice[..6]);
    arr
}

struct SignalInfo {
    gain: f32,
    dbm: Option<i8>,
    channel: Option<u16>,
}

fn radiotap_signal(data: &[u8]) -> Option<SignalInfo> {
    if data.len() < 8 {
        return None;
    }
    let rt_len = u16::from_le_bytes([data[2], data[3]]) as usize;
    if rt_len > data.len() {
        return None;
    }
    let present = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let mut offset = 8;
    let mut channel: Option<u16> = None;
    for bit in 0..32 {
        let is_present = (present & (1 << bit)) != 0;
        if !is_present {
            continue;
        }
        match bit {
            0 => {
                offset = align(offset, 8);
                offset += 8;
            }
            1 => offset += 1,
            2 => offset += 1,
            3 => {
                offset = align(offset, 2);
                if offset + 4 <= rt_len && offset + 4 <= data.len() {
                    let freq = u16::from_le_bytes([data[offset], data[offset + 1]]) as u32;
                    channel = freq_to_channel(freq);
                }
                offset += 4;
            }
            4 => {
                offset = align(offset, 2);
                offset += 2;
            }
            5 => {
                if offset < rt_len && rt_len <= data.len() && offset < data.len() {
                    let sig = data.get(offset).copied().map(|b| b as i8);
                    return Some(SignalInfo {
                        gain: sig.map(dbm_to_gain).unwrap_or(1.0),
                        dbm: sig,
                        channel,
                    });
                }
                offset += 1;
            }
            _ => {}
        }
    }
    Some(SignalInfo {
        gain: 1.0,
        dbm: None,
        channel,
    })
}

fn align(offset: usize, align: usize) -> usize {
    (offset + align - 1) & !(align - 1)
}

fn dbm_to_gain(dbm: i8) -> f32 {
    let dbm = dbm as f32;
    let normalized = ((dbm + 90.0) / 60.0).clamp(0.0, 1.0);
    0.2 + normalized * 0.8
}

fn freq_to_channel(freq: u32) -> Option<u16> {
    if freq >= 2412 && freq <= 2472 {
        Some(((freq as i32 - 2407) / 5) as u16)
    } else if freq == 2484 {
        Some(14)
    } else if freq >= 5000 && freq <= 5900 {
        Some(((freq as i32 - 5000) / 5) as u16)
    } else {
        None
    }
}
