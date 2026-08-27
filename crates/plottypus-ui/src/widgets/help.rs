use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::theme::Theme;
use crate::widgets::AppView;

const HELP: [&str; 16] = [
    "tab / shift-tab          move between boxes",
    "tab / ← →                related expand",
    "enter                    expand focused box",
    "esc                      close expand / help / search",
    "click ↗                  expand that box",
    "click a box              focus it",
    "drag the gutter          resize process column",
    "click ×                  back to home",
    "/                        search processes",
    "enter on a process       process details",
    "t / k / i                term / kill / interrupt in details",
    "x                        kill selected (then y/n)",
    "s                        settings (sampling, panes, processes)",
    "f                        pause live updates",
    "g / w                    glance / work",
    "q                        quit",
];

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme) {
    let lines: Vec<Line> = HELP
        .iter()
        .map(|s| Line::from(Span::styled((*s).to_owned(), theme.fg())))
        .collect();
    popup(frame, area, " help ", &lines, theme);
}

pub fn render_settings(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let lines: Vec<Line> = settings_lines(view)
        .into_iter()
        .map(|s| {
            let style = if is_section(&s) {
                theme.dim()
            } else {
                theme.fg()
            };
            Line::from(Span::styled(s, style))
        })
        .collect();
    popup(frame, area, " settings ", &lines, theme);
}

fn settings_lines(view: &AppView<'_>) -> Vec<String> {
    vec![
        String::from(" sampling"),
        format!(
            "1  interval     {} ms   [ / ]  0.25 / 0.5 / 1 s",
            view.interval_ms
        ),
        String::new(),
        String::from(" panes"),
        format!("2  gpu pane     {}", on_off(view.show_gpu)),
        format!("3  net pane     {}", on_off(view.show_net)),
        format!("5  disk pane    {}", on_off(view.show_disk)),
        format!("6  sensors      {}", on_off(view.show_fans)),
        format!("4  per-core     {}", on_off(view.show_cores)),
        String::new(),
        String::from(" processes"),
        format!("7  proc sort    {}", view.sort.label()),
        format!("8  threads col  {}", on_off(view.show_threads)),
        format!("9  proc tree    {}", on_off(view.show_tree)),
        String::new(),
        String::from(" ↗ expands a box · click a process for details"),
        String::from(" esc  close"),
    ]
}

fn is_section(line: &str) -> bool {
    matches!(line.trim(), "sampling" | "panes" | "processes")
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn popup(frame: &mut Frame, area: Rect, title: &str, lines: &[Line<'static>], theme: &Theme) {
    let width = 62.min(area.width.saturating_sub(2));
    let height = u16::try_from(lines.len() + 2).unwrap_or(8).min(area.height);
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines.to_vec()).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(title.to_owned())
                .border_style(theme.border(true))
                .style(ratatui::style::Style::default().fg(theme.fg)),
        ),
        rect,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_stays_inside() {
        let r = centered(Rect::new(0, 0, 20, 10), 40, 20);
        assert_eq!(r, Rect::new(0, 0, 20, 10));
    }

    #[test]
    fn help_lists_button_and_enter_expand() {
        let joined = HELP.join("\n");
        assert!(joined.contains("enter                    expand focused box"));
        assert!(joined.contains("click ↗                  expand that box"));
        assert!(joined.contains("click a box              focus it"));
        assert!(!joined.contains("click a box              expand"));
        assert!(!joined.contains("fullscreen"));
    }

    #[test]
    fn settings_has_sampling_panes_processes() {
        let fx = crate::widgets::tests_support::fixture("");
        let lines = settings_lines(&fx.view());
        let joined = lines.join("\n");
        let sampling = joined.find("sampling");
        let panes = joined.find("panes");
        let processes = joined.find("processes");
        assert!(sampling.is_some(), "{joined}");
        assert!(panes.is_some(), "{joined}");
        assert!(processes.is_some(), "{joined}");
        assert!(sampling < panes && panes < processes, "{joined}");
        assert!(joined.contains("interval"), "{joined}");
        assert!(joined.contains("[ / ]"), "{joined}");
        assert!(joined.contains("gpu pane"), "{joined}");
        assert!(joined.contains("proc sort"), "{joined}");
        assert!(is_section(" sampling"));
        assert!(is_section(" panes"));
        assert!(is_section(" processes"));
        assert!(!is_section("1  interval"));
    }
}
