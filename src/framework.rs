mod command;
mod module_subcommand;

use self::command::Command;
use crate::error::{ArgumentError, InternalError};
use chrono::Utc;
use log::*;
use serenity::{
    async_trait,
    builder::{CreateEmbed, CreateMessage},
    client::Context,
    framework::Framework,
    model::channel::Message,
    utils::Colour,
};
use std::{sync::Arc, time::Instant};
use structopt::{clap, StructOpt};
use tokio::sync::broadcast;

pub const COMMAND_PREFIX: &str = "-ct";
const NO_ACTIONS: &str = "There aren't any actions defined for this module. Add some with the `add-action` subcommand!";
const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub struct CaretakerFramework {
    msg_tx: broadcast::Sender<Arc<Message>>,
}

enum ProcessingError {
    CommandError(anyhow::Error),
    MessageError(anyhow::Error),
}

#[async_trait]
impl Framework for CaretakerFramework {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        // straight-up ignore bot messages
        if !is_from_user(&msg) {
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
            Err(ProcessingError::MessageError(e)) => warn!("Message processing failed: {}", e),
            Err(ProcessingError::CommandError(e)) => {
                error!("Command processing failed: {}", e);

                if let Err(e) = channel_id
                    .send_message(&ctx, |m| {
                        if let Some(clap::Error { message, .. }) = e.downcast_ref() {
                            codeblock_message(message, m)
                        } else if let Some(e) = e.downcast_ref::<ArgumentError>() {
                            argument_error_message(e, m)
                        } else {
                            internal_error_message(e, m)
                        }
                    })
                    .await
                {
                    error!("Failed to send error message to channel {}: {}", channel_id, e);
                }
            }
            _ => {}
        }

        debug!("Message processed in {:?}", start.elapsed());
    }
}

impl CaretakerFramework {
    pub fn new(msg_tx: broadcast::Sender<Arc<Message>>) -> Self {
        Self { msg_tx }
    }

    async fn process_message(&self, ctx: &Context, msg: Message) -> Result<(), ProcessingError> {
        if msg.content.starts_with(COMMAND_PREFIX) {
            self.process_management_command(ctx, msg)
                .await
                .map_err(ProcessingError::CommandError)
        } else {
            self.process_user_message(ctx, msg)
                .await
                .map_err(ProcessingError::MessageError)
        }
    }

    async fn process_management_command(&self, ctx: &Context, msg: Message) -> anyhow::Result<()> {
        let cmd_str = msg.content.strip_prefix(COMMAND_PREFIX).ok_or_else(|| {
            InternalError::ImpossibleCase(String::from("given message content does not start with COMMAND_PREFIX"))
        })?;
        let command = Command::from_iter_safe(shellwords::split(cmd_str)?)?;

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

fn is_from_user(msg: &Message) -> bool {
    !msg.author.bot
}
