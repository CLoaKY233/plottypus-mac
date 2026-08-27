use plottypus_core::{Process, bytes_short};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use crate::chrome::{cell, panel_block, push_kv, push_token};
use crate::layout::Panel;
use crate::spark;
use crate::theme::Theme;
use crate::widgets::{AppView, Focus};

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let listed = listed(view);
    let selected = selected_index(view, listed.iter().map(|r| r.proc.pid));
    let title = Line::from(Span::styled(
        format!(" proc  {}", listed.len()),
        theme.dim(),
    ));
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
    frame.render_widget(
        Paragraph::new(search_line(view, listed.len(), theme)),
        chunks[0],
    );

    let needle = view.proc.filter.to_ascii_lowercase();
    let hit_style = Style::default().fg(theme.hi);
    let table_rows = listed.iter().map(|row| {
        let proc = &row.proc;
        let mut cells = vec![
            Cell::from(Line::from(matched_spans(
                &format!("{:>7}", proc.pid),
                &needle,
                theme.dim(),
                hit_style,
            ))),
            Cell::from(Line::from(matched_spans(
                &tree_name(&proc.name, row.depth),
                &needle,
                theme.fg(),
                hit_style,
            ))),
            Cell::from(Span::styled(format!("{:>6.1}", proc.cpu), theme.cpu())),
            Cell::from(Span::styled(
                format!("{:>7}", bytes_short(proc.mem_bytes)),
                theme.fg(),
            )),
        ];
        if view.show_threads {
            cells.push(Cell::from(Span::styled(
                format!("{:>4}", proc.threads),
                theme.dim(),
            )));
        }
        Row::new(cells)
    });

    let mut widths = vec![
        Constraint::Length(7),
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Length(8),
    ];
    let mut headers = vec![
        sort_header("pid", view.sort == plottypus_core::ProcSort::Pid),
        String::from("name"),
        sort_header("cpu%", view.sort == plottypus_core::ProcSort::Cpu),
        sort_header("mem", view.sort == plottypus_core::ProcSort::Mem),
    ];
    if view.show_threads {
        widths.push(Constraint::Length(5));
        headers.push(String::from(" thr"));
    }

    let table = Table::new(table_rows, widths)
        .header(Row::new(headers).style(theme.dim()))
        .row_highlight_style(theme.selected())
        .column_spacing(2);

    let mut state = TableState::default();
    if !listed.is_empty() {
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

const ACTION_CHIPS: [(&str, &str, DetailAction); 4] = [
    ("t", "term", DetailAction::Term),
    ("k", "kill", DetailAction::Kill),
    ("i", "interrupt", DetailAction::Interrupt),
    ("esc", "close", DetailAction::Close),
];

pub fn detail_rect(area: Rect) -> Rect {
    let pad = 1u16;
    let width = area.width.saturating_sub(pad.saturating_mul(2)).max(22);
    let room = area.height.saturating_sub(pad);
    let height = (area.height.saturating_mul(3) / 4)
        .clamp(16.min(room), room)
        .max(1);
    Rect {
        x: area.x.saturating_add(pad),
        y: area.y.saturating_add(pad),
        width,
        height,
    }
}

pub fn detail_actions(area: Rect) -> Vec<(Rect, DetailAction)> {
    let rect = detail_rect(area);
    let inner_x = rect.x.saturating_add(1);
    let footer_y = rect.y.saturating_add(rect.height.saturating_sub(2));
    let mut out = Vec::new();
    let mut cursor = inner_x;
    for (key, verb, action) in ACTION_CHIPS {
        let w = 3 + key.len() as u16 + 1 + verb.len() as u16;
        let right = rect.x.saturating_add(rect.width);
        if cursor >= right {
            break;
        }
        let width = w.min(right.saturating_sub(cursor));
        out.push((
            Rect {
                x: cursor,
                y: footer_y,
                width,
                height: 1,
            },
            action,
        ));
        cursor = cursor.saturating_add(w);
    }
    out
}

fn sort_header(label: &str, active: bool) -> String {
    if active {
        format!("{label}↓")
    } else {
        label.to_owned()
    }
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
    let rect = detail_rect(area);
    frame.render_widget(ratatui::widgets::Clear, rect);
    let title = Line::from(Span::styled(
        format!(" process  {}", proc.name),
        theme.dim(),
    ));
    let block = ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .border_style(theme.border(true));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    render_dossier(frame, inner, view, proc, theme);
}

fn render_dossier(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    proc: &Process,
    theme: &Theme,
) {
    let show_spark = !proc.cpu_spark.is_empty() && area.height >= 10;
    let show_family = area.height >= 12;
    let show_command = area.height >= 8;
    let mut weights = vec![Constraint::Length(5)];
    if show_command {
        weights.push(Constraint::Length(3));
    }
    if show_spark {
        weights.push(Constraint::Fill(2));
    }
    if show_family {
        weights.push(Constraint::Length(3));
    }
    weights.push(Constraint::Length(1));
    let rows = Layout::vertical(weights).split(area);
    let bands = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(rows[0]);
    paint_identity(frame, bands[0], view, proc, theme);
    paint_live(frame, bands[1], proc, theme);
    let mut i = 1;
    if show_command {
        paint_command(frame, rows[i], proc, theme);
        i += 1;
    }
    if show_spark {
        let plot = cell(frame, rows[i], "cpu", theme);
        render_cpu_spark(frame, plot, &proc.cpu_spark, theme);
        i += 1;
    }
    if show_family {
        paint_family(frame, rows[i], view, proc, theme);
        i += 1;
    }
    if let Some(actions) = rows.get(i) {
        render_action_footer(frame, *actions, theme);
    }
}

fn paint_identity(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    proc: &Process,
    theme: &Theme,
) {
    let parent = view
        .snapshot
        .processes
        .iter()
        .find(|p| p.pid == proc.ppid)
        .map_or_else(|| String::from("—"), |p| p.name.clone());
    let status = if proc.status.is_empty() {
        String::from("—")
    } else {
        proc.status.to_owned()
    };
    let inner = cell(frame, area, "identity", theme);
    let mut spans = Vec::new();
    push_token(&mut spans, proc.name.clone(), big_style(theme));
    push_token(&mut spans, format!("@{}", proc.user), theme.dim());
    push_kv(&mut spans, theme, "state", status, theme.title());
    push_kv(
        &mut spans,
        theme,
        "up",
        started_ago(proc.start_unix, now_unix()),
        theme.fg(),
    );
    push_kv(
        &mut spans,
        theme,
        "pid",
        format!("{:>7}", proc.pid),
        theme.fg(),
    );
    push_kv(&mut spans, theme, "ppid", parent, theme.dim());
    frame.render_widget(
        Paragraph::new(wrap_spans(spans, usize::from(inner.width.max(1)))),
        inner,
    );
}

fn paint_live(frame: &mut Frame, area: Rect, proc: &Process, theme: &Theme) {
    let inner = cell(frame, area, "live", theme);
    let mut spans = Vec::new();
    push_kv(
        &mut spans,
        theme,
        "cpu",
        format!("{:>5.1}%", proc.cpu),
        theme.cpu(),
    );
    push_kv(
        &mut spans,
        theme,
        "mem",
        format!("{:>7}", bytes_short(proc.mem_bytes)),
        theme.fg(),
    );
    push_kv(
        &mut spans,
        theme,
        "threads",
        format!("{:>4}", proc.threads),
        theme.fg(),
    );
    frame.render_widget(
        Paragraph::new(wrap_spans(spans, usize::from(inner.width.max(1)))),
        inner,
    );
    if inner.height >= 2 {
        let bar = Rect {
            x: inner.x,
            y: inner.y.saturating_add(inner.height.saturating_sub(1)),
            width: inner.width,
            height: 1,
        };
        crate::chrome::render_fill_bar(frame, bar, (proc.cpu / 100.0).clamp(0.0, 1.0), theme.cpu);
    }
}

fn paint_command(frame: &mut Frame, area: Rect, proc: &Process, theme: &Theme) {
    let inner = cell(frame, area, "command", theme);
    let text = proc.command.as_deref().unwrap_or("—");
    let max = usize::from(inner.width.max(1));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            elide_middle(text, max),
            theme.fg(),
        ))),
        inner,
    );
}

