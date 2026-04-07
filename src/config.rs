use serenity::all::{RoleId, UserId};

use crate::utils::{load_file, write_file};

use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigData {
    #[serde(skip, default)]
    pub has_been_modified: bool,

    pub discord: DiscordConfigData,
}
impl ConfigData {
    pub fn load_from_file(path: &PathBuf) -> anyhow::Result<Self> {
        load_file(path)
    }
    pub fn write_to_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        write_file(path, &self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscordConfigData {
    pub admin_roles: Option<Vec<RoleId>>,
    pub admin_uids: Option<Vec<UserId>>,
}
