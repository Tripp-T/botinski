use anyhow::{Context, bail};
use axum::{RequestPartsExt, extract::FromRequestParts};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::{SqlitePool, prelude::FromRow};
use std::{net::IpAddr, str::FromStr};
use subtle::ConstantTimeEq;
use tracing::warn;
use uuid::Uuid;

use crate::{AppState, http::HttpError};

#[derive(FromRow, Debug, Clone)]
pub struct AppSession {
    pub id: Uuid,
    pub hashed_token: Vec<u8>,
    pub user_id: Uuid,
    pub user_agent: String,
    pub ip: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl AppSession {
    const DAYS_VALID: i64 = 14;

    pub async fn new(
        pool: &SqlitePool,
        user_id: Uuid,
        user_agent: String,
        ip: IpAddr,
    ) -> Result<(Self, String), sqlx::Error> {
        let mut token_bytes = [0u8; 32];
        rand::rng().fill(&mut token_bytes);
        let token_hash = Sha256::digest(token_bytes).to_vec();
        let token_base64 = BASE64_URL_SAFE_NO_PAD.encode(token_bytes);
        let now = Utc::now();

        let session = Self {
            id: Uuid::new_v4(),
            hashed_token: token_hash,
            user_id,
            user_agent,
            ip: ip.to_string(),
            created_at: now,
            expires_at: now + Duration::days(Self::DAYS_VALID),
        };

        sqlx::query!(
            "INSERT INTO sessions (id, hashed_token, user_id, user_agent, ip, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            session.id,
            session.hashed_token,
            session.user_id,
            session.user_agent,
            session.ip,
            session.created_at,
            session.expires_at,
        ).execute(pool).await?;

        Ok((session, token_base64))
    }
    pub async fn get_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            AppSession,
            r#"
            SELECT
                id AS "id: Uuid",
                hashed_token,
                user_id AS "user_id: Uuid",
                user_agent,
                ip,
                created_at AS "created_at: DateTime<Utc>",
                expires_at AS "expires_at: DateTime<Utc>"
            FROM sessions
            WHERE id = ?
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }
    pub async fn delete_by_id(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
            .execute(pool)
            .await
            .map(|_| ())
    }
    /// Removes all sessions whose `expires_at` is in the past. Returns count reaped.
    pub async fn delete_expired(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let now = Utc::now();
        sqlx::query!("DELETE FROM sessions WHERE expires_at < ?", now)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
    }
}
impl FromRequestParts<AppState> for AppSession {
    type Rejection = HttpError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(session) = parts.extensions.get::<Self>().cloned() {
            return Ok(session);
        }

        let Some(cookie_key) = parts.extensions.get::<tower_cookies::Key>().cloned() else {
            return Err(HttpError::from(anyhow::anyhow!("Missing cookie key")));
        };
        let Ok(cookie_jar) = parts.extract::<tower_cookies::Cookies>().await else {
            return Err(HttpError::from(anyhow::anyhow!("Failed to get cookies")));
        };
        let private_cookies = cookie_jar.private(&cookie_key);
        let Some(session_cookie) = private_cookies.get("user-session") else {
            return Err(HttpError::Unauthorized);
        };
        let session_cookie_value = AppSessionCookie::from_cookie_str(session_cookie.value())?;

        let Some(db_session) = Self::get_by_id(&state.db, session_cookie_value.id)
            .await
            .context("Failed to query for session")?
        else {
            private_cookies.remove(session_cookie);
            return Err(HttpError::Unauthorized);
        };

        if db_session.expires_at < Utc::now() {
            private_cookies.remove(session_cookie);
            warn!("Session cookie that outlived its expiration");
            return Err(HttpError::Unauthorized);
        }

        let token_bytes = match BASE64_URL_SAFE_NO_PAD
            .decode(session_cookie_value.token)
            .context("Failed to parse session token")
        {
            Ok(tb) => tb,
            Err(e) => {
                private_cookies.remove(session_cookie);
                warn!("Invalid session cookie token: {e}");
                return Err(HttpError::Unauthorized);
            }
        };
        let token_hash = Sha256::digest(token_bytes).to_vec();

