mod command;

use self::command::Command;
use crate::error::{ArgumentError, InternalError};
use chrono::Utc;
use log::*;
use serenity::{
    builder::{CreateEmbed, CreateInteractionResponseData},
    client::Context,
    model::{
        guild::Member,
        interactions::{
            ApplicationCommand, ApplicationCommandInteractionData, Interaction, InteractionData,
            InteractionResponseType,
        },
    },
    utils::Colour,
};
use std::str::FromStr;

const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub async fn build_commands(ctx: &Context) {
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

pub async fn process(ctx: Context, interact: Interaction) {
    debug!("{:?}", interact);

    match interact.data {
        Some(InteractionData::ApplicationCommand(ref cmd)) => {
            if let Err(e) = process_command(&ctx, &interact, cmd).await {
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

async fn process_command(
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
