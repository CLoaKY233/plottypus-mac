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
    if view.expanded.is_some() {
        return expanded_line(view, theme);
    }
    if view.surface == Surface::Glance {
        return Line::from(vec![
            Span::styled(" ?", theme.title()),
            Span::styled(" help", theme.dim()),
            Span::styled("   w", theme.title()),
            Span::styled(" processes", theme.dim()),
            Span::styled("   q", theme.title()),
            Span::styled(" quit", theme.dim()),
        ]);
    }
    work_line(theme, true)
}

fn expanded_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(" esc", theme.title()),
        Span::styled(" / ", theme.dim()),
        Span::styled("×", theme.title()),
        Span::styled("  home", theme.dim()),
    ];
    if view.surface == Surface::Glance && view.expanded != Some(Panel::Processes) {
        spans.extend([
            Span::styled("   ?", theme.title()),
            Span::styled(" help", theme.dim()),
            Span::styled("   w", theme.title()),
            Span::styled(" processes", theme.dim()),
            Span::styled("   q", theme.title()),
            Span::styled(" quit", theme.dim()),
        ]);
        return Line::from(spans);
    }
    spans.push(Span::styled("  ", theme.dim()));
    spans.extend(work_spans(theme, view.expanded == Some(Panel::Processes)));
    Line::from(spans)
}

fn work_line(theme: &Theme, kill: bool) -> Line<'static> {
    Line::from(work_spans(theme, kill))
}

fn work_spans(theme: &Theme, kill: bool) -> Vec<Span<'static>> {
    let mut spans = vec![
        Span::styled(" ?", theme.title()),
        Span::styled(" help", theme.dim()),
        Span::styled("   tab", theme.title()),
        Span::styled(" box", theme.dim()),
        Span::styled("   ↗", theme.title()),
        Span::styled(" expand", theme.dim()),
        Span::styled("   drag", theme.title()),
        Span::styled(" resize", theme.dim()),
        Span::styled("   /", theme.title()),
        Span::styled(" search", theme.dim()),
    ];
    if kill {
        spans.extend([
            Span::styled("   x", theme.title()),
            Span::styled(" kill", theme.dim()),
        ]);
    }
    spans.extend([
        Span::styled("   s", theme.title()),
        Span::styled(" settings", theme.dim()),
        Span::styled("   q", theme.title()),
        Span::styled(" quit", theme.dim()),
    ]);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;

    #[test]
    fn work_footer_lists_verbs() {
        let fx = fixture("");
        let theme = Theme::default();
        let text: String = footer_line(&fx.view(), &theme)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains('?'));
        assert!(text.contains('/'));
        assert!(text.contains('x'));
        assert!(text.contains('s'));
        assert!(text.contains('q'));
    }

    #[test]
    fn confirm_shows_pid() {
        let mut fx = fixture("");
        fx.confirm_kill = true;
        fx.proc.selected_pid = Some(904);
        let theme = Theme::default();
        let text: String = footer_line(&fx.view(), &theme)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("904"));
        assert!(text.contains("yes"));
    }
}
