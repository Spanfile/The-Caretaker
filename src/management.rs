use crate::{
    error::CaretakerError,
    module::{action::Action, Module},
    DbConnection, ShardMetadata,
};
use chrono::Utc;
use log::*;
use serenity::{
    async_trait,
    builder::CreateMessage,
    client::Context,
    framework::Framework,
    model::{channel::Message, id::ChannelId},
    utils::Colour,
};
use structopt::{clap, StructOpt};
use strum::{EnumMessage, VariantNames};

pub const COMMAND_PREFIX: &str = "-ct";
const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub struct Management {}

#[derive(StructOpt, Debug)]
#[structopt(
    global_settings(&[clap::AppSettings::NoBinaryName,
        clap::AppSettings::DisableHelpFlags,
        clap::AppSettings::DisableVersion]),
    set_term_width(0),
    name = COMMAND_PREFIX,
    no_version
)]
enum Command {
    /// Prints status about the current shard and the shards as a whole
    Status,
    /// Deliberately returns an error
    Fail,
    /// Configure the various Caretaker modules
    Module {
        /// The name of the module to configure
        #[structopt(
            possible_values(Module::VARIANTS),
            required_ifs(&[
                ("subcommand", "enable"),
                ("subcommand", "disable")
            ])
        )]
        module: Option<Module>,
        #[structopt(subcommand)]
        subcommand: ModuleSubcommand,
    },
}

#[derive(StructOpt, Debug)]
#[structopt(no_version)]
enum ModuleSubcommand {
    /// Enables or disables the given module
    SetEnabled {
        /// Boolean value indicating if the module is enabled or not
        #[structopt(parse(try_from_str))]
        enabled: bool,
    },
    /// Show the enabled status of the given module, or if no module is given, show the enabled statuses for all the
    /// modules
    GetEnabled,
    /// Shows all actions associated with the given module
    ListActions,
    /// Adds a new action to the given module
    AddAction {
        action: Action,
        channel: ChannelId,
        message: String,
    },
    /// Removes a given action from the module based on its index. Use the `list-actions` subcommand to see the action
    /// indices
    RemoveAction {
        /// The index of the action to remove
        index: usize,
    },
}

#[async_trait]
impl Framework for Management {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        info!("Dispatch called: '{:?}' by {}", msg.content, msg.author);
        debug!("{:#?}", msg);

        match self.process_message(&ctx, &msg).await {
            Ok(_) => debug!("Message processed succesfully"),
            Err(err) => {
                warn!("Message processing failed: {}", err);
                if let Err(e) = msg
                    .channel_id
                    .send_message(&ctx, |m| {
                        internal_error_message(err, m);
                        m
                    })
                    .await
                {
                    error!("Failed to send error message to channel {}: {}", msg.channel_id, e);
                }
            }
        }
    }
}

impl Management {
    pub fn new() -> Self {
        Self {}
    }

    async fn process_message(&self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        if !self.is_from_user(msg) {
            return Ok(());
        }

        let guild = if let Some(guild) = msg.guild(ctx).await {
            guild
        } else {
            // no guild: the message was probably a DM
            return Ok(());
        };

        if let Some(command) = msg.content.strip_prefix(COMMAND_PREFIX) {
            let command = match Command::from_iter_safe(command.split_whitespace()) {
                Ok(c) => c,
                Err(clap::Error { kind, message, .. }) => {
                    warn!("structopt returned {:?} error: {:?}", kind, message);
                    msg.channel_id
                        .send_message(&ctx, |m| {
                            codeblock_message(&message, m);
                            m
                        })
                        .await?;
                    return Ok(());
                }
            };

            info!(
                "{} ({}) in '{}' ({:?}): {:?}",
                msg.author.tag(),
                msg.author,
                guild.name,
                guild.id,
                command
            );
            command.run(ctx, msg).await?;
        }

        Ok(())
    }

    fn is_from_user(&self, msg: &Message) -> bool {
        !msg.author.bot
    }
}

