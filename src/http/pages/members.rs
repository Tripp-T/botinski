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
use chrono::Utc;
use maud::html;
use poise::serenity_prelude::GuildId;

#[debug_handler]
pub(super) async fn page_guild_members(
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

    let http_cache = state.discord_http()?;
    let Some(snap) = ({
        http_cache.cache.guild(guild_id).map(|g| {
            let role_name_for = |rid: poise::serenity_prelude::RoleId| -> Option<String> {
                g.roles.get(&rid).map(|r| r.name.clone())
            };
            let voice_user_ids: std::collections::HashSet<u64> =
                g.voice_states.keys().map(|u| u.get()).collect();

            let mut members: Vec<MemberRow> = g
                .members
                .values()
                .map(|m| MemberRow {
                    user_id: m.user.id.get(),
                    display_name: m.nick.clone().unwrap_or_else(|| {
                        m.user
                            .global_name
                            .clone()
                            .unwrap_or_else(|| m.user.name.clone())
                    }),
                    username: m.user.name.clone(),
                    is_bot: m.user.bot,
                    joined_at: m
                        .joined_at
                        .map(|t| t.with_timezone(&Utc).format("%Y-%m-%d").to_string()),
                    in_voice: voice_user_ids.contains(&m.user.id.get()),
                    role_names: m.roles.iter().filter_map(|r| role_name_for(*r)).collect(),
                })
                .collect();
            // Owner-first, then voice-active, then alphabetical.
            members.sort_by(|a, b| {
                b.in_voice.cmp(&a.in_voice).then(
                    a.display_name
                        .to_lowercase()
                        .cmp(&b.display_name.to_lowercase()),
                )
            });

            GuildSnapshot {
                name: g.name.clone(),
                member_count: g.member_count,
                cached_count: g.members.len(),
                voice_count: voice_user_ids.len(),
                members,
            }
        })
    }) else {
        return Err(HttpError::NotFound);
    };

    Ok(tmpl
        .set_title(format!("{} — Members", snap.name))
        .render(html! {
            div class="flex flex-col max-w-3xl mx-auto p-4 space-y-4" {
                div class="flex items-baseline justify-between" {
                    div {
                        h1 class="text-2xl font-bold tracking-tight" { (snap.name) }
                        div class="text-xs text-gray-500 mt-1" {
                            "Members"
                        }
                    }
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get())}
                        class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
                        { "← Overview" }
                }

                @if snap.cached_count < snap.member_count as usize {
                    div class="rounded-lg bg-amber-900/20 border border-amber-700/40 p-3 text-xs text-amber-200" {
                        "Showing " strong { (snap.cached_count) } " of "
                        strong { (snap.member_count) }
                        " members. The bot's cache fills over time once the GUILD_MEMBERS chunk events arrive; large guilds may take a few seconds after startup, and the privileged intent must be enabled in the Discord Developer Portal. "
                        (snap.voice_count) " currently in voice."
                    }
                } @else {
                    div class="text-xs text-gray-500" {
                        (snap.member_count) " members, "
                        (snap.voice_count) " in voice."
                    }
                }

                @if snap.members.is_empty() {
                    div class="rounded-lg bg-gray-900/60 border border-gray-800 p-6 text-center text-sm text-gray-400 italic" {
                        "No members in the local cache yet."
                    }
                } @else {
                    ul class="rounded-lg bg-gray-900/60 border border-gray-800 divide-y divide-gray-800" {
                        @for m in &snap.members {
                            li class="p-3 flex items-start gap-3" {
                                div class="flex-1 min-w-0" {
                                    div class="flex items-center gap-2 flex-wrap" {
                                        span class="text-sm text-gray-100" { (m.display_name) }
                                        @if m.username != m.display_name {
                                            span class="text-xs text-gray-500" {
                                                "@" (m.username)
                                            }
                                        }
                                        @if m.is_bot {
                                            span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-indigo-600/30 text-indigo-300" { "BOT" }
                                        }
                                        @if m.in_voice {
                                            span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-emerald-600/30 text-emerald-300" { "IN VOICE" }
                                        }
                                    }
                                    @if let Some(joined) = &m.joined_at {
                                        div class="text-xs text-gray-500 mt-0.5" {
                                            "Joined " (joined)
                                        }
                                    }
                                    @if !m.role_names.is_empty() {
                                        div class="mt-1 flex flex-wrap gap-1" {
                                            @for r in &m.role_names {
                                                span class="text-[10px] px-1.5 py-0.5 rounded bg-gray-800 text-gray-300" { (r) }
                                            }
                                        }
                                    }
                                }
                                div class="text-[10px] text-gray-500 font-mono shrink-0" { (m.user_id) }
                            }
                        }
                    }
                }
            }
        }))
}

struct GuildSnapshot {
    name: String,
    member_count: u64,
    cached_count: usize,
    voice_count: usize,
    members: Vec<MemberRow>,
}

struct MemberRow {
    user_id: u64,
    display_name: String,
    username: String,
    is_bot: bool,
    joined_at: Option<String>,
    in_voice: bool,
    role_names: Vec<String>,
}
