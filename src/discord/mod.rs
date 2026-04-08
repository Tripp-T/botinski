use {
    crate::{AppState, Opts},
    anyhow::{Context as AnyhowContext, Result},
    poise::{PrefixFrameworkOptions, serenity_prelude::*},
    std::sync::{Arc, LazyLock},
    tracing::{debug, info},
};

mod commands;

static INTENTS: LazyLock<GatewayIntents> = LazyLock::new(|| {
    GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::non_privileged()
});

type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, AppState, Error>;

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
                vec![age(), ping(), shutdown()]
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
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
        .await
        .context("Failed to create discord client")?;

    let shard_manager = client.shard_manager.clone();
    let shutdown_token = state.shutdown_token.clone();
    tokio::spawn(async move {
        shutdown_token.cancelled().await;
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
                INTENTS.bits()
            );
            Ok(())
        }
        // Unhandled
        _ => Ok(()),
    }
}

async fn is_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let config = &ctx.data().config.read().await.discord;

    let author = ctx.author();

    if config
        .admin_uids
        .as_ref()
        .is_some_and(|a| a.contains(&author.id))
    {
        // is listed in the admin user ids
        return Ok(true);
    }

    let Some(admin_role_ids) = config.admin_roles.as_ref() else {
        // no admin roles to check against
        ctx.reply("Permission denied")
            .await
            .context("Failed to reply")?;
        return Ok(false);
    };
    let Some(guild) = ctx.guild().map(|g| g.id) else {
        // is not in admin list and not in a guild to cross-check roles against
        ctx.reply("Permission denied")
            .await
            .context("Failed to reply")?;
        return Ok(false);
    };

    let author_membership = guild
        .member(&ctx, author.id)
        .await
        .context("Failed to fetch author guild membership")?;

    if author_membership
        .roles
        .iter()
        .any(|rid| admin_role_ids.contains(rid))
    {
        return Ok(true);
    }

    ctx.reply("Permission denied")
        .await
        .context("Failed to reply")?;
    Ok(false)
}