fn paint_family(frame: &mut Frame, area: Rect, view: &AppView<'_>, proc: &Process, theme: &Theme) {
    let children: Vec<&Process> = view
        .snapshot
        .processes
        .iter()
        .filter(|p| p.ppid == proc.pid)
        .collect();
    let inner = cell(frame, area, "family", theme);
    let kids: String = if children.is_empty() {
        String::from("—")
    } else {
        children
            .iter()
            .take(4)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join("  ")
    };
    let mut spans = Vec::new();
    push_kv(
        &mut spans,
        theme,
        "children",
        format!("{:>3}", children.len()),
        theme.fg(),
    );
    if !children.is_empty() {
        push_token(&mut spans, kids, theme.dim());
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
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
    for (key, verb, _) in ACTION_CHIPS {
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

fn search_line(view: &AppView<'_>, n: usize, theme: &Theme) -> Line<'static> {
    let active = view.searching || matches!(view.focus, Focus::Search);
    let mut query = view.proc.filter.clone();
    if active {
        query.push('▌');
    } else if query.is_empty() {
        query = String::from("/ to filter");
    }
    let label = if active { " search " } else { " / search " };
    Line::from(vec![
        Span::styled(label, if active { theme.title() } else { theme.dim() }),
        Span::styled(query, if active { theme.title() } else { theme.dim() }),
        Span::styled(format!("  {n} procs"), theme.dim()),
    ])
}

#[must_use]
pub fn selected_index(view: &AppView<'_>, pids: impl IntoIterator<Item = u32>) -> usize {
    let pids: Vec<u32> = pids.into_iter().collect();
    if let Some(pid) = view.proc.selected_pid
        && let Some(i) = pids.iter().position(|p| *p == pid)
    {
        return i;
    }
    view.proc.selected.min(pids.len().saturating_sub(1))
}

#[must_use]
pub fn filtered<'a>(view: &'a AppView<'_>) -> Vec<&'a Process> {
    listed(view).into_iter().map(|row| row.proc).collect()
}

struct Listed<'a> {
    proc: &'a Process,
    depth: u8,
}

fn listed<'a>(view: &'a AppView<'_>) -> Vec<Listed<'a>> {
    if view.show_tree {
        tree_rows(&view.snapshot.processes, &view.proc.filter, view.sort)
    } else {
        filter_sort_by(&view.snapshot.processes, &view.proc.filter, view.sort)
            .into_iter()
            .map(|proc| Listed { proc, depth: 0 })
            .collect()
    }
}

fn tree_name(name: &str, depth: u8) -> String {
    if depth == 0 {
        return name.to_owned();
    }
    let pad = "  ".repeat(usize::from(depth));
    format!("{pad}╰ {name}")
}

fn tree_rows<'a>(
    processes: &'a [Process],
    filter: &str,
    sort: plottypus_core::ProcSort,
) -> Vec<Listed<'a>> {
    use std::collections::{HashMap, HashSet};

    let needle = filter.to_ascii_lowercase();
    let by_pid: HashMap<u32, &Process> = processes.iter().map(|p| (p.pid, p)).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut roots = Vec::new();
    for proc in processes {
        if proc.ppid == 0 || proc.ppid == proc.pid || !by_pid.contains_key(&proc.ppid) {
            roots.push(proc.pid);
        } else {
            children.entry(proc.ppid).or_default().push(proc.pid);
        }
    }

    let keep = if needle.is_empty() {
        None
    } else {
        let mut keep = HashSet::new();
        for proc in processes {
            if matches_filter(proc, &needle) {
                let mut walk = Some(proc.pid);
                while let Some(pid) = walk {
                    if !keep.insert(pid) {
                        break;
                    }
                    walk = by_pid.get(&pid).and_then(|p| {
                        if p.ppid == 0 || p.ppid == p.pid {
                            None
                        } else {
                            Some(p.ppid)
                        }
                    });
                }
            }
        }
        Some(keep)
    };

    let sort_pids = |ids: &mut [u32]| {
        ids.sort_by(|&a, &b| {
            let pa = by_pid.get(&a);
            let pb = by_pid.get(&b);
            match (pa, pb, sort) {
                (Some(a), Some(b), plottypus_core::ProcSort::Cpu) => b.cpu.total_cmp(&a.cpu),
                (Some(a), Some(b), plottypus_core::ProcSort::Mem) => b.mem_bytes.cmp(&a.mem_bytes),
                (Some(a), Some(b), plottypus_core::ProcSort::Pid) => a.pid.cmp(&b.pid),
                _ => std::cmp::Ordering::Equal,
            }
        });
    };

    sort_pids(&mut roots);
    for kids in children.values_mut() {
        sort_pids(kids);
    }

    let mut out = Vec::new();
    for root in roots {
        walk_tree(root, 0, &by_pid, &children, keep.as_ref(), &mut out);
    }
    out
}

fn walk_tree<'a>(
    pid: u32,
    depth: u8,
    by_pid: &std::collections::HashMap<u32, &'a Process>,
    children: &std::collections::HashMap<u32, Vec<u32>>,
    keep: Option<&std::collections::HashSet<u32>>,
    out: &mut Vec<Listed<'a>>,
) {
    if let Some(keep) = keep
        && !keep.contains(&pid)
    {
        return;
    }
    if let Some(proc) = by_pid.get(&pid) {
        out.push(Listed { proc, depth });
    }
    if let Some(kids) = children.get(&pid) {
        for child in kids {
            walk_tree(*child, depth.saturating_add(1), by_pid, children, keep, out);
        }
    }
}

