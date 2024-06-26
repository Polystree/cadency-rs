use crate::{
    error::CadencyError,
    response::{Response, ResponseBuilder},
    utils,
};
use serenity::{
    all::{Builder, GuildId},
    async_trait,
    builder::{
        CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateInteractionResponseMessage,
    },
    client::Context,
    model::application::{Command, CommandInteraction, CommandOptionType},
    prelude::TypeMapKey,
};
use std::sync::Arc;

#[macro_export]
macro_rules! setup_commands {
    ($($command_struct:expr),* $(,)*) => {
        {
            let mut commands: Vec<std::sync::Arc<dyn cadency_core::CadencyCommand>> = Vec::new();
            $(
                let command = std::sync::Arc::new($command_struct);
                commands.push(command);
            )*
            commands
        }
    };
}

pub trait CadencyCommandBaseline {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn deferred(&self) -> bool;
    fn options(&self) -> Vec<CadencyCommandOption>;
}

pub struct CadencyCommandOption {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: CommandOptionType,
    pub required: bool,
}

#[async_trait]
pub trait CadencyCommand: Sync + Send + CadencyCommandBaseline {
    /// Construct the slash command that will be submited to the discord api
    async fn register(
        &self,
        ctx: &Context,
        scope: CommandsScope,
    ) -> Result<Command, serenity::Error> {
        let command_options: Vec<CreateCommandOption> = self
            .options()
            .into_iter()
            .map(|option| {
                CreateCommandOption::new(option.kind, option.name, option.description)
                    .required(option.required)
            })
            .collect();
        let command_builder = CreateCommand::new(self.name())
            .description(self.description())
            .set_options(command_options);
        match scope {
            CommandsScope::Global => {
                Ok(Command::create_global_command(&ctx.http, command_builder).await?)
            }
            CommandsScope::Guild(guild_id) => Ok(command_builder
                .execute(&ctx.http, (Some(guild_id), None))
                .await?),
        }
    }

    async fn execute<'a>(
        &self,
        ctx: &Context,
        command: &'a mut CommandInteraction,
        response_builder: &'a mut ResponseBuilder,
    ) -> Result<Response, CadencyError>;
}

pub(crate) struct Commands;

impl TypeMapKey for Commands {
    type Value = Vec<Arc<dyn CadencyCommand>>;
}

#[derive(Default, Copy, Clone, Debug)]
pub enum CommandsScope {
    /// Global command, see <https://discord.com/developers/docs/interactions/application-commands#get-global-application-commands>
    #[default]
    Global,
    /// Guild command, see <https://discord.com/developers/docs/interactions/application-commands#create-guild-application-command>
    Guild(GuildId),
}
impl TypeMapKey for CommandsScope {
    type Value = CommandsScope;
}

/// Submit slash commands to the discord api.
/// As global commands are cached for 1 hour, the activation can take some time.
/// For local testing it is recommended to create commands with a guild scope.
pub(crate) async fn setup_commands(ctx: &Context) -> Result<(), serenity::Error> {
    let commands = utils::get_commands(ctx).await;
    let commands_scope = utils::get_commands_scope(ctx).await;

    // No need to run this in parallel as serenity will enforce one-by-one execution
    for command in &commands {
        command.register(ctx, commands_scope).await?;
    }
    Ok(())
}

pub(crate) async fn command_not_implemented(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), CadencyError> {
    error!("The following command is not known: {:?}", command);

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Unknown command"),
            ),
        )
        .await
        .map_err(|err| {
            error!("Interaction response failed: {}", err);
            CadencyError::Response
        })
}
