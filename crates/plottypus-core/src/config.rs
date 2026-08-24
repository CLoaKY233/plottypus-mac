use std::time::Duration;

use crate::surface::Surface;

pub const INTERVAL_FAST: Duration = Duration::from_millis(500);
pub const INTERVAL_DEFAULT: Duration = Duration::from_secs(1);
pub const INTERVAL_SLOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub interval: Duration,
    pub surface: Option<Surface>,
    pub show_gpu: bool,
    pub show_net: bool,
    pub show_disk: bool,
    pub show_fans: bool,
    pub show_cores: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: INTERVAL_DEFAULT,
            surface: None,
            show_gpu: true,
            show_net: true,
            show_disk: true,
            show_fans: true,
            show_cores: true,
        }
    }
}

impl Config {
    #[must_use]
    pub fn cycle_interval(&self) -> Duration {
        if self.interval <= INTERVAL_FAST {
            INTERVAL_DEFAULT
        } else if self.interval <= INTERVAL_DEFAULT {
            INTERVAL_SLOW
        } else {
            INTERVAL_FAST
        }
    }

    #[must_use]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_cycles_three_steps() {
        let cfg = Config::default();
        assert_eq!(cfg.interval, INTERVAL_DEFAULT);
        let next = cfg.cycle_interval();
        assert_eq!(next, INTERVAL_SLOW);
        let cfg = cfg.with_interval(next);
        assert_eq!(cfg.cycle_interval(), INTERVAL_FAST);
        let cfg = cfg.with_interval(INTERVAL_FAST);
        assert_eq!(cfg.cycle_interval(), INTERVAL_DEFAULT);
    }
}
