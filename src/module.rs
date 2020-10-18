use strum::{EnumString, EnumVariantNames};

#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum Module {
    MassPing,
    Crosspost,
}
