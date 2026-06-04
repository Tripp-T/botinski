use crate::{
    AppState,
    http::{HttpError, components::component_card, templates::TemplateBase},
    models::{
        audit_log::{AuditLogEntry, NewAuditLogEntry},
        user::AppUser,
    },
};
use axum::{
    RequestPartsExt, debug_middleware,
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tracing::{Instrument, debug, error, info_span, warn};

#[debug_middleware]
pub async fn middleware_error_formatting(
    State(_): State<AppState>,
    tmpl: TemplateBase,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    if let Some(error) = response.extensions_mut().remove::<HttpError>() {
        // {:?} surfaces the full anyhow source chain for Internal errors;
        // shorter variants fall back to their derive(Debug) repr.
        error!("{error:?}");
        return (
            error.as_status(),
            tmpl.set_title(error.title()).render(component_card(
                error.title(),
                error.description(),
                true,
            )),
        )
            .into_response();
    };
    response
}

/// Audit log middleware. Logs every mutating HTTP request (POST/PUT/PATCH/DELETE)
/// with the resolved actor (if any) and the response status. Read-only methods
/// (GET, HEAD, OPTIONS) and SSE polling go unlogged to keep the table from
/// becoming a chat history.
#[debug_middleware]
pub async fn middleware_audit_log(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    if !matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return next.run(req).await;
    }

    let path = req.uri().path().to_string();
    let (mut parts, body) = req.into_parts();
    // Best-effort actor identification. If extraction fails (no session, etc.)
    // we record the action as anonymous.
    let actor = parts
        .extract_with_state::<AppUser, AppState>(&state)
        .await
        .ok();
    let req = Request::from_parts(parts, body);

    let response = next.run(req).await;
    let status = response.status();

    let action = format!("{method} {path}");
    let outcome = format!("http:{}", status.as_u16());
    let actor_id = actor
        .as_ref()
        .and_then(|u| u.discord_id().ok().map(|id| id.get().to_string()));
    let actor_name = actor.as_ref().map(|u| u.name.clone());

    let entry = NewAuditLogEntry {
        source: "web",
        actor_id: actor_id.as_deref(),
        actor_name: actor_name.as_deref(),
        guild_id: None,
        action: &action,
        detail: None,
        outcome: &outcome,
    };
    if let Err(e) = AuditLogEntry::insert(&state.db, entry).await {
        warn!("audit log: failed to record web request: {e}");
    }

    response
}

#[debug_middleware]
pub async fn middleware_http_trace(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start_time = Instant::now();

    let span = info_span!(
        "http_request",
        method = %method,
        path = %path,
        status_code = tracing::field::Empty,
        ms_elapsed = tracing::field::Empty,
    );

    let response = next.run(req).instrument(span.clone()).await;

    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();

    span.record("status_code", status_code);
    span.record("ms_elapsed", elapsed_ms);

    if let Some(err) = response.extensions().get::<HttpError>() {
        error!(parent: &span, "{err:?}");
    }

    debug!(parent: &span, "Request completed");

    response
}
