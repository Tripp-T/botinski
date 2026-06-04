//! Process-wide music coordinator: holds the songbird instance, the per-guild
//! `GuildPlayer` map, the per-guild SSE notify channel, and the shared HTTP
//! client used by yt-dlp probes and ffmpeg input handlers.

use super::player::GuildPlayer;
use poise::serenity_prelude::GuildId;
use songbird::Songbird;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::{Mutex, broadcast};

pub struct MusicManager {
    songbird: Arc<Songbird>,
    players: RwLock<HashMap<GuildId, Arc<Mutex<GuildPlayer>>>>,
    events: RwLock<HashMap<GuildId, broadcast::Sender<()>>>,
    http_client: reqwest::Client,
}

impl MusicManager {
    pub fn new() -> Self {
        Self {
            songbird: Songbird::serenity(),
            players: RwLock::new(HashMap::new()),
            events: RwLock::new(HashMap::new()),
            http_client: reqwest::Client::new(),
        }
    }
    fn event_sender(&self, guild_id: GuildId) -> broadcast::Sender<()> {
        if let Some(tx) = self.events.read().unwrap().get(&guild_id) {
            return tx.clone();
        }
        self.events
            .write()
            .unwrap()
            .entry(guild_id)
            .or_insert_with(|| broadcast::channel(32).0)
            .clone()
    }
    pub fn subscribe(&self, guild_id: GuildId) -> broadcast::Receiver<()> {
        self.event_sender(guild_id).subscribe()
    }
    pub fn notify(&self, guild_id: GuildId) {
        let _ = self.event_sender(guild_id).send(());
    }
    pub fn songbird(&self) -> Arc<Songbird> {
        self.songbird.clone()
    }
    pub fn http_client(&self) -> reqwest::Client {
        self.http_client.clone()
    }
    pub fn player(&self, guild_id: GuildId) -> Arc<Mutex<GuildPlayer>> {
        if let Some(p) = self.players.read().unwrap().get(&guild_id) {
            return p.clone();
        }
        self.players
            .write()
            .unwrap()
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(GuildPlayer::default())))
            .clone()
    }
    pub fn try_get_player(&self, guild_id: GuildId) -> Option<Arc<Mutex<GuildPlayer>>> {
        self.players.read().unwrap().get(&guild_id).cloned()
    }
    pub(super) fn all_guild_ids(&self) -> Vec<GuildId> {
        self.players.read().unwrap().keys().copied().collect()
    }
    pub fn is_connected(&self, guild_id: GuildId) -> bool {
        self.songbird.get(guild_id).is_some()
    }
}
