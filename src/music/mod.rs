use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use anyhow::{Context as _, anyhow, bail};
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use serde::Deserialize;
use songbird::{
    Event, EventContext, EventHandler as VoiceEventHandler, Songbird, TrackEvent,
    input::{
        AsyncAdapterStream, AsyncReadOnlySource, AudioStream, AudioStreamError, Compose, Input,
        YoutubeDl,
    },
    tracks::TrackHandle,
};
use std::{
    pin::Pin,
    process::Stdio,
    task::{Context as TaskContext, Poll},
};
use symphonia::core::io::MediaSource;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::process::{Child, ChildStdout};
use std::collections::HashSet;
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::{AppState, AppStateInner, models::guild_settings::GuildSettings};

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
    settings_loaded: bool,
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
    fn all_guild_ids(&self) -> Vec<GuildId> {
        self.players.read().unwrap().keys().copied().collect()
    }
    pub fn is_connected(&self, guild_id: GuildId) -> bool {
        self.songbird.get(guild_id).is_some()
    }
}

#[derive(Deserialize, Debug)]
struct YtdlpInfo {
    title: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    is_live: bool,
    url: Option<String>,
    webpage_url: Option<String>,
}

async fn probe_track(query: &str) -> anyhow::Result<YtdlpInfo> {
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

fn build_input(client: reqwest::Client, info: &YtdlpInfo, watch_url: &str) -> anyhow::Result<Input> {
    if info.is_live {
        let manifest_url = info
            .url
            .clone()
            .context("Live track missing manifest URL from yt-dlp")?;
        Ok(FfmpegInput::new(manifest_url).into())
    } else {
        Ok(YoutubeDl::new(client, watch_url.to_string()).into())
    }
}

/// Transmuxing input: spawns ffmpeg to convert any URL (HLS, DASH, etc.) into
/// matroska+opus on stdout, which symphonia + songbird's opus passthrough can
/// consume cleanly. Used for live streams where the raw stream format isn't
/// directly playable by symphonia.
pub struct FfmpegInput {
    url: String,
}

impl FfmpegInput {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl From<FfmpegInput> for Input {
    fn from(val: FfmpegInput) -> Self {
        Input::Lazy(Box::new(val))
    }
}

/// Wraps ffmpeg's stdout while keeping the Child handle alive so the subprocess
/// stays running until the source is dropped (at which point kill_on_drop kills it).
struct FfmpegStdout {
    _child: Child,
    stdout: ChildStdout,
}

impl AsyncRead for FfmpegStdout {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

#[async_trait::async_trait]
impl Compose for FfmpegInput {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let mut child = tokio::process::Command::new("ffmpeg")
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .args([
                "-loglevel",
                "error",
                "-i",
                &self.url,
                "-vn",
                "-c:a",
                "libopus",
                "-b:a",
                "128k",
                "-f",
                "matroska",
                "pipe:1",
            ])
            .spawn()
            .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AudioStreamError::Fail(
                std::io::Error::new(std::io::ErrorKind::Other, "ffmpeg stdout missing").into(),
            )
        })?;

        let wrapped = FfmpegStdout {
            _child: child,
            stdout,
        };
        let source = AsyncReadOnlySource::new(wrapped);
        let stream = AsyncAdapterStream::new(Box::new(source), 64 * 1024);

        Ok(AudioStream {
            input: Box::new(stream) as Box<dyn MediaSource>,
        })
    }

    fn should_create_async(&self) -> bool {
        true
    }
}

pub fn is_playlist_url(s: &str) -> bool {
    let s = s.trim().trim_end_matches('/');
    s.contains("youtube.com/playlist") || s.contains("music.youtube.com/playlist")
}

#[derive(Deserialize)]
struct YtdlPlaylist {
    title: Option<String>,
    #[serde(default)]
    entries: Vec<YtdlPlaylistEntry>,
}

#[derive(Deserialize)]
struct YtdlPlaylistEntry {
    id: Option<String>,
    title: Option<String>,
    duration: Option<f64>,
    url: Option<String>,
    webpage_url: Option<String>,
}

