use plottypus_core::History;

const LEVELS: usize = 4;
// Fill from the BOTTOM of the cell (btop "up"): dots 7-3-2-1 / 8-6-5-4.
const LEFT_DOTS: [u32; LEVELS] = [6, 2, 1, 0];
const RIGHT_DOTS: [u32; LEVELS] = [7, 5, 4, 3];

/// One braille column: two samples stacked into `height` glyphs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrailleCell {
    pub glyph: char,
    pub intensity: f32,
}

#[must_use]
pub fn braille_cell(left: u8, right: u8) -> char {
    let left = u32::from(left.min(LEVELS as u8));
    let right = u32::from(right.min(LEVELS as u8));
    let mut bits = 0_u32;
    for i in 0..left {
        if let Some(dot) = LEFT_DOTS.get(i as usize) {
            bits |= 1 << dot;
        }
    }
    for i in 0..right {
        if let Some(dot) = RIGHT_DOTS.get(i as usize) {
            bits |= 1 << dot;
        }
    }
    braille_from_bits(bits)
}

fn braille_from_bits(bits: u32) -> char {
    char::from_u32(0x2800 + (bits & 0xFF)).unwrap_or('\u{2800}')
}

/// btop's tiny-value bias: 1-row graphs get more, so 2% still lights a dot.
#[must_use]
pub fn bias_for_height(height: usize) -> f32 {
    if height <= 1 { 0.3 } else { 0.1 }
}

fn level_in_band(value: f32, lo: f32, hi: f32, bias: f32) -> u8 {
    if value <= 0.0 {
        return 0;
    }
    let span = (hi - lo).max(f32::EPSILON);
    let t = (value - lo) / span + bias;
    if t <= 0.0 {
        0
    } else if t >= 1.0 {
        LEVELS as u8
    } else {
        (t * LEVELS as f32).floor().min(LEVELS as f32) as u8
    }
}

#[must_use]
pub fn render_history(history: &History, width: u16, height: u16) -> Vec<String> {
    render_cells(history, width, height, 1.0)
        .into_iter()
        .map(|row| row.into_iter().map(|cell| cell.glyph).collect())
        .collect()
}

#[must_use]
pub fn render_cells(
    history: &History,
    width: u16,
    height: u16,
    scale: f32,
) -> Vec<Vec<BrailleCell>> {
    let height = height.max(1) as usize;
    let width = width as usize;
    if width == 0 {
        return vec![Vec::new(); height];
    }
    let blank = BrailleCell {
        glyph: '\u{2800}',
        intensity: 0.0,
    };
    let buckets = width.saturating_mul(2);
    let samples = history.downsample_norm(buckets, scale);
    if samples.is_empty() {
        return vec![vec![blank; width]; height];
    }

    let cells_from_samples = samples.len().div_ceil(2).min(width);
    let pad = width.saturating_sub(cells_from_samples);
    let mut rows = vec![vec![blank; pad]; height];
    let bias = bias_for_height(height);

    let mut i = 0;
    while i < samples.len() && rows.first().is_some_and(|row| row.len() < width) {
        let left_v = samples[i];
        let right_v = samples.get(i + 1).copied().unwrap_or(0.0);
        let intensity = left_v.max(right_v);
        let glyphs = stacked_cell(left_v, right_v, height, bias);
        for (r, glyph) in glyphs.into_iter().enumerate() {
            if let Some(row) = rows.get_mut(r) {
                row.push(BrailleCell { glyph, intensity });
            }
        }
        i += 2;
    }
    for row in &mut rows {
        row.resize(width, blank);
    }
    rows
}

#[must_use]
pub fn peak_column(history: &History, width: u16, scale: f32) -> Option<usize> {
    let width = usize::from(width);
    if width == 0 || history.is_empty() {
        return None;
    }
    let samples = history.downsample_norm(width.saturating_mul(2), scale);
    let mut best: Option<(usize, f32)> = None;
    for (col, pair) in samples.chunks(2).enumerate().take(width) {
        let intensity = pair.iter().copied().fold(0.0_f32, f32::max);
        if intensity <= 0.02 {
            continue;
        }
        if best.is_none_or(|(_, b)| intensity > b) {
            best = Some((col, intensity));
        }
    }
    best.filter(|(_, intensity)| *intensity > 0.02)
        .map(|(col, _)| col)
}

