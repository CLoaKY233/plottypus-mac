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
    let width = width.max(1) as usize;
    let samples = history.downsample_norm(width, scale);
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
    let data = bars_scaled(history, width, scale);
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
}
