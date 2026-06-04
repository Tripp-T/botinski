//! Server-sent events endpoint: streams `state` (full re-render) and `progress`
//! (1s tick) events to the music page so it stays live without polling.

use crate::{AppState, http::HttpError, models::user_role::AppUserRole};
use axum::{
    debug_handler,
    extract::{Path, State},
    response::{
        Html,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures::Stream;
use poise::serenity_prelude::GuildId;
use std::{convert::Infallible, time::Duration};
use tokio::sync::broadcast::error::RecvError;

use super::{render_state_for, require_member, view::render_progress};

async fn render_state_html(state: &AppState, guild_id: GuildId, is_admin: bool) -> String {
    match render_state_for(state, guild_id, is_admin).await {
        Ok(Html(markup)) => markup.into_string(),
        Err(_) => String::new(),
    }
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

#[debug_handler]
pub(super) async fn music_events_sse(
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
