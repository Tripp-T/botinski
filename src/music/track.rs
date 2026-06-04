//! Queue track types, formatting helpers, and the volume constants the
//! rest of the module clamps against.

use poise::serenity_prelude::UserId;
use std::time::Duration;
use uuid::Uuid;

pub const DEFAULT_VOLUME: f32 = 1.0;
/// Hard absolute ceiling for `set_volume`. Per-guild config can lower this further.
pub const MAX_VOLUME: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct Track {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub requested_by_name: String,
    #[allow(dead_code)]
    pub requested_by_id: UserId,
    pub is_live: bool,
}

impl Track {
    pub fn new(
        title: String,
        url: String,
        duration: Option<Duration>,
        requested_by_name: String,
        requested_by_id: UserId,
        is_live: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            url,
            duration,
            requested_by_name,
            requested_by_id,
            is_live,
        }
    }
}

pub fn format_duration(d: Option<Duration>) -> String {
    match d {
        None => "?".to_string(),
        Some(d) => format_secs(d),
    }
}

pub fn format_secs(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
