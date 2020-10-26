use crate::{
    error::{ArgumentError, InternalError},
    ext::DurationExt,
    module::{
        action::{Action, ActionKind},
        Module, ModuleKind,
    },
    BotUptime, DbConnection, ShardMetadata,
};
use chrono::Utc;
use humantime::format_duration;
use log::*;
use serenity::{
    async_trait,
    builder::CreateMessage,
    client::Context,
    framework::Framework,
    model::{channel::Message, id::ChannelId},
    utils::Colour,
};
use std::{borrow::Cow, time::Instant};
use structopt::{clap, StructOpt};
use strum::VariantNames;

pub const COMMAND_PREFIX: &str = "-ct";
const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';
const NO_ACTIONS: &str = "There aren't any actions defined for this module. Add some with the `add-action` subcommand!";

pub struct CaretakerFramework {}

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
            possible_values(ModuleKind::VARIANTS),
            required_ifs(&[
                ("subcommand", "enable"),
                ("subcommand", "disable")
            ])
        )]
        module: Option<ModuleKind>,
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
    ///
    /// The actions aren't dependent on each other and will run in parallel, so their exact order doesn't matter. The
    /// same kind of action can exist multiple times, even with the same parameters, with the exception of the
    /// `remove-message`-action.
    GetActions,
    /// Adds a new action to the given module
    ///
    /// The action may have additional required parameters. See their help with `add-action help <action>`. The same
    /// kind of action can be added multiple times, even with the same parameters as an already existing action, with
    /// the exception of the `remove-message`-action.
    AddAction {
        /// The action to add
        action: ActionKind,
        /// The message to send, if applicable.
        #[structopt(required_if("action", "notify"))]
        message: Option<String>,
        /// The channel to send the message to, if applicable.
        #[structopt(long = "in")]
        in_channel: Option<ChannelId>,
    },
    /// Removes a given action from the module based on its index. Use the `list-actions` subcommand to see the action
    /// indices
    RemoveAction {
        /// The index of the action to remove
        index: usize,
    },
}

#[async_trait]
impl Framework for CaretakerFramework {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        // straight-up ignore bot messages
        if !self.is_from_user(&msg) {
            return;
        }

        debug!("{:#?}", msg);

