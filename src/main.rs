mod audio;
mod config;
mod devices;
mod events;
mod sniffer;
mod ui;
mod web;

use crate::audio::AudioEngine;
use crate::config::AppConfig;
use crate::devices::DeviceTracker;
use crate::events::{EventKind, EventSettings, EventWindow, NoiseMode, PacketEvent, RateLimiter};
use crate::web::{AppState, ChannelController};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = Arc::new(AppConfig::from_env());

    tracing::info!(
        "Starting radioscope on {} (interface: {})",
        config.http_bind,
        config.monitor_interface
    );

    let audio_engine = AudioEngine::new()?;
    let audio_handle = audio_engine.handle();

    let audio_enabled = Arc::new(AtomicBool::new(true));
    let web_sound_enabled = Arc::new(AtomicBool::new(false));
    let volume_by_signal = Arc::new(AtomicBool::new(false));
    let (packet_notifier_tx, _) = broadcast::channel(64);
    let channel_controller = ChannelController::new(config.monitor_interface.clone());
    let channels_24 = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let channels_5 = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let event_settings = Arc::new(tokio::sync::RwLock::new(EventSettings::default()));
    let device_tracker = Arc::new(DeviceTracker::new());

    if let Err(err) = channel_controller.refresh_current().await {
        tracing::warn!("Unable to read initial channel: {err:?}");
    }

    let (packet_tx, mut packet_rx) = mpsc::unbounded_channel::<PacketEvent>();
    let _sniffer_thread = sniffer::spawn_sniffer(
        config.monitor_interface.clone(),
        packet_tx,
        Arc::clone(&device_tracker),
    );

    let audio_task_handle = audio_handle.clone();
    let audio_enabled_flag = audio_enabled.clone();
    let web_sound_flag = web_sound_enabled.clone();
    let packet_notifier = packet_notifier_tx.clone();
    let settings_handle = event_settings.clone();
    let volume_by_signal_flag = volume_by_signal.clone();
    let device_filter = device_tracker.clone();
    let audio_task = tokio::spawn(async move {
        let mut window = EventWindow::new(Duration::from_millis(100));
        let mut limiter = RateLimiter::new();
        let mut data_counter: u32 = 0;
        while let Some(evt) = packet_rx.recv().await {
            if !device_filter.allows(evt.src, evt.bssid) {
                continue;
            }
            let settings = settings_handle.read().await.clone();
            if !*settings.enabled.get(&evt.kind).unwrap_or(&true) {
                continue;
            }

            // Data tick aggregation
            if evt.kind == EventKind::DataTick {
                data_counter += 1;
                let threshold = match settings.mode {
                    NoiseMode::Crowded => 100,
                    NoiseMode::Sparse => 10,
                };
                if data_counter < threshold {
                    continue;
                }
                data_counter = 0;
            }

            let (max_mgmt, max_ctrl, max_data) = match settings.mode {
                NoiseMode::Crowded => (3, 2, 1),
                NoiseMode::Sparse => (5, 3, 1),
            };

            if !window.try_count(&evt.kind, max_mgmt, max_ctrl, max_data) {
                continue;
            }

            let min_gap = min_interval_for(&evt.kind, &settings.mode);
            if !limiter.allow(&evt.kind, evt.rate_key.clone(), min_gap) {
                continue;
            }

            if audio_enabled_flag.load(Ordering::Relaxed) {
                let sound = sound_for(&evt.kind);
                let gain = if volume_by_signal_flag.load(Ordering::Relaxed) {
                    evt.amplitude
                } else {
                    1.0
                };
                audio_task_handle.play(sound, evt.retry, gain);
            }
            if web_sound_flag.load(Ordering::Relaxed) {
                let _ = packet_notifier.send(evt.clone());
            }
        }
    });

    let state = AppState {
        config: config.clone(),
        audio_enabled,
        web_sound_enabled,
        volume_by_signal,
        packet_tx: packet_notifier_tx,
        channel: channel_controller,
        channels_24,
        channels_5,
        event_settings,
        device_tracker,
    };

    web::serve(state).await?;

    audio_task.abort();
    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));
    let _ = fmt().with_env_filter(env_filter).try_init();
}

fn min_interval_for(kind: &EventKind, mode: &NoiseMode) -> Duration {
    match kind {
        EventKind::Beacon => Duration::from_millis(333),
        EventKind::ProbeReq => Duration::from_millis(200),
        EventKind::ProbeResp => Duration::from_millis(200),
        EventKind::Assoc => Duration::from_millis(500),
        EventKind::Deauth => Duration::from_millis(500),
        EventKind::Eapol => Duration::from_millis(300),
        EventKind::Rts | EventKind::Cts => Duration::from_millis(150),
        EventKind::Ack => Duration::from_millis(match mode {
            NoiseMode::Crowded => 80,
            NoiseMode::Sparse => 40,
        }),
        EventKind::DataTick => Duration::from_millis(200),
    }
}

fn sound_for(kind: &EventKind) -> audio::SoundId {
    use audio::SoundId::*;
    match kind {
        EventKind::Beacon => BeaconTick,
        EventKind::ProbeReq => ProbeChirp,
        EventKind::ProbeResp => ProbeReply,
        EventKind::Assoc => AssocUp,
        EventKind::Deauth => DeauthZap,
        EventKind::Eapol => EapolMotif,
        EventKind::Rts => RtsKnock,
        EventKind::Cts => CtsKnockback,
        EventKind::Ack => AckClick,
        EventKind::DataTick => DataTick,
    }
}
