use crate::{
    AppState,
    http::{HttpError, components::component_card, templates::TemplateBase},
};
use anyhow::Context;
use axum::{Router, extract::State, response::IntoResponse, routing::get};

pub fn api_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/healthcheck", get(healthcheck))
}

async fn healthcheck(
    state: State<AppState>,
    tmpl: TemplateBase,
) -> Result<impl IntoResponse, HttpError> {
    sqlx::query("SELECT 1")
        .execute(&*state.db)
        .await
        .context("Failed to query database")?;
    Ok(tmpl.set_title("OK").render(component_card(
        "OK",
        "Healthcheck completed successfully",
        false,
    )))
}
