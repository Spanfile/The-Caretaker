use super::{resolve_module, respond, respond_embed, respond_success, SubcommandTrait};
use crate::{
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    module::{exclusion::Exclusion, Module},
    DbPool,
};
use serenity::{
    async_trait,
    client::Context,
    model::{
        interactions::{ApplicationCommandInteractionDataOption, Interaction},
        prelude::*,
    },
};
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum ExclusionSubcommand {
    Get,
    Add,
    Remove,
}

const NO_EXCLUSIONS: &str =
    "There aren't any exclusions defined for this module. They can be added with the `/module exclusion add` command.";

#[async_trait]
impl SubcommandTrait for ExclusionSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        let module = resolve_module(ctx, interact, cmd_options).await?;

        match self {
            ExclusionSubcommand::Get => get_exclusion(ctx, interact, module).await,
            ExclusionSubcommand::Add => add_exclusion(ctx, interact, cmd_options, module).await,
            ExclusionSubcommand::Remove => remove_exclusion(ctx, interact, cmd_options, module).await,
        }
    }
}

async fn get_exclusion(ctx: &Context, interact: &Interaction, module: Module) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let exclusions = module.get_exclusions(&db)?;

    if exclusions.is_empty() {
        respond(ctx, interact, |m| m.content(NO_EXCLUSIONS)).await
    } else {
        let mut exclusion_names = Vec::new();
        for excl in exclusions.iter() {
            let name_and_id = match excl {
                Exclusion::User(id) => {
                    let user = id.to_user(ctx).await?;
                    (format!("User: {}", user.tag()), id.0)
                }
                Exclusion::Role(id) => (
                    id.to_role_cached(&ctx.cache)
                        .await
                        .map_or_else(|| format!("Unknown role: {}", id.0), |r| format!("Role: {}", r.name)),
                    id.0,
                ),
            };
            exclusion_names.push(name_and_id);
        }

        respond_embed(ctx, interact, |e| {
            e.title(format!("Exclusions for the `{}` module", module.kind()));

            for (name, id) in exclusion_names.iter() {
                e.field(name, id.to_string(), false);
            }

            e
        })
        .await
    }
}

async fn add_exclusion(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    // have to do the option retrieval by hand since the command_option! macro only does one kind of option, not two
    let excl = cmd_options
        .get(1)
        .and_then(|opt| opt.resolved.as_ref())
        .map(|value| match value {
            ApplicationCommandInteractionDataOptionValue::User(user, _) => Ok(Exclusion::User(user.id)),
            ApplicationCommandInteractionDataOptionValue::Role(role) => Ok(Exclusion::Role(role.id)),
            value => Err(InternalError::ImpossibleCase(format!(
                "parsing subcommand failed: invalid value: {:?}",
                value
            ))),
        })
        .transpose()?
        .ok_or_else(|| InternalError::ImpossibleCase(String::from("parsing subcommand failed: missing argument")))?;

    // TODO: limit on how many exclusions can be added

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let exclusions = module.get_exclusions(&db)?;

    if exclusions.contains(excl) {
        return Err(ArgumentError::ExclusionAlreadyExists.into());
    }

    module.add_exclusion(excl, &db)?;
    respond_success(ctx, interact).await
}

async fn remove_exclusion(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    unimplemented!()
}