fn playlist_entry_url(entry: &YtdlPlaylistEntry) -> Option<String> {
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

async fn dump_playlist(url: &str) -> anyhow::Result<YtdlPlaylist> {
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

pub struct PlaylistResult {
    pub title: Option<String>,
    pub added: usize,
}

pub async fn enqueue_playlist(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    voice_channel_id: Option<ChannelId>,
    playlist_url: String,
    requester: (UserId, String),
) -> anyhow::Result<PlaylistResult> {
    ensure_settings_loaded(state, guild_id).await?;
    let playlist = dump_playlist(&playlist_url).await?;
    if playlist.entries.is_empty() {
        bail!("Playlist contains no entries");
    }

    let manager = &state.music;
    let call_lock = match manager.songbird().get(guild_id) {
        Some(c) => c,
        None => {
            let channel_id = voice_channel_id
                .context("Bot isn't in voice — initiate playback from Discord first")?;
            manager
                .songbird()
                .join(guild_id, channel_id)
                .await
                .context("Failed to join voice channel")?
        }
    };

    let player = manager.player(guild_id);
    let mut guard = player.lock().await;
    let mut added = 0usize;

    for entry in playlist.entries {
        let Some(track_url) = playlist_entry_url(&entry) else {
            continue;
        };
        let title = entry.title.clone().unwrap_or_else(|| track_url.clone());
        let track = Track::new(
            title,
            track_url.clone(),
            entry.duration.map(Duration::from_secs_f64),
            requester.1.clone(),
            requester.0,
            false,
        );

        if guard.current.is_none() {
            let src: Input = YoutubeDl::new(manager.http_client(), track_url).into();
            start_playback(state, guild_id, call_lock.clone(), &mut guard, src, track).await?;
        } else {
            guard.queue.push_back(track);
        }
        added += 1;
    }

    drop(guard);
    state.music.notify(guild_id);
    Ok(PlaylistResult {
        title: playlist.title,
        added,
    })
}

pub async fn enqueue(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    voice_channel_id: Option<ChannelId>,
    query: String,
    requester: (UserId, String),
) -> anyhow::Result<Track> {
    ensure_settings_loaded(state, guild_id).await?;
    let manager = &state.music;

    let info = probe_track(&query)
        .await
        .context("Failed to resolve track via yt-dlp")?;

    let title = info.title.clone().unwrap_or_else(|| query.clone());
    let watch_url = info.webpage_url.clone().unwrap_or_else(|| query.clone());
    let duration = if info.is_live {
        None
    } else {
        info.duration.map(Duration::from_secs_f64)
    };
    let track = Track::new(
        title,
        watch_url.clone(),
        duration,
        requester.1,
        requester.0,
        info.is_live,
    );

    let call_lock = match manager.songbird().get(guild_id) {
        Some(call) => call,
        None => {
            let channel_id = voice_channel_id
                .context("Bot isn't in voice — initiate playback from Discord first")?;
            manager
                .songbird()
                .join(guild_id, channel_id)
                .await
                .context("Failed to join voice channel")?
        }
    };

    let player = manager.player(guild_id);
    let mut guard = player.lock().await;
    if guard.current.is_some() {
        guard.queue.push_back(track.clone());
    } else {
        let input = build_input(manager.http_client(), &info, &watch_url)?;
        start_playback(state, guild_id, call_lock, &mut guard, input, track.clone()).await?;
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(track)
}

async fn start_playback(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    call_lock: Arc<Mutex<songbird::Call>>,
    guard: &mut GuildPlayer,
    input: Input,
    track: Track,
) -> anyhow::Result<()> {
    let handle = call_lock.lock().await.play_input(input);
    handle
        .add_event(
            Event::Track(TrackEvent::End),
            TrackEndHandler {
                state: state.clone(),
                guild_id,
            },
        )
        .map_err(|e| anyhow!("Failed to register track end event: {e}"))?;
    let _ = handle.set_volume(guard.volume);
    guard.current = Some(NowPlaying { track, handle });
    guard.idle_since = None;
    Ok(())
}

async fn ensure_settings_loaded(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    if guard.settings_loaded {
        return Ok(());
    }
    if let Some(settings) = GuildSettings::get(&state.db, guild_id)
        .await
        .context("Failed to load persisted guild settings")?
    {
        guard.max_volume = settings.max_volume.clamp(0.0, MAX_VOLUME);
        guard.volume = settings.volume.clamp(0.0, guard.max_volume);
        guard.idle_leave = Duration::from_secs(settings.idle_leave_secs.max(0) as u64);
    }
    guard.settings_loaded = true;
    Ok(())
}

pub async fn apply_settings(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    new_settings: &GuildSettings,
) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    guard.max_volume = new_settings.max_volume.clamp(0.0, MAX_VOLUME);
    guard.idle_leave = Duration::from_secs(new_settings.idle_leave_secs.max(0) as u64);
    // re-clamp volume to the (possibly lowered) cap and apply
    let clamped = guard.volume.min(guard.max_volume);
    if clamped != guard.volume {
        guard.volume = clamped;
        if let Some(np) = &guard.current {
            let _ = np.handle.set_volume(clamped);
        }
    }
    guard.settings_loaded = true;
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn set_volume(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    volume: f32,
) -> anyhow::Result<f32> {
    ensure_settings_loaded(state, guild_id).await?;
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    let volume = volume.clamp(0.0, guard.max_volume);
    guard.volume = volume;
    if let Some(np) = &guard.current {
        let _ = np.handle.set_volume(volume);
    }
    drop(guard);
    GuildSettings::upsert_volume(&state.db, guild_id, volume)
        .await
        .context("Failed to persist volume")?;
    state.music.notify(guild_id);
    Ok(volume)
}

struct TrackEndHandler {
    state: Arc<AppStateInner>,
    guild_id: GuildId,
}

#[async_trait::async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(states) = ctx {
            for (state, _) in *states {
                if let songbird::tracks::PlayMode::Errored(err) = &state.playing {
                    error!(
                        "Track in guild {} ended with error: {err:?}",
                        self.guild_id
                    );
                }
            }
        }
        if let Err(e) = advance_queue(&self.state, self.guild_id).await {
            error!("Failed to advance queue for {}: {e}", self.guild_id);
        }
        None
    }
}

