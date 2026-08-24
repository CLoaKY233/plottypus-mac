use plottypus_core::{Process, bytes_short};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use crate::chrome::panel_block;
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::{AppView, Focus};

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let rows = filtered(view);
    let selected = selected_index(view, &rows);
    let title = Line::from(Span::styled(format!(" proc  {} ", rows.len()), theme.dim()));
    let block = panel_block(
        Panel::Processes,
        title,
        view.is_focused(Panel::Processes),
        view.is_expanded(Panel::Processes),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(search_line(view, &rows, theme)), chunks[0]);

    let table_rows = rows.iter().map(|proc| {
        let gpu = if proc.gpu > 0.0 {
            format!("{:.1}", proc.gpu)
        } else {
            String::from("—")
        };
        let mut cells = vec![
            Cell::from(proc.pid.to_string()),
            Cell::from(proc.name.as_str()),
            Cell::from(format!("{:.1}", proc.cpu)),
            Cell::from(gpu),
            Cell::from(bytes_short(proc.mem_bytes)),
        ];
        if view.show_threads {
            cells.push(Cell::from(proc.threads.to_string()));
        }
        Row::new(cells)
    });

    let mut widths = vec![
        Constraint::Length(7),
        Constraint::Fill(1),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(8),
    ];
    let mut headers = vec!["pid", "name", "cpu%", "gpu", "mem"];
    if view.show_threads {
        widths.push(Constraint::Length(5));
        headers.push("thr");
    }

    let table = Table::new(table_rows, widths)
        .header(Row::new(headers).style(theme.dim()))
        .row_highlight_style(theme.selected())
        .column_spacing(1);

    let mut state = TableState::default();
    if !rows.is_empty() {
        state.select(Some(selected));
    }
    if chunks.len() > 1 && chunks[1].height > 0 {
        frame.render_stateful_widget(table, chunks[1], &mut state);
    }
    if let Some(pid) = view.detail_pid {
        render_detail(frame, area, view, pid, theme);
    }
}

fn render_detail(frame: &mut Frame, area: Rect, view: &AppView<'_>, pid: u32, theme: &Theme) {
    let Some(proc) = view.snapshot.processes.iter().find(|p| p.pid == pid) else {
        return;
    };
    let parent = view
        .snapshot
        .processes
        .iter()
        .find(|p| p.pid == proc.ppid)
        .map_or("—", |p| p.name.as_str());
    let gpu = if proc.gpu > 0.0 {
        format!("{:.1}%", proc.gpu)
    } else {
        String::from("unmeasured")
    };
    let lines = [
        format!(" {}   pid {}", proc.name, proc.pid),
        format!(" ppid {} ({parent})", proc.ppid),
        format!(
            " cpu {:.1}%   mem {}   threads {}",
            proc.cpu,
            bytes_short(proc.mem_bytes),
            proc.threads
        ),
        format!(" gpu {gpu}"),
        String::from(" enter / click again already selected · esc close"),
    ];
    let width = 48.min(area.width.saturating_sub(2));
    let height = 7.min(area.height);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(ratatui::widgets::Clear, rect);
    frame.render_widget(
        Paragraph::new(
            lines
                .into_iter()
                .map(|s| Line::from(Span::styled(s, theme.fg())))
                .collect::<Vec<_>>(),
        )
        .block(
            ratatui::widgets::Block::bordered()
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" process ")
                .border_style(theme.border(true))
                .style(ratatui::style::Style::default().bg(theme.bg)),
        ),
        rect,
    );
}

fn search_line(view: &AppView<'_>, rows: &[Process], theme: &Theme) -> Line<'static> {
    let active = view.searching || matches!(view.focus, Focus::Search);
    let mut query = view.proc.filter.clone();
    if active {
        query.push('▌');
    } else if query.is_empty() {
        query = String::from("type to filter");
    }
    let label = if active { " search " } else { " / search " };
    Line::from(vec![
        Span::styled(label, if active { theme.title() } else { theme.dim() }),
        Span::styled(query, if active { theme.title() } else { theme.dim() }),
        Span::styled(format!("   {} procs", rows.len()), theme.dim()),
    ])
}

#[must_use]
pub fn selected_index(view: &AppView<'_>, rows: &[Process]) -> usize {
    if let Some(pid) = view.proc.selected_pid
        && let Some(i) = rows.iter().position(|p| p.pid == pid)
    {
        return i;
    }
    view.proc.selected.min(rows.len().saturating_sub(1))
}

#[must_use]
pub fn filtered(view: &AppView<'_>) -> Vec<Process> {
    filter_sort_by(&view.snapshot.processes, &view.proc.filter, view.sort)
}

#[must_use]
#[cfg(test)]
pub fn filter_sort(processes: &[Process], filter: &str) -> Vec<Process> {
    filter_sort_by(processes, filter, plottypus_core::ProcSort::Cpu)
}

#[must_use]
pub fn filter_sort_by(
    processes: &[Process],
    filter: &str,
    sort: plottypus_core::ProcSort,
) -> Vec<Process> {
    let needle = filter.to_ascii_lowercase();
    let mut rows: Vec<Process> = processes
        .iter()
        .filter(|proc| needle.is_empty() || proc.name.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect();
    match sort {
        plottypus_core::ProcSort::Cpu => rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
        plottypus_core::ProcSort::Mem => rows.sort_by_key(|a| std::cmp::Reverse(a.mem_bytes)),
        plottypus_core::ProcSort::Pid => rows.sort_by_key(|p| p.pid),
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::{fixture, process};

    #[test]
    fn empty_filter_returns_all_sorted_by_cpu_desc() {
        let mut fx = fixture("");
        fx.snap.processes = vec![
            process(1, "WindowServer", 5.2),
            process(2, "Xcode", 48.1),
            process(3, "plottypus", 0.6),
        ];
        let rows = filtered(&fx.view());
        let names: Vec<&str> = rows.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Xcode", "WindowServer", "plottypus"]);
    }

    #[test]
    fn filter_is_case_insensitive_on_name() {
        let mut fx = fixture("xCoDe");
        fx.snap.processes = vec![
            process(1, "WindowServer", 5.2),
            process(2, "Xcode", 48.1),
            process(3, "xcodebuild", 12.0),
        ];
        let rows = filtered(&fx.view());
        let names: Vec<&str> = rows.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Xcode", "xcodebuild"]);
    }

    #[test]
    fn follows_pid_not_index() {
        let mut fx = fixture("");
        fx.snap.processes = vec![process(1, "a", 1.0), process(2, "b", 9.0)];
        fx.proc.selected_pid = Some(1);
        let rows = filtered(&fx.view());
        assert_eq!(selected_index(&fx.view(), &rows), 1);
    }

    #[test]
    fn unknown_filter_is_empty() {
        assert!(filter_sort(&[process(1, "Finder", 1.0)], "zzz").is_empty());
    }
}
