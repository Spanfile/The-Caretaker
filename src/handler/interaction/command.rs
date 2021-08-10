mod module;

use self::module::ModuleSubcommand;
use super::{enabled_string, respond, respond_embed, respond_success};
use crate::{
    error::{ArgumentError, InternalError},
    ext::{DurationExt, UserdataExt},
    guild_settings::GuildSettings,
    latency_counter::LatencyCounter,
    BotUptime, DbPool, ShardMetadata,
};
use chrono::Utc;
use humantime::format_duration;
use log::*;
use serenity::{
    async_trait,
    client::Context,
    model::{
        guild::Guild,
        interactions::{ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption, Interaction},
    },
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
    SetAdminRole,
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
macro_rules! optional_command_option {
    ($options:ident, $index:literal, $value_type:ident) => {
        if let Some(value) = $options.get($index).and_then(|opt| opt.resolved.as_ref()) {
            match value {
                ::serenity::model::interactions::ApplicationCommandInteractionDataOptionValue::$value_type(value) => {
                    Ok(Some(value))
                }
                value => Err($crate::error::InternalError::ImpossibleCase(format!(
                    "parsing subcommand failed: invalid value: {:?}",
                    value
                ))),
            }
        } else {
            Ok(None)
        }
    };
}

#[macro_export]
macro_rules! command_option {
    ($options:ident, $index:literal, $value_type:ident) => {
        $crate::optional_command_option!($options, $index, $value_type)?.ok_or_else(|| {
            $crate::error::InternalError::ImpossibleCase(String::from("parsing subcommand failed: missing argument"))
        })
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
            Command::SetAdminRole => set_admin_role(ctx, interact, &cmd.options).await,
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

async fn check_permission(ctx: &Context, interact: &Interaction) -> anyhow::Result<()> {
    let guild_id = interact.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;
    let member = interact.member.as_ref().ok_or(ArgumentError::NotSupportedInDM)?;

    if check_owner_permission(ctx, interact).await.is_ok() {
        debug!("Permission check ok: user {} is owner of {}", member.user.id, guild_id);
        return Ok(());
    }

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let guild_settings = GuildSettings::get_for_guild(guild_id, &db)?;
    let admin_role = guild_settings.get_admin_role();

    if let Some(true) = member
        .roles(&ctx.cache)
        .await
        .zip(admin_role)
        .map(|(roles, admin_role)| roles.iter().any(|r| r.id == admin_role))
    {
        debug!(
            "Permission check ok: user {} has admin role {:?} in {}",
            member.user.id, admin_role, guild_id
        );

        Ok(())
    } else {
        debug!(
            "Permission check failed: user {} doesn't have admin role {:?} in {}",
            member.user.id, admin_role, guild_id
        );

        Err(ArgumentError::NoPermission.into())
    }
}

async fn check_owner_permission(ctx: &Context, interact: &Interaction) -> anyhow::Result<()> {
    let guild_id = interact.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;
    let member = interact.member.as_ref().ok_or(ArgumentError::NotSupportedInDM)?;
    let guild = Guild::get(&ctx.http, guild_id).await?;

    if guild.owner_id == member.user.id {
        debug!("Permission check ok: user {} is owner of {}", member.user.id, guild_id);
        Ok(())
    } else {
        debug!(
            "Permission check ok: user {} is not the owner of {}",
            member.user.id, guild_id
        );

        Err(ArgumentError::NoPermission.into())
    }
}

async fn status_command(ctx: &Context, interact: &Interaction) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let shards = data.get_userdata::<ShardMetadata>()?;
    let latency = data.get_userdata::<LatencyCounter>()?;
    let own_shard = shards
        .get(&ctx.shard_id)
        .ok_or(InternalError::MissingOwnShardMetadata(ctx.shard_id))?;

    let own_started = own_shard.last_connected;
    let bot_started = *data.get_userdata::<BotUptime>()?;

    let own_uptime = (Utc::now() - own_started).round_to_seconds();
    let bot_uptime = (Utc::now() - bot_started).round_to_seconds();
    let total_guilds: usize = shards.values().map(|shard| shard.guilds).sum();

    let gw_latency = latency.get_gateway().await;
    let action_latency = latency.get_action().await;
    let message_latency = latency.get_message().await;

    respond_embed(ctx, interact, |e| {
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

        e.field(
            "Latencies",
            format!(
                "Avg. GW: {} ms\nAvg. msg: {} ms\nAvg. action: {} ms",
                gw_latency, action_latency, message_latency
            ),
            false,
        );

        // the serenity docs state that `You can also pass an instance of chrono::DateTime<Utc>, which will construct
        // the timestamp string out of it.`, but serenity itself implements the conversion only for references to
        // datetimes, not datetimes directly
        e.timestamp(&Utc::now())
    })
    .await
}

async fn set_admin_role(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
) -> anyhow::Result<()> {
    check_owner_permission(ctx, interact).await?;

    let admin_role = command_option!(cmd_options, 0, Role)?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;

    let mut guild_settings = GuildSettings::get_for_guild(admin_role.guild_id, &db)?;
    guild_settings.set_admin_role(admin_role.id, &db)?;

    respond_success(ctx, interact).await
}
