use super::Matcher;
use crate::module::{settings::InviteLinkSettings, ModuleKind};
use log::*;
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

const DISCORD_URLS: &[&str] = &["discord.com", "discord.gg"];
const INVITE_PATH: &str = "invite";

pub struct InviteLink {}

#[async_trait]
impl Matcher for InviteLink {
    type SettingsType = InviteLinkSettings;
    async fn build(_: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self) {
        (ModuleKind::InviteLink, Self {})
    }

    async fn is_match(&mut self, _: Self::SettingsType, msg: &Message) -> anyhow::Result<bool> {
        for url in msg.content.split_whitespace().filter_map(|word| Url::parse(word).ok()) {
            match url.host_str() {
                Some(host) if DISCORD_URLS.contains(&host) => {
                    debug!("{} is a Discord URL", url);

                    if let Some(last_segment) = url.path_segments().and_then(|s| s.last()) {
                        // if the last segment is something, but not "invite" (as in discord.com/invite), it's
                        // probably an invite
                        if !last_segment.is_empty() && last_segment != INVITE_PATH {
                            info!("{} looks like an invite", url);
                            return Ok(true);
                        }
                    }
                }
                _ => continue,
            }
        }

        Ok(false)
    }
}
