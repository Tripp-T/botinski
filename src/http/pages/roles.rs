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
use poise::serenity_prelude::{GuildId, Permissions, RoleId};

#[debug_handler]
pub(super) async fn page_guild_roles(
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
            // Without GUILD_MEMBERS intent the member cache is sparse — but we
            // can still count the members we DO have cached per role for a
            // rough "who has this role" hint.
            let mut member_count: std::collections::HashMap<RoleId, usize> =
                std::collections::HashMap::new();
            for m in g.members.values() {
                for r in &m.roles {
                    *member_count.entry(*r).or_insert(0) += 1;
                }
            }

            let mut roles: Vec<RoleRow> = g
                .roles
                .values()
                .map(|r| RoleRow {
                    name: r.name.clone(),
                    id: r.id,
                    color_hex: format!("{:06x}", r.colour.0),
                    has_color: r.colour.0 != 0,
                    position: r.position,
                    is_everyone: r.id.get() == guild_id.get(),
                    hoist: r.hoist,
                    mentionable: r.mentionable,
                    managed: r.managed,
                    permissions: format_perm_flags(r.permissions),
                    is_admin: r.permissions.contains(Permissions::ADMINISTRATOR),
                    cached_members: member_count.get(&r.id).copied().unwrap_or(0),
                })
                .collect();
            roles.sort_by(|a, b| b.position.cmp(&a.position).then(a.name.cmp(&b.name)));

            GuildSnapshot {
                name: g.name.clone(),
                cached_member_total: g.members.len(),
                guild_member_count: g.member_count,
                roles,
            }
        })
    }) else {
        return Err(HttpError::NotFound);
    };

    Ok(tmpl
        .set_title(format!("{} — Roles", snap.name))
        .render(html! {
            div class="flex flex-col max-w-3xl mx-auto p-4 space-y-4" {
                div class="flex items-baseline justify-between" {
                    div {
                        h1 class="text-2xl font-bold tracking-tight" { (snap.name) }
                        div class="text-xs text-gray-500 mt-1" {
                            (snap.roles.len()) " roles"
                        }
                    }
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get())}
                        class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
                        { "← Overview" }
                }

                @if snap.cached_member_total < snap.guild_member_count as usize {
                    div class="text-xs text-gray-500 italic" {
                        "Member counts reflect the bot's local cache ("
                        (snap.cached_member_total)
                        " of "
                        (snap.guild_member_count)
                        " populated). The GUILD_MEMBERS chunk events fill it over time after startup."
                    }
                }

                @if snap.roles.is_empty() {
                    div class="rounded-lg bg-gray-900/60 border border-gray-800 p-6 text-center text-sm text-gray-400 italic" {
                        "No roles visible."
                    }
                } @else {
                    ul class="rounded-lg bg-gray-900/60 border border-gray-800 divide-y divide-gray-800" {
                        @for r in &snap.roles {
                            (role_row(r))
                        }
                    }
                }
            }
        }))
}

struct GuildSnapshot {
    name: String,
    cached_member_total: usize,
    guild_member_count: u64,
    roles: Vec<RoleRow>,
}

struct RoleRow {
    name: String,
    id: RoleId,
    color_hex: String,
    has_color: bool,
    position: u16,
    is_everyone: bool,
    hoist: bool,
    mentionable: bool,
    managed: bool,
    permissions: Vec<&'static str>,
    is_admin: bool,
    cached_members: usize,
}

fn role_row(r: &RoleRow) -> Markup {
    html! {
        li class="p-3 space-y-1" {
            div class="flex items-center gap-2 flex-wrap" {
                @if r.has_color {
                    span class="w-3 h-3 rounded-full shrink-0"
                        style=(format!("background:#{}", r.color_hex)) {}
                } @else {
                    span class="w-3 h-3 rounded-full shrink-0 border border-gray-700" {}
                }
                span class="text-sm text-gray-100" { (r.name) }
                @if r.is_everyone {
                    span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-gray-700/60 text-gray-300" { "EVERYONE" }
                }
                @if r.managed {
                    span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-purple-600/30 text-purple-300" { "MANAGED" }
                }
                @if r.hoist {
                    span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-blue-600/30 text-blue-300" { "HOISTED" }
                }
                @if r.mentionable {
                    span class="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded bg-amber-600/30 text-amber-300" { "MENTIONABLE" }
                }
                span class="ml-auto text-[10px] text-gray-500 font-mono" {
                    "pos " (r.position) " · id " (r.id.get())
                }
            }
            div class="text-xs text-gray-500 ml-5" {
                (r.cached_members) " cached "
                @if r.cached_members == 1 { "member" } @else { "members" }
            }
            @if r.is_admin {
                div class="ml-5 mt-1 text-xs text-red-300 italic" {
                    "Administrator (all permissions)"
                }
            } @else if !r.permissions.is_empty() {
                details class="ml-5 mt-1" {
                    summary class="text-xs text-gray-500 cursor-pointer hover:text-gray-300" {
                        (r.permissions.len()) " "
                        @if r.permissions.len() == 1 { "permission" } @else { "permissions" }
                    }
                    div class="mt-1.5 flex flex-wrap gap-1.5" {
                        @for p in &r.permissions {
                            span class="text-[10px] px-1.5 py-0.5 rounded bg-gray-800 text-gray-300" { (p) }
                        }
                    }
                }
            }
        }
    }
}

fn format_perm_flags(perms: Permissions) -> Vec<&'static str> {
    let candidates: &[(Permissions, &'static str)] = &[
        (Permissions::MANAGE_GUILD, "Manage Server"),
        (Permissions::MANAGE_CHANNELS, "Manage Channels"),
        (Permissions::MANAGE_ROLES, "Manage Roles"),
        (Permissions::KICK_MEMBERS, "Kick Members"),
        (Permissions::BAN_MEMBERS, "Ban Members"),
        (Permissions::MANAGE_MESSAGES, "Manage Messages"),
        (Permissions::MANAGE_NICKNAMES, "Manage Nicknames"),
        (Permissions::MANAGE_GUILD_EXPRESSIONS, "Manage Expressions"),
        (Permissions::MANAGE_WEBHOOKS, "Manage Webhooks"),
        (Permissions::MENTION_EVERYONE, "Mention Everyone"),
        (Permissions::VIEW_AUDIT_LOG, "View Audit Log"),
        (Permissions::VIEW_CHANNEL, "View Channel"),
        (Permissions::SEND_MESSAGES, "Send Messages"),
        (Permissions::EMBED_LINKS, "Embed Links"),
        (Permissions::ATTACH_FILES, "Attach Files"),
        (Permissions::READ_MESSAGE_HISTORY, "Read History"),
        (Permissions::ADD_REACTIONS, "Add Reactions"),
        (Permissions::CONNECT, "Connect"),
        (Permissions::SPEAK, "Speak"),
        (Permissions::MUTE_MEMBERS, "Mute Members"),
        (Permissions::DEAFEN_MEMBERS, "Deafen Members"),
        (Permissions::MOVE_MEMBERS, "Move Members"),
        (Permissions::PRIORITY_SPEAKER, "Priority Speaker"),
        (Permissions::STREAM, "Video"),
        (Permissions::USE_VAD, "Use Voice Activity"),
        (Permissions::CHANGE_NICKNAME, "Change Nickname"),
        (Permissions::CREATE_INSTANT_INVITE, "Create Invite"),
    ];
    candidates
        .iter()
        .filter(|(p, _)| perms.contains(*p))
        .map(|(_, label)| *label)
        .collect()
}
