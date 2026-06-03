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
    response::{
        Html, IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures::Stream;
use maud::{Markup, html};
use poise::serenity_prelude::GuildId;
use serde::Deserialize;
use std::{collections::HashSet, convert::Infallible, time::Duration};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

pub fn music_router() -> Router<AppState> {
    Router::new()
        .route("/guilds/{guild_id}/music/state", get(state_partial))
        .route("/guilds/{guild_id}/music/events", get(music_events_sse))
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

fn btn_secondary(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-gray-700/60 hover:bg-gray-600 text-gray-100 text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

fn btn_primary(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

fn btn_danger(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-red-600/80 hover:bg-red-500 text-white text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

fn icon_pause() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            rect x="6" y="4" width="4" height="16" rx="1" {}
            rect x="14" y="4" width="4" height="16" rx="1" {}
        }
    }
}

fn icon_play() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            path d="M8 5v14l11-7z" {}
        }
    }
}

fn icon_skip() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            path d="M5 4v16l10-8L5 4z" {}
            rect x="17" y="4" width="2" height="16" rx="0.5" {}
        }
    }
}

fn icon_trash() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round"
            class="w-4 h-4 shrink-0" {
            path d="M3 6h18" {}
            path d="M19 6v14c0 1.1-.9 2-2 2H7c-1.1 0-2-.9-2-2V6" {}
            path d="M8 6V4c0-1.1.9-2 2-2h4c1.1 0 2 .9 2 2v2" {}
        }
    }
}

fn icon_power() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round"
            class="w-4 h-4 shrink-0" {
            path d="M12 2v10" {}
            path d="M18.4 6.6a9 9 0 1 1-12.77.04" {}
        }
    }
}

fn icon_arrow_up() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round"
            class="w-3.5 h-3.5 shrink-0" {
            path d="M12 19V5" {}
            path d="m5 12 7-7 7 7" {}
        }
    }
}

fn icon_arrow_down() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round"
            class="w-3.5 h-3.5 shrink-0" {
            path d="M12 5v14" {}
            path d="m19 12-7 7-7-7" {}
        }
    }
}

fn icon_x() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" stroke-width="2"
            stroke-linecap="round" stroke-linejoin="round"
            class="w-3.5 h-3.5 shrink-0" {
            path d="M18 6 6 18" {}
            path d="m6 6 12 12" {}
        }
    }
}

