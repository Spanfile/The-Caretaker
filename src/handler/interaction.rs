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
            .create_application_command(build_admin_subcommand)
    })
    .await
    {
        Ok(cmds) => trace!("{:#?}", cmds),
        Err(e) => error!("Registering commands for shard {} failed: {:?}", ctx.shard_id, e),
    }
}

fn module_option(
    required: bool,
) -> impl FnOnce(&mut CreateApplicationCommandOption) -> &mut CreateApplicationCommandOption {
    move |opt| {
        opt.kind(ApplicationCommandOptionType::String)
            .name("module")
            .description("The module")
            .required(required)
            .add_string_choice("Mass ping", "mass-ping")
            .add_string_choice("Crosspost", "crosspost")
            .add_string_choice("Emoji spam", "emoji-spam")
            .add_string_choice("Mention spam", "mention-spam")
            .add_string_choice("Selfbot", "selfbot")
            .add_string_choice("Invite link", "invite-link")
            .add_string_choice("Channel activity", "channel-activity")
            .add_string_choice("User activity", "user-activity")
    }
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
                .description("Get whether the module is enabled, or the enabled status of all modules")
                .create_sub_option(module_option(false))
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("set")
                .description("Set whether the module is enabled")
                .create_sub_option(module_option(true))
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
                .create_sub_option(module_option(true))
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("add")
                .description("Adds a new user/role exclusion to the module")
                .create_sub_option(module_option(true))
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("remove")
                .description("Removes a given user/role exclusion from the module")
                .create_sub_option(module_option(true))
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
                .create_sub_option(module_option(true))
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("add")
                .description("Adds a new action to the module")
                .create_sub_option(module_option(true))
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("action")
                        .description("The action to add")
                        .add_string_choice("Remove message", "remove-message")
                        .add_string_choice("Notify", "notify")
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
                .create_sub_option(module_option(true))
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::Integer)
                        .name("index")
                        .description("The index of the action to remove")
                        .required(true)
                })
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
                .create_sub_option(module_option(true))
        })
        .create_sub_option(|sub| {
            sub.kind(ApplicationCommandOptionType::SubCommand)
                .name("set")
                .description("Sets the value of a setting of the module")
                .create_sub_option(module_option(true))
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
                .create_sub_option(module_option(true))
                .create_sub_option(|sub| {
                    sub.kind(ApplicationCommandOptionType::String)
                        .name("name")
                        .description("The name of the setting")
                        .required(true)
                })
        })
}

fn build_admin_subcommand(opt: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    opt.name("set-admin-role")
        .description("Set the role that is allowed to control Caretaker")
        .create_option(|opt| {
            opt.kind(ApplicationCommandOptionType::Role)
                .name("role")
                .description("The role allowed to control Caretaker")
                .required(true)
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
    let command = Command::from_str(cmd.name.as_ref())
        .map_err(|e| InternalError::ImpossibleCase(format!("parsing command failed: {:?}", e)))?;

    match (&interact.member, &interact.user) {
        (Some(Member { user, .. }), None) => {
            info!("{} ({}) in {:?}: {:?}", user.tag(), user, interact.guild_id, command)
        }
        (None, Some(user)) => {
            info!("{} ({}) in DM: {:?}", user.tag(), user, command)
        }
        _ => warn!("Both interact.member and interact.user are None, running command anyways..."),
    };

    command.run(ctx, interact, cmd).await
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

fn enabled_string(enabled: bool) -> String {
    if enabled {
        format!("{} enabled", UNICODE_CHECK)
    } else {
        format!("{} disabled", UNICODE_CROSS)
    }
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
