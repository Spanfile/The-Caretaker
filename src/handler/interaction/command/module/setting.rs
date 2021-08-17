use super::{resolve_module, respond, respond_embed, SubcommandTrait};
use crate::{
    command_option,
    ext::UserdataExt,
    handler::interaction::respond_success,
    module::{settings::Settings, Module},
    DbPool,
};
use serenity::{
    async_trait,
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
    },
};
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum SettingSubcommand {
    Get,
    Set,
    Reset,
}

#[async_trait]
impl SubcommandTrait for SettingSubcommand {
    async fn run(
        self,
        ctx: &Context,
        interact: &ApplicationCommandInteraction,
        options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        let module = resolve_module(ctx, interact, options).await?;

        match self {
            SettingSubcommand::Get => get_settings(ctx, interact, module).await,
            SettingSubcommand::Set => set_setting(ctx, interact, options, module).await,
            SettingSubcommand::Reset => reset_setting(ctx, interact, options, module).await,
        }
    }
}

async fn get_settings(ctx: &Context, interact: &ApplicationCommandInteraction, module: Module) -> anyhow::Result<()> {
    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let settings = module.get_settings(&db)?;
    let values = settings.get_all();

    if values.is_empty() {
        respond(ctx, interact, |m| {
            m.content(format!("The `{}` module has no applicable settings.", module.kind()))
        })
        .await
    } else {
        respond_embed(ctx, interact, |e| {
            e.title(format!("Settings for the `{}` module", module.kind()));
            e.fields(values.into_iter().map(|(k, v)| {
                (
                    k, // field name
                    format!(
                        "{}\nValue: `{}` (default: `{}`)",
                        settings.description_for(k).unwrap(),
                        v,
                        settings.default_for(k).unwrap(),
                    ), // field value
                    false, // inline
                )
            }))
        })
        .await
    }
}

async fn set_setting(
    ctx: &Context,
    interact: &ApplicationCommandInteraction,
    options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    let name = command_option!(options, 1, String)?;
    let value = command_option!(options, 2, String)?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let mut settings = module.get_settings(&db)?;

    settings.set(name, value)?;
    module.set_settings(&settings, &db)?;
    respond_success(ctx, interact).await
}

async fn reset_setting(
    ctx: &Context,
    interact: &ApplicationCommandInteraction,
    options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    let name = command_option!(options, 1, String)?;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;
    let mut settings = module.get_settings(&db)?;

    settings.reset(name)?;
    module.set_settings(&settings, &db)?;
    respond_success(ctx, interact).await
}
