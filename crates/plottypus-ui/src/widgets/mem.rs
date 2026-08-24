use plottypus_core::{MemorySnapshot, Scale, bytes_short};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{
    Axis, Graph, GraphInk, panel_block, push_token, render_fill_bar, render_scaled_graph,
};

use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let mem = &view.snapshot.memory;
    let block = panel_block(
        Panel::Mem,
        title(mem, theme),
        view.is_focused(Panel::Mem),
        view.is_expanded(Panel::Mem),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let specs = spec_items(mem);
    let show_specs = area.height >= 5 && !specs.is_empty();
    let (body, spec_col) = if show_specs && inner.width >= 50 {
        let cols = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(16),
        ])
        .split(inner);
        (cols[0], Some(cols[2]))
    } else {
        (inner, None)
    };

    let spec_h = if show_specs && spec_col.is_none() {
        let room = body.height.saturating_sub(2);
        u16::try_from(specs.len()).unwrap_or(0).min(room)
    } else {
        0
    };
    let rows = if spec_h > 0 {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(spec_h),
        ])
        .split(body)
    } else if body.height >= 2 {
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(body)
    } else {
        Layout::vertical([Constraint::Fill(1)]).split(body)
    };

    let ratio = mem_ratio(mem.used_bytes, mem.total_bytes);
    render_fill_bar(frame, rows[0], ratio, theme.mem);
    if rows.len() > 1 {
        render_scaled_graph(
            frame,
            rows[1],
            Graph {
                history: view.mem_history,
                accent: theme.mem,
                theme,
                scale: Scale::Fixed(1.0),
                axis: Axis::Percent,
                ink: GraphInk::Load(view.snapshot.thermal),
            },
        );
    }
    let spec_area = spec_col.or_else(|| rows.get(2).copied());
    if let Some(spec_area) = spec_area {
        let take = usize::from(spec_area.height);
        let lines: Vec<Line> = specs
            .iter()
            .take(take)
            .map(|s| Line::from(Span::styled(s.clone(), theme.dim())))
            .collect();
        frame.render_widget(Paragraph::new(lines), spec_area);
    }
}

fn title(mem: &MemorySnapshot, theme: &Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(" mem  ".to_owned(), theme.dim()),
        Span::styled(bytes_short(mem.used_bytes), theme.title()),
        Span::styled(" / ", theme.dim()),
        Span::styled(bytes_short(mem.total_bytes), theme.title()),
    ];
    push_token(&mut spans, String::from("●"), theme.pressure(mem.pressure));
    Line::from(spans)
}

fn spec_items(mem: &MemorySnapshot) -> Vec<String> {
    let mut items = Vec::new();
    if mem.wired_bytes > 0 {
        items.push(format!("wired {}", bytes_short(mem.wired_bytes)));
    }
    if mem.compressed_bytes > 0 {
        items.push(format!("compr {}", bytes_short(mem.compressed_bytes)));
    }
    if mem.cache_bytes > 0 {
        items.push(format!("cache {}", bytes_short(mem.cache_bytes)));
    }
    if mem.swap_used_bytes > 0 || mem.swap_total_bytes > 0 {
        items.push(format!(
            "swap {} / {}",
            bytes_short(mem.swap_used_bytes),
            bytes_short(mem.swap_total_bytes)
        ));
    }
    items
}

fn mem_ratio(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::Pressure;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn title_used_over_total_and_pressure_dot() {
        let mut fx = fixture("");
        fx.snap.memory.used_bytes = 18 * 1024 * 1024 * 1024;
        fx.snap.memory.total_bytes = 36 * 1024 * 1024 * 1024;
        fx.snap.memory.pressure = Pressure::Warn;
        let text = line_text(&title(&fx.snap.memory, &Theme::default()));
        assert!(text.contains("mem"));
        assert!(text.contains("18.0G"));
        assert!(text.contains("36.0G"));
        assert!(text.contains('●'));
        assert!(!text.contains("nominal"));
    }

    #[test]
    fn specs_only_nonzero() {
        let mut mem = MemorySnapshot::default();
        assert!(spec_items(&mem).is_empty());
        mem.wired_bytes = 8 * 1024 * 1024 * 1024;
        mem.swap_used_bytes = 512 * 1024 * 1024;
        mem.swap_total_bytes = 2 * 1024 * 1024 * 1024;
        let items = spec_items(&mem);
        assert_eq!(items, ["wired 8.0G", "swap 512M / 2.0G"]);
    }

    #[test]
    fn ratio_handles_empty_total() {
        assert!((mem_ratio(18, 36) - 0.5).abs() < f32::EPSILON);
        assert!((mem_ratio(1, 0) - 0.0).abs() < f32::EPSILON);
    }
}