impl Command {
    async fn run(&self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;

        match self {
            Command::Fail => return Err(CaretakerError::DeliberateError.into()),
            Command::Status => {
                let shards = data
                    .get::<ShardMetadata>()
                    .ok_or(CaretakerError::NoShardMetadataCollection)?;
                let own_shard = shards
                    .get(&ctx.shard_id)
                    .ok_or(CaretakerError::MissingOwnShardMetadata(ctx.shard_id))?;

                msg.channel_id
                    .send_message(&ctx, |m| {
                        m.embed(|e| {
                            e.field("Shard", format!("{}/{}", own_shard.id + 1, shards.len()), true);
                            e.field("Guilds", format!("{}", own_shard.guilds), true);

                            if let Some(latency) = own_shard.latency {
                                e.field(
                                    "GW latency",
                                    format!("{}ms", latency.as_micros() as f32 / 1000f32),
                                    true,
                                );
                            } else {
                                e.field("GW latency", "n/a", true);
                            }

                            // the serenity docs state that `You can also pass an instance of chrono::DateTime<Utc>,
                            // which will construct the timestamp string out of it.`, but serenity itself implements the
                            // conversion only for references to datetimes, not datetimes directly
                            e.timestamp(&Utc::now());
                            e
                        })
                    })
                    .await?;
            }
            Command::Module { module, subcommand } => match (module, subcommand) {
                (Some(module), subcommand) => subcommand.run(*module, ctx, msg).await?,
                (None, ModuleSubcommand::GetEnabled) => {
                    let guild_id = msg.guild_id.ok_or(CaretakerError::NoGuildId)?.0;
                    let db = data
                        .get::<DbConnection>()
                        .ok_or(CaretakerError::NoDatabaseConnection)?
                        .lock()
                        .await;
                    let modules = Module::get_all_modules_for_guild(guild_id as i64, &db)?;

                    msg.channel_id
                        .send_message(&ctx, |m| {
                            m.embed(|e| {
                                e.title("Status of all modules");

                                for (module, enabled) in modules {
                                    e.field(module, enabled_string(enabled), true);
                                }
                                e
                            })
                        })
                        .await?;
                }
                (m, s) => panic!(
                    "impossible case while running module subcommand: module is {:?} and subcommand is {:?}",
                    m, s
                ),
            },
        };

        Ok(())
    }
}

impl ModuleSubcommand {
    async fn run(&self, module: Module, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;
        let db = data
            .get::<DbConnection>()
            .ok_or(CaretakerError::NoDatabaseConnection)?
            .lock()
            .await;
        let guild_id = msg.guild_id.ok_or(CaretakerError::NoGuildId)?.0;

        match self {
            ModuleSubcommand::SetEnabled { enabled } => {
                module.set_enabled_for_guild(guild_id as i64, *enabled, &db)?;
                react_success(ctx, msg).await?;
            }
            ModuleSubcommand::GetEnabled => {
                let enabled = module.get_enabled_for_guild(guild_id as i64, &db)?;
                msg.channel_id
                    .send_message(ctx, |m| {
                        m.content(format!(
                            "The `{}` module is: {}",
                            module.to_string(),
                            enabled_string(enabled)
                        ))
                    })
                    .await?;
            }
            ModuleSubcommand::ListActions => {
                let actions = module.get_actions_for_guild(guild_id as i64, &db)?;
                msg.channel_id
                    .send_message(ctx, |m| {
                        if actions.is_empty() {
                            m.content(
                                "There aren't any actions defined for this module. Add some with the `add-action` \
                                 subcommand!",
                            )
                        } else {
                            m.embed(|e| {
                                for (idx, action) in actions.iter().enumerate() {
                                    let name = format!(
                                        "{}: {}",
                                        idx,
                                        action.get_message().expect("missing message for action")
                                    );

                                    match action {
                                        Action::NotifyUser { message } => {
                                            e.field(name, format!("With: '{}'", message), false)
                                        }
                                        Action::NotifyIn { channel, message } => {
                                            e.field(name, format!("In {}, with: '{}'", channel, message), false)
                                        }
                                        Action::RemoveMessage => {
                                            // Discord requires the embed field to always have *some* value but they
                                            // don't document the requirement anywhere. omitting the value has Discord
                                            // respond with a very unhelpful error message that Serenity can't do
                                            // anything with, other than complain about invalid JSON
                                            e.field(name, "Remove the message, nothing special about it", false)
                                        }
                                    };
                                }

                                e
                            })
                        }
                    })
                    .await?;
            }
            ModuleSubcommand::AddAction {
                action,
                channel,
                message,
            } => {}
            ModuleSubcommand::RemoveAction { index } => {}
        }

        Ok(())
    }
}

fn internal_error_message<E>(err: E, m: &mut CreateMessage<'_>)
where
    E: AsRef<dyn std::error::Error>,
{
    m.embed(|e| {
        e.title("An internal error has occurred")
            .description(format!("```\n{}\n```", err.as_ref()))
            .timestamp(&Utc::now())
            .colour(Colour::RED)
    });
}

fn codeblock_message(message: &str, m: &mut CreateMessage<'_>) {
    m.content(format!("```\n{}\n```", message));
}

async fn react_success(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    msg.react(ctx, UNICODE_CHECK).await?;
    Ok(())
}

fn enabled_string(enabled: bool) -> String {
    if enabled {
        format!("{} enabled", UNICODE_CHECK)
    } else {
        format!("{} disabled", UNICODE_CROSS)
    }
}
