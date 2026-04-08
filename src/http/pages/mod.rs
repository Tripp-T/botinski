use axum::{
    Router, debug_handler, extract::State, http::StatusCode, response::IntoResponse, routing::get,
};
use maud::html;

use crate::{
    AppState,
    http::templates::{TemplateBase, template_error},
};

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