        let is_token_match: bool = db_session.hashed_token.ct_eq(&token_hash).into();

        if !is_token_match {
            private_cookies.remove(session_cookie);
            warn!("Invalid session token hash");
            return Err(HttpError::Unauthorized);
        }

        parts.extensions.insert(db_session.clone());

        Ok(db_session)
    }
}

pub struct AppSessionCookie {
    id: Uuid,
    token: String,
}
impl AppSessionCookie {
    pub fn new(id: Uuid, token: String) -> Self {
        Self { id, token }
    }
    pub fn from_cookie_str(cookie_str: &str) -> anyhow::Result<Self> {
        let Some((id, token)) = cookie_str.split_once(':') else {
            bail!("Missing ID token separator")
        };
        Ok(Self {
            id: Uuid::from_str(id).context("Invalid ID")?,
            token: token.to_string(),
        })
    }
    pub fn to_cookie_value(&self) -> String {
        format!("{}:{}", self.id, self.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::AppUser;
    use poise::serenity_prelude::UserId;
    use std::net::{IpAddr, Ipv4Addr};

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect to in-memory sqlite");
        sqlx::migrate!().run(&pool).await.expect("run migrations");
        pool
    }

    async fn seed_user(pool: &SqlitePool) -> Uuid {
        let user = AppUser::new(
            pool,
            UserId::new(1),
            "test".into(),
            "test@example.com".into(),
        )
        .await
        .unwrap();
        user.id
    }

    fn local_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    }

    #[tokio::test]
    async fn new_then_get_by_id_round_trip() {
        let pool = fresh_pool().await;
        let user_id = seed_user(&pool).await;
        let (session, _token) = AppSession::new(&pool, user_id, "ua".into(), local_ip())
            .await
            .unwrap();

        let fetched = AppSession::get_by_id(&pool, session.id).await.unwrap();
        let fetched = fetched.expect("session should exist");
        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.user_id, user_id);
        assert_eq!(fetched.user_agent, "ua");
        assert_eq!(fetched.ip, local_ip().to_string());
    }

    #[tokio::test]
    async fn delete_by_id_removes_session() {
        let pool = fresh_pool().await;
        let user_id = seed_user(&pool).await;
        let (session, _) = AppSession::new(&pool, user_id, "ua".into(), local_ip())
            .await
            .unwrap();

        AppSession::delete_by_id(&pool, session.id).await.unwrap();
        assert!(
            AppSession::get_by_id(&pool, session.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_expired_only_reaps_past_due_rows() {
        let pool = fresh_pool().await;
        let user_id = seed_user(&pool).await;
        let (live, _) = AppSession::new(&pool, user_id, "ua".into(), local_ip())
            .await
            .unwrap();

        // Insert an already-expired session directly so we don't have to wait.
        let expired_id = Uuid::new_v4();
        let expired_at = Utc::now() - Duration::days(1);
        let expired_created = Utc::now() - Duration::days(2);
        let hashed: Vec<u8> = vec![0u8; 32];
        let ip = local_ip().to_string();
        sqlx::query!(
            "INSERT INTO sessions (id, hashed_token, user_id, user_agent, ip, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            expired_id,
            hashed,
            user_id,
            "stale",
            ip,
            expired_created,
            expired_at,
        )
        .execute(&pool)
        .await
        .unwrap();

        let reaped = AppSession::delete_expired(&pool).await.unwrap();
        assert_eq!(reaped, 1);
        assert!(
            AppSession::get_by_id(&pool, expired_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            AppSession::get_by_id(&pool, live.id)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn cookie_round_trip_preserves_id_and_token() {
        let id = Uuid::new_v4();
        let token = "abc.def".to_string();
        let raw = AppSessionCookie::new(id, token.clone()).to_cookie_value();
        let parsed = AppSessionCookie::from_cookie_str(&raw).unwrap();
        assert_eq!(parsed.id, id);
        assert_eq!(parsed.token, token);
    }

    #[test]
    fn cookie_parse_rejects_missing_separator() {
        assert!(AppSessionCookie::from_cookie_str("nope").is_err());
    }
}
