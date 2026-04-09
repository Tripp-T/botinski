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
    pub async fn delete_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<()>, sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM sessions WHERE id = ?
            "#,
            id
        )
        .fetch_optional(pool)
        .await
        .map(|o| o.map(|_| ()))
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
            return Err(HttpError::Internal("Missing cookie key".to_string()));
        };
        let Ok(cookie_jar) = parts.extract::<tower_cookies::Cookies>().await else {
            return Err(HttpError::Internal("Failed to get cookies".to_string()));
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
