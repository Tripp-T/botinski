use crate::{
    AppState,
    http::HttpError,
    models::{guild_settings::GuildSettings, user_role::AppUserRole},
    music,
};
use anyhow::Context as _;
use axum::{
    Router, debug_handler,
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::post,
};
use axum_extra::extract::Form;
use poise::serenity_prelude::{GuildId, RoleId};
use serde::Deserialize;

pub fn settings_router() -> Router<AppState> {
    Router::new().route(
        "/guilds/{guild_id}/settings",
        post(action_update_settings),
    )
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub max_volume_percent: f32,
    pub idle_leave_secs: i64,
    #[serde(default)]
    pub admin_role_ids: Vec<String>,
}

#[debug_handler]
async fn action_update_settings(
    State(state): State<AppState>,
    role: AppUserRole,
    Path(guild_id): Path<u64>,
    Form(form): Form<SettingsForm>,
) -> Result<impl IntoResponse, HttpError> {
    let guild_id = GuildId::new(guild_id);
    if !role.is_authenticated() {
        return Err(HttpError::Unauthorized);
    }
    if !role.is_admin_of(guild_id) {
        return Err(HttpError::Forbidden);
    }

    let max_volume = (form.max_volume_percent / 100.0).clamp(0.0, music::MAX_VOLUME);
    let idle_leave_secs = form.idle_leave_secs.clamp(0, 3600);
    let admin_role_ids: Vec<RoleId> = form
        .admin_role_ids
        .iter()
        .filter_map(|s| s.parse::<u64>().ok())
        .map(RoleId::new)
        .collect();

    GuildSettings::upsert_max_volume(&state.db, guild_id, max_volume)
        .await
        .context("Failed to persist max volume")?;
    GuildSettings::upsert_idle_leave_secs(&state.db, guild_id, idle_leave_secs)
        .await
        .context("Failed to persist idle leave timeout")?;
    GuildSettings::upsert_admin_role_ids(&state.db, guild_id, &admin_role_ids)
        .await
        .context("Failed to persist admin role ids")?;

    let new_settings = GuildSettings::get(&state.db, guild_id)
        .await
        .context("Failed to reload settings")?
        .unwrap_or_default();
    music::apply_settings(&state, guild_id, &new_settings).await?;

    Ok(Redirect::to(&format!("/guilds/{}/settings", guild_id.get())))
}
