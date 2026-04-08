use {
    crate::AppState,
    askama::Template,
    axum::{RequestPartsExt, extract::FromRequestParts, http::StatusCode, response::Html},
    tower_cookies::Cookies,
    tracing::error,
};

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
        })
    }
}

pub struct MarkupDisplay(pub maud::Markup);
impl std::fmt::Display for MarkupDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.0)
    }
}
impl From<maud::Markup> for MarkupDisplay {
    fn from(value: maud::Markup) -> Self {
        Self(value)
    }
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

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct IndexTemplate {
    pub base: TemplateBase,
    pub content: MarkupDisplay,
}

#[derive(Template)]
#[template(path = "error.askama.html")]
pub struct ErrorTemplate<'a> {
    pub base: TemplateBase,
    pub error_title: &'a str,
    pub error_description: &'a str,
}
