use crate::config::AppConfig;
use crate::devices::{self, DeviceTracker};
use crate::events::{EventKind, EventSettings, NoiseMode, PacketEvent};
use crate::ui;
use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use axum::{
    Json, Router,
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::{RwLock, broadcast};
use tokio::time;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub audio_enabled: Arc<AtomicBool>,
    pub web_sound_enabled: Arc<AtomicBool>,
    pub volume_by_signal: Arc<AtomicBool>,
    pub packet_tx: broadcast::Sender<PacketEvent>,
    pub channel: ChannelController,
    pub channels_24: Arc<RwLock<Vec<ChannelInfo>>>,
    pub channels_5: Arc<RwLock<Vec<ChannelInfo>>>,
    pub event_settings: Arc<RwLock<EventSettings>>,
    pub device_tracker: Arc<DeviceTracker>,
}

#[derive(Clone)]
pub struct ChannelController {
    interface: Arc<String>,
    current: Arc<RwLock<Option<u16>>>,
}

impl ChannelController {
    pub fn new(interface: String) -> Self {
        Self {
            interface: Arc::new(interface),
            current: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn refresh_current(&self) -> Result<Option<u16>> {
        let detected = current_channel(&self.interface).await?;
        let mut guard = self.current.write().await;
        *guard = detected;
        Ok(*guard)
    }

    pub async fn current(&self) -> Option<u16> {
        *self.current.read().await
    }

    pub async fn set_channel(&self, channel: u16) -> Result<u16> {
        apply_channel(&self.interface, channel).await?;
        let mut guard = self.current.write().await;
        *guard = Some(channel);
        Ok(channel)
    }
}

pub async fn serve(state: AppState) -> Result<()> {
    {
        let iface = state.config.monitor_interface.clone();
        let channels_24 = Arc::clone(&state.channels_24);
        let channels_5 = Arc::clone(&state.channels_5);
        tokio::spawn(async move {
            match detect_supported_channels(&iface).await {
                Ok((c24, c5)) => {
                    *channels_24.write().await = c24;
                    *channels_5.write().await = c5;
                }
                Err(err) => {
                    tracing::warn!("Unable to detect supported channels: {err:?}");
                }
            }
        });
    }

    let router = Router::new()
        .route("/", get(index))
        .route("/api/settings", get(settings))
        .route("/api/channel", post(set_channel))
        .route("/api/sound", post(update_sound))
        .route("/api/events", get(events_settings).post(update_events))
        .route("/api/devices", get(devices))
        .route("/api/device-filters", post(update_device_filters))
        .route("/api/device-reset", post(reset_device_counts))
        .route("/api/shutdown", post(shutdown))
        .route("/ws/packets", get(ws_packets))
        .route("/ws/devices", get(ws_devices))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    let addr: SocketAddr = state.config.http_bind.parse()?;
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("UI listening on http://{addr}");

    axum::serve(listener, router)
        .with_graceful_shutdown(graceful_shutdown())
        .await?;

    Ok(())
}

async fn graceful_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Shutting down http server");
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    Html(ui::render_html(&state.config.monitor_interface))
}

#[derive(Serialize, Clone)]
pub struct ChannelInfo {
    channel: u16,
    enabled: bool,
}

#[derive(Serialize)]
struct EventToggle {
    id: EventKind,
    label: &'static str,
    enabled: bool,
}

#[derive(Serialize)]
struct SettingsResponse {
    monitor_interface: String,
    channel: Option<u16>,
    audio_jack: bool,
    web_ui_sound: bool,
    volume_by_signal: bool,
    available_channels_24ghz: Vec<ChannelInfo>,
    available_channels_5ghz: Vec<ChannelInfo>,
    packet_events: Vec<EventToggle>,
    mode: NoiseMode,
    data_tick_n: u32,
}

async fn settings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let channel = state.channel.current().await;
    let channels_24 = state.channels_24.read().await.clone();
    let channels_5 = state.channels_5.read().await.clone();
    let event_settings = state.event_settings.read().await.clone();
    let toggles = all_event_toggles(&event_settings);
    Ok(Json(SettingsResponse {
        monitor_interface: state.config.monitor_interface.clone(),
        channel,
        audio_jack: state.audio_enabled.load(Ordering::Relaxed),
        web_ui_sound: state.web_sound_enabled.load(Ordering::Relaxed),
        volume_by_signal: state.volume_by_signal.load(Ordering::Relaxed),
        available_channels_24ghz: channels_24,
        available_channels_5ghz: channels_5,
        packet_events: toggles,
        mode: event_settings.mode.clone(),
        data_tick_n: data_tick_for(&event_settings.mode),
    }))
}

