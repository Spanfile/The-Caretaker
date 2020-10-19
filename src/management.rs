use crate::{error::CaretakerError, module::Module, DbConnection, ShardMetadata};
use chrono::Utc;
use log::*;
use serenity::{
    async_trait, builder::CreateMessage, client::Context, framework::Framework, model::channel::Message, utils::Colour,
};
use structopt::{clap, StructOpt};
use strum::VariantNames;

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
    /// Enable the given module
    Enable,
    /// Disable the given module
    Disable,
    /// Show the status of the given module, or if no module is given, show the status for all the modules
    Status,
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

        if let Some(command) = msg.content.strip_prefix(COMMAND_PREFIX) {
            let command = match Command::from_iter_safe(command.split_whitespace()) {
                Ok(c) => c,
                Err(clap::Error { kind, message, .. }) => {
                    warn!("structopt returned {:?} error: {}", kind, message);
                    msg.channel_id
                        .send_message(&ctx, |m| {
                            codeblock_message(&message, m);
                            m
                        })
                        .await?;
                    return Ok(());
                }
            };

            debug!("{:#?}", command);
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
        match self {
            Command::Fail => return Err(CaretakerError::DeliberateError.into()),
            Command::Status => {
                let data = ctx.data.read().await;
                match data.get::<ShardMetadata>() {
                    None => return Err(CaretakerError::NoShardMetadataCollection.into()),
                    Some(shards) => {
                        match shards.get(&ctx.shard_id) {
                            None => return Err(CaretakerError::MissingOwnShardMetadata(ctx.shard_id).into()),
                            Some(own_shard) => {
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

                                            // the serenity docs state that `You can also pass an instance of
                                            // chrono::DateTime<Utc>, which will construct the timestamp string out of
                                            // it.`, but serenity itself
                                            // implements the conversion
                                            // only for references to datetimes, not
                                            // datetimes directly
                                            e.timestamp(&Utc::now());
                                            e
                                        })
                                    })
                                    .await?;
                            }
                        }
                    }
                }
            }
            Command::Module { module, subcommand } => match (module, subcommand) {
                (Some(module), subcommand) => subcommand.run(*module, ctx, msg).await?,
                (None, ModuleSubcommand::Status) => {
                    info!("yay status");
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
            ModuleSubcommand::Enable => {
                module.set_enabled_for_guild(guild_id as i64, true, &db)?;
                react_success(ctx, msg).await?;
            }
            ModuleSubcommand::Disable => {
                module.set_enabled_for_guild(guild_id as i64, false, &db)?;
                react_success(ctx, msg).await?;
            }
            ModuleSubcommand::Status => {
                let enabled = module.get_enabled_for_guild(guild_id as i64, &db)?;
                msg.channel_id
                    .send_message(&ctx, |m| {
                        m.content(format!(
                            "The `{}` module is: {}",
                            module.to_string(),
                            if enabled {
                                format!("{} enabled", UNICODE_CHECK)
                            } else {
                                format!("{} disabled", UNICODE_CROSS)
                            }
                        ))
                    })
                    .await?;
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

fn codeblock_message(message: &str, m: &mut CreateMessage<'_>) {
    m.content(format!("```\n{}\n```", message));
}

async fn react_success(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    msg.react(ctx, UNICODE_CHECK).await?;
    Ok(())
}
