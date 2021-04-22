use super::module_subcommand::ModuleSubcommand;
use crate::{
    error::{ArgumentError, InternalError},
    ext::{DurationExt, UserdataExt},
    module::{Module, ModuleKind},
    BotUptime, DbPool, ShardMetadata,
};
use chrono::Utc;
use humantime::format_duration;
use serenity::{client::Context, model::channel::Message};
use structopt::{clap, StructOpt};
use strum::VariantNames;

#[derive(StructOpt, Debug)]
#[structopt(
    global_settings(&[clap::AppSettings::NoBinaryName,
        clap::AppSettings::DisableHelpFlags,
        clap::AppSettings::DisableVersion,
        clap::AppSettings::AllowLeadingHyphen]),
    set_term_width(0),
    name = super::DEFAULT_COMMAND_PREFIX,
    no_version
)]
pub enum Command {
    /// Prints status about the current shard and the shards as a whole
    Status,
    /// Deliberately returns an error
    Fail,
    /// Reacts to the message with a success emoji
    Success,
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
    /// Sets Caretaker's command prefix
    SetPrefix {
        /// The custom prefix
        // leading hyphens are allowed on this value through the AllowLeadingHyphen clap setting
        prefix: String,
    },
    /// Resets Caretaker's command prefix to the default
    ResetPrefix,
}

impl Command {
    pub async fn run(self, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        match self {
            Command::Fail => Err(InternalError::DeliberateError.into()),
            Command::Success => super::react_success(ctx, &msg).await,
            Command::Status => status_command(ctx, msg).await,
            Command::Module { module, subcommand } => module_command(module, subcommand, ctx, msg).await,
            Command::SetPrefix { prefix } => {
                let guild_id = msg.guild_id.ok_or(InternalError::MissingGuildID)?;
                super::set_guild_prefix(ctx, guild_id, Some(prefix)).await?;
                super::react_success(ctx, &msg).await
            }
            // TODO: doesn't actually set the database column to NULL
            Command::ResetPrefix => {
                let guild_id = msg.guild_id.ok_or(InternalError::MissingGuildID)?;
                super::set_guild_prefix(ctx, guild_id, None).await?;
                super::react_success(ctx, &msg).await
            }
        }
    }
}

async fn status_command(ctx: &Context, msg: Message) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let shards = data.get_userdata::<ShardMetadata>()?;
    let own_shard = shards
        .get(&ctx.shard_id)
        .ok_or(InternalError::MissingOwnShardMetadata(ctx.shard_id))?;

    let own_uptime = own_shard.last_connected.elapsed().round_to_seconds();
    let bot_uptime = data.get_userdata::<BotUptime>()?.elapsed().round_to_seconds();

    super::respond_embed(ctx, &msg, |e| {
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
    .await
}

async fn module_command(
    module: Option<ModuleKind>,
    subcommand: ModuleSubcommand,
    ctx: &Context,
    msg: Message,
) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let guild_id = msg.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;

    match (module, subcommand) {
        (Some(module), subcommand) => {
            let module = {
                let db = data.get_userdata::<DbPool>()?.get()?;
                Module::get_module_for_guild(guild_id, module, &db)?
            };
            subcommand.run(module, ctx, msg).await
        }
        (None, ModuleSubcommand::GetEnabled) => {
            let modules = {
                let db = data.get_userdata::<DbPool>()?.get()?;
                Module::get_all_modules_for_guild(guild_id, &db)?
            };

            super::respond_embed(ctx, &msg, |e| {
                e.title("Status of all modules");

                for (kind, module) in modules {
                    e.field(kind, super::enabled_string(module.is_enabled()), true);
                }
                e
            })
            .await
        }
        (m, s) => Err(InternalError::ImpossibleCase(format!("module is {:?} and subcommand is {:?}", m, s)).into()),
    }
}
