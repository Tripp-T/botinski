use {
    enum_dispatch::enum_dispatch,
    serenity::all::{CreateCommand, ResolvedOption},
    std::{fmt::Debug, sync::LazyLock},
    strum::EnumIter,
};

pub static ENABLED_COMMANDS: LazyLock<[Commands; 1]> = LazyLock::new(|| [Commands::Ping(Ping)]);

#[enum_dispatch(CommandT)]
#[derive(Debug, EnumIter)]
pub enum Commands {
    Ping(Ping),
}

#[enum_dispatch]
pub trait CommandT {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn register(&self) -> CreateCommand;
    async fn run(&self, _options: &[ResolvedOption<'_>]) -> String;
}

#[derive(Debug, Default)]
pub struct Ping;
impl CommandT for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }
    fn description(&self) -> &'static str {
        "Pong!"
    }
    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name()).description(self.description())
    }
    async fn run(&self, _options: &[ResolvedOption<'_>]) -> String {
        "Pong!".into()
    }
}
