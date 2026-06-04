use crate::{
    AppState,
    http::{HttpError, templates::TemplateBase},
    models::{guild_settings::GuildSettings, user::AppUser, user_role::AppUserRole},
};
use axum::{
    Router, debug_handler,
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
};
use maud::html;
use poise::serenity_prelude::GuildId;

#[debug_handler]
pub async fn page_not_found(_: State<AppState>, _: TemplateBase) -> HttpError {
    HttpError::NotFound
}

pub fn pages_router(_state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(page_index))
        .route("/profile", get(page_profile))
        .route("/guilds", get(page_guilds))
        .route("/guilds/{guild_id}", get(page_guild_admin))
        .route("/guilds/{guild_id}/music", get(page_guild_music))
        .route("/guilds/{guild_id}/settings", get(page_guild_settings))
}

#[debug_handler]
async fn page_index(
    _state: State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
) -> impl IntoResponse {
    let signed_in = role.is_authenticated();
    tmpl.set_title("Home").render(html! {
        div class="max-w-3xl mx-auto p-4 space-y-6" {
            div class="space-y-2 pt-4" {
                h1 class="text-3xl font-bold tracking-tight text-gray-50" { "botinski" }
                p class="text-sm text-gray-400" {
                    "A modular, multi-platform bot framework written in Rust."
                }
            }
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-5 space-y-3" {
                @if signed_in {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Welcome back" }
                    p class="text-sm text-gray-300" {
                        "Pick a guild to manage music and per-server settings."
                    }
                    div class="flex gap-3 pt-1" {
                        a hx-boost="true" href="/guilds"
                            class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
                            { "Your guilds" }
                    }
                } @else {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Get started" }
                    p class="text-sm text-gray-300" {
                        "Sign in with Discord to manage music and per-guild settings for any server you administer."
                    }
                    div class="flex gap-3 pt-1" {
                        a href="/api/oauth/login"
                            class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
                            { "Sign in with Discord" }
                    }
                }
            }
        }
    })
}

#[debug_handler]
async fn page_profile(
    _state: State<AppState>,
    tmpl: TemplateBase,
    user: AppUser,
) -> impl IntoResponse {
    tmpl.set_title("Profile").render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                h1 class="text-2xl font-bold tracking-tight" { "Profile" }
                a hx-boost="true" href="/guilds" class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "View guilds →" }
            }
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-5 space-y-4" {
                div class="space-y-1" {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Account" }
                    div class="text-lg font-semibold text-gray-50" { (user.name) }
                    div class="text-xs text-gray-500 font-mono" { (user.email) }
                }
                div class="border-t border-gray-800 pt-4" {
                    button
                        hx-get="/api/oauth/logout"
                        hx-target="body"
                        class="px-4 py-1.5 rounded-md bg-red-600/80 hover:bg-red-500 text-white text-sm font-medium transition-colors cursor-pointer"
                        { "Sign out" }
                }
            }
        }
    })
}

#[debug_handler]
async fn page_guilds(
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
async fn page_guild_admin(
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

#[debug_handler]
async fn page_guild_music(
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
    let name = http_cache
        .cache
        .guild(guild_id)
        .map(|g| g.name.clone())
        .ok_or(HttpError::NotFound)?;

    Ok(tmpl.set_title(format!("{name} — Music")).render(html! {
        div class="flex flex-col max-w-3xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                h1 class="text-2xl font-bold tracking-tight" { (name) }
                a hx-boost="true" href="/guilds" class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "← All guilds" }
            }

            form
                method="post"
                action={"/api/guilds/" (guild_id.get()) "/music/play"}
                hx-post={"/api/guilds/" (guild_id.get()) "/music/play"}
                hx-target="#music-state"
                hx-swap="innerHTML"
                hx-on--after-request="this.reset()"
                class="flex gap-2 rounded-lg bg-gray-900/40 border border-gray-800 p-2" {
                input type="text"
                    name="query"
                    required
                    placeholder="URL, search query, or YouTube/YT Music playlist URL"
                    class="flex-1 px-3 py-1.5 bg-gray-950/60 border border-gray-700 rounded-md text-sm text-gray-100 placeholder:text-gray-500 focus:outline-none focus:border-blue-500 transition-colors";
                button type="submit"
                    class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors cursor-pointer"
                    { "Add" }
            }

            div
                hx-ext="sse"
                sse-connect={"/api/guilds/" (guild_id.get()) "/music/events"} {
                div
                    id="music-state"
                    sse-swap="state"
                { div class="text-sm text-gray-500 italic p-4" { "Connecting…" } }
            }
        }
        script src="/music.js" defer {}
    }))
}

