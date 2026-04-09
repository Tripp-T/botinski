use crate::{
    AppState,
    http::{HttpError, components::component_card, templates::TemplateBase},
    models::user::AppUser,
};
use axum::{Router, debug_handler, extract::State, response::IntoResponse, routing::get};
use maud::html;

#[debug_handler]
pub async fn page_not_found(_: State<AppState>, _: TemplateBase) -> HttpError {
    HttpError::NotFound
}

pub fn pages_router(_state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(page_index))
        .route("/profile", get(page_profile))
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
    tmpl.set_title("Profile").render(html! {
        h1."text-xl" { "Welcome to your profile, " (user.name) }
    })
}
