use crate::{
    AppState,
    http::{HttpError, templates::TemplateBase},
    models::user_role::AppUserRole,
};
use axum::{
    debug_handler,
    extract::{Path, State},
    response::IntoResponse,
};
use maud::{Markup, html};
use poise::serenity_prelude::{ChannelType, GuildId};

#[debug_handler]
pub(super) async fn page_guild_overview(
    State(state): State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);

    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }
    if !role.is_member_of(guild_id) {
        return Err(HttpError::Forbidden);
    }
    let is_admin = role.is_admin_of(guild_id);

    // Snapshot the relevant bits of the bot's serenity cache.
    let http_cache = state.discord_http()?;
    let Some(snapshot) = ({
        http_cache.cache.guild(guild_id).map(|g| GuildSnapshot {
            name: g.name.clone(),
            member_count: g.member_count,
            text_channels: g
                .channels
                .values()
                .filter(|c| matches!(c.kind, ChannelType::Text | ChannelType::News))
                .count(),
            voice_channels: g
                .channels
                .values()
                .filter(|c| matches!(c.kind, ChannelType::Voice | ChannelType::Stage))
                .count(),
            roles: g.roles.len().saturating_sub(1), // skip @everyone
            users_in_voice: g.voice_states.len(),
        })
    }) else {
        return Err(HttpError::NotFound);
    };

    // Music snapshot (cheap; we don't subscribe to SSE on this page).
    let bot_in_voice = state.music.is_connected(guild_id);
    let (now_playing, queue_len) = if let Some(player) = state.music.try_get_player(guild_id) {
        let guard = player.lock().await;
        (
            guard.current.as_ref().map(|np| np.track.title.clone()),
            guard.queue.len(),
        )
    } else {
        (None, 0)
    };

    Ok(tmpl.set_title(snapshot.name.clone()).render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            // Header
            div class="flex items-baseline justify-between" {
                div class="flex items-center gap-3" {
                    h1 class="text-2xl font-bold tracking-tight" { (snapshot.name) }
                    @if is_admin {
                        span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-blue-600/30 text-blue-300 border border-blue-700/50" { "ADMIN" }
                    }
                }
                a hx-boost="true" href="/guilds" class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "← All guilds" }
            }

            // Quick stats grid
            div class="grid grid-cols-2 sm:grid-cols-4 gap-3" {
                (stat_card("Members", &snapshot.member_count.to_string()))
                (stat_card("Text channels", &snapshot.text_channels.to_string()))
                (stat_card("Voice channels", &snapshot.voice_channels.to_string()))
                (stat_card("Roles", &snapshot.roles.to_string()))
            }

            // Music summary
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-4 space-y-2" {
                div class="flex items-center justify-between" {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Music" }
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get()) "/music"}
                        class="text-xs text-blue-400 hover:text-blue-300"
                        { "Open dashboard →" }
                }
                @if !bot_in_voice {
                    div class="text-sm text-gray-400 italic" {
                        "Bot isn't in a voice channel."
                    }
                } @else if let Some(title) = now_playing {
                    div class="text-sm font-medium text-gray-100 truncate" {
                        "♪ " (title)
                    }
                    div class="text-xs text-gray-500" {
                        (queue_len) " in queue"
                    }
                } @else {
                    div class="text-sm text-gray-400 italic" { "Connected, nothing playing." }
                }
            }

            // Voice activity
            @if snapshot.users_in_voice > 0 {
                div class="rounded-lg bg-gray-900/60 border border-gray-800 p-4" {
                    div class="text-xs uppercase tracking-wider text-gray-500 mb-1" { "Voice activity" }
                    div class="text-sm text-gray-200" {
                        (snapshot.users_in_voice)
                        " "
                        @if snapshot.users_in_voice == 1 { "person" } @else { "people" }
                        " in voice"
                    }
                }
            }

            // Admin section
            @if is_admin {
                div class="text-xs uppercase tracking-wider text-gray-500 pt-2" { "Manage" }
                div class="grid gap-3 sm:grid-cols-2" {
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get()) "/music"}
                        class="rounded-lg bg-gray-900/60 border border-gray-800 hover:border-gray-700 hover:bg-gray-900 p-4 transition-colors group" {
                        div class="text-sm font-medium text-gray-100 group-hover:text-white" { "Music" }
                        div class="text-xs text-gray-500 mt-0.5" { "Now playing, queue, controls" }
                    }
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get()) "/settings"}
                        class="rounded-lg bg-gray-900/60 border border-gray-800 hover:border-gray-700 hover:bg-gray-900 p-4 transition-colors group" {
                        div class="text-sm font-medium text-gray-100 group-hover:text-white" { "Settings" }
                        div class="text-xs text-gray-500 mt-0.5" { "Volume cap, idle timeout, admin roles" }
                    }
                }
            }
        }
    }))
}

struct GuildSnapshot {
    name: String,
    member_count: u64,
    text_channels: usize,
    voice_channels: usize,
    roles: usize,
    users_in_voice: usize,
}

fn stat_card(label: &str, value: &str) -> Markup {
    html! {
        div class="rounded-lg bg-gray-900/60 border border-gray-800 p-3" {
            div class="text-xs uppercase tracking-wider text-gray-500" { (label) }
            div class="text-xl font-semibold text-gray-50 mt-1 font-mono" { (value) }
        }
    }
}