#[derive(Deserialize)]
struct DevicesQuery {
    window_seconds: Option<u64>,
    window_minutes: Option<u64>,
}

#[derive(Serialize)]
struct DevicesResponse {
    window_seconds: u64,
    devices: Vec<devices::DeviceView>,
}

async fn devices(
    State(state): State<AppState>,
    Query(params): Query<DevicesQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let window = window_from_query(&params);
    let snapshot = state.device_tracker.snapshot(Duration::from_secs(window));
    Ok(Json(DevicesResponse {
        window_seconds: window,
        devices: snapshot,
    }))
}

fn window_from_query(params: &DevicesQuery) -> u64 {
    let base_seconds = if let Some(min) = params.window_minutes {
        min.saturating_mul(60)
    } else if let Some(sec) = params.window_seconds {
        sec
    } else {
        600
    };
    base_seconds.clamp(60, 7200)
}

#[derive(Deserialize)]
struct ChannelRequest {
    channel: u16,
}

#[derive(Serialize)]
struct ChannelResponse {
    channel: u16,
}

async fn set_channel(
    State(state): State<AppState>,
    Json(body): Json<ChannelRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let channel = state
        .channel
        .set_channel(body.channel)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to set channel: {err}"),
            )
        })?;

    tracing::info!("Monitor interface set to channel {channel}");
    Ok(Json(ChannelResponse { channel }))
}

#[derive(Deserialize)]
struct UpdateSoundRequest {
    audio_jack: Option<bool>,
    web_ui: Option<bool>,
    volume_by_signal: Option<bool>,
}

#[derive(Serialize)]
struct SoundResponse {
    audio_jack: bool,
    web_ui_sound: bool,
    volume_by_signal: bool,
}

async fn update_sound(
    State(state): State<AppState>,
    Json(body): Json<UpdateSoundRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if let Some(audio_jack) = body.audio_jack {
        state.audio_enabled.store(audio_jack, Ordering::Relaxed);
    }
    if let Some(web_ui) = body.web_ui {
        state.web_sound_enabled.store(web_ui, Ordering::Relaxed);
    }
    if let Some(v) = body.volume_by_signal {
        state.volume_by_signal.store(v, Ordering::Relaxed);
    }

    Ok(Json(SoundResponse {
        audio_jack: state.audio_enabled.load(Ordering::Relaxed),
        web_ui_sound: state.web_sound_enabled.load(Ordering::Relaxed),
        volume_by_signal: state.volume_by_signal.load(Ordering::Relaxed),
    }))
}

#[derive(Serialize)]
struct EventsResponse {
    mode: NoiseMode,
    data_tick_n: u32,
    events: Vec<EventToggle>,
}

async fn events_settings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let settings = state.event_settings.read().await.clone();
    Ok(Json(build_events_response(&settings)))
}

#[derive(Deserialize)]
struct UpdateEventsRequest {
    mode: Option<NoiseMode>,
    events: Option<Vec<EventToggleInput>>,
}

#[derive(Deserialize)]
struct EventToggleInput {
    id: EventKind,
    enabled: bool,
}

async fn update_events(
    State(state): State<AppState>,
    Json(body): Json<UpdateEventsRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let updated = {
        let mut settings = state.event_settings.write().await;
        if let Some(mode) = body.mode {
            settings.mode = mode;
        }
        if let Some(events) = body.events {
            for evt in events {
                settings.enabled.insert(evt.id, evt.enabled);
            }
        }
        settings.clone()
    };
    Ok(Json(build_events_response(&updated)))
}

