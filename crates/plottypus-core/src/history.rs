use std::collections::VecDeque;

const DEFAULT_CAPACITY: usize = 900;

/// How a history is mapped onto 0..=1 for drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// Always this max from 0 (MEM used-ratio stays 0..=1).
    Fixed(f32),
    /// Grow to a 1-2-5 ceiling of the window peak, never below `floor`.
    /// btop's percent rescale floors at 10% so a 1% series still has shape.
    Auto { floor: f32 },
    /// Window min/max plus pad, never thinner than `min_span`. Temps live here.
    Band { pad: f32, min_span: f32 },
}

impl Scale {
    /// Load / util: 10% floor, same rescale-down as btop.
    pub const LOAD: Self = Self::Auto { floor: 0.10 };
    /// Die / package °C: zoom to the recent band.
    pub const TEMP: Self = Self::Band {
        pad: 3.0,
        min_span: 12.0,
    };
    /// Fan RPM from 0, floor 500 so idle 0 stays a flat line.
    pub const FAN: Self = Self::Auto { floor: 500.0 };

    #[must_use]
    pub fn resolve(self, peak: f32) -> f32 {
        match self {
            Self::Fixed(max) => max.max(f32::EPSILON),
            Self::Auto { floor } => nice_ceiling(peak.max(floor).max(0.0)),
            Self::Band { pad, min_span } => nice_ceiling((peak + pad).max(min_span).max(0.0)),
        }
    }

    #[must_use]
    pub fn range(self, history: &History) -> ScaleRange {
        match self {
            Self::Fixed(max) => ScaleRange {
                min: 0.0,
                max: max.max(f32::EPSILON),
            },
            Self::Auto { floor } => ScaleRange {
                min: 0.0,
                max: nice_ceiling(history.max().unwrap_or(0.0).max(floor).max(0.0)),
            },
            Self::Band { pad, min_span } => band_range(history, pad, min_span),
        }
    }

    #[must_use]
    pub const fn hints_axis(self) -> bool {
        matches!(self, Self::Auto { .. } | Self::Band { .. })
    }
}

/// Inclusive draw window. Values map as `(v - min) / (max - min)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleRange {
    pub min: f32,
    pub max: f32,
}

impl ScaleRange {
    #[must_use]
    pub fn span(self) -> f32 {
        (self.max - self.min).max(f32::EPSILON)
    }
}

fn band_range(history: &History, pad: f32, min_span: f32) -> ScaleRange {
    let lo_v = history.min().unwrap_or(0.0);
    let hi_v = history.max().unwrap_or(0.0);
    let mut lo = lo_v - pad;
    let mut hi = hi_v + pad;
    let span = hi - lo;
    if span < min_span {
        let extra = (min_span - span) / 2.0;
        lo -= extra;
        hi += extra;
    }
    lo = lo.max(0.0);
    lo = snap_down(lo, 5.0);
    hi = snap_up(hi, 5.0);
    if hi <= lo {
        hi = lo + min_span.max(5.0);
    }
    ScaleRange { min: lo, max: hi }
}

fn snap_down(value: f32, step: f32) -> f32 {
    let step = step.max(f32::EPSILON);
    (value / step).floor() * step
}

fn snap_up(value: f32, step: f32) -> f32 {
    let step = step.max(f32::EPSILON);
    (value / step).ceil() * step
}

/// Next 1 / 2 / 5 × 10^n above `value`. Never 0.
#[must_use]
pub fn nice_ceiling(value: f32) -> f32 {
    if !value.is_finite() || value <= 0.0 {
        return 1.0;
    }
    let exp = value.log10().floor();
    let base = 10.0_f32.powf(exp);
    let n = value / base;
    let nice = if n <= 1.0 {
        1.0
    } else if n <= 2.0 {
        2.0
    } else if n <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * base
}

#[derive(Debug, Clone)]
pub struct History {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl History {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: f32) {
        if !value.is_finite() {
            return;
        }
        let value = value.max(0.0);
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[must_use]
    pub fn last(&self) -> Option<f32> {
        self.samples.back().copied()
    }

    #[must_use]
    pub fn max(&self) -> Option<f32> {
        self.samples.iter().copied().reduce(f32::max)
    }

    #[must_use]
    pub fn min(&self) -> Option<f32> {
        self.samples.iter().copied().reduce(f32::min)
    }

    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        self.samples.iter().copied()
    }

    #[must_use]
    pub fn downsample(&self, buckets: usize) -> Vec<f32> {
        if buckets == 0 || self.samples.is_empty() {
            return Vec::new();
        }
        let len = self.samples.len();
        if len <= buckets {
            return self.samples.iter().copied().collect();
        }
        let mut out = Vec::with_capacity(buckets);
        for i in 0..buckets {
            let start = i * len / buckets;
            let end = ((i + 1) * len / buckets).max(start + 1).min(len);
            let mut peak = 0.0_f32;
            for j in start..end {
                if let Some(v) = self.samples.get(j).copied() {
                    peak = peak.max(v);
                }
            }
            out.push(peak);
        }
        out
    }

