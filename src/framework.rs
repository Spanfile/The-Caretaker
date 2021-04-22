mod command;
mod module_subcommand;

use self::command::Command;
use crate::{
    error::{ArgumentError, InternalError},
    ext::UserdataExt,
    guild_settings::GuildSettings,
    models, DbPool,
};
use chrono::Utc;
use diesel::prelude::*;
use log::*;
use serenity::{
    async_trait,
    builder::{CreateEmbed, CreateMessage},
    client::Context,
    framework::Framework,
    model::{
        channel::{Message, MessageType},
        id::GuildId,
    },
    utils::Colour,
};
use std::{sync::Arc, time::Instant};
use structopt::{clap, StructOpt};
use thiserror::Error;
use tokio::sync::broadcast;

// TODO: per-guild command prefixes
pub const DEFAULT_COMMAND_PREFIX: &str = "-ct";
const NO_ACTIONS: &str = "There aren't any actions defined for this module. Add some with the `add-action` subcommand!";
const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub struct CaretakerFramework {
    msg_tx: broadcast::Sender<Arc<Message>>,
}

#[derive(Error, Debug)]
enum ProcessingError {
    #[error("Command processing failed")]
    Command(anyhow::Error),
    #[error("Message processing failed")]
    Message(anyhow::Error),
    #[error(transparent)]
    Internal(anyhow::Error),
}

#[async_trait]
impl Framework for CaretakerFramework {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        // straight-up ignore bot messages and non-regular messages
        if is_from_bot(&msg) || !is_regular(&msg) {
            return;
        }

        debug!("{:?}", msg);
        debug!(
            "Dispatch called {}ms later from message timestamp ({})",
            (Utc::now() - msg.timestamp).num_milliseconds(),
            msg.timestamp
        );

        let channel_id = msg.channel_id;
        let start = Instant::now();
        match self.process_message(&ctx, msg).await {
            Err(ProcessingError::Internal(e)) => error!("Internal error occurred: {}", e),
            Err(ProcessingError::Message(e)) => error!("Message processing failed: {}", e),
            Err(ProcessingError::Command(e)) => {
                if let Err(e) = channel_id
                    .send_message(&ctx, |m| {
                        if let Some(clap::Error { message, kind, .. }) = e.downcast_ref() {
                            warn!("Command processing failed with clap {:?} error: {}", kind, e);
                            codeblock_message(message, m)
                        } else if let Some(e) = e.downcast_ref::<ArgumentError>() {
                            warn!("Command processing failed with argument error: {}", e);
                            argument_error_message(e, m)
                        } else {
                            error!("Command processing failed with internal error: {}", e);
                            internal_error_message(e, m)
                        }
                    })
                    .await
                {
                    error!("Failed to send error message to channel {}: {}", channel_id, e);
                }
            }
            // handle Ok(_) explicitly (instead of _ => {}) so if any variants are added to ProcessingError in the
            // future, this match won't be exhaustive
            Ok(_) => {}
        }

        debug!("Message processed in {:?}", start.elapsed());
    }
}

impl CaretakerFramework {
    pub fn new(msg_tx: broadcast::Sender<Arc<Message>>) -> Self {
        Self { msg_tx }
    }

    async fn process_message(&self, ctx: &Context, msg: Message) -> Result<(), ProcessingError> {
        let guild_id = msg
            .guild_id
            .ok_or_else(|| ProcessingError::Internal(InternalError::MissingGuildID.into()))?;
        let pfx = get_guild_prefix(ctx, guild_id)
            .await
            .map_err(ProcessingError::Internal)?;

        if msg.content.starts_with(&pfx) {
            self.process_management_command(ctx, msg, &pfx)
                .await
                .map_err(ProcessingError::Command)
        } else {
            self.process_user_message(ctx, msg)
                .await
                .map_err(ProcessingError::Message)
        }
    }