fn matches_filter(proc: &Process, needle: &str) -> bool {
    needle.is_empty()
        || contains_ignore_ascii(&proc.name, needle)
        || u32_contains(proc.pid, needle)
        || proc
            .command
            .as_ref()
            .is_some_and(|c| contains_ignore_ascii(c, needle))
}

fn contains_ignore_ascii(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase().contains(needle)
}

fn u32_contains(n: u32, needle: &str) -> bool {
    if needle.is_empty() || !needle.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut buf = [0u8; 10];
    let mut x = n;
    let mut i = 10;
    loop {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
        if x == 0 || i == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).is_ok_and(|s| s.contains(needle))
}

#[must_use]
#[cfg(test)]
pub fn filter_sort(processes: &[Process], filter: &str) -> Vec<Process> {
    filter_sort_by(processes, filter, plottypus_core::ProcSort::Cpu)
        .into_iter()
        .cloned()
        .collect()
}

#[must_use]
pub fn filter_sort_by<'a>(
    processes: &'a [Process],
    filter: &str,
    sort: plottypus_core::ProcSort,
) -> Vec<&'a Process> {
    let needle = filter.to_ascii_lowercase();
    let mut rows: Vec<&Process> = processes
        .iter()
        .filter(|proc| matches_filter(proc, &needle))
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
        let view = fx.view();
        let rows = filtered(&view);
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
        let view = fx.view();
        let rows = filtered(&view);
        let names: Vec<&str> = rows.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Xcode", "xcodebuild"]);
    }

    #[test]
    fn follows_pid_not_index() {
        let mut fx = fixture("");
        fx.snap.processes = vec![process(1, "a", 1.0), process(2, "b", 9.0)];
        fx.proc.selected_pid = Some(1);
        let view = fx.view();
        let rows = filtered(&view);
        assert_eq!(selected_index(&view, rows.iter().map(|p| p.pid)), 1);
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
    fn tree_nests_children_under_parent() {
        let mut fx = fixture("");
        fx.show_tree = true;
        fx.snap.processes = vec![
            process(10, "parent", 1.0),
            process(11, "child", 9.0),
            process(12, "other", 5.0),
        ];
        fx.snap.processes[1].ppid = 10;
        let view = fx.view();
        let rows = listed(&view);
        let names: Vec<(u8, &str)> = rows
            .iter()
            .map(|r| (r.depth, r.proc.name.as_str()))
            .collect();
        assert!(
            names.contains(&(0, "parent")) && names.contains(&(1, "child")),
            "{names:?}"
        );
        let parent_at = names.iter().position(|r| r.1 == "parent").unwrap();
        let child_at = names.iter().position(|r| r.1 == "child").unwrap();
        assert!(child_at > parent_at, "{names:?}");
        assert_eq!(tree_name("child", 1), "  ╰ child");
    }

    #[test]
    fn tree_filter_keeps_ancestors() {
        let mut fx = fixture("child");
        fx.show_tree = true;
        fx.snap.processes = vec![process(10, "parent", 1.0), process(11, "child", 2.0)];
        fx.snap.processes[1].ppid = 10;
        let view = fx.view();
        let rows = listed(&view);
        let names: Vec<&str> = rows.iter().map(|r| r.proc.name.as_str()).collect();
        assert_eq!(names, ["parent", "child"]);
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
        assert!(text.contains("identity"), "{text}");
        assert!(text.contains("live"), "{text}");
        assert!(text.contains("command"), "{text}");
        assert!(text.contains("term"), "{text}");
        assert!(text.contains("interrupt"), "{text}");
        assert!(!text.contains("%%"), "{text}");
    }

    #[test]
    fn dossier_fills_most_of_the_pane() {
        let rect = detail_rect(ratatui::layout::Rect::new(0, 0, 80, 24));
        assert!(rect.height >= 16, "height {}", rect.height);
        assert!(rect.width >= 70, "width {}", rect.width);
        assert_eq!(rect.x, 1);
        assert_eq!(rect.y, 1);
    }

    #[test]
    fn sort_header_marks_the_active_column() {
        assert_eq!(sort_header("cpu%", true), "cpu%↓");
        assert_eq!(sort_header("cpu%", false), "cpu%");
    }
}
