use super::{Context, Error, is_admin};
use crate::music;
use anyhow::Context as _;
use std::time::Duration;

fn format_duration(d: Option<Duration>) -> String {
    match d {
        None => "?".to_string(),
        Some(d) => {
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
    }
}

/// Play a track, search, or queue a YouTube/YT Music playlist
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "URL, search query, or playlist URL"] query: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    let voice_channel_id = {
        let guild = ctx.guild().context("Guild not in cache")?;
        guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|v| v.channel_id)
    };
    let voice_channel_id =
        voice_channel_id.context("You must be in a voice channel for me to join")?;

    ctx.defer().await.ok();

    let requester = (ctx.author().id, ctx.author().name.clone());
    if music::is_playlist_url(&query) {
        let result = music::enqueue_playlist(
            ctx.data(),
            guild_id,
            Some(voice_channel_id),
            query,
            requester,
        )
        .await?;
        let from = result
            .title
            .as_deref()
            .map(|t| format!(" from **{t}**"))
            .unwrap_or_default();
        ctx.reply(format!("Queued **{}** track(s){}", result.added, from))
            .await?;
    } else {
        let track = music::enqueue(
            ctx.data(),
            guild_id,
            Some(voice_channel_id),
            query,
            requester,
        )
        .await?;
        ctx.reply(format!(
            "Queued **{}** ({})",
            track.title,
            format_duration(track.duration)
        ))
        .await?;
    }
    Ok(())
}

/// Show the current queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn queue(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    let player = ctx.data().music.player(guild_id);
    let guard = player.lock().await;
    let mut msg = String::new();
    match &guard.current {
        Some(np) => msg.push_str(&format!(
            "**Now playing:** {} ({})\n",
            np.track.title,
            format_duration(np.track.duration)
        )),
        None => msg.push_str("**Nothing playing**\n"),
    }
    if guard.queue.is_empty() {
        msg.push_str("_Queue is empty_");
    } else {
        msg.push_str("\n**Up next:**\n");
        for (i, t) in guard.queue.iter().enumerate().take(20) {
            msg.push_str(&format!(
                "{}. {} ({})\n",
                i + 1,
                t.title,
                format_duration(t.duration)
            ));
        }
        if guard.queue.len() > 20 {
            msg.push_str(&format!("…and {} more", guard.queue.len() - 20));
        }
    }
    ctx.reply(msg).await?;
    Ok(())
}

/// Show the currently playing track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    let player = ctx.data().music.player(guild_id);
    let guard = player.lock().await;
    let msg = match &guard.current {
        Some(np) => format!(
            "**Now playing:** {} ({}) — requested by {}",
            np.track.title,
            format_duration(np.track.duration),
            np.track.requested_by_name
        ),
        None => "Nothing playing".to_string(),
    };
    ctx.reply(msg).await?;
    Ok(())
}

/// Pause playback
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    music::pause(ctx.data(), guild_id).await?;
    ctx.reply("Paused").await?;
    Ok(())
}

/// Resume playback
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    music::resume(ctx.data(), guild_id).await?;
    ctx.reply("Resumed").await?;
    Ok(())
}

/// Skip the current track
#[poise::command(slash_command, prefix_command, guild_only, check = "is_admin")]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    music::skip(ctx.data(), guild_id).await?;
    ctx.reply("Skipped").await?;
    Ok(())
}

/// Clear the queue (current track keeps playing)
#[poise::command(slash_command, prefix_command, guild_only, check = "is_admin")]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    music::clear(ctx.data(), guild_id).await?;
    ctx.reply("Queue cleared").await?;
    Ok(())
}

/// Disconnect from voice and clear all state
#[poise::command(slash_command, prefix_command, guild_only, check = "is_admin")]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    music::leave(ctx.data(), guild_id).await?;
    ctx.reply("Left voice channel").await?;
    Ok(())
}

/// Set playback volume (0-200%). Persists across restarts.
#[poise::command(slash_command, prefix_command, guild_only, check = "is_admin")]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume percent (0-200, default 100)"] percent: u32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().context("Must be in a guild")?;
    let v = (percent.min(200) as f32) / 100.0;
    let applied = music::set_volume(ctx.data(), guild_id, v).await?;
    ctx.reply(format!(
        "Volume set to {}%",
        (applied * 100.0).round() as u32
    ))
    .await?;
    Ok(())
}
