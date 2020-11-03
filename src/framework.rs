use crate::{
    error::{ArgumentError, InternalError},
    ext::{DurationExt, Userdata},
    module::{
        action::{Action, ActionKind},
        cache::ModuleCache,
        settings::Settings,
        Module, ModuleKind,
    },
    BotUptime, DbPool, ShardMetadata,
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
use std::{borrow::Cow, sync::Arc, time::Instant};
use structopt::{clap, StructOpt};
use strum::VariantNames;
use tokio::sync::broadcast;

pub const COMMAND_PREFIX: &str = "-ct";
const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';
const NO_ACTIONS: &str = "There aren't any actions defined for this module. Add some with the `add-action` subcommand!";

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
        // TODO: the required_ifs don't work
        #[structopt(
            possible_values(ModuleKind::VARIANTS),
            required_ifs(&[
                ("subcommand", "set-enabled"),
                ("subcommand", "get-actions"),
                ("subcommand", "add-action"),
                ("subcommand", "remove-action"),
                ("subcommand", "get-settings"),
                ("subcommand", "set-setting"),
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
    /// Displays all settings and their values for the given module
    GetSettings,
    /// Sets the value of a setting of the given module
    SetSetting {
        /// The name of the setting
        name: String,
        /// The value of the setting
        value: String,
    },
    /// Resets the value of a setting of the given module to its default value
    ResetSetting {
        /// The name of the setting
        name: String,
    },
}

pub struct CaretakerFramework {
    msg_tx: broadcast::Sender<Arc<Message>>,
}

#[async_trait]
impl Framework for CaretakerFramework {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        // straight-up ignore bot messages
        if !is_from_user(&msg) {
            return;
        }

        debug!("{:#?}", msg);
        debug!(
            "Dispatch called {}ms later from message timestamp ({})",
            (Utc::now() - msg.timestamp).num_milliseconds(),
            msg.timestamp
        );

        let channel_id = msg.channel_id;
        let start = Instant::now();
        if let Err(e) = self.process_message(&ctx, msg).await {
            warn!("Message processing failed: {}", e);

            if let Err(e) = channel_id
                .send_message(&ctx, |m| {
                    if let Some(clap::Error { message, .. }) = e.downcast_ref() {
                        codeblock_message(message, m);
                    } else if let Some(e) = e.downcast_ref::<ArgumentError>() {
                        argument_error_message(e, m);
                    } else {
                        internal_error_message(e, m);
                    }
                    m
                })
                .await
            {
                error!("Failed to send error message to channel {}: {}", channel_id, e);
            }
        }

        debug!("Message processed succesfully. Processing took {:?}", start.elapsed());
    }
}

impl CaretakerFramework {
    pub fn new(msg_tx: broadcast::Sender<Arc<Message>>) -> Self {
        Self { msg_tx }
    }

    async fn process_message(&self, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        if msg.content.starts_with(COMMAND_PREFIX) {
            self.process_management_command(ctx, msg).await
        } else {
            self.process_user_message(ctx, msg).await
        }
    }

    async fn process_management_command(&self, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        let cmd_str = msg.content.strip_prefix(COMMAND_PREFIX).ok_or_else(|| {
            InternalError::ImpossibleCase(String::from("given message content does not start with COMMAND_PREFIX"))
        })?;
        let command = Command::from_iter_safe(shellwords::split(cmd_str)?)?;

        info!(
            "{} ({}) ({:?}): {:?}",
            msg.author.tag(),
            msg.author,
            msg.guild_id,
            command
        );
        command.run(ctx, msg).await
    }

    async fn process_user_message(&self, _ctx: &Context, msg: Message) -> anyhow::Result<()> {
        // dirty short-circuit side-effect
        if msg.guild_id.is_some() && self.msg_tx.send(Arc::new(msg)).is_err() {
            error!("Sending message to broadcast channel failed (channel closed)");
        }
        Ok(())
    }
}

impl Command {
    async fn run(self, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;

        match self {
            Command::Fail => return Err(InternalError::DeliberateError.into()),
            Command::Status => {
                let shards = data.get_userdata::<ShardMetadata>()?;
                let own_shard = shards
                    .get(&ctx.shard_id)
                    .ok_or(InternalError::MissingOwnShardMetadata(ctx.shard_id))?;

                let own_uptime = own_shard.last_connected.elapsed().round_to_seconds();
                let bot_uptime = data.get_userdata::<BotUptime>()?.elapsed().round_to_seconds();

                msg.channel_id
                    .send_message(&ctx, |m| {
                        m.embed(|e| {
                            e.field("Shard", format!("{}/{}", own_shard.id + 1, shards.len()), true);
                            e.field("Guilds", format!("{}", own_shard.guilds), true);
                            e.field("Bot uptime", format!("{}", format_duration(bot_uptime)), true);
                            e.field("Shard uptime", format!("{}", format_duration(own_uptime)), true);

                            if let Some(latency) = own_shard.latency {
                                e.field("GW latency", format!("{:?}", latency), true);
                            } else {
                                e.field("GW latency", "n/a", true);
                            }

                            // the serenity docs state that `You can also pass an instance of chrono::DateTime<Utc>,
                            // which will construct the timestamp string out of it.`, but serenity itself implements the
                            // conversion only for references to datetimes, not datetimes directly
                            e.timestamp(&Utc::now())
                        })
                    })
                    .await?;
            }
            Command::Module { module, subcommand } => {
                let guild_id = msg.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;

                match (module, subcommand) {
                    (Some(module), subcommand) => {
                        let module = {
                            let db = data.get_userdata::<DbPool>()?.get()?;
                            Module::get_module_for_guild(guild_id, module, &db)?
                        };
                        subcommand.run(module, ctx, msg).await?;
                    }
                    (None, ModuleSubcommand::GetEnabled) => {
                        let modules = {
                            let db = data.get_userdata::<DbPool>()?.get()?;
                            Module::get_all_modules_for_guild(guild_id, &db)?
                        };

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
                    (m, s) => {
                        return Err(InternalError::ImpossibleCase(format!(
                            "module is {:?} and subcommand is {:?}",
                            m, s
                        ))
                        .into())
                    }
                };
            }
        };

        Ok(())
    }
}

impl ModuleSubcommand {
    async fn run(self, mut module: Module, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        let data = ctx.data.read().await;
        let db = data.get_userdata::<DbPool>()?.get()?;
        let module_cache = data.get_userdata::<ModuleCache>()?;

        match self {
            ModuleSubcommand::SetEnabled { enabled } => {
                module.set_enabled(enabled, &db)?;
                module_cache.update(module).await?;

                react_success(ctx, &msg).await?;
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
                    ActionKind::Notify => {
                        if let Some(in_channel) = in_channel {
                            let channels = module.guild().channels(ctx).await?;
                            if !channels.contains_key(&in_channel) {
                                return Err(ArgumentError::ChannelNotInGuild(in_channel).into());
                            }
                        }

                        Action::notify(
                            in_channel,
                            message.as_deref().map(Cow::Borrowed).ok_or_else(|| {
                                InternalError::ImpossibleCase(format!(
                                    "message is {:?} while ActionKind is {}",
                                    message, action
                                ))
                            })?,
                        )
                    }
                    ActionKind::RemoveMessage => Action::remove_message(),
                };

                module.add_action(&action, &db)?;
                react_success(ctx, &msg).await?;
            }
            ModuleSubcommand::RemoveAction { index } => {
                if module.action_count(&db)? == 0 {
                    msg.channel_id.send_message(ctx, |m| m.content(NO_ACTIONS)).await?;
                } else {
                    module.remove_nth_action(index, &db)?;
                    react_success(ctx, &msg).await?;
                }
            }
            ModuleSubcommand::GetSettings => {
                let settings = module.get_settings(&db)?;

                msg.channel_id
                    .send_message(ctx, |m| {
                        m.embed(|e| {
                            e.title(format!("Settings for the `{}` module", module.kind()));
                            e.fields(settings.get_all().into_iter().map(|(k, v)| {
                                (
                                    k,
                                    format!(
                                        "{}\nValue: `{}` (default: `{}`)",
                                        settings.description_for(k).unwrap(),
                                        v,
                                        settings.default_for(k).unwrap(),
                                    ),
                                    false,
                                )
                            }))
                        })
                    })
                    .await?;
            }
            ModuleSubcommand::SetSetting { name, value } => {
                let mut settings = module.get_settings(&db)?;
                settings.set(&name, &value)?;
                module.set_settings(&settings, &db)?;
                react_success(ctx, &msg).await?;
            }
            ModuleSubcommand::ResetSetting { name } => {
                let mut settings = module.get_settings(&db)?;
                settings.reset(&name)?;
                module.set_settings(&settings, &db)?;
                react_success(ctx, &msg).await?;
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

fn argument_error_message(e: &ArgumentError, m: &mut CreateMessage<'_>) {
    m.content(format!("{} {}", UNICODE_CROSS, e));
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

fn is_from_user(msg: &Message) -> bool {
    !msg.author.bot
}
