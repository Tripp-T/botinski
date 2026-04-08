use super::{Context, Error, is_admin};
use anyhow::Context as _;
use poise::serenity_prelude as serenity;

/// Displays your or another user's account creation date
#[poise::command(slash_command, prefix_command)]
pub async fn age(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or_else(|| ctx.author());
    let response = format!("{}'s account was created at {}", u.name, u.created_at());
    ctx.say(response).await.context("Failed to respond")?;
    Ok(())
}

/// Replies to your message with a pong :)
#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong!").await.context("Failed to respond")?;
    Ok(())
}

/// Shuts down the server
#[poise::command(slash_command, prefix_command, check = "is_admin")]
pub async fn shutdown(ctx: Context<'_>) -> Result<(), Error> {
    // if this errors, skip aborting this function until shutdown signal is sent
    let res = ctx
        .reply("o7")
        .await
        .map(|_| ())
        .context("failed to send shutdown advisement message");
    ctx.data().shutdown().await?;
    res
}
