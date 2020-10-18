use crate::ShardMetadata;
use chrono::Utc;
use log::*;
use serenity::{
    async_trait, builder::CreateMessage, client::Context, framework::Framework, model::channel::Message, utils::Colour,
};
use structopt::{clap, StructOpt};

pub const COMMAND_PREFIX: &str = "-ct";

pub struct Management {}

#[derive(StructOpt, Debug)]
#[structopt(
    global_settings(&[clap::AppSettings::NoBinaryName,
        clap::AppSettings::DisableHelpFlags,
        clap::AppSettings::DisableVersion]),
    set_term_width(0),
    name = COMMAND_PREFIX,
    no_version
)]
enum Command {
    /// Prints status about the current shard and the shards as a whole
    Status,
    /// Deliberately returns an error
    Fail,
}

#[async_trait]
impl Framework for Management {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        info!("Dispatch called: '{}' by {}", msg.content, msg.author);
        debug!("{:#?}", msg);

        match self.process_message(&ctx, &msg).await {
            Ok(_) => debug!("Message processed succesfully"),
            Err(err) => {
                warn!("Message processing failed: {}", err);
                if let Err(e) = msg
                    .channel_id
                    .send_message(&ctx.http, |m| {
                        internal_error_message(err, m);
                        m
                    })
                    .await
                {
                    error!("Failed to send error message to channel {}: {}", msg.channel_id, e);
                }
            }
        }
    }
}

impl Management {
    pub fn new() -> Self {
        Self {}
    }

    async fn process_message(&self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        if !self.is_from_user(msg) {
            return Ok(());
        }

        if let Some(command) = msg.content.strip_prefix(COMMAND_PREFIX) {
            let command = match Command::from_iter_safe(command.split_whitespace()) {
                Ok(c) => c,
                Err(clap::Error { kind, message, .. }) => {
                    warn!("structopt returned {:?} error: {}", kind, message);
                    msg.channel_id
                        .send_message(&ctx.http, |m| {
                            codeblock_message(&message, m);
                            m
                        })
                        .await?;
                    return Ok(());
                }
            };

            debug!("{:#?}", command);
            command.run(ctx, msg).await?;
        }

        Ok(())
    }

    fn is_from_user(&self, msg: &Message) -> bool {
        !msg.author.bot
    }
}

impl Command {
    async fn run(&self, ctx: &Context, msg: &Message) -> anyhow::Result<()> {
        match self {
            Command::Status => {
                let data = ctx.data.read().await;
                if let Some(shards) = data.get::<ShardMetadata>() {
                    if let Some(own_shard) = shards.get(&ctx.shard_id) {
                        msg.channel_id
                            .send_message(&ctx.http, |m| {
                                m.embed(|e| {
                                    e.field("Shard", format_args!("{}/{}", own_shard.id + 1, shards.len()), true);
                                    e.field("Guilds", format_args!("{}", own_shard.guilds), true);

                                    if let Some(latency) = own_shard.latency {
                                        e.field(
                                            "GW latency",
                                            format_args!("{}ms", latency.as_micros() as f32 / 1000f32),
                                            true,
                                        );
                                    } else {
                                        e.field("GW latency", "n/a", true);
                                    }

                                    // the serenity docs state that `You can also pass an instance of
                                    // chrono::DateTime<Utc>, which will construct the timestamp string out of it.`, but
                                    // serenity itself implements the conversion only for references to datetimes, not
                                    // datetimes directly
                                    e.timestamp(&Utc::now());
                                    e
                                })
                            })
                            .await?;
                    } else {
                        warn!("Missing shard metadata for own shard {}", ctx.shard_id);
                    }
                } else {
                    warn!("Missing shard metadata collection in context userdata");
                }
            }
            Command::Fail => return Err(anyhow::anyhow!("a deliberate error")),
        };

        Ok(())
    }
}

fn internal_error_message<E>(err: E, m: &mut CreateMessage<'_>)
where
    E: AsRef<dyn std::error::Error>,
{
    m.embed(|e| {
        e.title("An internal error has occurred")
            .description(format_args!("```\n{}\n```", err.as_ref()))
            .colour(Colour::RED)
    });
}

fn codeblock_message(message: &str, m: &mut CreateMessage<'_>) {
    m.content(format_args!("```\n{}\n```", message));
}
