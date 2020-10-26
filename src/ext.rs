use std::time::Duration;

pub trait DurationExt {
    fn round_to_seconds(self) -> Duration;
}

impl DurationExt for Duration {
    fn round_to_seconds(self) -> Duration {
        Duration::from_secs(self.as_secs())
    }
}