pub fn render_progress(position: Option<Duration>, total: Option<Duration>) -> Markup {
    match (position, total) {
        (Some(pos), Some(total)) => {
            let pct = if total.as_secs_f64() > 0.0 {
                (pos.as_secs_f64() / total.as_secs_f64() * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            html! {
                div class="w-full h-1.5 bg-gray-700/60 rounded-full overflow-hidden" {
                    div class="h-full bg-blue-500 transition-all duration-1000 ease-linear"
                        style=(format!("width: {pct:.2}%")) {}
                }
                div class="flex justify-between text-xs text-gray-400 mt-1.5 font-mono" {
                    span { (music::format_secs(pos)) }
                    span { (music::format_secs(total)) }
                }
            }
        }
        (Some(pos), None) => html! {
            div class="text-xs text-gray-400 font-mono" { (music::format_secs(pos)) " / ?" }
        },
        _ => html! {},
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    None,
}

pub fn render_state(
    guild_id: GuildId,
    is_admin: bool,
    connected: bool,
    current: Option<&music::Track>,
    current_position: Option<Duration>,
    queue: &[music::Track],
    volume: f32,
    status: PlaybackStatus,
) -> Markup {
    let g = guild_id.get();
    let vol_pct = (volume * 100.0).round() as i32;
    html! {
        div class="space-y-4" {
            // Controls bar
            div class="flex items-center gap-2 p-2 rounded-lg bg-gray-900/40 border border-gray-800 overflow-x-auto" {
                @match status {
                    PlaybackStatus::Playing => (btn_secondary(icon_pause(), "Pause", format!("/api/guilds/{g}/music/pause"))),
                    PlaybackStatus::Paused => (btn_secondary(icon_play(), "Resume", format!("/api/guilds/{g}/music/resume"))),
                    PlaybackStatus::None => {}
                }
                @if is_admin {
                    (btn_primary(icon_skip(), "Skip", format!("/api/guilds/{g}/music/skip")))
                    (btn_danger(icon_trash(), "Clear queue", format!("/api/guilds/{g}/music/clear")))
                    (btn_danger(icon_power(), "Disconnect", format!("/api/guilds/{g}/music/leave")))
                }
                @if is_admin {
                    div class="flex items-center gap-2 ml-auto shrink-0" {
                        span class="text-xs text-gray-400" { "Vol" }
                        input type="range" name="percent" min="0" max="200" step="1"
                            value=(vol_pct)
                            class="accent-blue-500 w-32 h-1.5 cursor-pointer align-middle"
                            hx-post=(format!("/api/guilds/{g}/music/volume"))
                            hx-trigger="change"
                            hx-target="#music-state"
                            hx-swap="innerHTML"
                            hx-include="this"
                            oninput="this.nextElementSibling.textContent = this.value + '%'";
                        span class="text-xs text-gray-300 font-mono w-12 text-right" { (vol_pct) "%" }
                    }
                } @else {
                    div class="ml-auto text-xs text-gray-500 font-mono shrink-0" { "Vol " (vol_pct) "%" }
                }
            }

            // Now playing card
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-4" {
                @if !connected {
                    div class="text-gray-400 text-sm italic" {
                        "Bot isn't in a voice channel. Use /play in Discord to start, or add a track below once connected."
                    }
                } @else if let Some(t) = current {
                    div class="flex items-center gap-2 mb-2" {
                        div class="text-xs uppercase tracking-wider text-gray-500" { "Now playing" }
                        @if t.is_live {
                            span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-red-600 text-white" { "LIVE" }
                        }
                    }
                    div class="text-lg font-semibold text-gray-50 mb-3 break-words" { (t.title) }
                    div id="now-playing-progress" sse-swap="progress" {
                        (render_progress(current_position, t.duration))
                    }
                    div class="mt-2 text-xs text-gray-500" {
                        "Requested by " span class="text-gray-400" { (t.requested_by_name) }
                    }
                } @else {
                    div class="text-gray-400 text-sm italic" { "Nothing playing." }
                }
            }

            // Queue panel
            div class="rounded-lg bg-gray-900/60 border border-gray-800" {
                div class="flex items-center justify-between p-3 border-b border-gray-800" {
                    div class="text-sm font-medium text-gray-200" {
                        "Queue " span class="text-gray-500" { "(" (queue.len()) ")" }
                    }
                    @if !queue.is_empty() {
                        div class="flex flex-wrap gap-1.5 items-center" {
                            button type="button" data-bulk-select-all
                                class="px-2 py-1 rounded-md text-xs text-gray-300 hover:bg-gray-800 cursor-pointer transition-colors"
                                { "Select all" }
                            button type="button" data-bulk-deselect-all
                                class="px-2 py-1 rounded-md text-xs text-gray-300 hover:bg-gray-800 cursor-pointer transition-colors"
                                { "Deselect" }
                            @if is_admin {
                                button type="button"
                                    class="bulk-action inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs text-gray-200 bg-gray-700/60 hover:bg-gray-600 cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                    disabled
                                    hx-post=(format!("/api/guilds/{g}/music/move-up"))
                                    hx-target="#music-state"
                                    hx-swap="innerHTML"
                                    { (icon_arrow_up()) span { "Move up" } }
                                button type="button"
                                    class="bulk-action inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs text-gray-200 bg-gray-700/60 hover:bg-gray-600 cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                    disabled
                                    hx-post=(format!("/api/guilds/{g}/music/move-down"))
                                    hx-target="#music-state"
                                    hx-swap="innerHTML"
                                    { (icon_arrow_down()) span { "Move down" } }
                                button type="button"
                                    class="bulk-action inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs text-white bg-red-600/70 hover:bg-red-500 cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                    disabled
                                    hx-post=(format!("/api/guilds/{g}/music/remove"))
                                    hx-target="#music-state"
                                    hx-swap="innerHTML"
                                    { (icon_x()) span { "Remove (" span class="selected-count" { "0" } ")" } }
                            }
                        }
                    }
                }
                @if queue.is_empty() {
                    div class="p-6 text-center text-gray-500 text-sm italic" { "Queue is empty. Add something below." }
                } @else {
                    ul class="divide-y divide-gray-800 max-h-[60vh] overflow-y-auto" {
                        @for (i, t) in queue.iter().enumerate() {
                            li class="flex items-center gap-3 p-3 hover:bg-gray-800/40 transition-colors group has-[:checked]:bg-blue-900/20" {
                                @if is_admin {
                                    input type="checkbox" class="track-checkbox w-4 h-4 rounded border-gray-600 bg-gray-800 text-blue-500 focus:ring-blue-500 cursor-pointer"
                                        data-track-id=(t.id.to_string());
                                }
                                div class="text-xs text-gray-500 font-mono w-6 text-right" { (i + 1) }
                                div class="flex-1 min-w-0" {
                                    div class="text-sm text-gray-100 truncate flex items-center gap-2" {
                                        span class="truncate" { (t.title) }
                                        @if t.is_live {
                                            span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-red-600 text-white shrink-0" { "LIVE" }
                                        }
                                    }
                                    div class="text-xs text-gray-500 mt-0.5" {
                                        @if t.is_live {
                                            "LIVE"
                                        } @else {
                                            (music::format_duration(t.duration))
                                        }
                                        " · " (t.requested_by_name)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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
    Ok(Html(render_state(
        guild_id,
        is_admin,
        connected,
        current.as_ref(),
        current_position,
        &queue,
        volume,
        status,
    )))
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

async fn render_progress_html(state: &AppState, guild_id: GuildId) -> String {
    let Some(player) = state.music.try_get_player(guild_id) else {
        return String::new();
    };
    let guard = player.lock().await;
    let (position, duration) = if let Some(np) = &guard.current {
        let pos = np.handle.get_info().await.ok().map(|s| s.position);
        (pos, np.track.duration)
    } else {
        (None, None)
    };
    render_progress(position, duration).into_string()
}

async fn render_state_html(state: &AppState, guild_id: GuildId, is_admin: bool) -> String {
    match render_state_for(state, guild_id, is_admin).await {
        Ok(Html(markup)) => markup.into_string(),
        Err(_) => String::new(),
    }
}

async fn music_events_sse(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, HttpError> {
    let guild_id = GuildId::new(guild_id);
    let is_admin = require_member(&role, guild_id)?;
    let mut events = state.music.subscribe(guild_id);

    let stream = async_stream::stream! {
        // Initial full state on connect
        let html = render_state_html(&state, guild_id, is_admin).await;
        yield Ok(Event::default().event("state").data(html));

        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.tick().await; // consume the immediate tick

        loop {
            tokio::select! {
                _ = state.shutdown_token.cancelled() => return,
                ev = events.recv() => {
                    match ev {
                        Ok(()) | Err(RecvError::Lagged(_)) => {
                            let html = render_state_html(&state, guild_id, is_admin).await;
                            yield Ok(Event::default().event("state").data(html));
                        }
                        Err(RecvError::Closed) => return,
                    }
                }
                _ = ticker.tick() => {
                    let html = render_progress_html(&state, guild_id).await;
                    yield Ok(Event::default().event("progress").data(html));
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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

#[derive(Deserialize)]
struct VolumeForm {
    percent: f32,
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
