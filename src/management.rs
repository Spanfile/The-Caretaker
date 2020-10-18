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
                        m.embed(|e| {
                            e.title("An internal error has occurred")
                                .description(err)
                                .colour(Colour::RED)
                        })
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

            let msg_builder = command.run()?;
            msg.channel_id.send_message(&ctx.http, msg_builder).await?;
        }

        Ok(())
    }

    fn is_from_user(&self, msg: &Message) -> bool {
        !msg.author.bot
    }
}

impl Command {
    fn run<'a, 'b>(&self) -> anyhow::Result<Box<dyn FnOnce(&'b mut CreateMessage<'a>) -> &'b mut CreateMessage<'a>>> {
        match self {
            Command::Status => Ok(Box::new(|m| m)),
        }
    }
}
