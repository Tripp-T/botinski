use crate::{
    AppState,
    http::{HttpError, templates::TemplateBase},
    models::{guild_settings::GuildSettings, user_role::AppUserRole},
};
use axum::{
    debug_handler,
    extract::{Path, State},
    response::IntoResponse,
};
use maud::html;
use poise::serenity_prelude::{GuildId, RoleId};

#[debug_handler]
pub(super) async fn page_guild_settings(
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
    let empty_secs = settings.empty_channel_leave_secs;

    // Pull all guild roles (sans @everyone) for the admin-role picker.
    let mut all_roles: Vec<(RoleId, String, i64)> = http_cache
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
    let current_admin_ids: std::collections::HashSet<RoleId> =
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

                // Idle leave timeout (queue empty)
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

                // Empty channel leave timeout (no listeners)
                div class="space-y-1" {
                    label class="block text-sm font-medium text-gray-200" for="empty_channel_leave_secs"
                        { "Empty channel leave timeout" }
                    div class="text-xs text-gray-500" {
                        "Seconds the bot will stay in a voice channel with no other humans before disconnecting. 0 disables this check. Range 0–3600."
                    }
                    div class="flex items-center gap-3 mt-1" {
                        input id="empty_channel_leave_secs" type="number"
                            name="empty_channel_leave_secs"
                            min="0" max="3600" step="1"
                            value=(empty_secs)
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
