use fern::Dispatch;
use log::LevelFilter;
use serde::Deserialize;

#[derive(Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
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
    let mut dispatch = Dispatch::new()
        .format(move |out, msg, record| out.finish(format_args!("{{{}}} {} {}", record.target(), record.level(), msg)))
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
