use plottypus_core::Surface;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    frame.render_widget(Paragraph::new(footer_line(view, theme)), area);
}

fn footer_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    if view.confirm_kill {
        let who = view.proc.selected_pid.map_or_else(
            || String::from(" kill?  "),
            |pid| format!(" kill pid {pid}?  "),
        );
        return Line::from(vec![
            Span::styled(who, theme.title()),
            Span::styled("y", theme.title()),
            Span::styled(" yes  ", theme.dim()),
            Span::styled("n", theme.title()),
            Span::styled(" no", theme.dim()),
        ]);
    }
    if let Some(status) = view.status {
        return Line::from(Span::styled(format!(" {status} "), theme.warn_style()));
    }

    let mut spans = vec![];
    if view.expanded.is_some() {
        key(&mut spans, theme, "esc", "home");
    } else {
        key(&mut spans, theme, "?", "help");
        if view.surface == Surface::Glance {
            key(&mut spans, theme, "w", "work");
        }
        if process_actions_visible(view) {
            key(&mut spans, theme, "/", "search");
            key(&mut spans, theme, "x", "kill");
        }
        if view.frozen {
            key(&mut spans, theme, "f", "paused");
        }
    }
    key(&mut spans, theme, "q", "quit");
    Line::from(spans)
}

fn process_actions_visible(view: &AppView<'_>) -> bool {
    !view.searching
        && view.expanded.is_none()
        && view.surface != Surface::Glance
        && view.focus.panel() == Panel::Processes
}

fn key(spans: &mut Vec<Span<'static>>, theme: &Theme, k: &'static str, verb: &'static str) {
    spans.push(Span::raw("  "));
    spans.push(Span::styled(k.to_owned(), theme.title()));
    spans.push(Span::styled(format!(" {verb}"), theme.dim()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::Focus;
    use crate::widgets::tests_support::fixture;

    fn text(view: &AppView<'_>) -> String {
        footer_line(view, &Theme::default())
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn work_footer_is_quiet_without_a_selection_context() {
        let mut fx = fixture("");
        fx.focus = crate::widgets::Focus::Cpu;
        let t = text(&fx.view());
        assert!(t.contains('?') && t.contains("help"), "{t}");
        assert!(t.contains('q'), "{t}");
        assert!(!t.contains("search"), "{t}");
        assert!(!t.contains("kill"), "{t}");
        assert!(!t.contains("expand"), "{t}");
        assert!(!t.contains("resize"), "{t}");
    }

    #[test]
    fn kill_chip_only_over_the_process_table() {
        let mut fx = fixture("");
        fx.focus = Focus::Processes;
        let t = text(&fx.view());
        assert!(t.contains("search") && t.contains("kill"), "{t}");
        fx.focus = Focus::Mem;
        assert!(!text(&fx.view()).contains("kill"));
    }

    #[test]
    fn expanded_shows_home() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        let t = text(&fx.view());
        assert!(t.contains("home"), "{t}");
        assert!(!t.contains("kill"), "{t}");
    }

    #[test]
    fn paused_chip_when_frozen() {
        let mut fx = fixture("");
        fx.frozen = true;
        assert!(text(&fx.view()).contains("paused"));
    }

    #[test]
    fn confirm_shows_pid() {
        let mut fx = fixture("");
        fx.confirm_kill = true;
        fx.proc.selected_pid = Some(904);
        let t = text(&fx.view());
        assert!(t.contains("904"));
        assert!(t.contains("yes"));
    }
}
