use super::{resolve_module, respond, respond_embed, respond_success, SubcommandTrait};
use crate::module::Module;
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{ApplicationCommandInteractionDataOption, Interaction},
};
use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum ExclusionSubcommand {
    Get,
    Set,
    Remove,
}

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
            ExclusionSubcommand::Get => get_exclusion(ctx, interact, cmd_options, module).await,
            ExclusionSubcommand::Set => set_exclusion(ctx, interact, cmd_options, module).await,
            ExclusionSubcommand::Remove => remove_exclusion(ctx, interact, cmd_options, module).await,
        }
    }
}

async fn get_exclusion(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    unimplemented!()
}

async fn set_exclusion(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    unimplemented!()
}

async fn remove_exclusion(
    ctx: &Context,
    interact: &Interaction,
    cmd_options: &[ApplicationCommandInteractionDataOption],
    module: Module,
) -> anyhow::Result<()> {
    unimplemented!()
}
