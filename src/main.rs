use {
    crate::config::ConfigData,
    anyhow::{Context, Result, bail},
    clap::Parser,
    serde::{Deserialize, Serialize},
    std::{
        net::SocketAddr,
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool},
    },
    tokio::{
        signal,
        sync::{Mutex, oneshot},
        task::JoinSet,
    },
    tracing::{debug, error, info},
    tracing_subscriber::EnvFilter,
};

mod config;
mod http;
mod utils;

#[derive(Parser, Debug)]
struct Opts {
    #[clap(short, long, env = "CONFIG_PATH")]
    /// Path to the configuration file
    config: PathBuf,
    #[clap(long, env = "HTTP_ADDR", default_value = "127.0.0.1:3000")]
    /// Address and port the http server should bind to
    http_addr: SocketAddr,
}

type AppState = Arc<AppStateInner>;
struct AppStateInner {
    config: ConfigData,
    opts: Opts,
    shutdown_initiated: AtomicBool,
    shutdown_callbacks: Mutex<Vec<oneshot::Sender<()>>>,
}
impl AppStateInner {
    pub async fn new(opts: Opts) -> Result<Self> {
        Ok(AppStateInner {
            config: ConfigData::load_from_file(&opts.config).context("failed to load config")?,
            opts,
            shutdown_initiated: AtomicBool::default(),
            shutdown_callbacks: Mutex::default(),
        })
    }
    pub async fn register_shutdown_callback(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.shutdown_callbacks.lock().await.push(tx);
        rx
    }
    pub async fn shutdown(&self) -> Result<()> {
        let shutdown_previously_initiated = self
            .shutdown_initiated
            .swap(true, std::sync::atomic::Ordering::Acquire);
        if shutdown_previously_initiated {
            debug!("Shutdown called multiple times, ignoring subsequent call");
            return Ok(());
        }

        let mut has_failed_to_send_shutdown = false;
        for signal in self.shutdown_callbacks.lock().await.drain(..) {
            if !signal.is_closed() && signal.send(()).is_err() {
                has_failed_to_send_shutdown = true;
            }
        }
        // Save config if it was modified
        if self.config.has_been_modified {
            self.config
                .write_to_file(&self.opts.config)
                .context("failed to save modified config")?;
        }

        if has_failed_to_send_shutdown {
            bail!("Failed to send shutdown signal to one or more threads")
        } else {
            Ok(())
        }
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

    let mut threads = JoinSet::new();
    threads.spawn(shutdown_signal(state.clone()));
    threads.spawn(http::main(state.clone()));

    let mut threads_with_error = 0usize;
    while let Some(thread_result) = threads.join_next().await {
        match thread_result {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => {
                error!("thread exited with error: {e}");
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
    if threads_with_error.gt(&0) {
        bail!("{threads_with_error} returned an error")
    } else {
        Ok(())
    }
}

async fn shutdown_signal(state: AppState) -> Result<()> {
    let app_shutdown_signal = state.register_shutdown_callback().await;

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
        _ = app_shutdown_signal => {
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
