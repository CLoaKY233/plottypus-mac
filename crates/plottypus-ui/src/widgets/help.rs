use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::theme::Theme;
use crate::widgets::AppView;

const HELP: [&str; 14] = [
    "tab / shift-tab          move between boxes",
    "enter                    expand focused box",
    "esc                      close expand / help / search",
    "click ↗                  expand that box",
    "click a box              focus it",
    "drag the gutter          resize process column",
    "click ×                  back to home",
    "/                        search processes",
    "x                        kill selected (then y/n)",
    "s                        settings (interval, panes)",
    "f                        pause live updates",
    "g / w                    glance / work",
    "q                        quit",
    "",
];

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme) {
    popup(frame, area, " help ", &HELP, theme);
}

pub fn render_settings(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let interval = format!("1  interval     {} ms   ( [ ] or 1/2/3 )", view.interval_ms);
    let gpu = format!("2  gpu          {}", on_off(view.show_gpu));
    let net = format!("3  network      {}", on_off(view.show_net));
    let cores = format!("4  per-core     {}", on_off(view.show_cores));
    let disk = format!("5  disk         {}", on_off(view.show_disk));
    let fans = format!("6  fans         {}", on_off(view.show_fans));
    let lines = [
        interval.as_str(),
        gpu.as_str(),
        net.as_str(),
        cores.as_str(),
        disk.as_str(),
        fans.as_str(),
        "",
        "esc  close",
    ];
    popup(frame, area, " settings ", &lines, theme);
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn popup(frame: &mut Frame, area: Rect, title: &str, lines: &[&str], theme: &Theme) {
    let text: Vec<Line> = lines
        .iter()
        .map(|s| Line::from(Span::styled((*s).to_owned(), theme.fg())))
        .collect();
    let width = 56.min(area.width.saturating_sub(2));
    let height = u16::try_from(text.len() + 2).unwrap_or(8).min(area.height);
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(text).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(title.to_owned())
                .border_style(theme.border(true))
                .style(ratatui::style::Style::default().bg(theme.bg).fg(theme.fg)),
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
}
