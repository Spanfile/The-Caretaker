mod command;

use self::command::Command;
use crate::error::{ArgumentError, InternalError};
use chrono::Utc;
use log::*;
use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommandOption, CreateEmbed, CreateInteractionResponseData},
    client::Context,
    model::{
        guild::Member,
        interactions::{
            ApplicationCommand, ApplicationCommandInteractionData, ApplicationCommandOptionType, Interaction,
            InteractionData, InteractionResponseType,
        },
    },
    utils::Colour,
};
use std::str::FromStr;

const UNICODE_CHECK: char = '\u{2705}';
const UNICODE_CROSS: char = '\u{274C}';

pub async fn build_commands(ctx: &Context) {
    debug!("Registering commands for shard {}", ctx.shard_id);

    match ApplicationCommand::create_global_application_commands(&ctx.http, |cmds| {
        cmds.create_application_command(|cmd| cmd.name("status").description("Show the bot's status"))
            .create_application_command(|cmd| cmd.name("fail").description("Deliberately returns an error"))
            .create_application_command(|cmd| cmd.name("success").description("Responds with a success message"))
            .create_application_command(build_module_subcommand)
    })
    .await
    {
        Ok(cmds) => debug!("{:#?}", cmds),
        Err(e) => error!("Registering commands for shard {} failed: {:?}", ctx.shard_id, e),
    }
}

fn module_option(opt: &mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    opt.kind(ApplicationCommandOptionType::String)
        .name("module")
        .description("The module to edit")
        .required(true)
        .add_string_choice("mass-ping", "Mass ping")
        .add_string_choice("crosspost", "Crosspost")
        .add_string_choice("emoji-spam", "Emoji spam")
        .add_string_choice("mention-spam", "Mention spam")
        .add_string_choice("selfbot", "Selfbot")
        .add_string_choice("invite-link", "Invite link")
        .add_string_choice("channel-activity", "Channel activity")
        .add_string_choice("user-activity", "User activity")
}

fn build_module_subcommand(cmd: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    cmd.name("module")
        .description("Configure the different Caretaker modules")
        .create_option(build_enabled_subcommand)
        .create_option(build_exclusion_subcommand)
        .create_option(build_action_subcommand)
        .create_option(build_setting_subcommand)
}

fn build_enabled_subcommand(opt: &mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    opt.kind(ApplicationCommandOptionType::SubCommandGroup)
        .name("enabled")
        .description("Get or set whether the module is enabled")
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("get")
                .description("Get whether the module is enabled")
                .create_sub_option(module_option)
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("set")
                .description("Set whether the module is enabled")
                .create_sub_option(module_option)
                .create_sub_option(|opt| {
                    opt.kind(ApplicationCommandOptionType::Boolean)
                        .name("enabled")
                        .description("Is the module enabled")
                        .required(true)
                })
        })
}

fn build_exclusion_subcommand(opt: &mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    opt.kind(ApplicationCommandOptionType::SubCommandGroup)
        .name("exclusion")
        .description("Modify the module user/role exclusions")
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("get")
                .description("Shows all user/role exclusions for the module")
                .create_sub_option(module_option)
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("add")
                .description("Adds a new user/role exclusion to the module")
                .create_sub_option(module_option)
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("remove")
                .description("Removes a given user/role exclusion from the module")
                .create_sub_option(module_option)
        })
}

fn build_action_subcommand(opt: &mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    opt.kind(ApplicationCommandOptionType::SubCommandGroup)
        .name("action")
        .description("Modify the module actions")
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("get")
                .description("Shows all actions associated with the module")
                .create_sub_option(module_option)
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("add")
                .description("Adds a new action to the module")
                .create_sub_option(module_option)
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("action")
                        .description("The action to add")
                        .add_string_choice("remove-message", "Remove message")
                        .add_string_choice("notify", "Notify")
                        .required(true)
                })
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("message")
                        .description("The message to send, if applicable")
                })
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::Channel)
                        .name("channel")
                        .description("The channel to send the message to, if applicable")
                })
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("remove")
                .description("Removes a given action from the module based on its index")
                .create_sub_option(module_option)
        })
}

fn build_setting_subcommand(opt: &mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    opt.kind(ApplicationCommandOptionType::SubCommandGroup)
        .name("setting")
        .description("Modify the module settings")
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("get")
                .description("Displays all settings and their values for the module")
                .create_sub_option(module_option)
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("set")
                .description("Sets the value of a setting of the module")
                .create_sub_option(module_option)
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("name")
                        .description("The name of the setting")
                        .required(true)
                })
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("value")
                        .description("The value of the setting")
                        .required(true)
                })
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("reset")
                .description("Resets the value of a setting of the module to its default value")
                .create_sub_option(module_option)
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("name")
                        .description("The name of the setting")
                        .required(true)
                })
        })
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
