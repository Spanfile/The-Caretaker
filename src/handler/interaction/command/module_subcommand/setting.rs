use super::SubcommandTrait;
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{ApplicationCommandInteractionDataOption, Interaction},
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
        interact: &Interaction,
        cmd_options: &[ApplicationCommandInteractionDataOption],
    ) -> anyhow::Result<()> {
        match self {
            SettingSubcommand::Get => todo!(),
            SettingSubcommand::Set => todo!(),
            SettingSubcommand::Reset => todo!(),
        }
    }
}
