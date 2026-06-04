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
    pub async fn get(pool: &SqlitePool, guild_id: GuildId) -> Result<Option<Self>, sqlx::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect to in-memory sqlite");
        sqlx::migrate!().run(&pool).await.expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn get_returns_none_for_unconfigured_guild() {
        let pool = fresh_pool().await;
        let row = GuildSettings::get(&pool, GuildId::new(1)).await.unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn upsert_volume_then_get_round_trip() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(42);
        GuildSettings::upsert_volume(&pool, gid, 0.75)
            .await
            .unwrap();

        let row = GuildSettings::get(&pool, gid).await.unwrap().unwrap();
        assert!((row.volume - 0.75).abs() < 1e-6);
        // unchanged fields keep their column defaults
        assert!((row.max_volume - DEFAULT_MAX_VOLUME).abs() < 1e-6);
        assert_eq!(row.idle_leave_secs, DEFAULT_IDLE_LEAVE_SECS);
        assert!(row.admin_role_ids.is_empty());
    }

    #[tokio::test]
    async fn upsert_volume_updates_existing_row() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(7);
        GuildSettings::upsert_volume(&pool, gid, 0.5).await.unwrap();
        GuildSettings::upsert_volume(&pool, gid, 1.25)
            .await
            .unwrap();

        let row = GuildSettings::get(&pool, gid).await.unwrap().unwrap();
        assert!((row.volume - 1.25).abs() < 1e-6);
    }

    #[tokio::test]
    async fn upsert_max_volume_and_idle_leave_independently() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(99);
        GuildSettings::upsert_max_volume(&pool, gid, 1.5)
            .await
            .unwrap();
        GuildSettings::upsert_idle_leave_secs(&pool, gid, 0)
            .await
            .unwrap();

        let row = GuildSettings::get(&pool, gid).await.unwrap().unwrap();
        assert!((row.max_volume - 1.5).abs() < 1e-6);
        assert_eq!(row.idle_leave_secs, 0);
        // volume kept its column default
        assert!((row.volume - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn upsert_admin_role_ids_round_trip_through_db() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(123);
        let ids = vec![RoleId::new(1), RoleId::new(u64::MAX)];
        GuildSettings::upsert_admin_role_ids(&pool, gid, &ids)
            .await
            .unwrap();

        let row = GuildSettings::get(&pool, gid).await.unwrap().unwrap();
        assert_eq!(row.admin_role_ids, ids);

        // overwriting with empty wipes the list
        GuildSettings::upsert_admin_role_ids(&pool, gid, &[])
            .await
            .unwrap();
        let row = GuildSettings::get(&pool, gid).await.unwrap().unwrap();
        assert!(row.admin_role_ids.is_empty());
    }

    #[test]
    fn admin_role_ids_round_trip_large_snowflakes() {
        // Discord snowflakes can exceed 2^53; verify our string-encoding survives
        // the round trip without precision loss.
        let ids = vec![
            RoleId::new(1),
            RoleId::new(987654321098765432),
            RoleId::new(u64::MAX),
        ];
        let encoded = encode_admin_role_ids(&ids);
        let decoded = parse_admin_role_ids(&encoded);
        assert_eq!(decoded, ids);
    }

    #[test]
    fn admin_role_ids_empty_round_trip() {
        let encoded = encode_admin_role_ids(&[]);
        assert_eq!(encoded, "[]");
        let decoded = parse_admin_role_ids(&encoded);
        assert!(decoded.is_empty());
    }

    #[test]
    fn admin_role_ids_parse_garbage_yields_empty() {
        // Bad JSON shouldn't panic; we should yield an empty list and keep going.
        assert!(parse_admin_role_ids("not json").is_empty());
        assert!(parse_admin_role_ids("[1, 2, 3]").is_empty()); // numbers, not strings
    }
}