fn stacked_cell(left: f32, right: f32, height: usize, bias: f32) -> Vec<char> {
    let mut out = vec!['\u{2800}'; height];
    for (row, slot) in out.iter_mut().enumerate() {
        let band_hi = 1.0 - row as f32 / height as f32;
        let band_lo = 1.0 - (row + 1) as f32 / height as f32;
        let l = level_in_band(left, band_lo, band_hi, bias);
        let r = level_in_band(right, band_lo, band_hi, bias);
        *slot = braille_cell(l, r);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn bit_pattern_empty() {
        assert_eq!(braille_cell(0, 0), '\u{2800}');
    }

    #[test]
    fn bit_pattern_full() {
        assert_eq!(braille_cell(4, 4), '⣿');
        assert_eq!(braille_cell(4, 4), '\u{28ff}');
    }

    #[test]
    fn bit_pattern_left_one() {
        // left level 1 = bottom-left (dot 7)
        assert_eq!(braille_cell(1, 0), '⡀');
        assert_eq!(braille_cell(1, 0), '\u{2840}');
    }

    #[test]
    fn bit_pattern_right_full() {
        // right level 4 = dots 8,6,5,4
        assert_eq!(braille_cell(0, 4), '⢸');
        assert_eq!(braille_cell(0, 4), '\u{28b8}');
    }

    #[test]
    fn render_history_width_height() {
        let rows = render_history(&History::default(), 7, 3);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.chars().count(), 7);
        }
    }

    #[test]
    fn render_history_zero_width() {
        let rows = render_history(&History::default(), 0, 2);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(String::is_empty));
    }

    #[test]
    fn render_history_zero_height_becomes_one() {
        let rows = render_history(&History::default(), 3, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chars().count(), 3);
    }

    #[test]
    fn empty_history_is_blank_braille() {
        let rows = render_history(&History::default(), 4, 2);
        let blank = "\u{2800}".repeat(4);
        assert_eq!(rows, vec![blank.clone(), blank]);
        assert!(rows.iter().all(|row| row.chars().all(|c| c == '\u{2800}')));
    }

    #[test]
    fn full_history_fills_cells() {
        let mut history = History::with_capacity(8);
        for _ in 0..8 {
            history.push(1.0);
        }
        let rows = render_history(&history, 4, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], "⣿⣿⣿⣿");
        assert_eq!(rows[0].chars().count(), 4);
    }

    #[test]
    fn full_history_stacks_both_rows() {
        let mut history = History::with_capacity(8);
        for _ in 0..8 {
            history.push(1.0);
        }
        let rows = render_history(&history, 4, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], "⣿⣿⣿⣿");
        assert_eq!(rows[1], "⣿⣿⣿⣿");
    }

    #[test]
    fn short_history_pads_left() {
        let mut history = History::with_capacity(8);
        history.push(1.0);
        history.push(1.0);
        let rows = render_history(&history, 4, 1);
        assert_eq!(rows[0].chars().count(), 4);
        let chars: Vec<char> = rows[0].chars().collect();
        assert_eq!(chars[0], '\u{2800}');
        assert_eq!(chars[1], '\u{2800}');
        assert_eq!(chars[2], '\u{2800}');
        assert_eq!(chars[3], '⣿');
    }

    #[test]
    fn cells_carry_intensity() {
        let mut history = History::with_capacity(4);
        history.push(0.25);
        history.push(0.75);
        let rows = render_cells(&history, 1, 1, 1.0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 1);
        assert!((rows[0][0].intensity - 0.75).abs() < f32::EPSILON);
        assert_ne!(rows[0][0].glyph, '\u{2800}');
    }

    #[test]
    fn peak_column_marks_the_loudest_cell() {
        let mut history = History::with_capacity(8);
        for v in [0.1, 0.2, 0.05, 0.05, 0.9, 0.95, 0.1, 0.2] {
            history.push(v);
        }
        assert_eq!(peak_column(&history, 4, 1.0), Some(2));
    }

    #[test]
    fn peak_column_needs_data() {
        let history = History::with_capacity(8);
        assert_eq!(peak_column(&history, 4, 1.0), None);
        let mut flat = History::with_capacity(8);
        for _ in 0..8 {
            flat.push(0.0);
        }
        assert_eq!(peak_column(&flat, 4, 1.0), None);
    }

    #[test]
    fn tiny_value_lights_bottom_dot() {
        let mut history = History::with_capacity(4);
        history.push(0.04);
        history.push(0.04);
        let rows = render_cells(&history, 1, 4, 1.0);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0][0].glyph, '\u{2800}');
        assert_ne!(rows[3][0].glyph, '\u{2800}');
    }

    #[test]
    fn auto_scale_fills_height() {
        let mut history = History::with_capacity(4);
        history.push(80_000.0);
        history.push(80_000.0);
        let scale = history.scale(plottypus_core::Scale::Auto { floor: 1.0 });
        let rows = render_cells(&history, 1, 2, scale);
        assert_ne!(rows[0][0].glyph, '\u{2800}');
    }
}
