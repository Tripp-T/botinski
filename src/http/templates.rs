use askama::Template;
use axum::{
    RequestPartsExt,
    extract::FromRequestParts,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use tower_cookies::Cookies;
use tracing::error;

use crate::AppState;

#[derive(Clone)]
pub struct TemplateBase {
    dark_theme: bool,
    title: Option<String>,
}
impl TemplateBase {
    pub fn set_title<S: AsRef<str>>(mut self, title: S) -> Self {
        self.title = Some(title.as_ref().to_string());
        self
    }
}
impl FromRequestParts<AppState> for TemplateBase {
    type Rejection = StatusCode;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = parts.extract::<Cookies>().await.map_err(|(s, e)| {
            error!("Failed to parse cookies from request: {e}");
            s
        })?;
        let dark_theme_cookie = cookies.get("dark_theme");
        let dark_theme = match dark_theme_cookie {
            None => true,
            Some(mut dark_theme_cookie) => match dark_theme_cookie.value().parse::<bool>() {
                Ok(value) => value,
                Err(e) => {
                    error!("Invalid theme cookie: {e}");
                    dark_theme_cookie.set_value(true.to_string());
                    true
                }
            },
        };
        Ok(Self {
            dark_theme,
            title: None,
        })
    }
}

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct IndexTemplate<'a> {
    pub base: TemplateBase,
    pub name: &'a str,
}

pub trait TemplateAxumResponse {
    fn render_response(&self) -> Result<Html<String>, StatusCode>;
}
impl<T: Template> TemplateAxumResponse for T {
    fn render_response(&self) -> Result<Html<String>, StatusCode> {
        match self.render() {
            Ok(html) => Ok(Html(html)),
            Err(e) => {
                error!("Failed to render template: {e}");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