    async fn process_management_command(&self, ctx: &Context, msg: Message, pfx: &str) -> anyhow::Result<()> {
        let cmd_str = msg.content.strip_prefix(pfx).ok_or_else(|| {
            InternalError::ImpossibleCase(String::from("given message content does not start with COMMAND_PREFIX"))
        })?;
        let command = Command::from_iter_safe(
            shtring::split(cmd_str).map_err(|e| ArgumentError::MalformedCommand(e.to_string()))?,
        )?;

        info!(
            "{} ({}) ({:?}): {:?}",
            msg.author.tag(),
            msg.author,
            msg.guild_id,
            command
        );
        command.run(ctx, msg).await
    }

    async fn process_user_message(&self, _ctx: &Context, msg: Message) -> anyhow::Result<()> {
        // dirty short-circuit side-effect
        if msg.guild_id.is_some() && self.msg_tx.send(Arc::new(msg)).is_err() {
            error!("Sending message to broadcast channel failed (channel closed)");
        }
        Ok(())
    }
}

fn internal_error_message<'a, 'b, E>(err: E, m: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b>
where
    E: AsRef<dyn std::error::Error>,
{
    m.embed(|e| {
        e.title("An internal error has occurred")
            .description(format!("```\n{}\n```", err.as_ref()))
            .timestamp(&Utc::now())
            .colour(Colour::RED)
    })
}

fn argument_error_message<'a, 'b>(e: &ArgumentError, m: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> {
    m.content(format!("{} {}", UNICODE_CROSS, e))
}

fn codeblock_message<'a, 'b>(message: &str, m: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> {
    m.content(format!("```\n{}\n```", message))
}

async fn react_success(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    msg.react(ctx, UNICODE_CHECK).await?;
    Ok(())
}

async fn respond<'a, F>(ctx: &Context, msg: &Message, f: F) -> anyhow::Result<()>
where
    for<'b> F: FnOnce(&'b mut CreateMessage<'a>) -> &'b mut CreateMessage<'a>,
{
    msg.channel_id.send_message(ctx, f).await?;
    Ok(())
}

async fn respond_embed<F>(ctx: &Context, msg: &Message, f: F) -> anyhow::Result<()>
where
    F: FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
{
    msg.channel_id.send_message(ctx, |m| m.embed(f)).await?;
    Ok(())
}

fn enabled_string(enabled: bool) -> String {
    if enabled {
        format!("{} enabled", UNICODE_CHECK)
    } else {
        format!("{} disabled", UNICODE_CROSS)
    }
}

fn is_from_bot(msg: &Message) -> bool {
    msg.author.bot
}

fn is_regular(msg: &Message) -> bool {
    msg.kind == MessageType::Regular
}

async fn get_guild_prefix(ctx: &Context, guild: GuildId) -> anyhow::Result<String> {
    use crate::schema::guild_settings;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;

    let settings = guild_settings::table
        .filter(guild_settings::guild.eq(guild.0 as i64))
        .first::<models::GuildSettings>(&db)
        .optional()?
        .map_or_else(|| GuildSettings::default_with_guild(guild), GuildSettings::from);

    Ok(settings.prefix.unwrap_or_else(|| String::from(DEFAULT_COMMAND_PREFIX)))
}

async fn set_guild_prefix(ctx: &Context, guild: GuildId, prefix: Option<String>) -> anyhow::Result<()> {
    use crate::schema::guild_settings;

    let data = ctx.data.read().await;
    let db = data.get_userdata::<DbPool>()?.get()?;

    let new_settings = models::NewGuildSettings {
        guild: guild.0 as i64,
        prefix: prefix.as_deref(),
    };

    // return the inserted row's guild ID but don't store it anywhere, because this way diesel will error if the
    // insert affected no rows
    diesel::insert_into(guild_settings::table)
        .values(&new_settings)
        .on_conflict(guild_settings::guild)
        .do_update()
        .set(&new_settings)
        .returning(guild_settings::guild)
        .get_result::<i64>(&db)?;

    debug!("insert {:?}", new_settings);
    Ok(())
}
