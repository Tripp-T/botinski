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
    pub idle_since: Option<Instant>,
    pub volume: f32,
    pub max_volume: f32,
    pub idle_leave: Duration,
    pub(super) settings_loaded: bool,
}

impl Default for GuildPlayer {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            current: None,
            idle_since: None,
            volume: DEFAULT_VOLUME,
            max_volume: MAX_VOLUME,
            idle_leave: Duration::from_secs(300),
            settings_loaded: false,
        }
    }
}
