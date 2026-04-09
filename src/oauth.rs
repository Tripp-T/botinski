use anyhow::Context;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet, RedirectUrl, Scope,
    TokenUrl,
    basic::{BasicClient, BasicTokenResponse},
    reqwest,
    url::Url,
};

use crate::Opts;

pub struct OauthManager(
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
);
impl OauthManager {
    pub fn new(opts: &Opts) -> anyhow::Result<Self> {
        let client_id = ClientId::new(opts.discord_client_id.clone());
        let client_secret = ClientSecret::new(opts.discord_client_secret.clone());
        let auth_url = AuthUrl::new("https://discord.com/oauth2/authorize".to_string())
            .context("Invalid oauth auth URL")?;
        let token_url = TokenUrl::new("https://discord.com/api/oauth2/token".to_string())
            .context("Invalid oauth token URL")?;
        let redirect_url =
            RedirectUrl::new(format!("{}/api/oauth/callback", opts.http_remote_base_url))
                .context("Invalid HTTP_REMOTE_URL")?;

        let client = oauth2::basic::BasicClient::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        Ok(Self(client))
    }
    pub fn get_login_url(&self) -> (Url, CsrfToken) {
        self.0
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("identify".to_string()))
            .add_scope(Scope::new("guilds".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .url()
    }
    fn async_http_client() -> anyhow::Result<reqwest::Client> {
        reqwest::ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("HTTP client failed to build")
    }
    pub async fn exchange_code(&self, code: &str) -> anyhow::Result<BasicTokenResponse> {
        let http_client = Self::async_http_client()?;
        self.0
            .exchange_code(oauth2::AuthorizationCode::new(code.to_string()))
            .request_async(&http_client)
            .await
            .context("Failed to exchange oauth token")
    }
}
