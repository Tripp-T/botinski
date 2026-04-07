use std::sync::atomic::{AtomicBool, Ordering};

use serenity::all::{RoleId, UserId};
use tokio::sync::RwLock;
use tracing::warn;

use crate::utils::{load_file, write_file};

use super::*;

pub struct ConfigManager {
    data: RwLock<ConfigData>,
    config_path: PathBuf,
    has_been_modified: AtomicBool,
}
impl ConfigManager {
    pub fn new(opts: &Opts) -> Result<Self> {
        let data = ConfigData::new(&opts.config)?;
        Ok(Self {
            data: RwLock::new(data),
            config_path: opts.config.clone(),
            has_been_modified: AtomicBool::new(false),
        })
    }
    pub async fn shutdown(&self) -> Result<()> {
        if self.has_been_modified.load(Ordering::Acquire) {
            let data = self.data.read().await;
            data.write_to_file(&self.config_path)
                .context("Failed to save modified config file")?;
        };
        Ok(())
    }
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, ConfigData> {
        self.data.read().await
    }
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, ConfigData> {
        // If write is called, assume config was modified
        self.has_been_modified.store(true, Ordering::Release);
        self.data.write().await
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigData {
    pub discord: DiscordConfigData,
}
impl ConfigData {
    pub fn new(path: &PathBuf) -> Result<Self> {
        if !path.is_file() {
            warn!("Configuration file is not present, writing a default configuration");
            Self::default().write_to_file(path)?;
            warn!("A default configuration file has been written to '{path:?}'")
        }
        Self::load_from_file(path)
    }
    pub fn load_from_file(path: &PathBuf) -> anyhow::Result<Self> {
        load_file(path)
    }
    pub fn write_to_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        write_file(path, &self)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DiscordConfigData {
    pub admin_roles: Option<Vec<RoleId>>,
    pub admin_uids: Option<Vec<UserId>>,
}