#[derive(Deserialize)]
struct DeviceFilterRequest {
    devices: Vec<DeviceToggleInput>,
}

#[derive(Deserialize)]
struct DeviceToggleInput {
    mac: String,
    allowed: bool,
}

#[derive(Serialize)]
struct DeviceFilterResponse {
    updated: usize,
}

async fn update_device_filters(
    State(state): State<AppState>,
    Json(body): Json<DeviceFilterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut parsed = Vec::new();
    for item in body.devices {
        let mac = devices::parse_mac(&item.mac).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid MAC address: {}", item.mac),
            )
        })?;
        parsed.push((mac, item.allowed));
    }

    state.device_tracker.set_many(&parsed);
    Ok(Json(DeviceFilterResponse {
        updated: parsed.len(),
    }))
}

#[derive(Serialize)]
struct DeviceResetResponse {
    reset: bool,
}

async fn reset_device_counts(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    state.device_tracker.reset_counts();
    Ok(Json(DeviceResetResponse { reset: true }))
}

#[derive(Deserialize)]
struct ShutdownRequest {
    confirm: bool,
}

async fn shutdown(
    State(_state): State<AppState>,
    Json(body): Json<ShutdownRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !body.confirm {
        return Err((StatusCode::BAD_REQUEST, "Confirmation required".into()));
    }

    tokio::task::spawn_blocking(run_shutdown)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task join error: {err}"),
            )
        })?
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Shutdown failed: {err}"),
            )
        })?;

    tracing::info!("Shutdown requested via UI");
    Ok(StatusCode::ACCEPTED)
}

fn run_shutdown() -> Result<()> {
    let status = std::process::Command::new("systemctl")
        .arg("poweroff")
        .status()
        .map_err(|err| anyhow::anyhow!(err))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("shutdown exited with status {}", status))
    }
}

async fn ws_devices(
    State(state): State<AppState>,
    Query(params): Query<DevicesQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let window = window_from_query(&params);
    ws.on_upgrade(move |socket| handle_ws_devices(socket, state, window))
}

async fn ws_packets(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.packet_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        if !state.web_sound_enabled.load(Ordering::Relaxed) {
            continue;
        }
        let payload = match serde_json::to_string(&WsEvent {
            kind: evt.kind,
            retry: evt.retry,
            amplitude: evt.amplitude,
        }) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if socket.send(Message::Text(payload)).await.is_err() {
            break;
        }
    }
}

