//! HTTP API for the music subsystem.
//!
//! Layout:
//! - `view`: pure markup helpers (icons, buttons, full `render_state`).
//! - `sse`: the SSE event stream for live page updates.
//! - here: router, auth guards, action handlers, and the I/O bridge
//!   (`render_state_for`) shared by the action handlers and SSE.

use crate::{
    AppState,
    http::HttpError,
    models::{user::AppUser, user_role::AppUserRole},
    music,
};
use anyhow::Context as _;
use axum::{
    Form, Router, debug_handler,
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use maud::Markup;
use poise::serenity_prelude::GuildId;
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

mod sse;
mod view;

use view::{MusicView, PlaybackStatus, render_state};

pub fn music_router() -> Router<AppState> {
    Router::new()
        .route("/guilds/{guild_id}/music/state", get(state_partial))
        .route(
            "/guilds/{guild_id}/music/events",
            get(sse::music_events_sse),
        )
        .route("/guilds/{guild_id}/music/play", post(action_play))
        .route("/guilds/{guild_id}/music/pause", post(action_pause))
        .route("/guilds/{guild_id}/music/resume", post(action_resume))
        .route("/guilds/{guild_id}/music/skip", post(action_skip))
        .route("/guilds/{guild_id}/music/clear", post(action_clear))
        .route("/guilds/{guild_id}/music/leave", post(action_leave))
        .route("/guilds/{guild_id}/music/remove", post(action_remove))
        .route("/guilds/{guild_id}/music/move-up", post(action_move_up))
        .route("/guilds/{guild_id}/music/move-down", post(action_move_down))
        .route("/guilds/{guild_id}/music/volume", post(action_volume))
}

fn require_member(role: &AppUserRole, guild_id: GuildId) -> Result<bool, HttpError> {
    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }
    if !role.is_member_of(guild_id) {
        return Err(HttpError::Forbidden);
    }
    Ok(role.is_admin_of(guild_id))
}

fn require_admin(role: &AppUserRole, guild_id: GuildId) -> Result<(), HttpError> {
    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }
    if !role.is_admin_of(guild_id) {
        return Err(HttpError::Forbidden);
    }
    Ok(())
}

async fn render_state_for(
    state: &AppState,
    guild_id: GuildId,
    is_admin: bool,
) -> Result<Html<Markup>, HttpError> {
    let connected = state.music.is_connected(guild_id);
    let player = state.music.try_get_player(guild_id);
    let (current, current_position, queue, volume, status) = match player {
        Some(p) => {
            let guard = p.lock().await;
            let (pos, status) = if let Some(np) = &guard.current {
                match np.handle.get_info().await {
                    Ok(s) => {
                        let st = match s.playing {
                            songbird::tracks::PlayMode::Play => PlaybackStatus::Playing,
                            songbird::tracks::PlayMode::Pause => PlaybackStatus::Paused,
                            _ => PlaybackStatus::None,
                        };
                        (Some(s.position), st)
                    }
                    Err(_) => (None, PlaybackStatus::None),
                }
            } else {
                (None, PlaybackStatus::None)
            };
            (
                guard.current.as_ref().map(|np| np.track.clone()),
                pos,
                guard.queue.iter().cloned().collect::<Vec<_>>(),
                guard.volume,
                status,
            )
        }
        None => {
            let vol = crate::models::guild_settings::GuildSettings::get(&state.db, guild_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.volume.clamp(0.0, music::MAX_VOLUME))
                .unwrap_or(music::DEFAULT_VOLUME);
            (None, None, Vec::new(), vol, PlaybackStatus::None)
        }
    };
    Ok(Html(render_state(MusicView {
        guild_id,
        is_admin,
        connected,
        current: current.as_ref(),
        current_position,
        queue: &queue,
        volume,
        status,
    })))
}

#[debug_handler]
async fn state_partial(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    let is_admin = require_member(&role, guild_id)?;
    render_state_for(&state, guild_id, is_admin).await
}

#[derive(Deserialize)]
struct PlayForm {
    query: String,
}

#[derive(Deserialize)]
struct IdsForm {
    #[serde(default)]
    ids: String,
}

#[derive(Deserialize)]
struct VolumeForm {
    percent: f32,
}

fn parse_ids(form: &IdsForm) -> HashSet<Uuid> {
    form.ids
        .split(',')
        .filter_map(|s| Uuid::parse_str(s.trim()).ok())
        .collect()
}

#[debug_handler]
async fn action_play(
    State(state): State<AppState>,
    role: AppUserRole,
    user: AppUser,
    Path(guild_id): Path<u64>,
    Form(form): Form<PlayForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    let is_admin = require_member(&role, guild_id)?;
    let user_id = user.discord_id()?;
    let requester = (user_id, user.name.clone());
    if music::is_playlist_url(&form.query) {
        music::enqueue_playlist(&state, guild_id, None, form.query, requester)
            .await
            .context("Failed to enqueue playlist")?;
    } else {
        music::enqueue(&state, guild_id, None, form.query, requester)
            .await
            .context("Failed to enqueue track")?;
    }
    render_state_for(&state, guild_id, is_admin).await
}

#[debug_handler]
async fn action_pause(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    let is_admin = require_member(&role, guild_id)?;
    music::pause(&state, guild_id).await?;
    render_state_for(&state, guild_id, is_admin).await
}

#[debug_handler]
async fn action_resume(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    let is_admin = require_member(&role, guild_id)?;
    music::resume(&state, guild_id).await?;
    render_state_for(&state, guild_id, is_admin).await
}

#[debug_handler]
async fn action_skip(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::skip(&state, guild_id).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_clear(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::clear(&state, guild_id).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_leave(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::leave(&state, guild_id).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_remove(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
    Form(form): Form<IdsForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::remove_tracks(&state, guild_id, &parse_ids(&form)).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_move_up(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
    Form(form): Form<IdsForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::move_tracks_up(&state, guild_id, &parse_ids(&form)).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_move_down(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
    Form(form): Form<IdsForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::move_tracks_down(&state, guild_id, &parse_ids(&form)).await?;
    render_state_for(&state, guild_id, true).await
}

#[debug_handler]
async fn action_volume(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
    Form(form): Form<VolumeForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    require_admin(&role, guild_id)?;
    music::set_volume(&state, guild_id, form.percent / 100.0).await?;
    render_state_for(&state, guild_id, true).await
}
