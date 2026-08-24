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
        Row::new(vec![
            Cell::from(proc.pid.to_string()),
            Cell::from(proc.name.as_str()),
            Cell::from(format!("{:.1}", proc.cpu)),
            Cell::from(bytes_short(proc.mem_bytes)),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(7),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(["pid", "name", "cpu%", "mem"]).style(theme.dim()))
    .row_highlight_style(theme.selected())
    .column_spacing(1);

    let mut state = TableState::default();
    if !rows.is_empty() {
        state.select(Some(selected));
    }
    if chunks.len() > 1 && chunks[1].height > 0 {
        frame.render_stateful_widget(table, chunks[1], &mut state);
    }
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
    filter_sort(&view.snapshot.processes, &view.proc.filter)
}

#[must_use]
pub fn filter_sort(processes: &[Process], filter: &str) -> Vec<Process> {
    let needle = filter.to_ascii_lowercase();
    let mut rows: Vec<Process> = processes
        .iter()
        .filter(|proc| needle.is_empty() || proc.name.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect();
    rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
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
