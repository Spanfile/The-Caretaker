use log::*;
use serenity::{async_trait, client::Context, framework::Framework, model::channel::Message, utils::Colour};

const COMMAND_PREFIX: &str = "-ct ";

pub struct Management {}

#[async_trait]
impl Framework for Management {
    async fn dispatch(&self, ctx: Context, msg: Message) {
        info!("Dispatch called: '{}' by {}", msg.content, msg.author);
        debug!("{:#?}", msg);

        match self.process_message(&ctx, &msg).await {
            Ok(_) => debug!("Message processed succesfully"),
            Err(err) => {
                warn!("Message processing failed: {}", err);
                if msg
                    .channel_id
                    .send_message(&ctx.http, |m| {
                        m.embed(|mut e| {
                            e.title("An internal error has occurred")
                                .description(err)
                                .colour(Colour::RED)
                        })
                    })
                    .await
                    .is_err()
                {
                    error!("Failed to send error message to channel {}", msg.channel_id);
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
        if !self.should_process(msg) {
            return Ok(());
        }

        Err(anyhow::anyhow!("a deliberate error"))
    }

    fn should_process(&self, msg: &Message) -> bool {
        !msg.author.bot && msg.content.starts_with(COMMAND_PREFIX)
    }
}
