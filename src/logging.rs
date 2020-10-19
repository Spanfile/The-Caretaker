use fern::Dispatch;
use log::LevelFilter;
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        }
    }
}

pub fn setup_logging(log_level: LogLevel) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut dispatch = Dispatch::new()
        .format(move |out, msg, record| {
            let mut target = record.target().to_string();
            if target.starts_with("the_caretaker") {
                target = String::new();
            } else {
                target = format!("[{}] ", target);
            }

            out.finish(format_args!(
                "{: >11.3} {: >5} {}{}",
                start.elapsed().as_secs_f32(),
                record.level(),
                target,
                msg
            ))
        })
        .level(log_level.into())
        .chain(std::io::stdout());

    if log_level != LogLevel::Trace {
        dispatch = dispatch
            .level_for("tracing", LevelFilter::Warn)
            .level_for("serenity", LevelFilter::Warn)
            .level_for("h2", LevelFilter::Warn)
            .level_for("hyper", LevelFilter::Warn)
            .level_for("rustls", LevelFilter::Warn)
            .level_for("reqwest", LevelFilter::Warn)
            .level_for("tungstenite", LevelFilter::Warn);
    }

    dispatch.apply()?;
    Ok(())
}
