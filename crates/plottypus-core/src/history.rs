use std::collections::VecDeque;

const DEFAULT_CAPACITY: usize = 900;

/// How a history is mapped onto 0..=1 for drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// Always this max (CPU / GPU / MEM are 0..=1 → 100%).
    Fixed(f32),
    /// Grow to a 1-2-5 ceiling of the window peak, never below `floor`.
    Auto { floor: f32 },
}

impl Scale {
    #[must_use]
    pub fn resolve(self, peak: f32) -> f32 {
        match self {
            Self::Fixed(max) => max.max(f32::EPSILON),
            Self::Auto { floor } => nice_ceiling(peak.max(floor).max(0.0)),
        }
    }
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
        let scale = scale.max(f32::EPSILON);
        self.downsample(buckets)
            .into_iter()
            .map(|v| (v / scale).clamp(0.0, 1.0))
            .collect()
    }

    #[must_use]
    pub fn scale(&self, mode: Scale) -> f32 {
        mode.resolve(self.max().unwrap_or(0.0))
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
    }
}
