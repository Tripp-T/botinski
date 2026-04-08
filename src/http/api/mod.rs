use crate::{
    AppState,
    http::{components::component_card, pages::page_internal_error, templates::TemplateBase},
};
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    handler::Handler,
    response::IntoResponse,
    routing::get,
};

pub fn api_router(_state: &AppState) -> Router<AppState> {
    Router::new().route("/healthcheck", get(healthcheck))
}

async fn healthcheck(
    state: State<AppState>,
    tmpl: TemplateBase,
    req: Request<Body>,
) -> impl IntoResponse {
    let db_result = sqlx::query("SELECT 1").execute(&*state.db).await;
    if db_result.is_err() {
        return Err(page_internal_error.call(req, state.0).await);
    }
    Ok(tmpl
        .set_title("OK")
        .render(component_card("OK", "Healthcheck completed successfully")))
}
