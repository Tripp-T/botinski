use serenity::all::{RoleId, UserId};
use tracing::warn;

use crate::utils::{load_file, write_file};

use super::*;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigData {
    #[serde(skip, default)]
    pub has_been_modified: bool,

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
