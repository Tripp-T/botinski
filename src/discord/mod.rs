use {
    crate::{
        AppState,
        discord::commands::{CommandT, ENABLED_COMMANDS},
    },
    anyhow::{Context as AnyhowContext, Result},
    serenity::{
        Client,
        all::{
            Context, CreateInteractionResponse, CreateInteractionResponseMessage, EventHandler,
            GatewayIntents, GuildId, Interaction, Message, Ready,
        },
        async_trait,
    },
    std::sync::LazyLock,
    tracing::{debug, error, info},
};

mod commands;

static INTENTS: LazyLock<GatewayIntents> = LazyLock::new(|| {
    GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
});

pub async fn main(state: AppState) -> Result<()> {
    let handler = Handler(state.clone());
    let mut client = Client::builder(&state.opts.discord_token, *INTENTS)
        .event_handler(handler)
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

struct Handler(AppState);

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            println!("Received command interaction: {command:#?}");

            let content = match ENABLED_COMMANDS
                .iter()
                .find(|c| c.name().eq_ignore_ascii_case(&command.data.name))
            {
                Some(cmd) => Some(cmd.run(&command.data.options()).await),
                None => Some("Not Implemented :(".to_string()),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new().content(content);
                let builder = CreateInteractionResponse::Message(data);
                if let Err(why) = command.create_response(&ctx.http, builder).await {
                    println!("Cannot respond to slash command: {why}");
                }
            }
        }
    }

    async fn message(&self, _: Context, _: Message) {}

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, ctx: Context, ready: Ready) {
        let primary_guild = GuildId::new(self.0.config.discord.primary_guild_id);

        let commands = primary_guild
            .set_commands(
                &ctx.http,
                ENABLED_COMMANDS
                    .iter()
                    .map(|c| c.register())
                    .collect::<Vec<_>>(),
            )
            .await;
        match commands {
            Ok(c) => debug!("Registered {} slash command(s)", c.len()),
            Err(e) => error!("Failed to register slash commands: {e}"),
        }

        info!("Connected to Discord as '{}'", ready.user.name);
        info!(
            "Invite URL: https://discord.com/oauth2/authorize?client_id={}&scope=bot&permissions={}",
            ready.application.id,
            INTENTS.bits()
        )
    }
}
