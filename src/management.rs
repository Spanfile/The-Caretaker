use crate::ShardMetadata;
use chrono::Utc;
use log::*;
use serenity::{
    async_trait, builder::CreateMessage, client::Context, framework::Framework, model::channel::Message, utils::Colour,
};
use structopt::{clap, StructOpt};

const COMMAND_PREFIX: &str = "-ct";

pub struct Management {}

#[derive(StructOpt, Debug)]
#[structopt(setting(clap::AppSettings::NoBinaryName), setting(clap::AppSettings::ColorNever), name = COMMAND_PREFIX)]
enum Command {
    Status,
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
                Err(clap::Error {
                    kind: clap::ErrorKind::HelpDisplayed,
                    ..
                })
                | Err(clap::Error {
                    kind: clap::ErrorKind::VersionDisplayed,
                    ..
                }) => panic!(),
                Err(e) => return Err(e.into()),
            };

            debug!("{:#?}", command);
            match command {
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
                                        // chrono::DateTime<Utc>, which will construct the timestamp string out of it.`,
                                        // but serenity itself implements the
                                        // conversion only for references to datetimes, not
                                        // datetimes directly
                                        e.timestamp(&Utc::now());
                                        e
                                    })
                                })
                                .await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn is_from_user(&self, msg: &Message) -> bool {
        !msg.author.bot
    }
}

fn internal_error_message<E>(err: E, m: &mut CreateMessage<'_>)
where
    E: AsRef<dyn std::error::Error>,
{
    m.embed(|e| {
        e.title("An internal error has occurred")
            .description(err.as_ref())
            .colour(Colour::RED)
    });
}
