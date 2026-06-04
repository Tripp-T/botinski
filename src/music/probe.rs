//! yt-dlp shell-outs: single-track probe (`probe_track`), playlist enumeration
//! (`dump_playlist`), URL-shape detection, and the small Deserialize structs
//! that mirror yt-dlp's JSON output.

use anyhow::{Context as _, bail};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct YtdlpInfo {
    pub title: Option<String>,
    pub duration: Option<f64>,
    #[serde(default)]
    pub is_live: bool,
    pub url: Option<String>,
    pub webpage_url: Option<String>,
}

pub async fn probe_track(query: &str) -> anyhow::Result<YtdlpInfo> {
    let q = if query.starts_with("http://") || query.starts_with("https://") {
        query.to_string()
    } else {
        format!("ytsearch1:{query}")
    };
    let output = tokio::process::Command::new("yt-dlp")
        .args([
            "-j",
            "--no-playlist",
            "--no-warnings",
            "-f",
            "ba[abr>0][vcodec=none]/best",
            &q,
        ])
        .output()
        .await
        .context("Failed to spawn yt-dlp")?;
    if !output.status.success() {
        bail!(
            "yt-dlp failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    // For search queries yt-dlp may emit multiple JSON lines; take the first.
    let first = output
        .stdout
        .split(|&b| b == b'\n')
        .find(|line| !line.is_empty())
        .context("yt-dlp produced no output")?;
    serde_json::from_slice(first).context("Failed to parse yt-dlp output")
}

pub fn is_playlist_url(s: &str) -> bool {
    let s = s.trim().trim_end_matches('/');
    s.contains("youtube.com/playlist") || s.contains("music.youtube.com/playlist")
}

#[derive(Deserialize)]
pub struct YtdlPlaylist {
    pub title: Option<String>,
    #[serde(default)]
    pub entries: Vec<YtdlPlaylistEntry>,
}

#[derive(Deserialize)]
pub struct YtdlPlaylistEntry {
    pub id: Option<String>,
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub url: Option<String>,
    pub webpage_url: Option<String>,
}

pub fn playlist_entry_url(entry: &YtdlPlaylistEntry) -> Option<String> {
    if let Some(u) = entry.webpage_url.as_ref().filter(|u| u.starts_with("http")) {
        return Some(u.clone());
    }
    if let Some(u) = entry.url.as_ref().filter(|u| u.starts_with("http")) {
        return Some(u.clone());
    }
    entry
        .id
        .as_ref()
        .map(|id| format!("https://www.youtube.com/watch?v={id}"))
}

pub async fn dump_playlist(url: &str) -> anyhow::Result<YtdlPlaylist> {
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--flat-playlist", "-J", "--no-warnings", url])
        .output()
        .await
        .context("Failed to spawn yt-dlp for playlist dump")?;
    if !output.status.success() {
        bail!(
            "yt-dlp playlist dump failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("Failed to parse yt-dlp playlist JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_url_matches_youtube_and_yt_music() {
        assert!(is_playlist_url(
            "https://www.youtube.com/playlist?list=PLfoo"
        ));
        assert!(is_playlist_url("https://youtube.com/playlist?list=PLfoo"));
        assert!(is_playlist_url(
            "https://music.youtube.com/playlist?list=PLfoo"
        ));
        // trailing slash tolerated
        assert!(is_playlist_url("https://www.youtube.com/playlist/"));
    }

    #[test]
    fn watch_urls_with_list_param_are_not_classified_as_playlists() {
        // We deliberately treat watch URLs with a list= param as single videos —
        // user can pass a pure /playlist URL to actually expand the list.
        assert!(!is_playlist_url(
            "https://www.youtube.com/watch?v=abc&list=PLfoo"
        ));
        assert!(!is_playlist_url("https://youtu.be/abc"));
    }

    #[test]
    fn non_youtube_urls_are_not_classified_as_playlists() {
        assert!(!is_playlist_url("https://example.com/playlist"));
        assert!(!is_playlist_url("not a url at all"));
        assert!(!is_playlist_url(""));
    }

    fn entry(id: Option<&str>, url: Option<&str>, webpage_url: Option<&str>) -> YtdlPlaylistEntry {
        YtdlPlaylistEntry {
            id: id.map(str::to_string),
            title: None,
            duration: None,
            url: url.map(str::to_string),
            webpage_url: webpage_url.map(str::to_string),
        }
    }

    #[test]
    fn playlist_entry_url_prefers_webpage_url() {
        let e = entry(
            Some("abc"),
            Some("https://example.com/u"),
            Some("https://example.com/w"),
        );
        assert_eq!(
            playlist_entry_url(&e).as_deref(),
            Some("https://example.com/w")
        );
    }

    #[test]
    fn playlist_entry_url_falls_back_to_url_then_id() {
        let only_url = entry(Some("abc"), Some("https://example.com/u"), None);
        assert_eq!(
            playlist_entry_url(&only_url).as_deref(),
            Some("https://example.com/u")
        );

        let only_id = entry(Some("dQw4w9WgXcQ"), None, None);
        assert_eq!(
            playlist_entry_url(&only_id).as_deref(),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        );
    }

    #[test]
    fn playlist_entry_url_skips_non_http_url_field() {
        // Some flat-playlist entries put bare ids in `url`; we should treat those
        // as missing and fall through to the id-based fallback.
        let e = entry(Some("xyz"), Some("dQw4w9WgXcQ"), None);
        assert_eq!(
            playlist_entry_url(&e).as_deref(),
            Some("https://www.youtube.com/watch?v=xyz")
        );
    }

    #[test]
    fn playlist_entry_url_none_when_nothing_usable() {
        let e = entry(None, None, None);
        assert!(playlist_entry_url(&e).is_none());
    }
}
