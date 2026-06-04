use crate::{AppState, http::templates::TemplateBase, models::user::AppUser};
use axum::{debug_handler, extract::State, response::IntoResponse};
use maud::html;

#[debug_handler]
pub(super) async fn page_profile(
    _state: State<AppState>,
    tmpl: TemplateBase,
    user: AppUser,
) -> impl IntoResponse {
    tmpl.set_title("Profile").render(html! {
        div class="flex flex-col max-w-2xl mx-auto p-4 space-y-4" {
            div class="flex items-baseline justify-between" {
                h1 class="text-2xl font-bold tracking-tight" { "Profile" }
                a hx-boost="true" href="/guilds" class="text-xs text-gray-500 hover:text-gray-300 transition-colors" { "View guilds →" }
            }
            div class="rounded-lg bg-gray-900/60 border border-gray-800 p-5 space-y-4" {
                div class="space-y-1" {
                    div class="text-xs uppercase tracking-wider text-gray-500" { "Account" }
                    div class="text-lg font-semibold text-gray-50" { (user.name) }
                    div class="text-xs text-gray-500 font-mono" { (user.email) }
                }
                div class="border-t border-gray-800 pt-4" {
                    button
                        hx-get="/api/oauth/logout"
                        hx-target="body"
                        class="px-4 py-1.5 rounded-md bg-red-600/80 hover:bg-red-500 text-white text-sm font-medium transition-colors cursor-pointer"
                        { "Sign out" }
                }
            }
        }
    })
}
