use crate::{
    AppState,
    http::templates::{TemplateBase, template_error},
};
use axum::{
    Router, debug_handler, extract::State, http::StatusCode, response::IntoResponse, routing::get,
};
use maud::html;

#[debug_handler]
pub async fn page_not_found(_: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        tmpl.set_title("404").render(template_error(
            "page not found",
            "The requested resource could not be found",
        )),
    )
}

#[debug_handler]
pub async fn page_internal_error(_: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        tmpl.set_title("500").render(template_error(
            "internal server error",
            "An error was encountered while attempting to facilitate your request",
        )),
    )
}

pub fn pages_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/", get(page_index))
}

#[debug_handler]
async fn page_index(_state: State<AppState>, tmpl: TemplateBase) -> impl IntoResponse {
    tmpl.set_title("Home").render(html! {
        p { "Hello world!!!" }
        p class="text-red-400" { "From Rust btw "}
    })
}
