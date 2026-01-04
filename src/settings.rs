use crate::events::{EventKind, EventSettings, NoiseMode};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiSettings {
    pub audio_jack: bool,
    pub web_ui_sound: bool,
    pub volume_by_signal: bool,
    pub volume_percent: u32,
    pub mode: NoiseMode,
    pub events: HashMap<EventKind, bool>,
    pub channel: Option<u16>,
}

impl UiSettings {
    pub fn from_event_settings(
        audio_jack: bool,
        web_ui_sound: bool,
        volume_by_signal: bool,
        volume_percent: u32,
        event_settings: &EventSettings,
        channel: Option<u16>,
    ) -> Self {
        Self {
            audio_jack,
            web_ui_sound,
            volume_by_signal,
            volume_percent,
            mode: event_settings.mode.clone(),
            events: event_settings.enabled.clone(),
            channel,
        }
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    pool: SqlitePool,
}

impl SettingsStore {
    pub async fn connect(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create settings db parent {}", parent.display())
                })?;
            }
        }
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("settings db path is not valid UTF-8"))?;

        let options = SqliteConnectOptions::from_str(path_str)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::query(
            "
            CREATE TABLE IF NOT EXISTS ui_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                audio_jack INTEGER NOT NULL,
                web_ui_sound INTEGER NOT NULL,
                volume_by_signal INTEGER NOT NULL,
                volume_percent INTEGER NOT NULL,
                mode TEXT NOT NULL,
                events_json TEXT NOT NULL,
                channel INTEGER
            );
            INSERT INTO ui_settings (id, audio_jack, web_ui_sound, volume_by_signal, volume_percent, mode, events_json, channel)
            SELECT 1, 1, 0, 0, 25, 'crowded', '{}', NULL
            WHERE NOT EXISTS (SELECT 1 FROM ui_settings WHERE id = 1);
            ",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn load(&self) -> Result<UiSettings> {
        let row: (i64, i64, i64, i64, String, String, Option<i64>) = sqlx::query_as(
            "
            SELECT audio_jack, web_ui_sound, volume_by_signal, volume_percent, mode, events_json, channel
            FROM ui_settings WHERE id = 1
            ",
        )
        .fetch_one(&self.pool)
        .await?;

        let mode = match row.4.as_str() {
            "sparse" => NoiseMode::Sparse,
            _ => NoiseMode::Crowded,
        };
        let events: HashMap<EventKind, bool> = serde_json::from_str(&row.5).unwrap_or_default();
        Ok(UiSettings {
            audio_jack: row.0 != 0,
            web_ui_sound: row.1 != 0,
            volume_by_signal: row.2 != 0,
            volume_percent: (row.3 as u32).clamp(0, 100),
            mode,
            events,
            channel: row.6.map(|c| c as u16),
        })
    }

    pub async fn save(&self, settings: &UiSettings) -> Result<()> {
        let payload = settings.clone();
        let events_json = serde_json::to_string(&payload.events)?;
        sqlx::query(
            "
            INSERT INTO ui_settings (id, audio_jack, web_ui_sound, volume_by_signal, volume_percent, mode, events_json, channel)
            VALUES (1, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                audio_jack=excluded.audio_jack,
                web_ui_sound=excluded.web_ui_sound,
                volume_by_signal=excluded.volume_by_signal,
                volume_percent=excluded.volume_percent,
                mode=excluded.mode,
                events_json=excluded.events_json,
                channel=excluded.channel
            ",
        )
        .bind(payload.audio_jack as i64)
        .bind(payload.web_ui_sound as i64)
        .bind(payload.volume_by_signal as i64)
        .bind(payload.volume_percent as i64)
        .bind(mode_to_str(&payload.mode))
        .bind(events_json)
        .bind(payload.channel.map(|c| c as i64))
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn mode_to_str(mode: &NoiseMode) -> &'static str {
    match mode {
        NoiseMode::Crowded => "crowded",
        NoiseMode::Sparse => "sparse",
    }
}