#[debug_handler]
async fn page_guild_settings(
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
    let name = http_cache
        .cache
        .guild(guild_id)
        .map(|g| g.name.clone())
        .ok_or(HttpError::NotFound)?;

    let settings = GuildSettings::get(&state.db, guild_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let max_volume_pct = (settings.max_volume * 100.0).round() as i32;
    let idle_secs = settings.idle_leave_secs;

    // Pull all guild roles (sans @everyone) for the admin-role picker.
    let mut all_roles: Vec<(poise::serenity_prelude::RoleId, String, i64)> = http_cache
        .cache
        .guild(guild_id)
        .map(|g| {
            g.roles
                .iter()
                .filter(|(id, _)| id.get() != guild_id.get()) // skip @everyone
                .map(|(id, r)| (*id, r.name.clone(), r.position as i64))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // Sort by Discord role position descending (higher = closer to top)
    all_roles.sort_by(|a, b| b.2.cmp(&a.2));
    let current_admin_ids: std::collections::HashSet<poise::serenity_prelude::RoleId> =
        settings.admin_role_ids.iter().copied().collect();

    Ok(tmpl.set_title(format!("{name} — Settings")).render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                h1 class="text-2xl font-bold tracking-tight" { (name) }
                a hx-boost="true" href={"/guilds/" (guild_id.get())} class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "← Back to guild" }
            }
            div class="text-xs uppercase tracking-wider text-gray-500" { "Settings" }

            form
                method="post"
                action={"/api/guilds/" (guild_id.get()) "/settings"}
                class="rounded-lg bg-gray-900/60 border border-gray-800 p-5 space-y-5" {
                // Max volume
                div class="space-y-1" {
                    label class="block text-sm font-medium text-gray-200" for="max_volume_percent"
                        { "Max volume" }
                    div class="text-xs text-gray-500" {
                        "Upper bound on the volume slider and /volume command. 100 = unity gain. Range 0–200."
                    }
                    div class="flex items-center gap-3 mt-1" {
                        input id="max_volume_percent" type="number"
                            name="max_volume_percent"
                            min="0" max="200" step="1"
                            value=(max_volume_pct)
                            class="w-24 px-2 py-1 bg-gray-950/60 border border-gray-700 rounded-md text-sm text-gray-100 focus:outline-none focus:border-blue-500";
                        span class="text-sm text-gray-400" { "%" }
                    }
                }

                // Idle leave timeout
                div class="space-y-1" {
                    label class="block text-sm font-medium text-gray-200" for="idle_leave_secs"
                        { "Idle leave timeout" }
                    div class="text-xs text-gray-500" {
                        "Seconds after the queue empties before the bot disconnects. 0 disables auto-leave. Range 0–3600."
                    }
                    div class="flex items-center gap-3 mt-1" {
                        input id="idle_leave_secs" type="number"
                            name="idle_leave_secs"
                            min="0" max="3600" step="1"
                            value=(idle_secs)
                            class="w-24 px-2 py-1 bg-gray-950/60 border border-gray-700 rounded-md text-sm text-gray-100 focus:outline-none focus:border-blue-500";
                        span class="text-sm text-gray-400" { "seconds" }
                    }
                }

                // Admin roles
                div class="space-y-2 border-t border-gray-800 pt-4" {
                    label class="block text-sm font-medium text-gray-200" { "Admin roles" }
                    div class="text-xs text-gray-500" {
                        "Members with any checked role are treated as admins of this guild, in addition to those with Discord's ADMINISTRATOR permission."
                    }
                    @if all_roles.is_empty() {
                        div class="text-xs text-gray-500 italic mt-2" {
                            "No roles available — guild not in bot cache yet, or only @everyone exists."
                        }
                    } @else {
                        div class="mt-2 max-h-64 overflow-y-auto rounded-md border border-gray-800 bg-gray-950/40 divide-y divide-gray-800" {
                            @for (rid, name, _pos) in &all_roles {
                                @let checked = current_admin_ids.contains(rid);
                                label class="flex items-center gap-3 p-2 hover:bg-gray-800/40 cursor-pointer has-[:checked]:bg-blue-900/15" {
                                    input type="checkbox"
                                        name="admin_role_ids"
                                        value=(rid.get().to_string())
                                        checked?[checked]
                                        class="w-4 h-4 rounded border-gray-600 bg-gray-800 text-blue-500 focus:ring-blue-500 cursor-pointer";
                                    span class="text-sm text-gray-200 truncate" { (name) }
                                    span class="ml-auto text-[10px] text-gray-500 font-mono shrink-0" { (rid.get()) }
                                }
                            }
                        }
                    }
                }

                // Current volume info (read-only here; managed on music page)
                div class="text-xs text-gray-500 border-t border-gray-800 pt-4" {
                    "Current playback volume is "
                    span class="text-gray-300 font-mono" { (((settings.volume * 100.0).round() as i32)) "%" }
                    " — change it on the "
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get()) "/music"}
                        class="text-blue-400 underline" { "music page" }
                    " or via /volume in Discord."
                }

                div class="flex justify-end pt-2" {
                    button type="submit"
                        class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors cursor-pointer"
                        { "Save" }
                }
            }
        }
    }))
}
