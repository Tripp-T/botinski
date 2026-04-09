use anyhow::Context;
use axum::{RequestPartsExt, extract::FromRequestParts};
use sqlx::{SqlitePool, prelude::FromRow};
use uuid::Uuid;

use crate::{AppState, http::HttpError, models::session::AppSession};

#[derive(FromRow, Debug, Clone)]
pub struct AppUser {
    pub id: Uuid,
    discord_id: String,
    pub name: String,
    pub email: String,
}
impl AppUser {
    pub async fn new(
        pool: &SqlitePool,
        discord_id: poise::serenity_prelude::all::UserId,
        name: String,
        email: String,
    ) -> Result<Self, sqlx::Error> {
        let user = AppUser {
            id: Uuid::new_v4(),
            discord_id: discord_id.to_string(),
            name,
            email,
        };

        sqlx::query!(
            "INSERT INTO users (id, discord_id, name, email) VALUES (?, ?, ?, ?)",
            user.id,
            user.discord_id,
            user.name,
            user.email
        )
        .execute(pool)
        .await
        .map(|_| user)
    }
    pub async fn get_by_id(
        pool: &SqlitePool,
        user_id: Uuid,
    ) -> Result<Option<AppUser>, sqlx::Error> {
        sqlx::query_as!(
            AppUser,
            r#"
            SELECT id AS "id: Uuid", discord_id, name, email
            FROM users
            WHERE id = ?
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
    }
    pub async fn get_by_discord_id(
        pool: &SqlitePool,
        discord_id: poise::serenity_prelude::all::UserId,
    ) -> Result<Option<AppUser>, sqlx::Error> {
        let discord_id = discord_id.get().to_string();
        sqlx::query_as!(
            AppUser,
            r#"
            SELECT id AS "id: Uuid", discord_id, name, email
            FROM users
            WHERE discord_id = ?
            "#,
            discord_id
        )
        .fetch_optional(pool)
        .await
    }
    pub fn discord_id(&self) -> anyhow::Result<poise::serenity_prelude::all::UserId> {
        Ok(self
            .discord_id
            .parse::<u64>()
            .context("Failed to parse user discord_id to u64??")?
            .into())
    }
}
impl FromRequestParts<AppState> for AppUser {
    type Rejection = HttpError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(user) = parts.extensions.get::<AppUser>() {
            return Ok(user.clone());
        }
        let session = parts.extract_with_state::<AppSession, _>(state).await?;
        let user = AppUser::get_by_id(&state.db, session.user_id)
            .await
            .context("Failed to lookup session user")?
            .ok_or(HttpError::Unauthorized)?;
        parts.extensions.insert(user.clone());
        Ok(user)
    }
}
