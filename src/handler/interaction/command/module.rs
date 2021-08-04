mod action;
mod enabled;
mod exclusion;
mod setting;

use self::{
    action::ActionSubcommand, enabled::EnabledSubcommand, exclusion::ExclusionSubcommand, setting::SettingSubcommand,
};
use super::{enabled_string, respond, respond_embed, respond_success, run_subcommand, SubcommandTrait};
use crate::{
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    module::{Module, ModuleKind},
    DbPool,
};
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{
        ApplicationCommandInteractionDataOption, ApplicationCommandInteractionDataOptionValue, Interaction,
    },
};
use std::str::FromStr;
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum ModuleSubcommand {
    Enabled,
    Action,
    Setting,
    Exclusion,
}

#[async_trait]
impl SubcommandTrait for ModuleSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        match self {
            ModuleSubcommand::Enabled => run_subcommand::<EnabledSubcommand>(ctx, interact, cmd_options).await,
            ModuleSubcommand::Action => run_subcommand::<ActionSubcommand>(ctx, interact, cmd_options).await,
            ModuleSubcommand::Setting => run_subcommand::<SettingSubcommand>(ctx, interact, cmd_options).await,
            ModuleSubcommand::Exclusion => run_subcommand::<ExclusionSubcommand>(ctx, interact, cmd_options).await,
        }
    }
}

async fn resolve_optional_module(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
) -> anyhow::Result<Option<Module>> {
    let kind_option = if let Some(opt) = cmd_options.first().and_then(|opt| opt.resolved.as_ref()) {
        Result::<_, anyhow::Error>::Ok(opt)
    } else {
        return Ok(None);
    };

    let kind = kind_option
        .and_then(|opt| match opt {
            ApplicationCommandInteractionDataOptionValue::String(value) => Ok(value),
            value => Err(InternalError::ImpossibleCase(format!(
                "parsing module setting subcommand failed: invalid module value: {:?}",
                value
            ))
            .into()),
        })
        .and_then(|opt| {
            let kind = ModuleKind::from_str(opt)?;
            Ok(kind)
        })?;

    let data = ctx.data.read().await;
    let guild_id = interact.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;

    let db = data.get_userdata::<DbPool>()?.get()?;
    Ok(Some(Module::get_module_for_guild(guild_id, kind, &db)?))
}

async fn resolve_module(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
) -> anyhow::Result<Module> {
    match resolve_optional_module(ctx, interact, cmd_options).await? {
        Some(module) => Ok(module),
        None => Err(InternalError::ImpossibleCase(String::from(
            "parsing module setting subcommand failed: missing module value",
        ))
        .into()),
    }
}
