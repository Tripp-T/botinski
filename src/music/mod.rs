//! Music subsystem. Layout:
//!
//! - `track`:   data model + format helpers + volume constants.
//! - `player`:  per-guild queue + currently-playing handle.
//! - `manager`: process-wide coordinator (songbird, per-guild player map,
//!   SSE notify channel, shared HTTP client).
//! - `probe`:   yt-dlp shell-outs (track probe + playlist enumerate).
//! - `ffmpeg`:  custom songbird `Compose` input that transmuxes any URL via
//!   ffmpeg into matroska+opus for live streams.
//!
//! This file holds the action API (`enqueue`, `skip`, `set_volume`, …) used
//! by the slash commands and HTTP layer, plus the internal coordination glue
//! (`start_playback`, `advance_queue`, `TrackEndHandler`, `ensure_settings_loaded`,
//! `idle_reaper`).

mod ffmpeg;
mod manager;
mod player;
mod probe;
mod track;

pub use ffmpeg::FfmpegInput;
pub use manager::MusicManager;
pub use player::{GuildPlayer, NowPlaying};
pub use probe::is_playlist_url;
pub use track::{DEFAULT_VOLUME, MAX_VOLUME, NewTrack, Track, format_duration, format_secs};

use anyhow::{Context as _, anyhow, bail};
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use songbird::{
    Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent,
    input::{Input, YoutubeDl},
};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::{
    AppState, AppStateInner,
    models::{guild_queue, guild_settings::GuildSettings},
};
use probe::{YtdlpInfo, dump_playlist, playlist_entry_url, probe_track};

fn build_input(
    client: reqwest::Client,
    info: &YtdlpInfo,
    watch_url: &str,
) -> anyhow::Result<Input> {
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

/// Build an Input for a previously-stored Track. Live tracks need a fresh
/// manifest URL on every play (yt-dlp manifests expire); on failure we fall
/// back to the lazy YoutubeDl path.
async fn build_input_for_track(client: reqwest::Client, track: &Track) -> Input {
    if track.is_live {
        match probe_track(&track.url).await {
            Ok(info) => match build_input(client.clone(), &info, &track.url) {
                Ok(input) => return input,
                Err(e) => warn!(
                    "Failed to build live input for {}: {e}; falling back to YoutubeDl",
                    track.url
                ),
            },
            Err(e) => warn!(
                "Failed to refresh live stream for {}: {e}; falling back to YoutubeDl",
                track.url
            ),
        }
    }
    YoutubeDl::new(client, track.url.clone()).into()
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
        let track = Track::new(NewTrack {
            title,
            url: track_url.clone(),
            duration: entry.duration.map(Duration::from_secs_f64),
            requested_by_name: requester.1.clone(),
            requested_by_id: requester.0,
            is_live: false,
        });

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
    persist_queue(state, guild_id).await;
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
    let track = Track::new(NewTrack {
        title,
        url: watch_url.clone(),
        duration,
        requested_by_name: requester.1,
        requested_by_id: requester.0,
        is_live: info.is_live,
    });

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
    } else if let Some(restored_head) = guard.queue.pop_front() {
        // Queue has restored items from a prior session. Resume them in
        // order; the user's new track goes to the back so we preserve the
        // original ordering they built up before the restart.
        let head_input = build_input_for_track(manager.http_client(), &restored_head).await;
        start_playback(
            state,
            guild_id,
            call_lock,
            &mut guard,
            head_input,
            restored_head,
        )
        .await?;
        guard.queue.push_back(track.clone());
    } else {
        let input = build_input(manager.http_client(), &info, &watch_url)?;
        start_playback(state, guild_id, call_lock, &mut guard, input, track.clone()).await?;
    }
    drop(guard);
    state.music.notify(guild_id);
    persist_queue(state, guild_id).await;
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
    // First time we see this guild post-restart: rehydrate the persisted queue.
    // Both the "current" and "upcoming" entries from the prior session land in
    // `queue`; the next /play will pop the head and start it playing.
    match guild_queue::load(&state.db, guild_id).await {
        Ok(tracks) => {
            for t in tracks {
                guard.queue.push_back(t);
            }
        }
        Err(e) => warn!("Failed to load persisted queue for {guild_id}: {e}"),
    }
    guard.settings_loaded = true;
    Ok(())
}

/// Snapshot the player's current+queue and write it to the persistence table.
/// Called from every mutator that changes queue/current. Failures are logged
/// but never propagate — losing a queue snapshot shouldn't fail the request.
async fn persist_queue(state: &Arc<AppStateInner>, guild_id: GuildId) {
    let snapshot: Vec<Track> = {
        let player = state.music.player(guild_id);
        let guard = player.lock().await;
        guard
            .current
            .as_ref()
            .map(|np| np.track.clone())
            .into_iter()
            .chain(guard.queue.iter().cloned())
            .collect()
    };
    if let Err(e) = guild_queue::save(&state.db, guild_id, &snapshot).await {
        warn!("Failed to persist queue for {guild_id}: {e}");
    }
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
                    error!("Track in guild {} ended with error: {err:?}", self.guild_id);
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
        let input = build_input_for_track(manager.http_client(), &next).await;
        start_playback(state, guild_id, call_lock, &mut guard, input, next).await
    } else {
        guard.idle_since = Some(Instant::now());
        Ok(())
    };
    drop(guard);
    state.music.notify(guild_id);
    persist_queue(state, guild_id).await;
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
    // advance_queue runs in the End event handler and persists then; nothing
    // to do here.
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
    persist_queue(state, guild_id).await;
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
    persist_queue(state, guild_id).await;
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
    persist_queue(state, guild_id).await;
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
    persist_queue(state, guild_id).await;
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
    persist_queue(state, guild_id).await;
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
