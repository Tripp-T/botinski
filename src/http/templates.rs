use {
    crate::{AppState, http::HttpError, models::user::AppUser},
    anyhow::bail,
    axum::{RequestPartsExt, extract::FromRequestParts, response::Html},
    maud::{Markup, html},
    tower_cookies::Cookies,
    tracing::error,
};

#[derive(Clone, Default)]
pub struct TemplateBase {
    dark_theme: bool,
    title: Option<String>,
    user: Option<AppUser>,
}
impl TemplateBase {
    pub fn set_title<S: AsRef<str>>(mut self, title: S) -> Self {
        self.title = Some(title.as_ref().to_string());
        self
    }
    pub fn render(&self, body: Markup) -> Html<Markup> {
        Html(html! {
            (maud::DOCTYPE)

            html lang="en" class=@if self.dark_theme {"dark"} @else {""}   {
                head {
                    meta charset="UTF-8" {}
                    meta name="viewport" content="width=device-width, initial-scale=1.0" {}
                    title {
                        @if let Some(title) = &self.title { (title) } @else { "" }
                    }
                    script src="/htmx.2.0.8.min.js" {}
                    link href="/output.css" rel="stylesheet" {}
                }
                body class="dark:bg-gray-950 dark:text-white bg-gray-100" {
                    nav class="w-full py-1 flex border-b border-gray-500" {
                        div class="px-2 w-full max-w-3xl mx-auto" hx-boost="true" {
                            a href="/" { "Home" }
                        }
                    }
                    div id="content" class="p-2" {
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
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(key) = parts.extensions.get::<tower_cookies::Key>() else {
            return Err(HttpError::Internal("Missing cookie key".to_string()));
        };
        let cookies = parts.extract::<Cookies>().await.map_err(|(_, e)| {
            error!("Failed to parse cookies from request: {e}");
            HttpError::BadRequest("Failed to parse cookies from request".to_string())
        })?;
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
            dark_theme,
            title: None,
            user: None,
        })
    }
}
