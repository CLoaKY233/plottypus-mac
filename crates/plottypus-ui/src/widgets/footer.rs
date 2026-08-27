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
        let who = confirm_who(view);
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
    if view.detail_pid.is_some() {
        key(&mut spans, theme, "t", "term");
        key(&mut spans, theme, "k", "kill");
        key(&mut spans, theme, "i", "interrupt");
        key(&mut spans, theme, "esc", "close");
    } else if view.expanded.is_some() {
        key(&mut spans, theme, "tab", "related");
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

#[must_use]
pub fn footer_hit(
    view: &AppView<'_>,
    area: Rect,
    col: u16,
    row: u16,
) -> Option<crate::layout::Hit> {
    if row != area.y || col < area.x || col >= area.x.saturating_add(area.width) {
        return None;
    }
    if view.confirm_kill {
        return confirm_hit(view, area, col);
    }
    let mut x = area.x;
    for (k, verb, hit) in footer_chips(view) {
        let w = 2 + k.len() as u16 + 1 + verb.len() as u16;
        if col >= x && col < x.saturating_add(w) {
            return Some(hit);
        }
        x = x.saturating_add(w);
    }
    None
}

fn confirm_who(view: &AppView<'_>) -> String {
    let signal = if view.confirm_signal.is_empty() {
        "TERM"
    } else {
        view.confirm_signal
    };
    view.confirm_pid.map_or_else(
        || format!(" {signal}?  "),
        |pid| {
            let name = view
                .snapshot
                .processes
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.name.as_str())
                .filter(|n| !n.is_empty());
            match name {
                Some(name) => format!(" {signal} {name} ({pid})?  "),
                None => format!(" {signal} pid {pid}?  "),
            }
        },
    )
}

fn confirm_hit(view: &AppView<'_>, area: Rect, col: u16) -> Option<crate::layout::Hit> {
    use crate::layout::Hit;
    let who_w = u16::try_from(confirm_who(view).chars().count()).unwrap_or(0);
    let y_x = area.x.saturating_add(who_w);
    if col == y_x {
        return Some(Hit::ConfirmYes);
    }
    let n_x = y_x.saturating_add(7);
    if col == n_x {
        return Some(Hit::ConfirmNo);
    }
    None
}

fn footer_chips(view: &AppView<'_>) -> Vec<(&'static str, &'static str, crate::layout::Hit)> {
    use crate::layout::Hit;
    let mut chips = Vec::new();
    if view.detail_pid.is_some() {
        chips.push(("t", "term", Hit::DetailTerm));
        chips.push(("k", "kill", Hit::DetailKill));
        chips.push(("i", "interrupt", Hit::DetailInterrupt));
        chips.push(("esc", "close", Hit::ExpandClose));
    } else if view.expanded.is_some() {
        chips.push(("esc", "home", Hit::ExpandClose));
    } else {
        chips.push(("?", "help", Hit::Help));
        if process_actions_visible(view) {
            chips.push(("/", "search", Hit::Search));
            chips.push(("x", "kill", Hit::Kill));
        }
    }
    chips.push(("q", "quit", Hit::Quit));
    chips
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
        fx.confirm_pid = Some(904);
        fx.confirm_signal = "KILL";
        fx.proc.selected_pid = Some(1);
        let t = text(&fx.view());
        assert!(t.contains("904"), "{t}");
        assert!(t.contains("KILL"), "{t}");
        assert!(t.contains("Xcode"), "{t}");
        assert!(!t.contains("TERM pid 1"), "{t}");
        assert!(t.contains("yes"));
    }

    #[test]
    fn footer_hit_only_paints_visible_chips() {
        use crate::layout::Hit;
        let mut fx = fixture("");
        fx.focus = Focus::Cpu;
        let area = Rect::new(0, 20, 80, 1);
        assert_eq!(footer_hit(&fx.view(), area, 2, 20), Some(Hit::Help));
        assert_eq!(footer_hit(&fx.view(), area, 30, 20), None);
        fx.focus = Focus::Processes;
        assert_eq!(footer_hit(&fx.view(), area, 14, 20), Some(Hit::Search));
        fx.detail_pid = Some(904);
        assert_eq!(footer_hit(&fx.view(), area, 2, 20), Some(Hit::DetailTerm));
    }

    #[test]
    fn detail_footer_lists_signals() {
        let mut fx = fixture("");
        fx.detail_pid = Some(904);
        let t = text(&fx.view());
        assert!(t.contains("term"), "{t}");
        assert!(t.contains("kill"), "{t}");
        assert!(t.contains("interrupt"), "{t}");
        assert!(t.contains("close"), "{t}");
    }
}
