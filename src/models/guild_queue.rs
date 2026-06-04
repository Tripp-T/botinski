//! Persisted per-guild track queue. Restored on the first interaction with
//! a guild after a bot restart so user-built queues survive redeploys.
//!
//! The "currently playing" track is stored as the first element of the JSON
//! array; on restore everything becomes the upcoming queue (we can't resume
//! mid-track because we don't preserve playback position).

use crate::music::Track;
use anyhow::Context;
use poise::serenity_prelude::GuildId;
use sqlx::SqlitePool;

pub async fn load(pool: &SqlitePool, guild_id: GuildId) -> anyhow::Result<Vec<Track>> {
    let key = guild_id.get().to_string();
    let row = sqlx::query!("SELECT tracks FROM guild_queue WHERE guild_id = ?", key)
        .fetch_optional(pool)
        .await
        .context("Failed to load guild_queue row")?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    Ok(serde_json::from_str(&row.tracks).unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, guild_id: GuildId, tracks: &[Track]) -> anyhow::Result<()> {
    let key = guild_id.get().to_string();
    let blob = serde_json::to_string(tracks).context("Failed to serialise queue")?;
    sqlx::query!(
        "INSERT INTO guild_queue (guild_id, tracks) VALUES (?, ?) \
         ON CONFLICT(guild_id) DO UPDATE SET tracks = excluded.tracks",
        key,
        blob
    )
    .execute(pool)
    .await
    .context("Failed to upsert guild_queue")
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{NewTrack, Track};
    use poise::serenity_prelude::UserId;
    use std::time::Duration;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect to in-memory sqlite");
        sqlx::migrate!().run(&pool).await.expect("run migrations");
        pool
    }

    fn sample_tracks() -> Vec<Track> {
        vec![
            Track::new(NewTrack {
                title: "first".into(),
                url: "https://www.youtube.com/watch?v=a".into(),
                duration: Some(Duration::from_secs(123)),
                requested_by_name: "alice".into(),
                requested_by_id: UserId::new(1),
                is_live: false,
            }),
            Track::new(NewTrack {
                title: "second (live)".into(),
                url: "https://www.youtube.com/watch?v=b".into(),
                duration: None,
                requested_by_name: "bob".into(),
                requested_by_id: UserId::new(2),
                is_live: true,
            }),
        ]
    }

    #[tokio::test]
    async fn load_returns_empty_for_unknown_guild() {
        let pool = fresh_pool().await;
        let tracks = load(&pool, GuildId::new(1)).await.unwrap();
        assert!(tracks.is_empty());
    }

    #[tokio::test]
    async fn save_then_load_round_trip_preserves_track_data() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(42);
        let original = sample_tracks();
        save(&pool, gid, &original).await.unwrap();

        let restored = load(&pool, gid).await.unwrap();
        assert_eq!(restored.len(), original.len());
        for (a, b) in restored.iter().zip(original.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.title, b.title);
            assert_eq!(a.url, b.url);
            assert_eq!(a.duration, b.duration);
            assert_eq!(a.requested_by_id, b.requested_by_id);
            assert_eq!(a.is_live, b.is_live);
        }
    }

    #[tokio::test]
    async fn save_overwrites_previous_row() {
        let pool = fresh_pool().await;
        let gid = GuildId::new(7);
        save(&pool, gid, &sample_tracks()).await.unwrap();
        save(&pool, gid, &[]).await.unwrap();
        assert!(load(&pool, gid).await.unwrap().is_empty());
    }
}
