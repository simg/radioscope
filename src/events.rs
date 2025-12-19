use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    Beacon,
    ProbeReq,
    ProbeResp,
    Assoc,
    Deauth,
    Eapol,
    Rts,
    Cts,
    Ack,
    DataTick,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RateKey {
    None,
    Bssid([u8; 6]),
    Tx([u8; 6]),
    Pair([u8; 6], [u8; 6]),
}

impl RateKey {
    pub fn none() -> Self {
        RateKey::None
    }
}

#[derive(Clone, Debug)]
pub struct PacketEvent {
    pub kind: EventKind,
    pub rate_key: RateKey,
    pub retry: bool,
    pub amplitude: f32,
    pub src: Option<[u8; 6]>,
    pub bssid: Option<[u8; 6]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NoiseMode {
    Crowded,
    Sparse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventSettings {
    pub mode: NoiseMode,
    pub enabled: HashMap<EventKind, bool>,
}

impl Default for EventSettings {
    fn default() -> Self {
        let mut enabled = HashMap::new();
        enabled.insert(EventKind::Beacon, true);
        enabled.insert(EventKind::ProbeReq, true);
        enabled.insert(EventKind::ProbeResp, true);
        enabled.insert(EventKind::Assoc, true);
        enabled.insert(EventKind::Deauth, true);
        enabled.insert(EventKind::Eapol, true);
        enabled.insert(EventKind::Rts, true);
        enabled.insert(EventKind::Cts, true);
        enabled.insert(EventKind::Ack, true);
        enabled.insert(EventKind::DataTick, true);
        Self {
            mode: NoiseMode::Crowded,
            enabled,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EventWindow {
    start: Instant,
    counts_mgmt: u32,
    counts_ctrl: u32,
    counts_data: u32,
    window: Duration,
}

impl EventWindow {
    pub fn new(window: Duration) -> Self {
        Self {
            start: Instant::now(),
            counts_mgmt: 0,
            counts_ctrl: 0,
            counts_data: 0,
            window,
        }
    }

    pub fn refresh(&mut self) {
        if self.start.elapsed() >= self.window {
            self.start = Instant::now();
            self.counts_mgmt = 0;
            self.counts_ctrl = 0;
            self.counts_data = 0;
        }
    }

    pub fn try_count(
        &mut self,
        kind: &EventKind,
        max_mgmt: u32,
        max_ctrl: u32,
        max_data: u32,
    ) -> bool {
        self.refresh();
        match kind {
            EventKind::Beacon
            | EventKind::ProbeReq
            | EventKind::ProbeResp
            | EventKind::Assoc
            | EventKind::Deauth
            | EventKind::Eapol => {
                if self.counts_mgmt >= max_mgmt {
                    return false;
                }
                self.counts_mgmt += 1;
            }
            EventKind::Rts | EventKind::Cts | EventKind::Ack => {
                if self.counts_ctrl >= max_ctrl {
                    return false;
                }
                self.counts_ctrl += 1;
            }
            EventKind::DataTick => {
                if self.counts_data >= max_data {
                    return false;
                }
                self.counts_data += 1;
            }
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct RateLimiter {
    last_seen: HashMap<(EventKind, RateKey), Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_seen: HashMap::new(),
        }
    }

    pub fn allow(&mut self, kind: &EventKind, key: RateKey, min_gap: Duration) -> bool {
        let now = Instant::now();
        let entry_key = (kind.clone(), key);
        match self.last_seen.get_mut(&entry_key) {
            Some(last) => {
                if now.duration_since(*last) < min_gap {
                    return false;
                }
                *last = now;
                true
            }
            None => {
                self.last_seen.insert(entry_key, now);
                true
            }
        }
    }
}
