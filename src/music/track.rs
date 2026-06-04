//! Queue track types, formatting helpers, and the volume constants the
//! rest of the module clamps against.

use poise::serenity_prelude::UserId;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

pub const DEFAULT_VOLUME: f32 = 1.0;
/// Hard absolute ceiling for `set_volume`. Per-guild config can lower this further.
pub const MAX_VOLUME: f32 = 2.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub requested_by_name: String,
    pub requested_by_id: UserId,
    pub is_live: bool,
}

/// Caller-supplied fields for [`Track::new`]. Lets call sites read
/// `is_live: true` instead of guessing which positional bool means what.
pub struct NewTrack {
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub requested_by_name: String,
    pub requested_by_id: UserId,
    pub is_live: bool,
}

impl Track {
    pub fn new(t: NewTrack) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: t.title,
            url: t.url,
            duration: t.duration,
            requested_by_name: t.requested_by_name,
            requested_by_id: t.requested_by_id,
            is_live: t.is_live,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_secs_under_an_hour() {
        assert_eq!(format_secs(Duration::from_secs(0)), "0:00");
        assert_eq!(format_secs(Duration::from_secs(5)), "0:05");
        assert_eq!(format_secs(Duration::from_secs(75)), "1:15");
        assert_eq!(format_secs(Duration::from_secs(3599)), "59:59");
    }

    #[test]
    fn format_secs_over_an_hour_includes_h_segment() {
        assert_eq!(format_secs(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(format_secs(Duration::from_secs(3665)), "1:01:05");
        assert_eq!(
            format_secs(Duration::from_secs(36_000 + 600 + 5)),
            "10:10:05"
        );
    }

    #[test]
    fn format_duration_renders_question_mark_for_unknown() {
        assert_eq!(format_duration(None), "?");
        assert_eq!(format_duration(Some(Duration::from_secs(90))), "1:30");
    }

    #[test]
    fn track_new_assigns_unique_ids() {
        let make = || {
            Track::new(NewTrack {
                title: "a".into(),
                url: "u".into(),
                duration: None,
                requested_by_name: "r".into(),
                requested_by_id: poise::serenity_prelude::UserId::new(1),
                is_live: false,
            })
        };
        assert_ne!(make().id, make().id);
    }
}
