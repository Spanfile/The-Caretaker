use super::SubcommandTrait;
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
        match self {
            ExclusionSubcommand::Get => todo!(),
            ExclusionSubcommand::Set => todo!(),
            ExclusionSubcommand::Remove => todo!(),
        }
    }
}
