//! Per-guild player state: the queue, the currently-playing handle, and the
//! cached per-guild settings (`volume`, `max_volume`, `idle_leave`) hydrated
//! on first interaction.

use super::track::{DEFAULT_VOLUME, MAX_VOLUME, Track};
use songbird::tracks::TrackHandle;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub struct NowPlaying {
    pub track: Track,
    pub handle: TrackHandle,
}

pub struct GuildPlayer {
    pub queue: VecDeque<Track>,
    pub current: Option<NowPlaying>,
    /// When the queue+current became simultaneously empty. Drives the
    /// "idle in voice with nothing to play" disconnect.
    pub idle_since: Option<Instant>,
    /// When the bot's voice channel became listener-empty (no humans
    /// besides the bot). Drives the "alone in voice" disconnect.
    pub empty_since: Option<Instant>,
    pub volume: f32,
    pub max_volume: f32,
    pub idle_leave: Duration,
    pub empty_channel_leave: Duration,
    pub(super) settings_loaded: bool,
}

impl Default for GuildPlayer {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            current: None,
            idle_since: None,
            empty_since: None,
            volume: DEFAULT_VOLUME,
            max_volume: MAX_VOLUME,
            idle_leave: Duration::from_secs(300),
            empty_channel_leave: Duration::from_secs(60),
            settings_loaded: false,
        }
    }
}
