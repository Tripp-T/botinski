//! Pure markup helpers for the music page: icons, buttons, and the
//! `render_state` / `render_progress` functions that produce the HTML the
//! action handlers and SSE stream both swap in.

use crate::music;
use maud::{Markup, html};
use poise::serenity_prelude::GuildId;
use std::time::Duration;

pub(super) fn btn_secondary(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-gray-700/60 hover:bg-gray-600 text-gray-100 text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

pub(super) fn btn_primary(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

pub(super) fn btn_danger(icon: Markup, label: &str, action_post: String) -> Markup {
    html! {
        button class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-red-600/80 hover:bg-red-500 text-white text-sm font-medium transition-colors cursor-pointer"
            hx-post=(action_post)
            hx-target="#music-state"
            hx-swap="innerHTML" { (icon) span { (label) } }
    }
}

pub(super) fn icon_pause() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            rect x="6" y="4" width="4" height="16" rx="1" {}
            rect x="14" y="4" width="4" height="16" rx="1" {}
        }
    }
}

pub(super) fn icon_play() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            path d="M8 5v14l11-7z" {}
        }
    }
}

pub(super) fn icon_skip() -> Markup {
    html! {
        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
            fill="currentColor" class="w-4 h-4 shrink-0" {
            path d="M5 4v16l10-8L5 4z" {}
            rect x="17" y="4" width="2" height="16" rx="0.5" {}
        }
    }
}

pub(super) fn icon_trash() -> Markup {
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

pub(super) fn icon_power() -> Markup {
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

pub(super) fn icon_arrow_up() -> Markup {
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

pub(super) fn icon_arrow_down() -> Markup {
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

pub(super) fn icon_x() -> Markup {
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

pub(super) fn render_progress(position: Option<Duration>, total: Option<Duration>) -> Markup {
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
pub(super) enum PlaybackStatus {
    Playing,
    Paused,
    None,
}

pub(super) struct MusicView<'a> {
    pub guild_id: GuildId,
    pub is_admin: bool,
    pub connected: bool,
    pub current: Option<&'a music::Track>,
    pub current_position: Option<Duration>,
    pub queue: &'a [music::Track],
    pub volume: f32,
    pub status: PlaybackStatus,
}

pub(super) fn render_state(view: MusicView<'_>) -> Markup {
    let MusicView {
        guild_id,
        is_admin,
        connected,
        current,
        current_position,
        queue,
        volume,
        status,
    } = view;
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
                            hx-trigger="change throttle:300ms"
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
