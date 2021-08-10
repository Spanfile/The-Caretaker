use crate::{ext::UserdataExt, latency_counter::LatencyCounter};
use chrono::Utc;
use log::*;
use serenity::{
    client::Context,
    model::channel::{Message, MessageType},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;

pub async fn process(ctx: &Context, msg: Message, msg_tx: &broadcast::Sender<Arc<Message>>) {
    // straight-up ignore bot messages and non-regular messages
    if is_from_bot(&msg) || !is_regular(&msg) {
        return;
    }

    let delay = (Utc::now() - msg.timestamp).num_milliseconds();
    // debug!("{:?}", msg);
    debug!(
        "Message handler called {}ms later from message timestamp ({})",
        delay, msg.timestamp
    );

    if let Err(e) = process_message(msg, msg_tx) {
        error!("Message processing failed: {}", e)
    }

    let data = ctx.data.read().await;
    match data.get_userdata::<LatencyCounter>() {
        Ok(latency) => latency.tick_message(Duration::from_millis(delay as u64)).await,
        Err(e) => error!("Failed to tick message handler latency: {:?}", e),
    }
}

fn process_message(msg: Message, msg_tx: &broadcast::Sender<Arc<Message>>) -> anyhow::Result<()> {
    // dirty short-circuit side-effect
    if msg.guild_id.is_some() && msg_tx.send(Arc::new(msg)).is_err() {
        error!("Sending message to broadcast channel failed (channel closed)");
    }
    Ok(())
}

fn is_from_bot(msg: &Message) -> bool {
    msg.author.bot
}

fn is_regular(msg: &Message) -> bool {
    msg.kind == MessageType::Regular
}
