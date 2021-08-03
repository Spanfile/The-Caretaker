use super::{
    enabled_string, resolve_module, resolve_optional_module, respond, respond_embed, respond_success, SubcommandTrait,
};
use crate::{
    command_option,
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    module::{cache::ModuleCache, Module},
    DbPool,
};
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{
        ApplicationCommandInteractionDataOption, ApplicationCommandInteractionDataOptionValue, Interaction,
    },
};
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum EnabledSubcommand {
    Get,
    Set,
}

#[async_trait]
impl SubcommandTrait for EnabledSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        match self {
            EnabledSubcommand::Get => {
                if let Some(module) = resolve_optional_module(ctx, interact, cmd_options).await? {
                    respond(ctx, interact, |m| {
                        m.content(format!(
                            "The `{}` module is: {}",
                            module.kind(),
                            enabled_string(module.is_enabled())
                        ))
                    })
                    .await
                } else {
                    let modules = {
                        let data = ctx.data.read().await;
                        let guild_id = interact.guild_id.ok_or(ArgumentError::NotSupportedInDM)?;
                        let db = data.get_userdata::<DbPool>()?.get()?;
                        Module::get_all_modules_for_guild(guild_id, &db)?
                    };

                    respond_embed(ctx, interact, |e| {
                        e.title("Status of all modules");

                        for (kind, module) in modules {
                            e.field(kind, super::enabled_string(module.is_enabled()), true);
                        }
                        e
                    })
                    .await
                }
            }
            EnabledSubcommand::Set => {
                let mut module = resolve_module(ctx, interact, cmd_options).await?;
                let enabled = *command_option!(cmd_options, 1, Boolean);

                let data = ctx.data.read().await;
                let db = data.get_userdata::<DbPool>()?.get()?;
                let module_cache = data.get_userdata::<ModuleCache>()?;

                module.set_enabled(enabled, &db)?;
                module_cache.update(module).await?;

                respond_success(ctx, interact).await
            }
        }
    }
}
