mod command;

use self::command::Command;
use crate::{
    error::{ArgumentError, InternalError},
    framework, ShardMetadata, VERSION,
};
use chrono::Utc;
use log::*;
use serenity::{
    async_trait,
    builder::{CreateEmbed, CreateInteractionResponseData},
    client::{bridge::gateway::event::ShardStageUpdateEvent, Context, EventHandler},
    gateway::ConnectionStage,
    model::prelude::*,
    utils::Colour,
};
use std::str::FromStr;

const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        debug!("{:#?}", ready);
        if let Some(s) = ready.shard {
            let (shard, shards) = (s[0], s[1]);
            info!(
                "Shard {} / {} ready! # of guilds: {}. Session ID: {}. Connected as {}",
                shard + 1,
                shards,
                ready.guilds.len(),
                ready.session_id,
                ready.user.tag()
            );

            self.set_info_activity(&ctx, shard, shards).await;
            self.build_commands(&ctx).await;
            self.insert_shard_metadata(&ctx, shard, ready.guilds.len()).await;
        } else {
            error!("Session ready, but shard was None");
        }
    }

    async fn resume(&self, ctx: Context, _: ResumedEvent) {
        debug!("Shard {}: resumed", ctx.shard_id);
    }

    async fn shard_stage_update(&self, ctx: Context, update: ShardStageUpdateEvent) {
        info!(
            "Shard {}: transitioned from {} to {}",
            update.shard_id, update.old, update.new
        );

        if let (ConnectionStage::Resuming, ConnectionStage::Connected) = (update.old, update.new) {
            info!("Shard {}: reconnected, resetting last connected time", update.shard_id);
            self.reset_shard_last_connected(&ctx, update.shard_id.0).await;
        }
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        debug!("Shard {}: cache ready. # of guilds: {}", ctx.shard_id, guilds.len());
        debug!("{:?}", guilds);
    }

    async fn interaction_create(&self, ctx: Context, interact: Interaction) {
        debug!("{:?}", interact);

        match interact.data {
            Some(InteractionData::ApplicationCommand(ref cmd)) => {
                if let Err(e) = self.process_command(&ctx, &interact, cmd).await {
                    if let Err(e) = respond(&ctx, &interact, |msg| {
                        if let Some(e) = e.downcast_ref::<ArgumentError>() {
                            debug!("Command processing failed with argument error: {}", e);
                            argument_error_message(e, msg)
                        } else {
                            error!("Command processing failed with internal error: {}", e);
                            internal_error_message(&e, msg)
                        }
                    })
                    .await
                    {
                        error!("Failed responding to interaction: {:?}", e);
                    }
                }
            }
            Some(InteractionData::MessageComponent(_msg)) => {}
            _ => (),
        }
    }
}

impl Handler {
    async fn set_info_activity(&self, ctx: &Context, shard: u64, shards: u64) {
        ctx.set_activity(Activity::playing(&format!(
            "{} [{}] [{}/{}]",
            framework::DEFAULT_COMMAND_PREFIX,
            VERSION,
            shard + 1,
            shards
        )))
        .await;
    }

    async fn insert_shard_metadata(&self, ctx: &Context, shard: u64, guilds: usize) {
        let mut data = ctx.data.write().await;
        if let Some(shard_meta) = data.get_mut::<ShardMetadata>() {
            shard_meta.insert(
                shard,
                ShardMetadata {
                    id: shard,
                    guilds,
                    latency: None,
                    last_connected: Utc::now(),
                },
            );
        } else {
            error!("No shard collection in context userdata");
        }
    }

    async fn reset_shard_last_connected(&self, ctx: &Context, shard: u64) {
        let mut data = ctx.data.write().await;
        if let Some(meta_collection) = data.get_mut::<ShardMetadata>() {
            if let Some(shard_meta) = meta_collection.get_mut(&shard) {
                shard_meta.last_connected = Utc::now();
            } else {
                error!("No shard metadata for shard {}", shard);
            }
        } else {
            error!("No shard collection in context userdata");
        }
    }

    async fn build_commands(&self, ctx: &Context) {
        debug!("Registering commands for shard {}", ctx.shard_id);

        let commands = ApplicationCommand::create_global_application_commands(&ctx.http, |cmds| {
            cmds.create_application_command(|cmd| {
                cmd.name("status")
                    .description("Prints status about the current shard and the shards as a whole")
            })
            .create_application_command(|cmd| cmd.name("fail").description("Deliberately returns an error"))
            .create_application_command(|cmd| cmd.name("success").description("Responds with a success message"))
        })
        .await;

        debug!("{:#?}", commands);
    }

    async fn process_command(
        &self,
        ctx: &Context,
        interact: &Interaction,
        cmd: &ApplicationCommandInteractionData,
    ) -> anyhow::Result<()> {
        match Command::from_str(cmd.name.as_ref()) {
            Ok(command) => {
                match (&interact.member, &interact.user) {
                    (Some(Member { user, .. }), None) => {
                        info!("{} ({}) in {:?}: {:?}", user.tag(), user, interact.guild_id, command)
                    }
                    (None, Some(user)) => info!("{} ({}) in DM: {:?}", user.tag(), user, command),
                    _ => warn!("Both interact.member and interact.user are None, running command anyways..."),
                };

                command.run(ctx, interact, cmd).await
            }
            Err(e) => {
                error!("Failed to parse slash command: {:?}", e);
                Err(InternalError::ImpossibleCase(String::from("parsing command failed")).into())
            }
        }
    }
}

fn argument_error_message<'a>(
    e: &ArgumentError,
    m: &'a mut CreateInteractionResponseData,
) -> &'a mut CreateInteractionResponseData {
    m.content(format!("{} {}", UNICODE_CROSS, e))
}

fn internal_error_message<'a>(
    err: &anyhow::Error,
    m: &'a mut CreateInteractionResponseData,
) -> &'a mut CreateInteractionResponseData {
    m.create_embed(|e| {
        e.title("An internal error has occurred")
            .description(format!("```\n{}\n```", err))
            .timestamp(&Utc::now())
            .colour(Colour::RED)
    })
}

async fn respond_success(ctx: &Context, interact: &Interaction) -> anyhow::Result<()> {
    respond(ctx, interact, |msg| msg.content(UNICODE_CHECK)).await?;
    Ok(())
}

async fn respond_embed<F>(ctx: &Context, interact: &Interaction, f: F) -> anyhow::Result<()>
where
    F: FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
{
    respond(ctx, interact, |msg| msg.create_embed(f)).await?;
    Ok(())
}

async fn respond<F>(ctx: &Context, interact: &Interaction, f: F) -> anyhow::Result<()>
where
    F: FnOnce(&mut CreateInteractionResponseData) -> &mut CreateInteractionResponseData,
{
    interact
        .create_interaction_response(&ctx.http, |resp| {
            resp.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|msg| f(msg))
        })
        .await?;
    Ok(())
}