        let start = Instant::now();
        match self.process_message(&ctx, &msg).await {
            Ok(_) => debug!("Message processed succesfully. Processing took {:?}", start.elapsed()),
            Err(err) => {
                error!(
                    "Message processing failed: {} (processing took {:?})",
                    err,
                    start.elapsed()
                );

                if err.downcast_ref::<ArgumentError>().is_some() {
                    if let Err(e) = msg
                        .channel_id
                        .send_message(&ctx, |m| {
                            argument_error_message(err, m);
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
}

impl CaretakerFramework {
    pub fn new() -> Self {
        Self {}
    }

    async fn process_message(&self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        let guild = if let Some(guild) = msg.guild(ctx).await {
            guild
        } else {
            // no guild: the message was probably a DM
            return Ok(());
        };

        if let Some(command) = msg.content.strip_prefix(COMMAND_PREFIX) {
            let command = match Command::from_iter_safe(shellwords::split(command)?) {
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
    async fn run(self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;

        match self {
            Command::Fail => return Err(InternalError::DeliberateError.into()),
            Command::Status => {
                let shards = data
                    .get::<ShardMetadata>()
                    .ok_or(InternalError::MissingUserdata("ShardMetadata"))?;
                let own_shard = shards
                    .get(&ctx.shard_id)
                    .ok_or(InternalError::MissingOwnShardMetadata(ctx.shard_id))?;

                let own_uptime = own_shard.last_connected.elapsed().round_to_seconds();
                let bot_uptime = data
                    .get::<BotUptime>()
                    .ok_or(InternalError::MissingUserdata("BotUptime"))?
                    .elapsed()
                    .round_to_seconds();

                msg.channel_id
                    .send_message(&ctx, |m| {
                        m.embed(|e| {
                            e.field("Shard", format!("{}/{}", own_shard.id + 1, shards.len()), true);
                            e.field("Guilds", format!("{}", own_shard.guilds), true);
                            e.field("Bot uptime", format!("{}", format_duration(bot_uptime)), true);
                            e.field("Shard uptime", format!("{}", format_duration(own_uptime)), true);

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
            Command::Module { module, subcommand } => {
                let guild_id = msg.guild_id.ok_or(InternalError::NoGuildId)?;
                let db = data
                    .get::<DbConnection>()
                    .ok_or(InternalError::MissingUserdata("DbConnection"))?
                    .lock()
                    .await;

                match (module, subcommand) {
                    (Some(module), subcommand) => {
                        let module = Module::get_module_for_guild(guild_id, module, &db)?;
                        // drop our lock to the db so the subcommand can retrieve its own lock to it when it needs.
                        // do this instead of just passing the connection as a reference, since the *reference* cannot
                        // be held between await points
                        drop(db);
                        subcommand.run(module, ctx, msg).await?;
                    }
                    (None, ModuleSubcommand::GetEnabled) => {
                        let modules = Module::get_all_modules_for_guild(guild_id, &db)?;

                        msg.channel_id
                            .send_message(&ctx, |m| {
                                m.embed(|e| {
                                    e.title("Status of all modules");

                                    for (kind, module) in modules {
                                        e.field(kind, enabled_string(module.enabled()), true);
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
                };
            }
        };

        Ok(())
    }
}

impl ModuleSubcommand {
    async fn run(self, module: Module, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;
        let db = data
            .get::<DbConnection>()
            .ok_or(InternalError::MissingUserdata("DbConnection"))?
            .lock()
            .await;

        match self {
            ModuleSubcommand::SetEnabled { enabled } => {
                module.set_enabled(enabled, &db)?;
                react_success(ctx, msg).await?;
            }
            ModuleSubcommand::GetEnabled => {
                msg.channel_id
                    .send_message(ctx, |m| {
                        m.content(format!(
                            "The `{}` module is: {}",
                            module.kind(),
                            enabled_string(module.enabled())
                        ))
                    })
                    .await?;
            }
            ModuleSubcommand::GetActions => {
                let actions = module.get_actions(&db)?;
                msg.channel_id
                    .send_message(ctx, |m| {
                        if actions.is_empty() {
                            m.content(NO_ACTIONS)
                        } else {
                            m.embed(|e| {
                                e.title(format!("Actions for the `{}` module", module.kind()));

                                for (idx, action) in actions.into_iter().enumerate() {
                                    let name = format!("{}: {}", idx, action.friendly_name());
                                    e.field(name, action.description(), false);
                                }

                                e
                            })
                        }
                    })
                    .await?;
            }
            ModuleSubcommand::AddAction {
                action,
                in_channel,
                message,
            } => {
                let action = match action {
                    ActionKind::Notify { .. } => Action::notify(
                        in_channel,
                        message
                            .as_deref()
                            .map(Cow::Borrowed)
                            .expect("message is None while ActionKind is Notify. this shouldn't happen"),
                    ),
                    ActionKind::RemoveMessage => Action::remove_message(),
                };

                module.add_action(action, &db)?;
                react_success(ctx, msg).await?;
            }
            ModuleSubcommand::RemoveAction { index } => {
                if module.action_count(&db)? == 0 {
                    msg.channel_id.send_message(ctx, |m| m.content(NO_ACTIONS)).await?;
                } else {
                    module.remove_nth_action(index, &db)?;
                    react_success(ctx, msg).await?;
                }
            }
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

fn argument_error_message<E>(err: E, m: &mut CreateMessage<'_>)
where
    E: AsRef<dyn std::error::Error>,
{
    m.content(format!("{} {}", UNICODE_CROSS, err.as_ref()));
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
