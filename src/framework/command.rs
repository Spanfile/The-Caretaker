use super::module_subcommand::ModuleSubcommand;
use crate::{
    error::{ArgumentError, InternalError},
    ext::{DurationExt, UserdataExt},
    guild_settings::GuildSettings,
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
        clap::AppSettings::DisableHelpSubcommand,
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
            Command::SetPrefix { prefix } => update_guild_prefix_command(ctx, msg, Some(prefix)).await,
            Command::ResetPrefix => update_guild_prefix_command(ctx, msg, None).await,
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
    let bot_uptime = (Utc::now() - *data.get_userdata::<BotUptime>()?).round_to_seconds();
    let total_guilds: usize = shards.values().map(|shard| shard.guilds).sum();

    super::respond_embed(ctx, &msg, |e| {
        e.field(
            "Shard / total shards",
            format!("{}/{}", own_shard.id + 1, shards.len(),),
            true,
        );
        e.field(
            "Guilds / total guilds",
            format!("{}/{}", own_shard.guilds, total_guilds),
            true,
        );
        e.field(
            "Bot / shard uptime",
            format!("{} / {}", format_duration(bot_uptime), format_duration(own_uptime)),
            false,
        );

        let mut latencies = String::new();
        if let Some(latency) = own_shard.latency {
            latencies += &format!("GW: {} ms\n", latency.as_millis());
        } else {
            latencies += "GW: n/a\n"
        }

        e.field("Latencies", latencies, false);

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

async fn update_guild_prefix_command(ctx: &Context, msg: Message, pfx: Option<String>) -> anyhow::Result<()> {
    let guild_id = msg.guild_id.ok_or(InternalError::MissingGuildID)?;
    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;

    let mut settings = GuildSettings::get_for_guild(guild_id, &db)?;
    settings.set_guild_prefix(pfx, &db)?;

    super::react_success(ctx, &msg).await
}
