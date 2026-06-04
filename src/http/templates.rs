use {
    crate::{
        AppState,
        http::HttpError,
        models::{user::AppUser, user_role::AppUserRole},
    },
    axum::{RequestPartsExt, extract::FromRequestParts, response::Html},
    maud::{Markup, html},
    tower_cookies::Cookies,
    tracing::error,
};

#[derive(Clone, Default)]
pub struct TemplateBase {
    path: String,
    dark_theme: bool,
    title: Option<String>,
    user: Option<AppUser>,
    is_global_admin: bool,
}
impl TemplateBase {
    pub fn set_title<S: AsRef<str>>(mut self, title: S) -> Self {
        self.title = Some(title.as_ref().to_string());
        self
    }
    pub fn render(&self, body: Markup) -> Html<Markup> {
        struct NavLinkProps {
            hx_boost: bool,
        }
        impl Default for NavLinkProps {
            fn default() -> Self {
                Self { hx_boost: true }
            }
        }
        let nav_link = |title: &str, path: &str, props: NavLinkProps| -> maud::Markup {
            let is_active = self.path == path || (path != "/" && self.path.starts_with(path));
            html! {
                a
                    hx-boost=(props.hx_boost)
                    href=(path)
                    data-active?[is_active]
                    class="px-2 py-1 rounded-md text-sm text-gray-400 hover:text-gray-100 hover:bg-gray-800/60 data-[active=true]:text-gray-100 data-[active=true]:bg-gray-800/80 transition-colors"
                {(title)}
            }
        };
        Html(html! {
            (maud::DOCTYPE)
            html."dark"[self.dark_theme] lang="en" {
                head {
                    meta charset="UTF-8" {}
                    meta name="viewport" content="width=device-width, initial-scale=1.0" {}
                    title {
                        @if let Some(title) = &self.title { (title) " | botinski" } @else { "botinski" }
                    }
                    script src="/htmx.2.0.8.min.js" {}
                    script src="/htmx-ext-sse.2.2.2.js" {}
                    link href="/output.css" rel="stylesheet" {}
                }
                body class="min-h-screen bg-gray-950 text-gray-100 antialiased" {
                    nav class="sticky top-0 z-20 bg-gray-950/80 backdrop-blur border-b border-gray-800" {
                        div class="px-4 py-2.5 max-w-3xl mx-auto flex items-center" hx-boost="true" {
                            div class="flex items-center gap-1" {
                                a href="/" class="mr-2 font-semibold text-gray-100 tracking-tight" { "botinski" }
                                (nav_link("Home", "/", NavLinkProps::default()))
                                @if self.user.is_some() {
                                    (nav_link("Guilds", "/guilds", NavLinkProps::default()))
                                }
                            }
                            div class="ml-auto flex items-center gap-1" {
                                @if self.is_global_admin {
                                    (nav_link("Admin", "/admin/audit-log", NavLinkProps::default()))
                                }
                                @if self.user.is_some() {
                                    (nav_link("Profile", "/profile", NavLinkProps::default()))
                                } @else {
                                    (nav_link("Login", "/api/oauth/login", NavLinkProps {
                                        hx_boost: false,
                                    }))
                                }
                            }
                        }
                    }
                    div id="content" {
                        (body)
                    }
                }
            }
        })
    }
}
impl FromRequestParts<AppState> for TemplateBase {
    type Rejection = HttpError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = parts.extract::<Cookies>().await.map_err(|(_, e)| {
            error!("Failed to parse cookies from request: {e}");
            HttpError::BadRequest("Failed to parse cookies from request".to_string())
        })?;
        let user = match parts.extract_with_state::<AppUser, _>(state).await {
            Ok(u) => Some(u),
            Err(HttpError::Unauthorized) => None,
            Err(e) => return Err(e),
        };
        // RoleCache makes the AppUserRole extraction effectively free here.
        let is_global_admin = matches!(
            parts.extract_with_state::<AppUserRole, _>(state).await,
            Ok(AppUserRole::GlobalAdmin)
        );
        let dark_theme = cookies
            .get("dark_theme")
            .map(|mut cookie| match cookie.value().parse::<bool>() {
                Ok(value) => value,
                Err(e) => {
                    error!("Invalid theme cookie: {e}");
                    cookie.set_value(true.to_string());
                    true
                }
            })
            // no cookie is set, default to true
            .unwrap_or(true);
        Ok(Self {
            path: parts.uri.path().to_string(),
            dark_theme,
            title: None,
            user,
            is_global_admin,
        })
    }
}