async fn advance_queue(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let manager = &state.music;
    let player = manager.player(guild_id);
    let mut guard = player.lock().await;
    guard.current = None;

    let result = if let Some(next) = guard.queue.pop_front() {
        let call_lock = manager
            .songbird()
            .get(guild_id)
            .context("No active call when advancing queue")?;
        let input: Input = if next.is_live {
            match probe_track(&next.url).await {
                Ok(info) => build_input(manager.http_client(), &info, &next.url)?,
                Err(e) => {
                    warn!(
                        "Failed to refresh live stream for {}: {e}; falling back to YoutubeDl",
                        next.url
                    );
                    YoutubeDl::new(manager.http_client(), next.url.clone()).into()
                }
            }
        } else {
            YoutubeDl::new(manager.http_client(), next.url.clone()).into()
        };
        start_playback(state, guild_id, call_lock, &mut guard, input, next).await
    } else {
        guard.idle_since = Some(Instant::now());
        Ok(())
    };
    drop(guard);
    state.music.notify(guild_id);
    result
}

pub async fn skip(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let guard = player.lock().await;
    if let Some(np) = &guard.current {
        np.handle
            .stop()
            .map_err(|e| anyhow!("Failed to stop track: {e}"))?;
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn pause(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let guard = player.lock().await;
    if let Some(np) = &guard.current {
        np.handle
            .pause()
            .map_err(|e| anyhow!("Failed to pause: {e}"))?;
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn resume(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let guard = player.lock().await;
    if let Some(np) = &guard.current {
        np.handle
            .play()
            .map_err(|e| anyhow!("Failed to resume: {e}"))?;
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn clear(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    guard.queue.clear();
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn remove_tracks(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    ids: &HashSet<Uuid>,
) -> anyhow::Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    let before = guard.queue.len();
    guard.queue.retain(|t| !ids.contains(&t.id));
    let removed = before - guard.queue.len();
    drop(guard);
    state.music.notify(guild_id);
    Ok(removed)
}

pub async fn move_tracks_up(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    ids: &HashSet<Uuid>,
) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    let q = &mut guard.queue;
    for i in 1..q.len() {
        if ids.contains(&q[i].id) && !ids.contains(&q[i - 1].id) {
            q.swap(i - 1, i);
        }
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn move_tracks_down(
    state: &Arc<AppStateInner>,
    guild_id: GuildId,
    ids: &HashSet<Uuid>,
) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    let q = &mut guard.queue;
    if q.len() < 2 {
        return Ok(());
    }
    for i in (0..q.len() - 1).rev() {
        if ids.contains(&q[i].id) && !ids.contains(&q[i + 1].id) {
            q.swap(i, i + 1);
        }
    }
    drop(guard);
    state.music.notify(guild_id);
    Ok(())
}

pub async fn leave(state: &Arc<AppStateInner>, guild_id: GuildId) -> anyhow::Result<()> {
    let player = state.music.player(guild_id);
    let mut guard = player.lock().await;
    if let Some(np) = guard.current.take() {
        let _ = np.handle.stop();
    }
    guard.queue.clear();
    guard.idle_since = None;
    drop(guard);
    if state.music.is_connected(guild_id) {
        state
            .music
            .songbird()
            .remove(guild_id)
            .await
            .context("Failed to leave voice channel")?;
    }
    state.music.notify(guild_id);
    Ok(())
}

pub async fn idle_reaper(state: AppState) -> anyhow::Result<()> {
    use tokio::time;
    let mut interval = time::interval(time::Duration::from_secs(60));
    interval.tick().await;
    loop {
        tokio::select! {
            _ = state.shutdown_token.cancelled() => return Ok(()),
            _ = interval.tick() => {
                for guild_id in state.music.all_guild_ids() {
                    let Some(player) = state.music.try_get_player(guild_id) else { continue };
                    let should_leave = {
                        let guard = player.lock().await;
                        let timeout = guard.idle_leave;
                        timeout > Duration::ZERO
                            && guard.current.is_none()
                            && guard.queue.is_empty()
                            && guard.idle_since.is_some_and(|t| t.elapsed() >= timeout)
                    };
                    if should_leave {
                        debug!("Idle reaper: disconnecting from guild {guild_id}");
                        if let Err(e) = leave(&state, guild_id).await {
                            warn!("Idle reaper: failed to leave guild {guild_id}: {e}");
                        }
                    }
                }
            }
        }
    }
}
