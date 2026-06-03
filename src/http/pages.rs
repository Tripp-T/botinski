use crate::{
    AppState,
    http::{
        HttpError,
        components::{ButtonColor, ButtonProps, component_button, component_card},
        templates::TemplateBase,
    },
    models::{user::AppUser, user_role::AppUserRole},
};
use axum::{
    Router,
    debug_handler,
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
}

#[debug_handler]
async fn page_index(_state: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    tmpl.set_title("Home")
        .render(component_card("Hello World", "description", false))
}

#[debug_handler]
async fn page_profile(
    _state: State<AppState>,
    tmpl: TemplateBase,
    user: AppUser,
) -> impl IntoResponse {
    tmpl.set_title("Profile").render(component_card(
        format!("{}'s Profile", user.name),
        html! {
            (component_button(ButtonProps {
                color: ButtonColor::Red,
                hx_get: Some("/api/oauth/logout"),
                hx_target: Some("body"),
                class: Some("w-full"),
                ..Default::default()
            }, "Logout"))
        },
        false,
    ))
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
        div class="flex flex-col max-w-xl mx-auto space-y-2" {
            h1 class="text-xl font-bold" { "Your mutual guilds" }
            @if guilds_info.is_empty() {
                p class="text-gray-400" { "You're not in any guilds the bot is in." }
            } @else {
                ul class="space-y-1" {
                    @for (gid, name, is_admin) in &guilds_info {
                        li class="border border-gray-500 rounded-md p-2 flex items-center justify-between" {
                            span { (name) }
                            div class="flex gap-3" {
                                a hx-boost="true"
                                    href={"/guilds/" (gid.get()) "/music"}
                                    class="text-blue-400 underline"
                                    { "Music" }
                                @if *is_admin {
                                    a hx-boost="true"
                                        href={"/guilds/" (gid.get())}
                                        class="text-blue-400 underline"
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
        div class="flex flex-col max-w-xl mx-auto space-y-2" {
            h1 class="text-xl font-bold" { "Manage: " (name) }
            p { "Member count: " (member_count) }
            ul class="space-y-1 list-disc list-inside" {
                li {
                    a hx-boost="true"
                        href={"/guilds/" (guild_id.get()) "/music"}
                        class="text-blue-400 underline"
                        { "Music" }
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
        script { (maud::PreEscaped(MUSIC_PAGE_SCRIPT)) }
    }))
}

const MUSIC_PAGE_SCRIPT: &str = r#"
(() => {
  const selected = new Set();

  function updateBulkUI() {
    const count = selected.size;
    document.querySelectorAll('.selected-count').forEach(el => el.textContent = count);
    document.querySelectorAll('.bulk-action').forEach(btn => btn.disabled = count === 0);
  }

  function reapplySelection() {
    const present = new Set();
    document.querySelectorAll('.track-checkbox').forEach(cb => {
      const id = cb.dataset.trackId;
      present.add(id);
      cb.checked = selected.has(id);
    });
    for (const id of [...selected]) if (!present.has(id)) selected.delete(id);
    updateBulkUI();
  }

  document.addEventListener('change', e => {
    const cb = e.target.closest('.track-checkbox');
    if (!cb) return;
    const id = cb.dataset.trackId;
    if (cb.checked) selected.add(id); else selected.delete(id);
    updateBulkUI();
  });

  document.addEventListener('click', e => {
    if (e.target.closest('[data-bulk-select-all]')) {
      document.querySelectorAll('.track-checkbox').forEach(cb => {
        cb.checked = true;
        selected.add(cb.dataset.trackId);
      });
      updateBulkUI();
    } else if (e.target.closest('[data-bulk-deselect-all]')) {
      selected.clear();
      document.querySelectorAll('.track-checkbox').forEach(cb => cb.checked = false);
      updateBulkUI();
    }
  });

  document.body.addEventListener('htmx:configRequest', e => {
    const trig = e.detail.elt;
    if (trig && trig.classList && trig.classList.contains('bulk-action')) {
      e.detail.parameters['ids'] = [...selected].join(',');
    }
  });

  document.body.addEventListener('htmx:afterSwap', reapplySelection);
})();
"#;
