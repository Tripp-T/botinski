use crate::{AppState, http::HttpError, http::templates::TemplateBase};
use axum::{Router, debug_handler, extract::State, routing::get};

mod admin;
mod guilds;
mod index;
mod music;
mod overview;
mod profile;
mod settings;

#[debug_handler]
pub async fn page_not_found(_: State<AppState>, _: TemplateBase) -> HttpError {
    HttpError::NotFound
}

pub fn pages_router(_state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(index::page_index))
        .route("/profile", get(profile::page_profile))
        .route("/guilds", get(guilds::page_guilds))
        .route("/guilds/{guild_id}", get(overview::page_guild_overview))
        .route("/guilds/{guild_id}/music", get(music::page_guild_music))
        .route(
            "/guilds/{guild_id}/settings",
            get(settings::page_guild_settings),
        )
        .route("/admin/audit-log", get(admin::page_audit_log))
}
