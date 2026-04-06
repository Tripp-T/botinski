use {
    crate::AppState,
    anyhow::{Context as AnyhowContext, Result},
    serenity::{
        Client,
        all::{Context, EventHandler, GatewayIntents, Message, Ready},
        async_trait,
    },
    tracing::{error, info},
};

pub async fn main(state: AppState) -> Result<()> {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&state.opts.discord_token, intents)
        .event_handler(Handler)
        .await
        .context("Failed to create discord client")?;

    let shard_manager = client.shard_manager.clone();

    let shutdown_rx = state.register_shutdown_callback().await;
    tokio::spawn(async move {
        if let Err(e) = shutdown_rx.await {
            error!("failed to receive shutdown signal: {e}")
        };
        shard_manager.shutdown_all().await;
    });

    client.start().await.context("Discord client error:")
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    //
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            // Sending a message can fail, due to a network error, an authentication error, or lack
            // of permissions to post in the channel, so log to stdout when some error happens,
            // with a description of it.
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                error!("Error sending message: {why:?}");
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}
