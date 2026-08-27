use plottypus_core::{MemorySnapshot, Scale, bytes_short};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

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

    let ratio = mem_ratio(mem.used_bytes, mem.total_bytes);
    if inner.height >= 2 {
        render_scaled_graph(
            frame,
            inner,
            Graph {
                history: view.mem_history,
                accent: theme.mem,
                theme,
                scale: Scale::Fixed(1.0),
                axis: Axis::Percent,
                ink: GraphInk::Load(view.snapshot.thermal),
            },
        );
    } else {
        render_fill_bar(frame, inner, ratio, theme.mem);
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

#[cfg(test)]
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
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn compact_mem_no_bar_when_graph() {
        use crate::widgets::tests_support::fixture;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut fx = fixture("");
        fx.snap.memory.used_bytes = 18 * 1024 * 1024 * 1024;
        fx.snap.memory.total_bytes = 36 * 1024 * 1024 * 1024;
        fx.mem.push(0.4);
        fx.mem.push(0.5);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &fx.view(), &Theme::default()))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(
            !text.contains('━'),
            "used-ratio bar must not sit above the graph: {text}"
        );
    }
}
