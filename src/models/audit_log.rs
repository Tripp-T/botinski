//! Append-only audit log of user-initiated actions.
//!
//! Two sources: `discord` (slash commands) and `web` (mutating HTTP requests).
//! Viewable to global admins only via `/admin/audit-log`.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    #[allow(dead_code)]
    pub id: i64,
    pub occurred_at: DateTime<Utc>,
    pub source: String,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    pub guild_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub outcome: String,
}

pub struct NewAuditLogEntry<'a> {
    pub source: &'a str,
    pub actor_id: Option<&'a str>,
    pub actor_name: Option<&'a str>,
    pub guild_id: Option<&'a str>,
    pub action: &'a str,
    pub detail: Option<&'a str>,
    pub outcome: &'a str,
}

impl AuditLogEntry {
    pub async fn insert(pool: &SqlitePool, e: NewAuditLogEntry<'_>) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        sqlx::query!(
            "INSERT INTO audit_log (occurred_at, source, actor_id, actor_name, guild_id, action, detail, outcome) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            now,
            e.source,
            e.actor_id,
            e.actor_name,
            e.guild_id,
            e.action,
            e.detail,
            e.outcome
        )
        .execute(pool)
        .await
        .map(|_| ())
    }

    /// Most-recent `limit` entries, newest first.
    pub async fn recent(pool: &SqlitePool, limit: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            AuditLogEntry,
            r#"
            SELECT
                id AS "id!: i64",
                occurred_at AS "occurred_at: DateTime<Utc>",
                source,
                actor_id,
                actor_name,
                guild_id,
                action,
                detail,
                outcome
            FROM audit_log
            ORDER BY occurred_at DESC
            LIMIT ?
            "#,
            limit
        )
        .fetch_all(pool)
        .await
    }
}
