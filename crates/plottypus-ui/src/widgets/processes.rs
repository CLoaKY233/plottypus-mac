use plottypus_core::{Process, bytes_short};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use crate::chrome::{panel_block, push_kv, push_token};
use crate::layout::Panel;
use crate::spark;
use crate::theme::Theme;
use crate::widgets::{AppView, Focus};

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let rows = filtered(view);
    let selected = selected_index(view, &rows);
    let title = Line::from(Span::styled(format!(" proc  {}", rows.len()), theme.dim()));
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

    let needle = view.proc.filter.to_ascii_lowercase();
    let hit_style = Style::default().fg(theme.hi);
    let table_rows = rows.iter().map(|proc| {
        let gpu = if proc.gpu > 0.0 {
            format!("{:.1}", proc.gpu)
        } else {
            String::from("—")
        };
        let mut cells = vec![
            Cell::from(Line::from(matched_spans(
                &proc.pid.to_string(),
                &needle,
                theme.fg(),
                hit_style,
            ))),
            Cell::from(Line::from(matched_spans(
                &proc.name,
                &needle,
                theme.fg(),
                hit_style,
            ))),
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

pub enum DetailAction {
    Term,
    Kill,
    Interrupt,
    Close,
}

pub fn detail_rect(area: Rect) -> Rect {
    let width = 72.min(area.width.saturating_sub(2));
    let height = 11.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub fn detail_actions(area: Rect) -> Vec<(Rect, DetailAction)> {
    let rect = detail_rect(area);
    let footer_y = rect.y + rect.height - 2;
    let labels = [
        (" t term", DetailAction::Term),
        (" k kill", DetailAction::Kill),
        (" i interrupt", DetailAction::Interrupt),
        (" esc close", DetailAction::Close),
    ];
    let mut out = Vec::new();
    let mut cursor = rect.x.saturating_add(1);
    for (label, action) in labels {
        let w = label.chars().count() as u16;
        out.push((
            Rect {
                x: cursor,
                y: footer_y,
                width: w,
                height: 1,
            },
            action,
        ));
        cursor = cursor.saturating_add(w + 3);
    }
    out
}

fn started_ago(start_unix: i64, now_unix: i64) -> String {
    let secs = (now_unix - start_unix).clamp(0, i64::MAX);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d", secs / 86_400)
    }
}

fn elide_middle(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_owned();
    }
    let keep = max.saturating_sub(1) / 2;
    let head: String = path.chars().take(keep).collect();
    let tail: String = path.chars().skip(path.chars().count() - keep).collect();
    format!("{head}…{tail}")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn render_detail(frame: &mut Frame, area: Rect, view: &AppView<'_>, pid: u32, theme: &Theme) {
    let Some(proc) = view.snapshot.processes.iter().find(|p| p.pid == pid) else {
        return;
    };
    let now = now_unix();
    let parent: String = view
        .snapshot
        .processes
        .iter()
        .find(|p| p.pid == proc.ppid)
        .map_or_else(|| String::from("—"), |p| p.name.clone());

    let rect = detail_rect(area);
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(Line::from(Span::styled(" process", theme.dim())))
        .border_style(theme.border(true));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let spark_h = u16::from(!proc.cpu_spark.is_empty() && inner.height >= 6);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(spark_h),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let mut head = vec![Span::styled(proc.name.clone(), big_style(theme))];
    push_token(&mut head, format!("@{}", proc.user), theme.dim());
    frame.render_widget(Paragraph::new(Line::from(head)), rows[0]);

    if let Some(command) = &proc.command {
        let max = usize::from(inner.width.saturating_sub(1)).max(8);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                elide_middle(command, max),
                theme.fg(),
            ))),
            rows[1],
        );
    }

    if spark_h > 0 && rows.len() > 2 {
        render_cpu_spark(frame, rows[2], &proc.cpu_spark, theme);
    }

    let gpu = if proc.gpu > 0.0 {
        format!("{:.1}", proc.gpu)
    } else {
        String::from("unmeasured")
    };
    let state = if proc.status.is_empty() {
        String::from("—")
    } else {
        proc.status.to_owned()
    };
    let mut facts = Vec::new();
    push_kv(
        &mut facts,
        theme,
        "cpu",
        format!("{:.1}%", proc.cpu),
        theme.cpu(),
    );
    push_kv(
        &mut facts,
        theme,
        "mem",
        bytes_short(proc.mem_bytes),
        theme.fg(),
    );
    push_kv(
        &mut facts,
        theme,
        "threads",
        proc.threads.to_string(),
        theme.fg(),
    );
    push_kv(&mut facts, theme, "gpu", gpu, theme.dim());
    push_kv(&mut facts, theme, "state", state, theme.title());
    push_kv(
        &mut facts,
        theme,
        "up",
        started_ago(proc.start_unix, now),
        theme.fg(),
    );
    push_kv(&mut facts, theme, "pid", proc.pid.to_string(), theme.fg());
    push_kv(&mut facts, theme, "ppid", parent, theme.dim());
    let facts_row = rows[3];
    let wrapped = wrap_spans(facts, usize::from(facts_row.width.max(1)));
    frame.render_widget(Paragraph::new(wrapped), facts_row);
    render_action_footer(frame, rows[4], theme);
}

