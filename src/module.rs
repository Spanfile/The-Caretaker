use strum::{EnumString, EnumVariantNames};

#[derive(Debug, EnumString, EnumVariantNames, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum Module {
    MassPing,
    Crosspost,
}
