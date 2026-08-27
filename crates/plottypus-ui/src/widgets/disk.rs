use plottypus_core::{DiskSnapshot, Scale, bytes_short};
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

    if inner.height >= 2 {
        render_scaled_graph(
            frame,
            inner,
            Graph {
                history: view.disk_history,
                accent: theme.disk,
                theme,
                scale: Scale::Auto { floor: 1_024.0 },
                axis: Axis::Bytes,
                ink: GraphInk::Flat,
            },
        );
    } else {
        render_fill_bar(frame, inner, disk.used_ratio(), theme.disk);
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
