use crate::devices::{format_mac, parse_mac};
use crate::settings::{SettingsStore, TrustedApRecord};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RsnProfile {
    pub akms: Vec<String>,
    pub ciphers: Vec<String>,
    pub pmf_required: bool,
    pub pmf_capable: bool,
    pub transition_mode: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitiesFingerprint {
    pub supported_rates: Vec<u8>,
    pub has_ht: bool,
    pub has_vht: bool,
    pub has_he: bool,
    pub beacon_interval: Option<u16>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum EvilSeverity {
    Info,
    Warning,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvilEvent {
    pub ssid: String,
    pub bssid: String,
    pub channel: Option<u16>,
    pub rssi: Option<i8>,
    pub score: i32,
    pub severity: EvilSeverity,
    pub message: String,
    pub reasons: Vec<String>,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug)]
pub struct ApObservation {
    pub ssid: Option<String>,
    pub bssid: Option<[u8; 6]>,
    pub channel: Option<u16>,
    pub rssi: Option<i8>,
    pub rsn: Option<RsnProfile>,
    pub capabilities: Option<CapabilitiesFingerprint>,
}

#[derive(Clone, Debug)]
struct ObservedAp {
    ssid: String,
    channels_seen: HashSet<u16>,
    last_seen: Instant,
    last_rssi: Option<i8>,
    rsn: Option<RsnProfile>,
    capabilities: Option<CapabilitiesFingerprint>,
    ssids_seen: HashSet<String>,
    teleport_count: u32,
}

#[derive(Clone, Debug)]
pub struct TrustedAp {
    pub ssid: String,
    pub rsn_profile: Option<RsnProfile>,
    pub vendor_oui: Option<String>,
    pub channels_seen: HashSet<u16>,
    pub capabilities_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApView {
    pub ssid: String,
    pub bssid: String,
    pub trusted: bool,
    pub last_seen_ms: Option<u64>,
    pub channel: Option<u16>,
    pub rssi: Option<i8>,
    pub rsn: Option<RsnProfile>,
    pub vendor_oui: Option<String>,
    pub capabilities_fingerprint: Option<String>,
}

#[derive(Clone)]
pub struct EvilTwinMonitor {
    trusted: ArcState<HashMap<[u8; 6], TrustedAp>>,
    by_ssid: ArcState<HashMap<String, HashSet<[u8; 6]>>>,
    observed: ArcState<HashMap<[u8; 6], ObservedAp>>,
    events: ArcState<Vec<EvilEvent>>,
    last_event: ArcState<HashMap<String, Instant>>,
    settings: SettingsStore,
}

type ArcState<T> = std::sync::Arc<RwLock<T>>;
const EVIL_EVENT_COOLDOWN: Duration = Duration::from_secs(5);

impl EvilTwinMonitor {
    pub async fn new(settings: SettingsStore) -> Result<Self> {
        let mut trusted = HashMap::new();
        let mut by_ssid: HashMap<String, HashSet<[u8; 6]>> = HashMap::new();
        for rec in settings.list_trusted_aps().await? {
            if let Some(mac) = parse_mac(&rec.bssid) {
                let entry = TrustedAp {
                    ssid: rec.ssid.clone(),
                    rsn_profile: rec
                        .rsn_profile_json
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok()),
                    vendor_oui: rec.vendor_oui.clone(),
                    channels_seen: rec
                        .channels_seen_json
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default(),
                    capabilities_fingerprint: rec.capabilities_fingerprint.clone(),
                };
                by_ssid.entry(rec.ssid.clone()).or_default().insert(mac);
                trusted.insert(mac, entry);
            }
        }

        Ok(Self {
            trusted: ArcState::new(RwLock::new(trusted)),
            by_ssid: ArcState::new(RwLock::new(by_ssid)),
            observed: ArcState::new(RwLock::new(HashMap::new())),
            events: ArcState::new(RwLock::new(Vec::new())),
            last_event: ArcState::new(RwLock::new(HashMap::new())),
            settings,
        })
    }

    pub async fn observe(&self, obs: ApObservation) -> Option<EvilEvent> {
        let ssid = obs.ssid.clone()?;
        let bssid = obs.bssid?;
        let now = Instant::now();
        let prev_rssi = {
            let mut guard = self.observed.write().await;
            let entry = guard.entry(bssid).or_insert_with(|| ObservedAp {
                ssid: ssid.clone(),
                channels_seen: HashSet::new(),
                last_seen: now,
                last_rssi: None,
                rsn: None,
                capabilities: None,
                ssids_seen: HashSet::new(),
                teleport_count: 0,
            });
            let prev = entry.last_rssi;
            entry.last_seen = now;
            entry.ssid = ssid.clone();
            let mut new_channel = false;
            if let Some(ch) = obs.channel {
                new_channel = !entry.channels_seen.contains(&ch);
                entry.channels_seen.insert(ch);
            }
            if let Some(rssi) = obs.rssi {
                entry.last_rssi = Some(rssi);
            }
            if let Some(rsn) = obs.rsn.clone() {
                entry.rsn = Some(rsn);
            }
            if let Some(cap) = obs.capabilities.clone() {
                entry.capabilities = Some(cap);
            }
            entry.ssids_seen.insert(ssid.clone());
            if obs.channel.is_some() && new_channel && entry.channels_seen.len() > 1 {
                entry.teleport_count = entry.teleport_count.saturating_add(1);
            }
            prev
        };

        let trusted_map = self.trusted.read().await;
        let by_ssid = self.by_ssid.read().await;
        let is_trusted_bssid = trusted_map.contains_key(&bssid);
        let is_trusted_ssid = by_ssid
            .get(&ssid)
            .map(|s| s.contains(&bssid) || !s.is_empty())
            .unwrap_or(false);
        drop(by_ssid);
        drop(trusted_map);

        // Skip scoring when nothing is trusted yet.
        if !is_trusted_bssid && !is_trusted_ssid {
            return None;
        }

        let mut score = 0;
        let mut message = "Informational".to_string();
        let mut reasons: Vec<String> = Vec::new();

        // Rule 1: unknown BSSID on trusted SSID
        let mut elevate = false;
        {
            let trusted_ssids = self.by_ssid.read().await;
            if let Some(list) = trusted_ssids.get(&ssid) {
                if !list.contains(&bssid) {
                    score += 40;
                    message = format!(
                        "Trusted SSID seen on unknown AP {} (new BSSID).",
                        format_mac(&bssid)
                    );
                    reasons.push("Trusted SSID on new BSSID".to_string());
                }
            }
        }

        // Rule 2 + 3: fingerprint / downgrade
        if let Some(trusted) = self.trusted.read().await.get(&bssid).cloned() {
            if let Some(cur_rsn) = obs.rsn.as_ref() {
                if let Some(stored) = trusted.rsn_profile.as_ref() {
                    if !rsn_matches(stored, cur_rsn) {
                        score += 35;
                        message = format!(
                            "Trusted AP fingerprint changed: RSN differs (stored: {}; seen: {}). Possible spoof.",
                            summarize_rsn(stored),
                            summarize_rsn(cur_rsn)
                        );
                        reasons.push(format!(
                            "RSN mismatch: stored {}, seen {}",
                            summarize_rsn(stored),
                            summarize_rsn(cur_rsn)
                        ));
                    }
                    if is_security_downgrade(stored, cur_rsn) {
                        score += 30;
                        message = format!(
                            "Security downgraded for trusted SSID (stored: {}; seen: {}).",
                            summarize_rsn(stored),
                            summarize_rsn(cur_rsn)
                        );
                        reasons.push(format!(
                            "Security downgrade: stored {}, seen {}",
                            summarize_rsn(stored),
                            summarize_rsn(cur_rsn)
                        ));
                    }
                }
            }
            if let Some(cur_cap) = obs.capabilities.as_ref() {
                if let Some(stored_cap) = &trusted.capabilities_fingerprint {
                    let current_fingerprint =
                        capability_string(cur_cap, obs.channel, obs.rssi, None);
                    if &current_fingerprint != stored_cap {
                        score += 35;
                        message = format!(
                            "Trusted AP fingerprint changed: capabilities differ (stored: {}; seen: {}). Possible spoof.",
                            stored_cap,
                            current_fingerprint
                        );
                        reasons.push(format!(
                            "Capabilities mismatch: stored {}, seen {}",
                            stored_cap,
                            current_fingerprint
                        ));
                    }
                }
            }
            if let Some(ch) = obs.channel {
                if !trusted.channels_seen.contains(&ch) {
                    score += 20;
                    elevate = true;
                    reasons.push(format!(
                        "Channel anomaly: {} not in trusted set [{}]",
                        ch,
                        summarize_channels(&trusted.channels_seen)
                    ));
                }
            }
        }

        // Rule 4: anomalies (strong/sudden RSSI)
        if let Some(rssi) = obs.rssi {
            let is_jump = prev_rssi
                .map(|p| rssi.saturating_sub(p) >= 10)
                .unwrap_or(true);
            if rssi > -30 && is_jump {
                score += 20;
                elevate = true;
                let detail = prev_rssi
                    .map(|p| format!("(prev {} dBm -> {} dBm)", p, rssi))
                    .unwrap_or_else(|| format!("({} dBm seen, no prior sample)", rssi));
                reasons.push(format!("Strong/sudden RSSI anomaly {}", detail));
            }
        }

        // Rule 5: KARMA style
        if let Some(seen) = self.observed.read().await.get(&bssid) {
            if seen.ssids_seen.len() >= 3 {
                score += 50;
                message = "AP is impersonating multiple networks.".to_string();
                reasons.push(format!(
                    "KARMA behavior: responded to {} SSIDs",
                    seen.ssids_seen.len()
                ));
            }
            if elevate && score > 0 {
                score += 20;
            }
        }

        let severity = match score {
            s if s >= 90 => EvilSeverity::Critical,
            s if s >= 70 => EvilSeverity::High,
            s if s >= 40 => EvilSeverity::Warning,
            _ => EvilSeverity::Info,
        };

        if score == 0 {
            return None;
        }

        {
            let mut gate = self.last_event.write().await;
            if let Some(last) = gate.get(&ssid) {
                if last.elapsed() < EVIL_EVENT_COOLDOWN {
                    return None;
                }
            }
            gate.insert(ssid.clone(), now);
        }

        let event = EvilEvent {
            ssid: ssid.clone(),
            bssid: format_mac(&bssid),
            channel: obs.channel,
            rssi: obs.rssi,
            score,
            severity,
            message,
            reasons,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or_default(),
        };

        {
            let mut guard = self.events.write().await;
            guard.push(event.clone());
            if guard.len() > 300 {
                let excess = guard.len() - 300;
                guard.drain(0..excess);
            }
        }

        Some(event)
    }

    pub async fn trust(&self, ssid: &str, bssid_str: &str) -> Result<TrustedAp> {
        let bssid = parse_mac(bssid_str).ok_or_else(|| anyhow!("Invalid BSSID {}", bssid_str))?;
        let obs = {
            let guard = self.observed.read().await;
            guard.get(&bssid).cloned()
        };
        let observation = obs.ok_or_else(|| anyhow!("No observation for {}", bssid_str))?;
        let capabilities_fingerprint = observation.capabilities.as_ref().map(|c| {
            capability_string(
                c,
                observation.channels_seen.iter().cloned().max(),
                observation.last_rssi,
                None,
            )
        });
        let record = TrustedApRecord {
            ssid: ssid.to_string(),
            bssid: format_mac(&bssid),
            rsn_profile_json: observation
                .rsn
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok()),
            vendor_oui: Some(format!("{:02X}{:02X}{:02X}", bssid[0], bssid[1], bssid[2])),
            channels_seen_json: Some(serde_json::to_string(
                &observation
                    .channels_seen
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>(),
            )?),
            capabilities_fingerprint: capabilities_fingerprint.clone(),
        };
        self.settings.upsert_trusted_ap(&record).await?;

        let trusted_entry = TrustedAp {
            ssid: ssid.to_string(),
            rsn_profile: observation.rsn.clone(),
            vendor_oui: record.vendor_oui.clone(),
            channels_seen: observation.channels_seen.clone(),
            capabilities_fingerprint,
        };
        {
            let mut guard = self.trusted.write().await;
            guard.insert(bssid, trusted_entry.clone());
        }
        {
            let mut guard = self.by_ssid.write().await;
            guard.entry(ssid.to_string()).or_default().insert(bssid);
        }
        Ok(trusted_entry)
    }

    pub async fn untrust(&self, bssid_str: &str) -> Result<()> {
        let bssid = parse_mac(bssid_str).ok_or_else(|| anyhow!("Invalid BSSID {}", bssid_str))?;
        self.settings.delete_trusted_ap(bssid_str).await?;
        {
            let mut by_ssid = self.by_ssid.write().await;
            if let Some(entry) = self.trusted.write().await.remove(&bssid) {
                if let Some(set) = by_ssid.get_mut(&entry.ssid) {
                    set.remove(&bssid);
                }
            }
        }
        Ok(())
    }

    pub async fn snapshot(&self, window: Duration) -> (Vec<ApView>, Vec<EvilEvent>) {
        let now = Instant::now();
        let observed = self.observed.read().await;
        let trusted = self.trusted.read().await;
        let mut aps: Vec<ApView> = Vec::new();

        for (bssid, ap) in observed.iter() {
            let age = now.saturating_duration_since(ap.last_seen);
            if age > window {
                continue;
            }
            let trusted_entry = trusted.get(bssid);
            aps.push(ApView {
                ssid: ap.ssid.clone(),
                bssid: format_mac(bssid),
                trusted: trusted_entry.is_some(),
                last_seen_ms: Some(age.as_millis().min(u128::from(u64::MAX)) as u64),
                channel: ap.channels_seen.iter().cloned().max(),
                rssi: ap.last_rssi,
                rsn: ap.rsn.clone(),
                vendor_oui: trusted_entry
                    .and_then(|t| t.vendor_oui.clone())
                    .or_else(|| Some(format!("{:02X}{:02X}{:02X}", bssid[0], bssid[1], bssid[2]))),
                capabilities_fingerprint: trusted_entry
                    .and_then(|t| t.capabilities_fingerprint.clone())
                    .or_else(|| {
                        ap.capabilities.as_ref().map(|c| {
                            capability_string(
                                c,
                                ap.channels_seen.iter().cloned().max(),
                                ap.last_rssi,
                                None,
                            )
                        })
                    }),
            });
        }

        for (bssid, t) in trusted.iter() {
            if aps.iter().any(|ap| ap.bssid == format_mac(bssid)) {
                continue;
            }
            aps.push(ApView {
                ssid: t.ssid.clone(),
                bssid: format_mac(bssid),
                trusted: true,
                last_seen_ms: None,
                channel: t.channels_seen.iter().cloned().max(),
                rssi: None,
                rsn: t.rsn_profile.clone(),
                vendor_oui: t.vendor_oui.clone(),
                capabilities_fingerprint: t.capabilities_fingerprint.clone(),
            });
        }

        aps.sort_by(|a, b| a.ssid.cmp(&b.ssid).then(a.bssid.cmp(&b.bssid)));

        let events = self
            .events
            .read()
            .await
            .iter()
            .rev()
            .take(200)
            .cloned()
            .collect();

        (aps, events)
    }
}

fn rsn_matches(a: &RsnProfile, b: &RsnProfile) -> bool {
    a.akms == b.akms && a.ciphers == b.ciphers && a.pmf_required == b.pmf_required
}

fn is_security_downgrade(stored: &RsnProfile, current: &RsnProfile) -> bool {
    (stored.akms.iter().any(|a| a.contains("SAE"))
        && !current.akms.iter().any(|a| a.contains("SAE")))
        || (stored.akms.iter().any(|a| a.contains("EAP"))
            && current.akms.iter().all(|a| !a.contains("EAP")))
        || (stored.pmf_required && !current.pmf_required)
}

pub fn capability_string(
    cap: &CapabilitiesFingerprint,
    channel: Option<u16>,
    rssi: Option<i8>,
    beacon_interval: Option<u16>,
) -> String {
    format!(
        "rates:{:?}|ht:{}|vht:{}|he:{}|bi:{}|ch:{:?}|rssi:{:?}",
        cap.supported_rates,
        cap.has_ht,
        cap.has_vht,
        cap.has_he,
        beacon_interval
            .or(cap.beacon_interval)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        channel,
        rssi
    )
}

fn summarize_rsn(profile: &RsnProfile) -> String {
    let akm = profile.akms.join("|");
    let cipher = profile.ciphers.join("|");
    let pmf = if profile.pmf_required {
        "pmf:required"
    } else if profile.pmf_capable {
        "pmf:capable"
    } else {
        "pmf:off"
    };
    let transition = profile
        .transition_mode
        .as_deref()
        .unwrap_or("-");
    format!("akms={akm} ciphers={cipher} {pmf} transition={transition}")
}

fn summarize_channels(channels: &HashSet<u16>) -> String {
    let mut sorted = BTreeSet::new();
    for ch in channels {
        sorted.insert(*ch);
    }
    let list: Vec<String> = sorted.into_iter().map(|c| c.to_string()).collect();
    list.join(",")
}
