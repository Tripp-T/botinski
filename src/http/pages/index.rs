use crate::{AppState, http::templates::TemplateBase, models::user_role::AppUserRole};
use axum::{debug_handler, extract::State, response::IntoResponse};
use maud::html;

#[debug_handler]
pub(super) async fn page_index(
    _state: State<AppState>,
    tmpl: TemplateBase,
    role: AppUserRole,
) -> impl IntoResponse {
    let signed_in = role.is_authenticated();
    tmpl.set_title("Home").render(html! {
        div class="max-w-3xl mx-auto p-4 space-y-6" {
            div class="space-y-2 pt-4" {
                h1 class="text-3xl font-bold tracking-tight text-gray-50" { "botinski" }
                p class="text-sm text-gray-400" {
                    "A Discord music bot — slash commands, per-guild settings, and a live SSE-driven web dashboard. Written in Rust."
                }
            }
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-5 space-y-3" {
                @if signed_in {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Welcome back" }
                    p class="text-sm text-gray-300" {
                        "Pick a guild to manage music and per-server settings."
                    }
                    div class="flex gap-3 pt-1" {
                        a hx-boost="true" href="/guilds"
                            class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
                            { "Your guilds" }
                    }
                } @else {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Get started" }
                    p class="text-sm text-gray-300" {
                        "Sign in with Discord to manage music and per-guild settings for any server you administer."
                    }
                    div class="flex gap-3 pt-1" {
                        a href="/api/oauth/login"
                            class="px-4 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
                            { "Sign in with Discord" }
                    }
                }
            }
        }
    })
}
