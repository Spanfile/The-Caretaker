use super::{resolve_module, respond, respond_embed, respond_success, SubcommandTrait};
use crate::{
    command_option,
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    module::{
        action::{Action, ActionKind},
        Module,
    },
    optional_command_option, DbPool,
};
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{ApplicationCommandInteractionDataOption, Interaction},
};
use std::{borrow::Cow, convert::TryInto, str::FromStr};
use strum::EnumString;

const NO_ACTIONS: &str =
    "There aren't any actions defined for this module. Add some with the `/module action add` command!";

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum ActionSubcommand {
    Get,
    Add,
    Remove,
}

#[async_trait]
impl SubcommandTrait for ActionSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        let module = resolve_module(ctx, interact, cmd_options).await?;

        match self {
            ActionSubcommand::Get => get_actions(ctx, interact, module).await,
            ActionSubcommand::Add => add_action(ctx, interact, cmd_options, module).await,
            ActionSubcommand::Remove => remove_action(ctx, interact, cmd_options, module).await,
        }
    }
}

async fn get_actions(ctx: &Context, interact: &Interaction, module: Module) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let actions = module.get_actions(&db)?;

    if actions.is_empty() {
        respond(ctx, interact, |m| m.content(NO_ACTIONS)).await
    } else {
        respond_embed(ctx, interact, |e| {
            e.title(format!("Actions for the `{}` module", module.kind()));

            for (idx, action) in actions.into_iter().enumerate() {
                let name = format!("{}: {}", idx, action.friendly_name());
                e.field(name, action.description(), false);
            }

            e
        })
        .await
    }
}

async fn add_action(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    // TODO: limit on how many actions can be added

    let action_kind = ActionKind::from_str(command_option!(cmd_options, 1, String)?)
        .map_err(|e| InternalError::ImpossibleCase(format!("invalid action: {:?}", e)))?;

    let message = optional_command_option!(cmd_options, 2, String)?.map(|val| val.as_str());
    let in_channel = optional_command_option!(cmd_options, 3, Channel)?.map(|ch| ch.id);

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let action = match action_kind {
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
                        message, action_kind
                    ))
                })?,
            )
        }
        ActionKind::RemoveMessage => Action::remove_message(),
    };

    module.add_action(&action, &db)?;
    respond_success(ctx, interact).await
}

async fn remove_action(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    let index = *command_option!(cmd_options, 1, Integer)?;
    let index = index.try_into().map_err(|_| ArgumentError::I64OutOfRange(index))?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;

    if module.action_count(&db)? == 0 {
        respond(ctx, interact, |m| m.content(NO_ACTIONS)).await
    } else {
        module.remove_nth_action(index, &db)?;
        respond_success(ctx, interact).await
    }
}
