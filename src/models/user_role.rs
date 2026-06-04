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
    ttl: Duration,
}

impl Default for RoleCache {
    fn default() -> Self {
        Self::with_ttl(ROLE_CACHE_TTL)
    }
}

impl RoleCache {
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, user_id: Uuid) -> Option<AppUserRole> {
        let guard = self.inner.read().unwrap();
        let (role, cached_at) = guard.get(&user_id)?;
        (cached_at.elapsed() < self.ttl).then(|| role.clone())
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

    /// Drops entries older than the cache's TTL. Intended for a periodic
    /// background sweep so dormant users don't permanently occupy memory.
    pub fn sweep(&self) -> usize {
        let mut guard = self.inner.write().unwrap();
        let before = guard.len();
        guard.retain(|_, (_, cached_at)| cached_at.elapsed() < self.ttl);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn gid(n: u64) -> GuildId {
        GuildId::new(n)
    }

    #[test]
    fn anonymous_classifier_methods() {
        let role = AppUserRole::Anonymous;
        assert!(!role.is_authenticated());
        assert!(!role.is_member_of(gid(1)));
        assert!(!role.is_admin_of(gid(1)));
        assert!(role.mutual_guilds().is_empty());
    }

    #[test]
    fn foreign_is_authenticated_but_not_a_member() {
        let role = AppUserRole::Foreign;
        assert!(role.is_authenticated());
        assert!(!role.is_member_of(gid(1)));
        assert!(!role.is_admin_of(gid(1)));
        assert!(role.mutual_guilds().is_empty());
    }

    #[test]
    fn member_classifies_by_guild_list() {
        let role = AppUserRole::Member {
            guilds: vec![gid(1), gid(2)],
        };
        assert!(role.is_authenticated());
        assert!(role.is_member_of(gid(1)));
        assert!(role.is_member_of(gid(2)));
        assert!(!role.is_member_of(gid(3)));
        // member ≠ admin
        assert!(!role.is_admin_of(gid(1)));
        assert_eq!(role.mutual_guilds(), &[gid(1), gid(2)]);
    }

    #[test]
    fn guild_admin_member_and_admin_lists_are_independent() {
        let role = AppUserRole::GuildAdmin {
            member: vec![gid(1), gid(2)],
            admin: vec![gid(1)],
        };
        assert!(role.is_member_of(gid(1)));
        assert!(role.is_member_of(gid(2)));
        assert!(role.is_admin_of(gid(1)));
        assert!(!role.is_admin_of(gid(2)));
        assert!(!role.is_admin_of(gid(3)));
        assert_eq!(role.mutual_guilds(), &[gid(1), gid(2)]);
    }

    #[test]
    fn global_admin_passes_every_check() {
        let role = AppUserRole::GlobalAdmin;
        assert!(role.is_authenticated());
        assert!(role.is_member_of(gid(42)));
        assert!(role.is_admin_of(gid(42)));
        // mutual_guilds is empty even for GlobalAdmin — callers should special-case
        // by checking matches!(role, GlobalAdmin) when they need "all bot guilds".
        assert!(role.mutual_guilds().is_empty());
    }

    #[test]
    fn role_cache_returns_none_on_empty() {
        let cache = RoleCache::default();
        assert!(cache.get(Uuid::new_v4()).is_none());
    }

    #[test]
    fn role_cache_round_trip_within_ttl() {
        let cache = RoleCache::with_ttl(Duration::from_secs(60));
        let id = Uuid::new_v4();
        cache.put(id, AppUserRole::Foreign);
        assert!(matches!(cache.get(id), Some(AppUserRole::Foreign)));
    }

    #[test]
    fn role_cache_invalidate_removes_entry() {
        let cache = RoleCache::default();
        let id = Uuid::new_v4();
        cache.put(id, AppUserRole::Foreign);
        cache.invalidate(id);
        assert!(cache.get(id).is_none());
    }

    #[test]
    fn role_cache_get_returns_none_after_ttl() {
        let cache = RoleCache::with_ttl(Duration::from_millis(20));
        let id = Uuid::new_v4();
        cache.put(id, AppUserRole::Foreign);
        std::thread::sleep(Duration::from_millis(40));
        assert!(cache.get(id).is_none());
    }

    #[test]
    fn role_cache_sweep_drops_expired_only() {
        let cache = RoleCache::with_ttl(Duration::from_millis(30));
        let old = Uuid::new_v4();
        cache.put(old, AppUserRole::Foreign);
        std::thread::sleep(Duration::from_millis(40));
        let fresh = Uuid::new_v4();
        cache.put(fresh, AppUserRole::GlobalAdmin);

        let dropped = cache.sweep();
        assert_eq!(dropped, 1);
        assert!(cache.get(old).is_none());
        assert!(cache.get(fresh).is_some());
    }
}
