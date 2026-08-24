use plottypus_core::{History, Scale, bits_per_sec};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::braille::render_cells;
use crate::layout::Panel;
use crate::spark;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    None,
    Percent,
    Bits,
}

#[must_use]
pub fn panel_block<'a>(
    _panel: Panel,
    title: Line<'a>,
    focused: bool,
    expanded: bool,
    theme: &Theme,
) -> Block<'a> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(focused || expanded))
        .style(Style::default().bg(theme.bg).fg(theme.fg))
        .title(title);
    if expanded {
        block.title(Line::from(Span::styled(" × ", theme.title())).right_aligned())
    } else {
        block
    }
}

pub fn render_scaled_graph(
    frame: &mut Frame,
    area: Rect,
    history: &History,
    accent: Color,
    theme: &Theme,
    scale: Scale,
    axis: Axis,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let max = history.scale(scale);
    if area.height == 1 {
        frame.render_widget(
            spark::widget_scaled(history, area.width, max, Style::default().fg(accent)),
            area,
        );
        if axis != Axis::None && area.width >= 8 {
            frame.render_widget(
                Paragraph::new(Span::styled(axis_label(max, axis), theme.dim())),
                area,
            );
        }
        return;
    }
    let rows = render_cells(history, area.width, area.height, max);
    for (i, row) in rows.iter().enumerate() {
        let y = area.y.saturating_add(i as u16);
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        let spans: Vec<Span> = row
            .iter()
            .map(|cell| {
                if cell.glyph == '\u{2800}' {
                    Span::raw(" ")
                } else {
                    Span::styled(cell.glyph.to_string(), Style::default().fg(accent))
                }
            })
            .collect();
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
        );
    }
    if axis != Axis::None && area.width >= 8 {
        frame.render_widget(
            Paragraph::new(Span::styled(format!(" {}", axis_label(max, axis)), theme.dim())),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width.min(10),
                height: 1,
            },
        );
    }
}

fn axis_label(value: f32, axis: Axis) -> String {
    match axis {
        Axis::Percent => format!("{:.0}%", (value * 100.0).round()),
        Axis::Bits => bits_per_sec(value.max(0.0) as u64),
        Axis::None => String::new(),
    }
}

pub fn render_fill_bar(frame: &mut Frame, area: Rect, ratio: f32, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = ((f32::from(area.width) * ratio).round() as u16).min(area.width);
    let mut line = String::new();
    for x in 0..area.width {
        line.push(if x < filled { '━' } else { '─' });
    }
    frame.render_widget(
        Paragraph::new(Span::styled(line, Style::default().fg(color))),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    #[test]
    fn expanded_block_has_close_mark() {
        let theme = Theme::default();
        let block = panel_block(
            Panel::Cpu,
            Line::from(Span::raw(" cpu ")),
            true,
            true,
            &theme,
        );
        let _ = block;
    }

    #[test]
    fn percent_axis_labels() {
        assert_eq!(axis_label(1.0, Axis::Percent), "100%");
        assert_eq!(axis_label(0.5, Axis::Percent), "50%");
    }
}
