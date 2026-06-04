use crate::{
    AppState,
    http::{HttpError, templates::TemplateBase},
    models::{audit_log::AuditLogEntry, user_role::AppUserRole},
};
use axum::{debug_handler, extract::State, response::IntoResponse};
use maud::html;

const RECENT_LIMIT: i64 = 200;

#[debug_handler]
pub(super) async fn page_audit_log(
    State(state): State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
) -> Result<impl IntoResponse, HttpError> {
    if !matches!(role, AppUserRole::GlobalAdmin) {
        // Hide existence from non-global-admins. 404 not 403.
        return Err(HttpError::NotFound);
    }

    let entries = AuditLogEntry::recent(&state.db, RECENT_LIMIT)
        .await
        .map_err(|e| HttpError::from(anyhow::anyhow!("Failed to load audit log: {e}")))?;

    Ok(tmpl.set_title("Audit log").render(html! {
        div class="flex flex-col max-w-5xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                div {
                    h1 class="text-2xl font-bold tracking-tight" { "Audit log" }
                    div class="text-xs text-gray-500 mt-1" {
                        "Last " (entries.len()) " of up to " (RECENT_LIMIT) " entries, newest first."
                    }
                }
                span class="text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded bg-blue-600/30 text-blue-300 border border-blue-700/50" { "GLOBAL ADMIN" }
            }

            @if entries.is_empty() {
                div class="rounded-lg bg-gray-900/60 border border-gray-800 p-6 text-center text-sm text-gray-400 italic" {
                    "No entries yet. Slash commands and mutating web requests will appear here as they happen."
                }
            } @else {
                div class="rounded-lg bg-gray-900/60 border border-gray-800 overflow-hidden" {
                    div class="overflow-x-auto" {
                        table class="w-full text-sm" {
                            thead class="text-xs uppercase tracking-wider text-gray-500 bg-gray-950/60" {
                                tr {
                                    th class="text-left px-3 py-2 font-medium" { "Time (UTC)" }
                                    th class="text-left px-3 py-2 font-medium" { "Src" }
                                    th class="text-left px-3 py-2 font-medium" { "Actor" }
                                    th class="text-left px-3 py-2 font-medium" { "Guild" }
                                    th class="text-left px-3 py-2 font-medium" { "Action" }
                                    th class="text-left px-3 py-2 font-medium" { "Detail" }
                                    th class="text-left px-3 py-2 font-medium" { "Outcome" }
                                }
                            }
                            tbody class="divide-y divide-gray-800" {
                                @for e in &entries {
                                    tr class="hover:bg-gray-800/30" {
                                        td class="px-3 py-1.5 text-xs text-gray-400 font-mono whitespace-nowrap" {
                                            (e.occurred_at.format("%Y-%m-%d %H:%M:%S"))
                                        }
                                        td class="px-3 py-1.5" {
                                            span class={
                                                "text-[10px] font-bold tracking-wider px-1.5 py-0.5 rounded "
                                                @if e.source == "discord" { "bg-indigo-600/30 text-indigo-300" }
                                                @else { "bg-emerald-600/30 text-emerald-300" }
                                            } { (e.source.to_uppercase()) }
                                        }
                                        td class="px-3 py-1.5 text-gray-200" {
                                            @match e.actor_name.as_deref() {
                                                Some(n) => (n),
                                                None => span class="text-gray-500 italic" { "anonymous" },
                                            }
                                            @if let Some(id) = &e.actor_id {
                                                div class="text-[10px] text-gray-500 font-mono" { (id) }
                                            }
                                        }
                                        td class="px-3 py-1.5 text-xs text-gray-400 font-mono" {
                                            (e.guild_id.as_deref().unwrap_or("—"))
                                        }
                                        td class="px-3 py-1.5 text-gray-100 font-mono text-xs" {
                                            (e.action)
                                        }
                                        td class="px-3 py-1.5 text-gray-400 text-xs max-w-xs truncate" {
                                            (e.detail.as_deref().unwrap_or(""))
                                        }
                                        td class="px-3 py-1.5 text-xs font-mono" {
                                            @if e.outcome == "ok" {
                                                span class="text-emerald-400" { "ok" }
                                            } @else if e.outcome.starts_with("err") {
                                                span class="text-red-400" { (e.outcome) }
                                            } @else if e.outcome.starts_with("http:") {
                                                @let code = e.outcome.trim_start_matches("http:");
                                                @let is_2xx = code.starts_with('2');
                                                @let is_3xx = code.starts_with('3');
                                                span class={
                                                    "font-mono "
                                                    @if is_2xx { "text-emerald-400" }
                                                    @else if is_3xx { "text-gray-300" }
                                                    @else { "text-red-400" }
                                                } { (code) }
                                            } @else {
                                                (e.outcome)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}
