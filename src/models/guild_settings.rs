use anyhow::Context;
use poise::serenity_prelude::{GuildId, RoleId};
use sqlx::SqlitePool;

pub const DEFAULT_MAX_VOLUME: f32 = 2.0;
pub const DEFAULT_IDLE_LEAVE_SECS: i64 = 300;

#[derive(Debug, Clone)]
pub struct GuildSettings {
    pub volume: f32,
    pub max_volume: f32,
    pub idle_leave_secs: i64,
    pub admin_role_ids: Vec<RoleId>,
}

impl Default for GuildSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            max_volume: DEFAULT_MAX_VOLUME,
            idle_leave_secs: DEFAULT_IDLE_LEAVE_SECS,
            admin_role_ids: Vec::new(),
        }
    }
}

fn parse_admin_role_ids(raw: &str) -> Vec<RoleId> {
    serde_json::from_str::<Vec<String>>(raw)
        .ok()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.parse::<u64>().ok())
        .map(RoleId::new)
        .collect()
}

fn encode_admin_role_ids(ids: &[RoleId]) -> String {
    let strs: Vec<String> = ids.iter().map(|r| r.get().to_string()).collect();
    serde_json::to_string(&strs).unwrap_or_else(|_| "[]".to_string())
}

impl GuildSettings {
    pub async fn get(
        pool: &SqlitePool,
        guild_id: GuildId,
    ) -> Result<Option<Self>, sqlx::Error> {
        let key = guild_id.get().to_string();
        let row = sqlx::query!(
            "SELECT volume, max_volume, idle_leave_secs, admin_role_ids FROM guild_settings WHERE guild_id = ?",
            key
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| Self {
            volume: r.volume as f32,
            max_volume: r.max_volume as f32,
            idle_leave_secs: r.idle_leave_secs,
            admin_role_ids: parse_admin_role_ids(&r.admin_role_ids),
        }))
    }

    pub async fn upsert_volume(
        pool: &SqlitePool,
        guild_id: GuildId,
        volume: f32,
    ) -> Result<(), sqlx::Error> {
        let key = guild_id.get().to_string();
        let v = volume as f64;
        sqlx::query!(
            "INSERT INTO guild_settings (guild_id, volume) VALUES (?, ?) \
             ON CONFLICT(guild_id) DO UPDATE SET volume = excluded.volume",
            key,
            v
        )
        .execute(pool)
        .await
        .map(|_| ())
    }

    pub async fn upsert_max_volume(
        pool: &SqlitePool,
        guild_id: GuildId,
        max_volume: f32,
    ) -> Result<(), sqlx::Error> {
        let key = guild_id.get().to_string();
        let v = max_volume as f64;
        sqlx::query!(
            "INSERT INTO guild_settings (guild_id, max_volume) VALUES (?, ?) \
             ON CONFLICT(guild_id) DO UPDATE SET max_volume = excluded.max_volume",
            key,
            v
        )
        .execute(pool)
        .await
        .map(|_| ())
    }

    pub async fn upsert_idle_leave_secs(
        pool: &SqlitePool,
        guild_id: GuildId,
        idle_leave_secs: i64,
    ) -> Result<(), sqlx::Error> {
        let key = guild_id.get().to_string();
        sqlx::query!(
            "INSERT INTO guild_settings (guild_id, idle_leave_secs) VALUES (?, ?) \
             ON CONFLICT(guild_id) DO UPDATE SET idle_leave_secs = excluded.idle_leave_secs",
            key,
            idle_leave_secs
        )
        .execute(pool)
        .await
        .map(|_| ())
    }

    pub async fn upsert_admin_role_ids(
        pool: &SqlitePool,
        guild_id: GuildId,
        ids: &[RoleId],
    ) -> anyhow::Result<()> {
        let key = guild_id.get().to_string();
        let encoded = encode_admin_role_ids(ids);
        sqlx::query!(
            "INSERT INTO guild_settings (guild_id, admin_role_ids) VALUES (?, ?) \
             ON CONFLICT(guild_id) DO UPDATE SET admin_role_ids = excluded.admin_role_ids",
            key,
            encoded
        )
        .execute(pool)
        .await
        .context("Failed to upsert admin_role_ids")
        .map(|_| ())
    }
}
