use {
    crate::config::ConfigData,
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
mod discord;
mod http;
mod utils;

#[derive(Parser, Debug)]
struct Opts {
    // General
    #[clap(short, long, env = "CONFIG_PATH")]
    /// Path to the configuration file
    config: PathBuf,

    // HTTP
    #[clap(long, env = "HTTP_ADDR", default_value = "127.0.0.1:3000")]
    /// Address and port the http server should bind to
    http_addr: SocketAddr,

    // Discord
    #[clap(long, env = "DISCORD_TOKEN")]
    discord_token: String,
    #[clap(long, env = "DISCORD_CLIENT_ID")]
    discord_client_id: String,
    #[clap(long, env = "DISCORD_CLIENT_SECRET")]
    discord_client_secret: String,
    #[clap(long, env = "DISCORD_SKIP_REGISTER_COMMANDS")]
    discord_skip_register_commands: bool,
}

type AppState = Arc<AppStateInner>;
struct AppStateInner {
    config: ConfigData,
    opts: Opts,
    shutdown_token: CancellationToken,
}
impl AppStateInner {
    pub async fn new(opts: Opts) -> Result<Self> {
        Ok(AppStateInner {
            config: ConfigData::new(&opts.config)?,
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

        // Save config if it was modified
        if self.config.has_been_modified {
            self.config
                .write_to_file(&self.opts.config)
                .context("failed to save modified config")?;
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
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
