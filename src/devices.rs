use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceRole {
    Ap,
    Client,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceView {
    pub mac: String,
    pub bssid: Option<String>,
    pub role: DeviceRole,
    pub age_ms: u64,
    pub rssi_dbm: Option<i8>,
    pub frames: u64,
    pub allowed: bool,
    pub ssid: Option<String>,
    pub channel: Option<u16>,
}

#[derive(Debug)]
struct TrackedDevice {
    mac: [u8; 6],
    bssid: Option<[u8; 6]>,
    role: DeviceRole,
    last_seen: Instant,
    last_rssi: Option<i8>,
    frames: u64,
    ssid: Option<String>,
    channel: Option<u16>,
}

#[derive(Clone, Default)]
pub struct DeviceTracker {
    devices: Arc<RwLock<HashMap<[u8; 6], TrackedDevice>>>,
    blocked: Arc<RwLock<HashSet<[u8; 6]>>>,
    ssid_cache: Arc<RwLock<HashMap<[u8; 6], String>>>,
    channel_cache: Arc<RwLock<HashMap<[u8; 6], u16>>>,
}

impl DeviceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(
        &self,
        mac: [u8; 6],
        bssid: Option<[u8; 6]>,
        role: DeviceRole,
        rssi_dbm: Option<i8>,
        ssid: Option<String>,
        channel: Option<u16>,
    ) {
        let now = Instant::now();
        let mut guard = self.devices.write().expect("device tracker poisoned");
        let entry = guard.entry(mac).or_insert(TrackedDevice {
            mac,
            bssid,
            role: DeviceRole::Unknown,
            last_seen: now,
            last_rssi: None,
            frames: 0,
            ssid: None,
            channel: None,
        });
        entry.last_seen = now;
        entry.frames = entry.frames.saturating_add(1);
        if let Some(dbm) = rssi_dbm {
            entry.last_rssi = Some(dbm);
        }
        if let Some(b) = bssid {
            entry.bssid = Some(b);
        }
        entry.role = merge_role(entry.role, role);
        if let Some(name) = ssid {
            entry.ssid = Some(name.clone());
            if let Some(b) = bssid {
                if let Ok(mut cache) = self.ssid_cache.write() {
                    cache.insert(b, name);
                }
            }
        } else if let Some(b) = bssid {
            if entry.ssid.is_none() {
                if let Ok(cache) = self.ssid_cache.read() {
                    if let Some(name) = cache.get(&b) {
                        entry.ssid = Some(name.clone());
                    }
                }
            }
        }
        if let Some(ch) = channel {
            entry.channel = Some(ch);
            if let Some(b) = bssid {
                if let Ok(mut cache) = self.channel_cache.write() {
                    cache.insert(b, ch);
                }
            }
        } else if entry.channel.is_none() {
            if let Some(b) = bssid {
                if let Ok(cache) = self.channel_cache.read() {
                    if let Some(ch) = cache.get(&b) {
                        entry.channel = Some(*ch);
                    }
                }
            }
        }
    }

    pub fn snapshot(&self, window: Duration) -> Vec<DeviceView> {
        let now = Instant::now();
        let guard = self.devices.read().expect("device tracker poisoned");
        let blocked = self.blocked.read().expect("device tracker poisoned");
        let cache = self.ssid_cache.read().expect("device tracker poisoned");
        let channel_cache = self.channel_cache.read().expect("device tracker poisoned");
        let mut list: Vec<DeviceView> = guard
            .values()
            .filter_map(|dev| {
                let age = now.duration_since(dev.last_seen);
                if age > window {
                    return None;
                }
                let ssid = dev
                    .ssid
                    .clone()
                    .or_else(|| dev.bssid.and_then(|b| cache.get(&b).cloned()));
                let channel = dev
                    .channel
                    .or_else(|| dev.bssid.and_then(|b| channel_cache.get(&b).copied()));
                Some(DeviceView {
                    mac: format_mac(&dev.mac),
                    bssid: dev.bssid.map(|b| format_mac(&b)),
                    role: dev.role,
                    age_ms: age.as_millis().min(u128::from(u64::MAX)) as u64,
                    rssi_dbm: dev.last_rssi,
                    frames: dev.frames,
                    allowed: !blocked.contains(&dev.mac),
                    ssid,
                    channel,
                })
            })
            .collect();

        list.sort_by(|a, b| {
            let role_order = role_rank(&a.role).cmp(&role_rank(&b.role));
            if role_order != Ordering::Equal {
                return role_order;
            }
            b.frames.cmp(&a.frames)
        });
        list
    }

    pub fn allows(&self, src: Option<[u8; 6]>, bssid: Option<[u8; 6]>) -> bool {
        let blocked = self.blocked.read().expect("device tracker poisoned");
        if let Some(mac) = src {
            if blocked.contains(&mac) {
                return false;
            }
        }
        if let Some(id) = bssid {
            if blocked.contains(&id) {
                return false;
            }
        }
        true
    }

    pub fn set_many(&self, toggles: &[([u8; 6], bool)]) {
        let mut blocked = self.blocked.write().expect("device tracker poisoned");
        for (mac, allowed) in toggles {
            if *allowed {
                blocked.remove(mac);
            } else {
                blocked.insert(*mac);
            }
        }
    }

    pub fn reset_counts(&self) {
        if let Ok(mut guard) = self.devices.write() {
            for dev in guard.values_mut() {
                dev.frames = 0;
            }
        }
    }
}

fn merge_role(current: DeviceRole, new_role: DeviceRole) -> DeviceRole {
    match (current, new_role) {
        (DeviceRole::Ap, _) | (_, DeviceRole::Ap) => DeviceRole::Ap,
        (DeviceRole::Client, _) | (_, DeviceRole::Client) => DeviceRole::Client,
        _ => DeviceRole::Unknown,
    }
}

fn role_rank(role: &DeviceRole) -> u8 {
    match role {
        DeviceRole::Ap => 0,
        DeviceRole::Client => 1,
        DeviceRole::Unknown => 2,
    }
}

pub fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

pub fn parse_mac(input: &str) -> Option<[u8; 6]> {
    let cleaned: String = input.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() != 12 {
        return None;
    }
    let mut bytes = [0u8; 6];
    for i in 0..6 {
        let idx = i * 2;
        let part = u8::from_str_radix(&cleaned[idx..idx + 2], 16).ok()?;
        bytes[i] = part;
    }
    Some(bytes)
}
