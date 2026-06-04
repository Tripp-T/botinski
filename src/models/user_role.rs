use axum::{RequestPartsExt, extract::FromRequestParts};
use poise::serenity_prelude::{Cache, GuildId, Member, Permissions, RoleId};
use tracing::warn;

use std::{
    collections::HashMap,
    sync::RwLock,
    time::{Duration, Instant},
};
use uuid::Uuid;

use crate::{
    AppState,
    http::HttpError,
    models::{guild_settings::GuildSettings, user::AppUser},
};

/// How long a computed `AppUserRole` is reused without re-running the Discord
/// API + DB classification. Tradeoff: any role change (admin role added/removed,
/// guild joined/left) takes up to this long to surface to that user's web view.
pub const ROLE_CACHE_TTL: Duration = Duration::from_secs(60);

/// Per-user role memo to keep the `AppUserRole` extractor off Discord's API
/// (N `get_member` calls + N DB lookups per request). Keyed by local `AppUser.id`
/// so logout can invalidate without consulting Discord.
pub struct RoleCache {
    inner: RwLock<HashMap<Uuid, (AppUserRole, Instant)>>,
}

impl Default for RoleCache {
    fn default() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl RoleCache {
    pub fn get(&self, user_id: Uuid) -> Option<AppUserRole> {
        let guard = self.inner.read().unwrap();
        let (role, cached_at) = guard.get(&user_id)?;
        (cached_at.elapsed() < ROLE_CACHE_TTL).then(|| role.clone())
    }

    pub fn put(&self, user_id: Uuid, role: AppUserRole) {
        self.inner
            .write()
            .unwrap()
            .insert(user_id, (role, Instant::now()));
    }

    pub fn invalidate(&self, user_id: Uuid) {
        self.inner.write().unwrap().remove(&user_id);
    }

    /// Drops entries older than [`ROLE_CACHE_TTL`]. Intended for a periodic
    /// background sweep so dormant users don't permanently occupy memory.
    pub fn sweep(&self) -> usize {
        let mut guard = self.inner.write().unwrap();
        let before = guard.len();
        guard.retain(|_, (_, cached_at)| cached_at.elapsed() < ROLE_CACHE_TTL);
        before - guard.len()
    }
}

#[derive(Clone, Default)]
pub enum AppUserRole {
    /// Not authenticated.
    #[default]
    Anonymous,
    /// Authenticated, but not a member of any guild the bot is in.
    Foreign,
    /// Member of one or more guilds the bot is in.
    Member { guilds: Vec<GuildId> },
    /// Member of one or more guilds, and admin in at least one.
    GuildAdmin {
        member: Vec<GuildId>,
        admin: Vec<GuildId>,
    },
    /// Listed in the global `admin_uids` config — implicitly admin everywhere.
    GlobalAdmin,
}

impl AppUserRole {
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, Self::Anonymous)
    }
    pub fn is_admin_of(&self, guild_id: GuildId) -> bool {
        match self {
            Self::GlobalAdmin => true,
            Self::GuildAdmin { admin, .. } => admin.contains(&guild_id),
            _ => false,
        }
    }
    pub fn is_member_of(&self, guild_id: GuildId) -> bool {
        matches!(self, Self::GlobalAdmin) || self.mutual_guilds().contains(&guild_id)
    }
    pub fn mutual_guilds(&self) -> &[GuildId] {
        match self {
            Self::Member { guilds } => guilds,
            Self::GuildAdmin { member, .. } => member,
            _ => &[],
        }
    }
}

impl FromRequestParts<AppState> for AppUserRole {
    type Rejection = HttpError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(role) = parts.extensions.get::<Self>().cloned() {
            return Ok(role);
        }

        let user = match parts.extract_with_state::<AppUser, _>(state).await {
            Ok(u) => u,
            Err(HttpError::Unauthorized) => return Ok(Self::Anonymous),
            Err(e) => return Err(e),
        };

        // Process-wide cache keyed by local user id, with a short TTL. Avoids
        // hammering Discord's `get_member` and the guild_settings DB on every
        // authenticated page load.
        if let Some(role) = state.role_cache.get(user.id) {
            parts.extensions.insert(role.clone());
            return Ok(role);
        }

        let discord_user_id = user.discord_id()?;

        let (admin_uids, admin_roles) = {
            let cfg = state.config.read().await;
            (
                cfg.discord.admin_uids.clone().unwrap_or_default(),
                cfg.discord.admin_roles.clone().unwrap_or_default(),
            )
        };

        if admin_uids.contains(&discord_user_id) {
            let role = Self::GlobalAdmin;
            state.role_cache.put(user.id, role.clone());
            parts.extensions.insert(role.clone());
            return Ok(role);
        }

        let http_cache = state.discord_http()?;
        let bot_guilds: Vec<GuildId> = http_cache.cache.guilds();

        let mut member_of = Vec::new();
        let mut admin_of = Vec::new();
        for guild_id in bot_guilds {
            let member = match http_cache.http.get_member(guild_id, discord_user_id).await {
                Ok(m) => m,
                Err(poise::serenity_prelude::Error::Http(ref e))
                    if e.status_code().map(|s| s.as_u16()) == Some(404) =>
                {
                    continue;
                }
                Err(e) => {
                    warn!("Failed to fetch member for guild {guild_id}: {e}");
                    continue;
                }
            };
            member_of.push(guild_id);

            // Merge global config admin_roles with per-guild persisted admin_role_ids.
            let per_guild_admin = GuildSettings::get(&state.db, guild_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.admin_role_ids)
                .unwrap_or_default();
            let mut combined_admin_roles: Vec<RoleId> = admin_roles.clone();
            combined_admin_roles.extend(per_guild_admin);

            if member_is_admin(&http_cache.cache, guild_id, &member, &combined_admin_roles) {
                admin_of.push(guild_id);
            }
        }

        let role = if !admin_of.is_empty() {
            Self::GuildAdmin {
                member: member_of,
                admin: admin_of,
            }
        } else if !member_of.is_empty() {
            Self::Member { guilds: member_of }
        } else {
            Self::Foreign
        };
        state.role_cache.put(user.id, role.clone());
        parts.extensions.insert(role.clone());
        Ok(role)
    }
}

fn member_is_admin(
    cache: &Cache,
    guild_id: GuildId,
    member: &Member,
    admin_roles: &[RoleId],
) -> bool {
    if member.roles.iter().any(|r| admin_roles.contains(r)) {
        return true;
    }
    let Some(guild) = cache.guild(guild_id) else {
        return false;
    };
    if guild.owner_id == member.user.id {
        return true;
    }
    member.roles.iter().any(|role_id| {
        guild
            .roles
            .get(role_id)
            .is_some_and(|r| r.permissions.contains(Permissions::ADMINISTRATOR))
    })
}
