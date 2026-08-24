use plottypus_core::{DiskSnapshot, Scale, bytes_short};
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
    let disk = &view.snapshot.disk;
    let block = panel_block(
        Panel::Disk,
        title(disk, theme),
        view.is_focused(Panel::Disk),
        view.is_expanded(Panel::Disk),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if disk.volumes.is_empty() {
        return;
    }

    let extras = extra_lines(disk, inner.height >= 6);
    let extra_h = u16::try_from(extras.len())
        .unwrap_or(0)
        .min(inner.height.saturating_sub(2));
    let rows = if extra_h > 0 {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(extra_h),
        ])
        .split(inner)
    } else if inner.height >= 2 {
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner)
    } else {
        Layout::vertical([Constraint::Fill(1)]).split(inner)
    };

    render_fill_bar(frame, rows[0], disk.used_ratio(), theme.disk);
    if rows.len() > 1 {
        render_scaled_graph(
            frame,
            rows[1],
            Graph {
                history: view.disk_history,
                accent: theme.disk,
                theme,
                scale: Scale::Fixed(1.0),
                axis: Axis::Percent,
                ink: GraphInk::Load(view.snapshot.thermal),
            },
        );
    }
    if let Some(extra) = rows.get(2) {
        let lines: Vec<Line> = extras
            .into_iter()
            .map(|s| Line::from(Span::styled(s, theme.dim())))
            .collect();
        frame.render_widget(Paragraph::new(lines), *extra);
    }
}

fn title(disk: &DiskSnapshot, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(" disk  ".to_owned(), theme.dim())];
    match disk.primary() {
        None => spans.push(Span::styled("—", theme.dim())),
        Some(vol) => {
            push_token(&mut spans, vol.name.clone(), theme.dim());
            spans.push(Span::styled(bytes_short(vol.used_bytes), theme.title()));
            spans.push(Span::styled(" / ".to_owned(), theme.dim()));
            spans.push(Span::styled(bytes_short(vol.total_bytes), theme.title()));
        }
    }
    Line::from(spans)
}

fn extra_lines(disk: &DiskSnapshot, show_volumes: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if disk.read_bps > 0 || disk.write_bps > 0 {
        lines.push(format!(
            "↓{}/s  ↑{}/s",
            bytes_short(disk.read_bps),
            bytes_short(disk.write_bps)
        ));
    }
    if show_volumes {
        for vol in disk
            .volumes
            .iter()
            .skip(usize::from(disk.primary().is_some()))
        {
            if disk.primary().is_some_and(|p| p.mount == vol.mount) {
                continue;
            }
            lines.push(format!(
                "{}  {} / {}",
                vol.name,
                bytes_short(vol.used_bytes),
                bytes_short(vol.total_bytes)
            ));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use plottypus_core::DiskVolume;

    #[test]
    fn title_dash_when_empty() {
        let text: String = title(&DiskSnapshot::default(), &Theme::default())
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("disk"));
        assert!(text.contains('—'));
    }

    #[test]
    fn title_primary_volume() {
        let disk = DiskSnapshot {
            volumes: vec![DiskVolume {
                name: String::from("Macintosh HD"),
                mount: String::from("/"),
                used_bytes: 400 * 1024 * 1024 * 1024,
                total_bytes: 926 * 1024 * 1024 * 1024,
            }],
            read_bps: 0,
            write_bps: 0,
        };
        let text: String = title(&disk, &Theme::default())
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("Macintosh HD"));
        assert!(text.contains("400.0G"));
        assert!(text.contains("926.0G"));
    }
}
