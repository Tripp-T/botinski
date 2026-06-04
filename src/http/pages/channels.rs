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
use poise::serenity_prelude::{ChannelType, GuildId, PermissionOverwriteType};

#[debug_handler]
pub(super) async fn page_guild_channels(
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

    // Snapshot what we need; the maud render runs after we drop the cache ref.
    let Some(snap) = ({
        http_cache.cache.guild(guild_id).map(|g| {
            let role_name_for = |rid: poise::serenity_prelude::RoleId| -> String {
                g.roles
                    .get(&rid)
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| format!("@{}", rid.get()))
            };
            let mut channels: Vec<ChannelRow> = g
                .channels
                .values()
                .map(|c| ChannelRow {
                    name: c.name.clone(),
                    kind: c.kind,
                    position: c.position,
                    parent_id: c.parent_id,
                    topic: c.topic.clone(),
                    nsfw: c.nsfw,
                    overrides: c
                        .permission_overwrites
                        .iter()
                        .map(|o| {
                            let label = match o.kind {
                                PermissionOverwriteType::Role(rid)
                                    if rid.get() == guild_id.get() =>
                                {
                                    "@everyone".to_string()
                                }
                                PermissionOverwriteType::Role(rid) => {
                                    format!("@{}", role_name_for(rid))
                                }
                                PermissionOverwriteType::Member(uid) => {
                                    format!("user {}", uid.get())
                                }
                                _ => "unknown".to_string(),
                            };
                            OverwriteRow {
                                label,
                                allow: format_perm_flags(o.allow),
                                deny: format_perm_flags(o.deny),
                            }
                        })
                        .collect(),
                })
                .collect();
            // Discord orders channels by (parent_id, position). We separate categories.
            channels.sort_by(|a, b| a.position.cmp(&b.position));
            GuildSnapshot {
                name: g.name.clone(),
                channels,
            }
        })
    }) else {
        return Err(HttpError::NotFound);
    };

    // Group non-category channels by parent category id for rendering.
    let category_names: std::collections::HashMap<u64, String> = http_cache
        .cache
        .guild(guild_id)
        .map(|g| {
            g.channels
                .iter()
                .filter(|(_, c)| matches!(c.kind, ChannelType::Category))
                .map(|(cid, c)| (cid.get(), c.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    let mut by_parent: std::collections::HashMap<Option<u64>, Vec<&ChannelRow>> =
        std::collections::HashMap::new();
    for c in &snap.channels {
        if matches!(c.kind, ChannelType::Category) {
            continue;
        }
        by_parent
            .entry(c.parent_id.map(|c| c.get()))
            .or_default()
            .push(c);
    }
    let mut categories: Vec<(Option<String>, Vec<&ChannelRow>)> = by_parent
        .into_iter()
        .map(|(parent_id, mut chans)| {
            chans.sort_by_key(|c| c.position);
            let name = parent_id.and_then(|pid| category_names.get(&pid).cloned());
            (name, chans)
        })
        .collect();
    categories.sort_by_key(|(name, _)| name.clone().unwrap_or_default());

    Ok(tmpl
        .set_title(format!("{} — Channels", snap.name))
        .render(html! {
            div class="flex flex-col max-w-3xl mx-auto p-4 space-y-4" {
                div class="flex items-baseline justify-between" {
                    div {
                        h1 class="text-2xl font-bold tracking-tight" { (snap.name) }
                        div class="text-xs text-gray-500 mt-1" { "Channels" }
                    }
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get())}
                        class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
                        { "← Overview" }
                }

                @if snap.channels.is_empty() {
                    div class="rounded-lg bg-gray-900/60 border border-gray-800 p-6 text-center text-sm text-gray-400 italic" {
                        "No channels visible — guild not in bot cache yet."
                    }
                } @else {
                    @for (category, chans) in &categories {
                        div class="rounded-lg bg-gray-900/60 border border-gray-800" {
                            div class="px-3 py-2 border-b border-gray-800 text-xs uppercase tracking-wider text-gray-500" {
                                @match category {
                                    Some(name) => (name),
                                    None => "Top level",
                                }
                            }
                            ul class="divide-y divide-gray-800" {
                                @for c in chans {
                                    (channel_row(c))
                                }
                            }
                        }
                    }
                }
            }
        }))
}

struct GuildSnapshot {
    name: String,
    channels: Vec<ChannelRow>,
}

struct ChannelRow {
    name: String,
    kind: ChannelType,
    position: u16,
    parent_id: Option<poise::serenity_prelude::ChannelId>,
    topic: Option<String>,
    nsfw: bool,
    overrides: Vec<OverwriteRow>,
}

struct OverwriteRow {
    label: String,
    allow: Vec<&'static str>,
    deny: Vec<&'static str>,
}

fn channel_row(c: &ChannelRow) -> Markup {
    html! {
        li class="p-3 space-y-1" {
            div class="flex items-center gap-2" {
                span class="text-xs text-gray-500 font-mono w-6 text-right" { "#" (c.position) }
                span class="text-[10px] uppercase tracking-wider text-gray-500 px-1.5 py-0.5 rounded border border-gray-700" {
                    (channel_kind_label(c.kind))
                }
                span class="text-sm text-gray-100 truncate" { (c.name) }
                @if c.nsfw {
                    span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-red-600/30 text-red-300" { "NSFW" }
                }
            }
            @if let Some(topic) = &c.topic {
                @if !topic.is_empty() {
                    div class="text-xs text-gray-400 ml-8" { (topic) }
                }
            }
            @if !c.overrides.is_empty() {
                details class="ml-8 mt-1" {
                    summary class="text-xs text-gray-500 cursor-pointer hover:text-gray-300" {
                        (c.overrides.len()) " permission "
                        @if c.overrides.len() == 1 { "override" } @else { "overrides" }
                    }
                    div class="mt-2 space-y-1.5" {
                        @for o in &c.overrides {
                            div class="text-xs space-y-0.5 border-l-2 border-gray-800 pl-3" {
                                div class="text-gray-200 font-medium" { (o.label) }
                                @if !o.allow.is_empty() {
                                    div class="text-emerald-400" {
                                        "+ " (o.allow.join(", "))
                                    }
                                }
                                @if !o.deny.is_empty() {
                                    div class="text-red-400" {
                                        "− " (o.deny.join(", "))
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

fn channel_kind_label(k: ChannelType) -> &'static str {
    match k {
        ChannelType::Text => "TEXT",
        ChannelType::Voice => "VOICE",
        ChannelType::Stage => "STAGE",
        ChannelType::News => "ANNOUNCE",
        ChannelType::NewsThread | ChannelType::PublicThread | ChannelType::PrivateThread => {
            "THREAD"
        }
        ChannelType::Forum => "FORUM",
        ChannelType::Category => "CATEGORY",
        _ => "OTHER",
    }
}

fn format_perm_flags(perms: poise::serenity_prelude::Permissions) -> Vec<&'static str> {
    use poise::serenity_prelude::Permissions as P;
    let candidates: &[(P, &'static str)] = &[
        (P::ADMINISTRATOR, "Administrator"),
        (P::MANAGE_GUILD, "Manage Server"),
        (P::MANAGE_CHANNELS, "Manage Channels"),
        (P::MANAGE_ROLES, "Manage Roles"),
        (P::KICK_MEMBERS, "Kick Members"),
        (P::BAN_MEMBERS, "Ban Members"),
        (P::MANAGE_MESSAGES, "Manage Messages"),
        (P::MENTION_EVERYONE, "Mention Everyone"),
        (P::VIEW_CHANNEL, "View Channel"),
        (P::SEND_MESSAGES, "Send Messages"),
        (P::EMBED_LINKS, "Embed Links"),
        (P::ATTACH_FILES, "Attach Files"),
        (P::READ_MESSAGE_HISTORY, "Read History"),
        (P::CONNECT, "Connect"),
        (P::SPEAK, "Speak"),
        (P::MUTE_MEMBERS, "Mute Members"),
        (P::DEAFEN_MEMBERS, "Deafen Members"),
        (P::MOVE_MEMBERS, "Move Members"),
        (P::PRIORITY_SPEAKER, "Priority Speaker"),
        (P::STREAM, "Video"),
    ];
    candidates
        .iter()
        .filter(|(p, _)| perms.contains(*p))
        .map(|(_, label)| *label)
        .collect()
}
