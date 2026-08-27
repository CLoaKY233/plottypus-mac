use plottypus_core::History;
use ratatui::style::Style;
use ratatui::widgets::{Sparkline, SparklineBar};

#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn bars(history: &History, width: u16) -> Vec<u64> {
    bars_scaled(history, width, 1.0)
}

#[must_use]
pub fn bars_scaled(history: &History, width: u16, scale: f32) -> Vec<u64> {
    bars_scaled_range(history, width, 0.0, scale)
}

#[must_use]
pub fn bars_scaled_range(history: &History, width: u16, min: f32, max: f32) -> Vec<u64> {
    let width = width.max(1) as usize;
    let samples = history.downsample_norm_range(width, min, max);
    if samples.is_empty() {
        return vec![0; width];
    }
    let mut out = vec![0_u64; width.saturating_sub(samples.len())];
    out.extend(samples.iter().map(|v| {
        let n = (v * 100.0).round() as u64;
        if *v > 0.0 { n.max(4) } else { 0 }
    }));
    if out.len() > width {
        out.drain(0..out.len() - width);
    }
    out
}

#[must_use]
pub fn widget_scaled(
    history: &History,
    width: u16,
    scale: f32,
    style: Style,
) -> Sparkline<'static> {
    widget_scaled_range(history, width, 0.0, scale, style)
}

#[must_use]
pub fn widget_scaled_range(
    history: &History,
    width: u16,
    min: f32,
    max: f32,
    style: Style,
) -> Sparkline<'static> {
    let data = bars_scaled_range(history, width, min, max);
    Sparkline::default()
        .data(data.into_iter().map(SparklineBar::from).collect::<Vec<_>>())
        .max(100)
        .style(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zeros() {
        assert_eq!(bars(&History::default(), 4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn pads_left_and_scales() {
        let mut h = History::with_capacity(4);
        h.push(1.0);
        let out = bars(&h, 3);
        assert_eq!(out, vec![0, 0, 100]);
    }

    #[test]
    fn forty_col_spark_keeps_last_twenty_identity() {
        let mut h = History::with_capacity(80);
        for _ in 0..60 {
            h.push(0.0);
        }
        for i in 0..20 {
            h.push(0.5 + i as f32 * 0.01);
        }
        let out = bars_scaled_range(&h, 40, 0.0, 1.0);
        assert_eq!(out.len(), 40);
        assert!(
            out[..20].iter().all(|v| *v == 0),
            "older zeros leaked: {:?}",
            &out[..20]
        );
        for i in 0..20 {
            let want = ((0.5 + i as f32 * 0.01) * 100.0).round() as u64;
            assert_eq!(out[20 + i], want.max(4), "col {}", 20 + i);
        }
    }
}
