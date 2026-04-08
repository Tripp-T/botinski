use {
    crate::{config::ConfigManager, db::DBManager},
    anyhow::{Context, Result, bail},
    clap::Parser,
    serde::{Deserialize, Serialize},
    std::{net::SocketAddr, path::PathBuf, sync::Arc},
    tokio::{
        signal,
        task::{AbortHandle, JoinSet},
    },
    tokio_util::sync::CancellationToken,
    tracing::{debug, error, info},
    tracing_subscriber::EnvFilter,
};

mod config;
mod db;
mod discord;
mod http;
mod models;
mod utils;

#[derive(Parser, Debug)]
struct Opts {
    // General
    #[clap(short, long, env = "CONFIG_PATH")]
    /// Path to the configuration file
    config: PathBuf,
    #[clap(long, env = "DATABASE_URL")]
    /// URL to use to connect to the DB
    database_url: String,
    #[clap(long, env = "DATABASE_MAX_CONNECTIONS", default_value_t = 5)]
    /// Maximum number of connections to the database
    database_max_connections: u32,

    // HTTP
    #[clap(long, env = "HTTP_ADDR", default_value = "127.0.0.1:3000")]
    /// Address and port the http server should bind to
    http_addr: SocketAddr,
    #[clap(long, env = "HTTP_SITE_ROOT")]
    /// Path to the site root
    http_site_root: PathBuf,

    // Discord
    #[clap(long, env = "DISCORD_TOKEN")]
    /// Authentication for bot functionality
    discord_token: String,
    #[clap(long, env = "DISCORD_CLIENT_ID")]
    /// Authentication for SSO via HTTP server
    discord_client_id: String,
    #[clap(long, env = "DISCORD_CLIENT_SECRET")]
    /// Authentication for SSO via HTTP server
    discord_client_secret: String,
    #[clap(long, env = "DISCORD_SKIP_REGISTER_COMMANDS")]
    /// If slash command registration should be skipped
    discord_skip_register_commands: bool,
}

type AppState = Arc<AppStateInner>;
struct AppStateInner {
    config: ConfigManager,
    db: DBManager,
    opts: Opts,
    shutdown_token: CancellationToken,
}
impl AppStateInner {
    pub async fn new(opts: Opts) -> Result<Self> {
        Ok(AppStateInner {
            config: ConfigManager::new(&opts)?,
            db: DBManager::new(&opts).await?,
            opts,
            shutdown_token: CancellationToken::new(),
        })
    }
    pub async fn shutdown(&self) -> Result<()> {
        if self.shutdown_token.is_cancelled() {
            debug!("Shutdown called multiple times, ignoring subsequent call");
            return Ok(());
        }
        self.shutdown_token.cancel();

        self.config.shutdown().await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("botinski=info")),
        )
        .init();
    info!("Starting application");
    let opts = Opts::parse();
    let state: AppState = Arc::new(AppStateInner::new(opts).await?);

    let mut thread_pool = JoinSet::new();
    wrap_thread(&mut thread_pool, shutdown_signal(state.clone()), "Core");
    wrap_thread(&mut thread_pool, http::main(state.clone()), "HTTP");
    wrap_thread(&mut thread_pool, discord::main(state.clone()), "Discord");

    let mut threads_with_error = 0usize;
    while let Some(thread_result) = thread_pool.join_next().await {
        match thread_result {
            Ok((name, Ok(()))) => info!("thread '{name}' exited cleanly"),
            Ok((name, Err(e))) => {
                error!("thread '{name}' exited with error: {e}");
                threads_with_error += 1;
                if let Err(e) = state.shutdown().await {
                    error!("{e}")
                };
                continue;
            }
            Err(e) => {
                error!("thread panicked: {e}");
                threads_with_error += 1;
                if let Err(e) = state.shutdown().await {
                    error!("{e}")
                };
                break;
            }
        }
    }
    if threads_with_error > 0 {
        bail!("{threads_with_error} thread(s) returned an error")
    } else {
        Ok(())
    }
}

fn wrap_thread<F, S: Into<String>>(
    thread_pool: &mut JoinSet<(String, Result<()>)>,
    thread: F,
    name: S,
) -> AbortHandle
where
    F: Future<Output = Result<()>>,
    F: Send + 'static,
{
    let name = name.into();
    thread_pool.spawn(async { (name, thread.await) })
}

async fn shutdown_signal(state: AppState) -> Result<()> {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .context("failed to install Ctrl+C handler")
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = state.shutdown_token.cancelled() => {
            info!("Received app shutdown signal");
            }
        r = ctrl_c => {
            r?;
            info!("Received Ctrl+C");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
    }
    state.shutdown().await
}
