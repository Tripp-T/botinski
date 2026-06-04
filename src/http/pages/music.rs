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
pub(super) async fn page_guild_music(
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
