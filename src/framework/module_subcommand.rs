use super::{enabled_string, react_success, NO_ACTIONS};
use crate::{
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    module::{
        action::{Action, ActionKind},
        cache::ModuleCache,
        settings::Settings,
        Module,
    },
    DbPool,
};
use serenity::{
    client::Context,
    model::{channel::Message, id::ChannelId},
};
use std::borrow::Cow;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(no_version)]
pub enum ModuleSubcommand {
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

impl ModuleSubcommand {
    pub async fn run(self, mut module: Module, ctx: &Context, msg: Message) -> anyhow::Result<()> {
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
                let values = settings.get_all();

                if values.is_empty() {
                    msg.channel_id
                        .send_message(ctx, |m| {
                            m.content(format!("The `{}` module has no applicable settings.", module.kind()))
                        })
                        .await?;
                } else {
                    msg.channel_id
                        .send_message(ctx, |m| {
                            m.embed(|e| {
                                e.title(format!("Settings for the `{}` module", module.kind()));
                                e.fields(values.into_iter().map(|(k, v)| {
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