    /// Downsample then divide by `scale` so callers get 0..=1 columns.
    #[must_use]
    pub fn downsample_norm(&self, buckets: usize, scale: f32) -> Vec<f32> {
        self.downsample_norm_range(buckets, 0.0, scale)
    }

    /// Downsample then map `min..=max` onto 0..=1.
    #[must_use]
    pub fn downsample_norm_range(&self, buckets: usize, min: f32, max: f32) -> Vec<f32> {
        let span = (max - min).max(f32::EPSILON);
        self.downsample(buckets)
            .into_iter()
            .map(|v| ((v - min) / span).clamp(0.0, 1.0))
            .collect()
    }

    #[must_use]
    pub fn range(&self, mode: Scale) -> ScaleRange {
        mode.range(self)
    }

    #[must_use]
    pub fn scale(&self, mode: Scale) -> f32 {
        self.range(mode).max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_and_caps() {
        let mut h = History::with_capacity(3);
        h.push(-0.5);
        h.push(0.4);
        h.push(1.5);
        h.push(0.2);
        assert_eq!(h.len(), 3);
        let values: Vec<f32> = h.iter().collect();
        assert_eq!(values, vec![0.4, 1.5, 0.2]);
    }

    #[test]
    fn nice_ceiling_steps() {
        assert!((nice_ceiling(0.0) - 1.0).abs() < f32::EPSILON);
        assert!((nice_ceiling(12.4) - 20.0).abs() < f32::EPSILON);
        assert!((nice_ceiling(100.0) - 100.0).abs() < f32::EPSILON);
        assert!((nice_ceiling(101.0) - 200.0).abs() < f32::EPSILON);
        assert!((nice_ceiling(5_000_000.0) - 5_000_000.0).abs() < 1.0);
        assert!((nice_ceiling(5_100_000.0) - 10_000_000.0).abs() < 1.0);
    }

    #[test]
    fn auto_scale_uses_nice_peak() {
        let mut h = History::with_capacity(8);
        h.push(1_200_000.0);
        h.push(3_400_000.0);
        let scale = h.scale(Scale::Auto { floor: 1.0 });
        assert!((scale - 5_000_000.0).abs() < 1.0);
        let norm = h.downsample_norm(2, scale);
        assert!(norm.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(norm[1] > 0.5);
    }

    #[test]
    fn downsample_uses_peak() {
        let mut h = History::with_capacity(8);
        for v in [0.1, 0.9, 0.2, 0.3, 0.05, 0.4, 0.8, 0.1] {
            h.push(v);
        }
        let bins = h.downsample(2);
        assert_eq!(bins.len(), 2);
        assert!((bins[0] - 0.9).abs() < f32::EPSILON);
        assert!((bins[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn downsample_short_history_is_identity() {
        let mut h = History::with_capacity(8);
        h.push(0.2);
        h.push(0.4);
        assert_eq!(h.downsample(8), vec![0.2, 0.4]);
        assert!(h.downsample(0).is_empty());
    }

    #[test]
    fn last_and_max() {
        let mut h = History::default();
        assert!(h.last().is_none());
        h.push(0.2);
        h.push(0.7);
        h.push(0.1);
        assert!((h.last().unwrap_or(0.0) - 0.1).abs() < f32::EPSILON);
        assert!((h.max().unwrap_or(0.0) - 0.7).abs() < f32::EPSILON);
        assert!((h.min().unwrap_or(0.0) - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn load_auto_floors_at_ten_percent() {
        let mut h = History::with_capacity(8);
        h.push(0.012);
        h.push(0.018);
        let range = h.range(Scale::LOAD);
        assert!((range.min - 0.0).abs() < f32::EPSILON);
        assert!((range.max - 0.10).abs() < 1e-5);
        let norm = h.downsample_norm_range(2, range.min, range.max);
        assert!(norm.iter().all(|v| *v > 0.05), "{norm:?}");
        assert!(norm[1] > 0.10, "{norm:?}");
    }

    #[test]
    fn temp_band_zooms_to_the_window() {
        let mut h = History::with_capacity(8);
        h.push(38.0);
        h.push(42.0);
        h.push(40.0);
        let range = h.range(Scale::TEMP);
        assert!(range.min <= 35.0, "{range:?}");
        assert!(range.max >= 45.0, "{range:?}");
        assert!(range.span() >= 12.0);
        let norm = h.downsample_norm_range(3, range.min, range.max);
        assert!(norm.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(norm[1] > norm[0], "{norm:?}");
    }
}
