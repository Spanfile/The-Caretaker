use circular_queue::CircularQueue;
use paste::paste;
use serenity::prelude::TypeMapKey;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

const DEFAULT_CAPACITY: usize = 10;

#[derive(Debug, Clone)]
pub struct LatencyCounter {
    gateway: Arc<RwLock<CircularQueue<Duration>>>,
    action: Arc<RwLock<CircularQueue<Duration>>>,
    message: Arc<RwLock<CircularQueue<Duration>>>,
}

impl TypeMapKey for LatencyCounter {
    type Value = LatencyCounter;
}

macro_rules! tick_and_get_fns {
    ($($name:ident),+) => {
        $(
            paste! {
                pub async fn [<tick_ $name>](&self, latency: Duration) {
                    self.$name.write().await.push(latency);
                }

                pub async fn [<get_ $name>](&self) -> u128 {
                    let durations = self.$name.read().await;
                    if durations.is_empty() {
                        0
                    }else {
                        (durations.asc_iter().fold(0, |acc, dur| acc + dur.as_micros()) / durations.len() as u128) / 1000
                    }
                }
            }
        )+
    };
}

impl LatencyCounter {
    pub fn new() -> Self {
        Self {
            gateway: Arc::new(RwLock::new(CircularQueue::with_capacity(DEFAULT_CAPACITY))),
            action: Arc::new(RwLock::new(CircularQueue::with_capacity(DEFAULT_CAPACITY))),
            message: Arc::new(RwLock::new(CircularQueue::with_capacity(DEFAULT_CAPACITY))),
        }
    }

    tick_and_get_fns! {gateway, action, message}
}
