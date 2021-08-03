mod module_subcommand;

use self::module_subcommand::ModuleSubcommand;
use super::{enabled_string, respond, respond_embed, respond_success};
use crate::{
    error::InternalError,
    ext::{DurationExt, UserdataExt},
    BotUptime, ShardMetadata,
};
use chrono::Utc;
use humantime::format_duration;
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption, Interaction},
};
use std::str::FromStr;
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum Command {
    Status,
    Fail,
    Success,
    Module,
}

#[async_trait]
trait SubcommandTrait {
    async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()>;
}

#[macro_export]
macro_rules! command_option {
    ($options:ident, $index:literal, $value_type:ident) => {
        $options
            .get($index)
            .and_then(|opt| opt.resolved.as_ref())
            .ok_or_else(|| InternalError::ImpossibleCase(String::from("parsing subcommand failed: missing argument")))
            .and_then(|opt| match opt {
                ApplicationCommandInteractionDataOptionValue::$value_type(value) => Ok(value),
                value => Err(InternalError::ImpossibleCase(format!(
                    "parsing subcommand failed: invalid value: {:?}",
                    value
                ))),
            })?
    };
}

impl Command {
    pub async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd: &ApplicationCommandInteractionData,
    ) -> anyhow::Result<()> {
        match self {
            Command::Fail => Err(InternalError::DeliberateError.into()),
            Command::Success => super::respond_success(ctx, interact).await,
            Command::Status => status_command(ctx, interact).await,
            Command::Module => run_subcommand::<ModuleSubcommand>(ctx, interact, &cmd.options).await,
        }
    }
}

async fn run_subcommand<S>(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
) -> anyhow::Result<()>
where
    S: SubcommandTrait + FromStr,
    <S as FromStr>::Err: std::fmt::Debug,
{
    let (subcommand, sub) = cmd_options
        .first()
        .ok_or_else(|| InternalError::ImpossibleCase(String::from("parsing subcommand failed: missing subcommand")))
        .and_then(|sub| {
            S::from_str(sub.name.as_ref())
                .map(|subcommand| (subcommand, sub))
                .map_err(|e| InternalError::ImpossibleCase(format!("parsing subcommand failed: {:?}", e)))
        })?;
    subcommand.run(ctx, interact, &sub.options).await
}

async fn status_command(ctx: &Context, interact: &Interaction) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let shards = data.get_userdata::<ShardMetadata>()?;
    let own_shard = shards
        .get(&ctx.shard_id)
        .ok_or(InternalError::MissingOwnShardMetadata(ctx.shard_id))?;

    let own_started = own_shard.last_connected;
    let bot_started = *data.get_userdata::<BotUptime>()?;

    let own_uptime = (Utc::now() - own_started).round_to_seconds();
    let bot_uptime = (Utc::now() - bot_started).round_to_seconds();
    let total_guilds: usize = shards.values().map(|shard| shard.guilds).sum();

    super::respond_embed(ctx, interact, |e| {
        e.field(
            "Shard / total shards",
            format!("{} / {}", own_shard.id + 1, shards.len(),),
            true,
        );
        e.field(
            "Guilds / total guilds",
            format!("{} / {}", own_shard.guilds, total_guilds),
            true,
        );
        e.field(
            "Bot / shard uptime",
            format!(
                "{}, since <t:{}> /\n{}, since <t:{}>",
                format_duration(bot_uptime),
                bot_started.timestamp(),
                format_duration(own_uptime),
                own_started.timestamp(),
            ),
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
