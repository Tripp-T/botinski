use poise::serenity_prelude::GuildId;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct GuildSettings {
    pub volume: f32,
}

impl Default for GuildSettings {
    fn default() -> Self {
        Self { volume: 1.0 }
    }
}

impl GuildSettings {
    pub async fn get(
        pool: &SqlitePool,
        guild_id: GuildId,
    ) -> Result<Option<Self>, sqlx::Error> {
        let key = guild_id.get().to_string();
        let row = sqlx::query!("SELECT volume FROM guild_settings WHERE guild_id = ?", key)
            .fetch_optional(pool)
            .await?;
        Ok(row.map(|r| Self {
            volume: r.volume as f32,
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
}
