use super::SubcommandTrait;
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{ApplicationCommandInteractionDataOption, Interaction},
};
use strum::EnumString;

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
        match self {
            ActionSubcommand::Get => todo!(),
            ActionSubcommand::Add => todo!(),
            ActionSubcommand::Remove => todo!(),
        }
    }
}
