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
use maud::html;
use poise::serenity_prelude::GuildId;

#[debug_handler]
pub(super) async fn page_guilds(
    State(state): State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
) -> Result<impl IntoResponse, HttpError> {
    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }

    let http_cache = state.discord_http()?;
    let global_admin = matches!(role, AppUserRole::GlobalAdmin);

    let guilds_info: Vec<(GuildId, String, bool)> = if global_admin {
        http_cache
            .cache
            .guilds()
            .into_iter()
            .filter_map(|gid| {
                http_cache
                    .cache
                    .guild(gid)
                    .map(|g| (gid, g.name.clone(), true))
            })
            .collect()
    } else {
        role.mutual_guilds()
            .iter()
            .filter_map(|gid| {
                http_cache
                    .cache
                    .guild(*gid)
                    .map(|g| (*gid, g.name.clone(), role.is_admin_of(*gid)))
            })
            .collect()
    };

    Ok(tmpl.set_title("Guilds").render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                h1 class="text-2xl font-bold tracking-tight" { "Guilds" }
                span class="text-xs text-gray-500" { "Mutual with botinski" }
            }
            @if guilds_info.is_empty() {
                div class="rounded-lg bg-gray-900/60 border border-gray-800 p-6 text-center" {
                    div class="text-sm text-gray-400 italic" {
                        "You're not in any guilds the bot is in."
                    }
                }
            } @else {
                ul class="rounded-lg bg-gray-900/60 border border-gray-800 divide-y divide-gray-800 overflow-hidden" {
                    @for (gid, name, is_admin) in &guilds_info {
                        li class="flex items-center justify-between gap-3 p-4 hover:bg-gray-800/30 transition-colors" {
                            div class="flex items-center gap-3 min-w-0" {
                                span class="text-sm text-gray-100 truncate" { (name) }
                                @if *is_admin {
                                    span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-blue-600/30 text-blue-300 border border-blue-700/50 shrink-0" { "ADMIN" }
                                }
                            }
                            div class="flex items-center gap-1 shrink-0" {
                                a hx-boost="true"
                                    href={"/guilds/" (gid.get()) "/music"}
                                    class="px-2.5 py-1 rounded-md text-xs text-gray-300 hover:text-white hover:bg-gray-700/60 transition-colors"
                                    { "Music" }
                                @if *is_admin {
                                    a hx-boost="true"
                                        href={"/guilds/" (gid.get())}
                                        class="px-2.5 py-1 rounded-md text-xs text-gray-300 hover:text-white hover:bg-gray-700/60 transition-colors"
                                        { "Manage" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}

#[debug_handler]
pub(super) async fn page_guild_admin(
    State(state): State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);

    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }
    if !role.is_admin_of(guild_id) {
        return Err(HttpError::Forbidden);
    }

    let http_cache = state.discord_http()?;
    let guild_info = http_cache
        .cache
        .guild(guild_id)
        .map(|g| (g.name.clone(), g.member_count));
    let Some((name, member_count)) = guild_info else {
        return Err(HttpError::NotFound);
    };

    Ok(tmpl.set_title(format!("Manage {name}")).render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                div {
                    h1 class="text-2xl font-bold tracking-tight" { (name) }
                    div class="text-xs text-gray-500 mt-1" { (member_count) " members" }
                }
                a hx-boost="true" href="/guilds" class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "← All guilds" }
            }
            div class="text-xs uppercase tracking-wider text-gray-500" { "Manage" }
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
                    div class="text-xs text-gray-500 mt-0.5" { "Volume cap, idle timeout" }
                }
            }
        }
    }))
}
