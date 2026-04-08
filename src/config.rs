use super::*;
use crate::utils::{load_file, write_file};
use poise::serenity_prelude::all::{RoleId, UserId};
use std::hash::{DefaultHasher, Hash, Hasher};
use tokio::sync::RwLock;
use tracing::warn;

pub struct ConfigManager {
    data_hash: u64,
    data: RwLock<ConfigData>,
    config_path: PathBuf,
}
impl ConfigManager {
    pub fn new(opts: &Opts) -> Result<Self> {
        let data = ConfigData::new(&opts.config)?;
        Ok(Self {
            data_hash: data.get_hash(),
            data: RwLock::new(data),
            config_path: opts.config.clone(),
        })
    }
    pub async fn shutdown(&self) -> Result<()> {
        if self.data_hash != self.data.read().await.get_hash() {
            info!("Config has been modified since last read, saving...");
            let data = self.data.read().await;
            data.write_to_file(&self.config_path)
                .context("Failed to save modified config file")?;
        }
        Ok(())
    }
    /// Returns a RwLockReadGuard of [ConfigData]
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, ConfigData> {
        self.data.read().await
    }
    /// Returns a RwLockWriteGuard of [ConfigData]
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, ConfigData> {
        self.data.write().await
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Hash)]
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
    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
    pub fn load_from_file(path: &PathBuf) -> anyhow::Result<Self> {
        load_file(path)
    }
    pub fn write_to_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        write_file(path, &self)
    }
}

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct DiscordConfigData {
    pub command_prefix: String,
    pub admin_roles: Option<Vec<RoleId>>,
    pub admin_uids: Option<Vec<UserId>>,
}
impl Default for DiscordConfigData {
    fn default() -> Self {
        Self {
            command_prefix: "\\".to_string(),
            admin_roles: None,
            admin_uids: None,
        }
    }
}