async fn handle_ws_devices(mut socket: WebSocket, state: AppState, window: u64) {
    let mut interval = time::interval(Duration::from_secs(10));
    if send_devices_snapshot(&mut socket, &state, window)
        .await
        .is_err()
    {
        return;
    }
    loop {
        interval.tick().await;
        if send_devices_snapshot(&mut socket, &state, window)
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn send_devices_snapshot(
    socket: &mut WebSocket,
    state: &AppState,
    window: u64,
) -> Result<(), ()> {
    let snapshot = state.device_tracker.snapshot(Duration::from_secs(window));
    let payload = serde_json::to_string(&DevicesResponse {
        window_seconds: window,
        devices: snapshot,
    })
    .map_err(|_| ())?;
    socket.send(Message::Text(payload)).await.map_err(|_| ())
}

#[derive(Serialize)]
struct WsEvent {
    kind: EventKind,
    retry: bool,
    amplitude: f32,
}

async fn current_channel(interface: &str) -> Result<Option<u16>> {
    let output = Command::new("iw")
        .args(["dev", interface, "info"])
        .output()
        .await
        .with_context(|| format!("Failed to read channel for {interface}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("iw dev {interface} info failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("channel ") {
            if let Some(token) = rest.split_whitespace().next() {
                if let Ok(channel) = token.parse::<u16>() {
                    return Ok(Some(channel));
                }
            }
        }
    }
    Ok(None)
}

async fn apply_channel(interface: &str, channel: u16) -> Result<()> {
    let status = Command::new("iw")
        .args(["dev", interface, "set", "channel", &channel.to_string()])
        .status()
        .await
        .with_context(|| format!("Failed to set channel {channel} on {interface}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "iw set channel exited with status {status}"
        ))
    }
}

async fn detect_supported_channels(
    interface: &str,
) -> Result<(Vec<ChannelInfo>, Vec<ChannelInfo>)> {
    let phy = detect_phy(interface).await?;
    let output = Command::new("iw")
        .args(["phy", &phy, "info"])
        .output()
        .await
        .with_context(|| format!("Failed to read supported channels for {phy}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("iw phy info failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut channels_24: BTreeMap<u16, bool> = BTreeMap::new();
    let mut channels_5: BTreeMap<u16, bool> = BTreeMap::new();

    for line in stdout.lines() {
        let line = line.trim_start();
        if !line.starts_with('*') {
            continue;
        }
        let is_disabled = line.to_lowercase().contains("disabled");
        // Example: "* 2412 MHz [1] (20.0 dBm)"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let freq_mhz = parts
            .get(1)
            .and_then(|s| s.trim_end_matches(".0").parse::<f32>().ok());
        let channel_num = line.find('[').and_then(|start| {
            line[start + 1..]
                .find(']')
                .and_then(|end| line[start + 1..start + 1 + end].trim().parse::<u16>().ok())
        });

        if let (Some(freq), Some(ch)) = (freq_mhz, channel_num) {
            if freq < 3000.0 {
                channels_24.insert(ch, !is_disabled);
            } else if freq < 6000.0 {
                channels_5.insert(ch, !is_disabled);
            }
        }
    }

    let c24 = if channels_24.is_empty() {
        (1..=14)
            .map(|ch| ChannelInfo {
                channel: ch,
                enabled: false,
            })
            .collect()
    } else {
        channels_24
            .into_iter()
            .map(|(ch, enabled)| ChannelInfo {
                channel: ch,
                enabled,
            })
            .collect()
    };
    let c5 = if channels_5.is_empty() {
        vec![]
    } else {
        channels_5
            .into_iter()
            .map(|(ch, enabled)| ChannelInfo {
                channel: ch,
                enabled,
            })
            .collect()
    };
    Ok((c24, c5))
}

fn all_event_toggles(settings: &EventSettings) -> Vec<EventToggle> {
    use EventKind::*;
    let order = vec![
        Beacon, ProbeReq, ProbeResp, Assoc, Deauth, Eapol, Rts, Cts, Ack, DataTick,
    ];
    order
        .into_iter()
        .map(|id| EventToggle {
            label: event_label(&id),
            enabled: *settings.enabled.get(&id).unwrap_or(&true),
            id,
        })
        .collect()
}

fn event_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Beacon => "Beacon",
        EventKind::ProbeReq => "Probe request",
        EventKind::ProbeResp => "Probe response",
        EventKind::Assoc => "Association / Reassociation",
        EventKind::Deauth => "Deauthentication / Disassociation",
        EventKind::Eapol => "EAPOL (handshake)",
        EventKind::Rts => "RTS",
        EventKind::Cts => "CTS",
        EventKind::Ack => "ACK / Block ACK",
        EventKind::DataTick => "Data tick",
    }
}

fn data_tick_for(mode: &NoiseMode) -> u32 {
    match mode {
        NoiseMode::Crowded => 100,
        NoiseMode::Sparse => 10,
    }
}

fn build_events_response(settings: &EventSettings) -> EventsResponse {
    EventsResponse {
        mode: settings.mode.clone(),
        data_tick_n: data_tick_for(&settings.mode),
        events: all_event_toggles(settings),
    }
}

async fn detect_phy(interface: &str) -> Result<String> {
    let output = Command::new("iw")
        .args(["dev", interface, "info"])
        .output()
        .await
        .with_context(|| format!("Failed to query phy for {interface}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("iw dev {interface} info failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("wiphy ") {
            return Ok(format!("phy{}", rest.trim()));
        }
    }

    anyhow::bail!("Could not find wiphy for interface {interface}")
}
