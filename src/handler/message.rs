use chrono::Utc;
use log::*;
use serenity::model::channel::{Message, MessageType};
use std::{sync::Arc, time::Instant};
use tokio::sync::broadcast;

pub async fn process(msg: Message, msg_tx: &broadcast::Sender<Arc<Message>>) {
    // straight-up ignore bot messages and non-regular messages
    if is_from_bot(&msg) || !is_regular(&msg) {
        return;
    }

    debug!("{:?}", msg);
    debug!(
        "Message processing called {}ms later from message timestamp ({})",
        (Utc::now() - msg.timestamp).num_milliseconds(),
        msg.timestamp
    );

    let start = Instant::now();
    if let Err(e) = process_message(msg, msg_tx).await {
        error!("Message processing failed: {}", e)
    }

    debug!("Message processed in {:?}", start.elapsed());
}

async fn process_message(msg: Message, msg_tx: &broadcast::Sender<Arc<Message>>) -> anyhow::Result<()> {
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
