use std::net::SocketAddr;

use crate::{
    AppState,
    http::{HttpError, components::component_card, templates::TemplateBase},
    models::{
        session::{AppSession, AppSessionCookie},
        user::AppUser,
    },
};
use anyhow::Context;
use axum::{
    Extension, Router, debug_handler,
    extract::{ConnectInfo, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use oauth2::TokenResponse;
use poise::serenity_prelude;
use serde::Deserialize;
use tower_cookies::{Cookie, Cookies};
use tracing::warn;

pub fn api_router(_state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/oauth/login", get(oauth_login))
        .route("/oauth/callback", get(oauth_callback))
}

async fn healthcheck(
    State(state): State<AppState>,
    tmpl: TemplateBase,
) -> Result<impl IntoResponse, HttpError> {
    sqlx::query("SELECT 1")
        .execute(&*state.db)
        .await
        .context("Failed to query database")?;
    Ok(tmpl.set_title("OK").render(component_card(
        "OK",
        "Healthcheck completed successfully",
        false,
    )))
}

#[debug_handler]
async fn oauth_login(State(state): State<AppState>, cookies: Cookies) -> Redirect {
    let (url, csrf) = state.oauth.get_login_url();
    cookies.add({
        use tower_cookies::cookie::time;
        let mut cookie = Cookie::new("csrf", csrf.into_secret());
        cookie.set_expires(tower_cookies::cookie::Expiration::DateTime(
            time::OffsetDateTime::now_utc() + time::Duration::minutes(5),
        ));
        cookie
    });
    Redirect::to(url.as_str())
}

// Request payload returned by Discord during the callback
#[derive(Debug, Deserialize)]
struct OAuthRequest {
    code: String,
    state: String,
}

#[debug_handler]
async fn oauth_callback(
    State(state): State<AppState>,
    cookies: Cookies,
    Extension(cookie_key): Extension<tower_cookies::Key>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<OAuthRequest>,
) -> Result<Response, HttpError> {
    let Some(csrf) = cookies.get("csrf") else {
        return Err(HttpError::BadRequest("Missing CSRF cookie".into()));
    };

    if csrf.value() != query.state {
        return Err(HttpError::BadRequest("Expired CSRF token".into()));
    }

    let token = state.oauth.exchange_code(&query.code).await?;
    let discord_client =
        serenity_prelude::Http::new(&format!("Bearer {}", token.access_token().secret()));
    let current_user = discord_client
        .get_current_user()
        .await
        .context("Failed to fetch current discord user")?;

    let db_user = match AppUser::get_by_discord_id(&state.db, current_user.id)
        .await
        .context("Failed to query DB for existing user")?
    {
        Some(user) => user,
        None => AppUser::new(
            &state.db,
            current_user.id,
            current_user.name.clone(),
            current_user
                .email
                .clone()
                .context("Missing discord user email")?,
        )
        .await
        .context("Failed to create new user")?,
    };

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| {
            v.to_str()
                .inspect_err(|e| warn!("Failed to parse user-agent: {e}"))
                .ok()
        })
        .unwrap_or("unknown")
        .to_string();

    let (session, session_token) = AppSession::new(&state.db, db_user.id, user_agent, addr.ip())
        .await
        .context("Failed to create user session")?;

    let private_cookies = cookies.private(&cookie_key);
    private_cookies.add({
        use tower_cookies::cookie::time;
        let mut cookie = Cookie::new(
            "user-session",
            AppSessionCookie::new(session.id, session_token).to_cookie_value(),
        );
        cookie.set_expires(time::OffsetDateTime::now_utc() + time::Duration::days(15));
        cookie
    });
    Ok(Redirect::to("/profile").into_response())
}
