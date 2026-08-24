use std::path::Path;
use std::time::Duration;

use crate::surface::Surface;

pub const INTERVAL_FAST: Duration = Duration::from_millis(500);
pub const INTERVAL_DEFAULT: Duration = Duration::from_secs(1);
pub const INTERVAL_SLOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcSort {
    #[default]
    Cpu,
    Mem,
    Pid,
}

impl ProcSort {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Mem,
            Self::Mem => Self::Pid,
            Self::Pid => Self::Cpu,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Mem => "mem",
            Self::Pid => "pid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub interval: Duration,
    pub surface: Option<Surface>,
    pub show_gpu: bool,
    pub show_net: bool,
    pub show_disk: bool,
    pub show_fans: bool,
    pub show_cores: bool,
    pub show_threads: bool,
    pub proc_sort: ProcSort,
    pub proc_ratio: u16,
}

pub const PROC_RATIO_MIN: u16 = 35;
pub const PROC_RATIO_MAX: u16 = 72;
pub const PROC_RATIO_DEFAULT: u16 = 55;

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: INTERVAL_DEFAULT,
            surface: None,
            show_gpu: true,
            show_net: true,
            show_disk: false,
            show_fans: true,
            show_cores: true,
            show_threads: false,
            proc_sort: ProcSort::Cpu,
            proc_ratio: PROC_RATIO_DEFAULT,
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

    /// Unknown keys are ignored, invalid values fall back per key; a missing
    /// or unreadable file yields the default config.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        let mut cfg = Config::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"');
            cfg.apply(key.trim(), value);
        }
        cfg
    }

    fn apply(&mut self, key: &str, value: &str) {
        match key {
            "interval_ms" => {
                if let Ok(ms) = value.parse::<u64>() {
                    match ms {
                        500 => self.interval = INTERVAL_FAST,
                        1000 => self.interval = INTERVAL_DEFAULT,
                        2000 => self.interval = INTERVAL_SLOW,
                        _ => {}
                    }
                }
            }
            "surface" => match value {
                "work" => self.surface = Some(Surface::Work),
                "glance" => self.surface = Some(Surface::Glance),
                _ => {}
            },
            "show_gpu" => self.show_gpu = value.parse::<bool>().unwrap_or(self.show_gpu),
            "show_net" => self.show_net = value.parse::<bool>().unwrap_or(self.show_net),
            "show_disk" => self.show_disk = value.parse::<bool>().unwrap_or(self.show_disk),
            "show_fans" => self.show_fans = value.parse::<bool>().unwrap_or(self.show_fans),
            "show_cores" => self.show_cores = value.parse::<bool>().unwrap_or(self.show_cores),
            "show_threads" => {
                self.show_threads = value.parse::<bool>().unwrap_or(self.show_threads);
            }
            "sort" => match value {
                "cpu" => self.proc_sort = ProcSort::Cpu,
                "mem" => self.proc_sort = ProcSort::Mem,
                "pid" => self.proc_sort = ProcSort::Pid,
                _ => {}
            },
            "proc_ratio" => {
                if let Ok(ratio) = value.parse::<u16>() {
                    self.proc_ratio = ratio.clamp(PROC_RATIO_MIN, PROC_RATIO_MAX);
                }
            }
            _ => {}
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let surface = match self.surface {
            None => String::from("# surface = work\n"),
            Some(Surface::Work) => String::from("surface = \"work\"\n"),
            Some(Surface::Glance) => String::from("surface = \"glance\"\n"),
        };
        let sort = match self.proc_sort {
            ProcSort::Cpu => "cpu",
            ProcSort::Mem => "mem",
            ProcSort::Pid => "pid",
        };
        std::fs::write(
            path,
            format!(
                "interval_ms = {}\n\
                 {surface}\
                 show_gpu = {}\n\
                 show_net = {}\n\
                 show_disk = {}\n\
                 show_fans = {}\n\
                 show_cores = {}\n\
                 show_threads = {}\n\
                 sort = \"{sort}\"\n\
                 proc_ratio = {}\n",
                self.interval.as_millis(),
                self.show_gpu,
                self.show_net,
                self.show_disk,
                self.show_fans,
                self.show_cores,
                self.show_threads,
                self.proc_ratio,
            ),
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("plottypus-test-{name}.toml"))
    }

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

    #[test]
    fn defaults_show_net_but_hide_disk() {
        let cfg = Config::default();
        assert!(cfg.show_net);
        assert!(!cfg.show_disk);
        assert_eq!(cfg.proc_ratio, PROC_RATIO_DEFAULT);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let path = tmp("roundtrip");
        let cfg = Config {
            interval: INTERVAL_FAST,
            show_gpu: false,
            surface: Some(Surface::Glance),
            proc_sort: ProcSort::Mem,
            proc_ratio: 66,
            ..Config::default()
        };
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path);
        assert_eq!(loaded, cfg);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_is_defaults() {
        assert_eq!(Config::load(&tmp("definitely-missing")), Config::default());
    }

    #[test]
    fn garbage_values_fall_back_per_key() {
        let path = tmp("garbage");
        std::fs::write(
            &path,
            "interval_ms = 99999\nproc_ratio = 5\nshow_fans = maybe\nsort = nope\n",
        )
        .unwrap();
        let cfg = Config::load(&path);
        assert_eq!(cfg.interval, INTERVAL_DEFAULT);
        assert_eq!(cfg.proc_ratio, PROC_RATIO_MIN);
        assert!(cfg.show_fans);
        assert_eq!(cfg.proc_sort, ProcSort::Cpu);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn partial_file_overrides_only_named_keys() {
        let path = tmp("partial");
        std::fs::write(&path, "# comment\nshow_threads = true\n").unwrap();
        let cfg = Config::load(&path);
        assert!(cfg.show_threads);
        assert!(!cfg.show_disk);
        let _ = std::fs::remove_file(&path);
    }
}
