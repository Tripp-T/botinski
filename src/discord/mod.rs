use crate::{AppState, Opts};
use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::{PrefixFrameworkOptions, serenity_prelude::*};
use songbird::serenity::SerenityInit;
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

mod commands;
mod music_commands;

static INTENTS: LazyLock<GatewayIntents> = LazyLock::new(|| {
    GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::non_privileged()
});

type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, AppState, Error>;

pub struct DiscordHttpCache {
    pub http: Arc<poise::serenity_prelude::Http>,
    pub cache: Arc<poise::serenity_prelude::Cache>,
}
impl CacheHttp for DiscordHttpCache {
    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.cache)
    }
    fn http(&self) -> &Http {
        &self.http
    }
}

pub async fn main(state: AppState, opts: Arc<Opts>) -> Result<()> {
    let framework_state = state.clone();
    let discord_skip_register_commands = Arc::new(opts.discord_skip_register_commands);
    let discord_token = opts.discord_token.clone();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: PrefixFrameworkOptions {
                prefix: Some(
                    state
                        .config
                        .read()
                        .await
                        .discord
                        .command_prefix
                        .clone()
                        .to_string(),
                ),
                ..Default::default()
            },
            commands: {
                use commands::*;
                use music_commands::*;
                vec![
                    age(),
                    ping(),
                    shutdown(),
                    roll(),
                    coin_flip(),
                    play(),
                    skip(),
                    queue(),
                    pause(),
                    resume(),
                    clear(),
                    leave(),
                    nowplaying(),
                    volume(),
                ]
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                if framework_state.discord_http.get().is_none() {
                    framework_state
                        .discord_http
                        .set(DiscordHttpCache {
                            http: ctx.http.clone(),
                            cache: ctx.cache.clone(),
                        })
                        .map_err(|e| anyhow!("Failed to share discord http cache: {e}"))?;
                }
                if !discord_skip_register_commands.as_ref() {
                    poise::builtins::register_globally(ctx, &framework.options().commands)
                        .await
                        .context("failed to register commands")?;
                    debug!(
                        "Registered {} slash commands",
                        framework.options().commands.len()
                    );
                }
                Ok(framework_state)
            })
        })
        .build();

    let mut client = Client::builder(&discord_token, *INTENTS)
        .framework(framework)
        .register_songbird_with(state.music.songbird())
        .await
        .context("Failed to create discord client")?;

    let shard_manager = client.shard_manager.clone();
    let shutdown_token = state.shutdown_token.clone();
    tokio::spawn(async move {
        use tokio::time;
        let start = time::Instant::now();
        shutdown_token.cancelled().await;
        // ensure some time has elapsed since the function starts to allow the function caller to properly receive the shutdown signal
        let min_duration = time::Duration::from_millis(500);
        let elapsed = start.elapsed();
        if elapsed < min_duration {
            time::sleep(min_duration - elapsed).await
        }
        debug!("Received shutdown event");
        shard_manager.shutdown_all().await;
    });

    client.start().await.context("client error")
}

async fn event_handler(
    _ctx: &poise::serenity_prelude::Context,
    event: &FullEvent,
    _framework: poise::FrameworkContext<'_, AppState, Error>,
    _state: &AppState,
) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot, .. } => {
            info!("Connected to Discord as '{}'", data_about_bot.user.name);
            info!(
                "Invite URL: https://discord.com/oauth2/authorize?client_id={}&scope=bot&permissions={}",
                data_about_bot.application.id,
                (Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES).bits()
            );
            Ok(())
        }
        // Unhandled
        _ => Ok(()),
    }
}

async fn is_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let data = ctx.data();
    let config = &data.config.read().await.discord;

    let author = ctx.author();

    if config
        .admin_uids
        .as_ref()
        .is_some_and(|a| a.contains(&author.id))
    {
        return Ok(true);
    }

    let member_role_ids: Vec<RoleId> = match ctx.author_member().await {
        Some(m) => m.roles.to_vec(),
        None => Vec::new(),
    };

    if let Some(admin_role_ids) = config.admin_roles.as_ref()
        && member_role_ids.iter().any(|r| admin_role_ids.contains(r))
    {
        return Ok(true);
    }

    if let Some(guild_id) = ctx.guild_id() {
        let settings = crate::models::guild_settings::GuildSettings::get(&data.db, guild_id)
            .await
            .context("Failed to load guild settings for admin check")?
            .unwrap_or_default();
        if !settings.admin_role_ids.is_empty()
            && member_role_ids
                .iter()
                .any(|r| settings.admin_role_ids.contains(r))
        {
            return Ok(true);
        }
    }

    ctx.reply("Permission denied")
        .await
        .context("Failed to reply")?;
    Ok(false)
}
