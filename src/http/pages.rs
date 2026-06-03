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
            p class="text-gray-400 italic" { "Admin actions coming soon." }
        }
    }))
}