fn render_cpu_spark(frame: &mut Frame, area: Rect, samples: &[f32], theme: &Theme) {
    if area.width == 0 || area.height == 0 || samples.is_empty() {
        return;
    }
    let mut history = plottypus_core::History::with_capacity(samples.len().max(1));
    for sample in samples {
        history.push((*sample / 100.0).clamp(0.0, 1.0));
    }
    frame.render_widget(
        spark::widget_scaled(&history, area.width, 1.0, theme.cpu()),
        area,
    );
}

fn render_action_footer(frame: &mut Frame, footer: Rect, theme: &Theme) {
    let mut spans = Vec::new();
    for (key, verb) in [
        ("t", "term"),
        ("k", "kill"),
        ("i", "interrupt"),
        ("esc", "close"),
    ] {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(key.to_owned(), theme.title()));
        spans.push(Span::styled(format!(" {verb}"), theme.dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), footer);
}

fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default()];
    let mut used = 1usize;
    for span in spans {
        let len = span.content.chars().count();
        if used > 1 && used + len > width {
            lines.push(Line::default());
            used = 1;
            if span.content.starts_with(' ') {
                let trimmed = span.content.trim_start().to_owned();
                if trimmed.is_empty() {
                    continue;
                }
                push_to_last(&mut lines, Span::styled(trimmed, span.style), &mut used);
                continue;
            }
        }
        push_to_last(&mut lines, span, &mut used);
    }
    lines
}

fn push_to_last(lines: &mut [Line<'static>], span: Span<'static>, used: &mut usize) {
    let len = span.content.chars().count();
    if let Some(last) = lines.last_mut() {
        last.spans.push(span);
    }
    *used += len;
}

fn big_style(theme: &Theme) -> Style {
    use ratatui::style::Modifier;
    theme.title().add_modifier(Modifier::BOLD)
}

fn matched_spans(text: &str, needle: &str, base: Style, hit: Style) -> Vec<Span<'static>> {
    if needle.is_empty() {
        return vec![Span::styled(text.to_owned(), base)];
    }
    let lower = text.to_ascii_lowercase();
    match lower.find(needle) {
        None => vec![Span::styled(text.to_owned(), base)],
        Some(at) => {
            let start = text[..at].chars().count();
            let len = needle.chars().count();
            let chars: Vec<char> = text.chars().collect();
            let head: String = chars[..start].iter().collect();
            let mid: String = chars[start..start + len].iter().collect();
            let tail: String = chars[start + len..].iter().collect();
            vec![
                Span::styled(head, base),
                Span::styled(mid, hit),
                Span::styled(tail, base),
            ]
        }
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
        Span::styled(format!("  {} procs", rows.len()), theme.dim()),
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
        .filter(|proc| {
            needle.is_empty()
                || proc.name.to_ascii_lowercase().contains(&needle)
                || proc.pid.to_string().contains(&needle)
        })
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
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

    #[test]
    fn numeric_filter_matches_pids_too() {
        let mut fx = fixture("");
        fx.snap.processes = vec![process(904, "Xcode", 1.0), process(1234, "Finder", 2.0)];
        fx.proc.filter = String::from("904");
        let view = fx.view();
        let rows = filtered(&view);
        let names: Vec<&str> = rows.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Xcode"]);
    }

    #[test]
    fn matches_are_highlighted() {
        let theme = Theme::default();
        let hit = ratatui::style::Style::default().fg(theme.hi);
        let spans = matched_spans("WindowServer", "win", theme.fg(), hit);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content.as_ref(), "Win");
        assert_eq!(spans[1].style.fg, Some(theme.hi));
        assert_eq!(spans[0].style.fg, Some(theme.fg));
        let plain = matched_spans("Finder", "zzz", theme.fg(), hit);
        assert_eq!(plain.len(), 1);
    }

    #[test]
    fn started_ago_steps() {
        assert_eq!(started_ago(100, 140), "40s");
        assert_eq!(started_ago(0, 180), "3m");
        assert_eq!(started_ago(0, 7_200), "2h0m");
    }

    #[test]
    fn elide_keeps_head_and_tail() {
        let path = "/Applications/Xcode.app/Contents/MacOS/Xcode";
        let short = elide_middle(path, 16);
        assert!(short.contains('…'), "{short}");
        assert!(short.starts_with("/App"), "{short}");
        assert!(short.ends_with("code"), "{short}");
        assert_eq!(elide_middle("short", 16), "short");
    }

    #[test]
    fn detail_paints_identity_and_actions() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut fx = fixture("");
        fx.snap.processes[0].user = String::from("cloaky");
        fx.snap.processes[0].command = Some(String::from("/Applications/Xcode.app/Xcode"));
        fx.snap.processes[0].status = "running";
        fx.snap.processes[0].start_unix = 1_700_000_000;
        fx.snap.processes[0].cpu_spark = vec![1.0, 4.0, 12.0, 8.0];
        fx.detail_pid = Some(904);
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
            text.push('\n');
        }
        assert!(text.contains("Xcode"), "{text}");
        assert!(text.contains("@cloaky"), "{text}");
        assert!(text.contains("running"), "{text}");
        assert!(text.contains("term"), "{text}");
        assert!(text.contains("interrupt"), "{text}");
        assert!(!text.contains("%%"), "{text}");
    }
}
