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
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
        ApplicationCommandInteractionDataOptionValue,
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
const MAX_EXCLUSIONS: usize = 10;

#[async_trait]
impl SubcommandTrait for ExclusionSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &ApplicationCommandInteraction,
        options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        let module = resolve_module(ctx, interact, options).await?;

        match self {
            ExclusionSubcommand::Get => get_exclusion(ctx, interact, module).await,
            ExclusionSubcommand::Add => add_exclusion(ctx, interact, options, module).await,
            ExclusionSubcommand::Remove => remove_exclusion(ctx, interact, options, module).await,
        }
    }
}

async fn get_exclusion(ctx: &Context, interact: &ApplicationCommandInteraction, module: Module) -> anyhow::Result<()> {
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
            e.title(format!(
                "Exclusions for the `{}` module ({} out of {})",
                module.kind(),
                exclusions.len(),
                MAX_EXCLUSIONS
            ));

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
    interact: &ApplicationCommandInteraction,
    options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    let excl = get_exclusion_option(options)?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let exclusions = module.get_exclusions(&db)?;

    if exclusions.len() >= MAX_EXCLUSIONS {
        Err(ArgumentError::ExclusionLimit(exclusions.len(), MAX_EXCLUSIONS).into())
    } else if exclusions.contains(excl) {
        Err(ArgumentError::ExclusionAlreadyExists.into())
    } else {
        module.add_exclusion(excl, &db)?;
        respond_success(ctx, interact).await
    }
}

async fn remove_exclusion(
    ctx: &Context,
    interact: &ApplicationCommandInteraction,
    options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    let excl = get_exclusion_option(options)?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let exclusions = module.get_exclusions(&db)?;

    if exclusions.contains(excl) {
        module.remove_exclusion(excl, &db)?;
        respond_success(ctx, interact).await
    } else {
        Err(ArgumentError::NoSuchExclusion.into())
    }
}

fn get_exclusion_option(options: &[ApplicationCommandInteractionDataOption]) -> Result<Exclusion, InternalError> {
    // have to do the option retrieval by hand since the command_option! macro only does one kind of option, not two
    options
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
        .ok_or_else(|| InternalError::ImpossibleCase(String::from("parsing subcommand failed: missing argument")))
}
