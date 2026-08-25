use std::time::Duration;

use crossterm::event::{
    self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use plottypus_core::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Quit,
    Tick,
    Resize,
    Help,
    Settings,
    Glance,
    Work,
    Search,
    FilterChar(char),
    FilterBackspace,
    FilterCancel,
    Move(i32),
    Kill,
    ConfirmYes,
    ConfirmNo,
    CycleInterval,
    Freeze,
    ToggleGpu,
    ToggleNet,
    ToggleCores,
    ToggleDisk,
    ToggleFans,
    ToggleThreads,
    ToggleTree,
    CycleSort,
    Expand,
    NextPanel,
    PrevPanel,
    DetailTerm,
    DetailKill,
    DetailInterrupt,
    Click { col: u16, row: u16 },
    Drag { col: u16, row: u16 },
    MouseUp,
}

#[derive(Debug, Clone, Copy)]
pub struct Modes {
    pub searching: bool,
    pub settings: bool,
    pub expanded: bool,
    pub detail: bool,
}

pub fn poll(timeout: Duration, modes: Modes) -> Result<Option<Event>> {
    if !event::poll(timeout).map_err(|err| Error::terminal(err.to_string()))? {
        return Ok(Some(Event::Tick));
    }
    match event::read().map_err(|err| Error::terminal(err.to_string()))? {
        TermEvent::Resize(_, _) => Ok(Some(Event::Resize)),
        TermEvent::Key(key) if key.kind == KeyEventKind::Press => Ok(map_key_in_mode(
            key,
            modes.searching,
            modes.settings,
            modes.expanded,
            modes.detail,
        )),
        TermEvent::Mouse(mouse) => Ok(map_mouse(mouse.kind, mouse.column, mouse.row)),
        _ => Ok(None),
    }
}

fn map_mouse(kind: MouseEventKind, col: u16, row: u16) -> Option<Event> {
    match kind {
        MouseEventKind::Down(_) => Some(Event::Click { col, row }),
        MouseEventKind::Drag(_) => Some(Event::Drag { col, row }),
        MouseEventKind::Up(_) => Some(Event::MouseUp),
        MouseEventKind::ScrollDown => Some(Event::Move(1)),
        MouseEventKind::ScrollUp => Some(Event::Move(-1)),
        _ => None,
    }
}

pub(crate) fn map_key_in_mode(
    key: KeyEvent,
    searching: bool,
    settings: bool,
    expanded: bool,
    detail: bool,
) -> Option<Event> {
    let _ = expanded;
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Some(Event::Quit);
    }
    if matches!(key.code, KeyCode::Esc) {
        return Some(Event::FilterCancel);
    }
    if matches!(key.code, KeyCode::Char('?')) {
        return Some(Event::Help);
    }
    if settings {
        return map_settings_key(key);
    }
    if searching {
        return map_search_key(key);
    }
    if detail {
        return match key.code {
            KeyCode::Char('t') => Some(Event::DetailTerm),
            KeyCode::Char('k') => Some(Event::DetailKill),
            KeyCode::Char('i') => Some(Event::DetailInterrupt),
            KeyCode::Enter => Some(Event::FilterCancel),
            _ => None,
        };
    }
    map_normal_key(key)
}

fn map_settings_key(key: KeyEvent) -> Option<Event> {
    match key.code {
        KeyCode::Char('1' | '[' | ']') => Some(Event::CycleInterval),
        KeyCode::Char('2') => Some(Event::ToggleGpu),
        KeyCode::Char('3') => Some(Event::ToggleNet),
        KeyCode::Char('4') => Some(Event::ToggleCores),
        KeyCode::Char('5') => Some(Event::ToggleDisk),
        KeyCode::Char('6') => Some(Event::ToggleFans),
        KeyCode::Char('7') => Some(Event::CycleSort),
        KeyCode::Char('8') => Some(Event::ToggleThreads),
        KeyCode::Char('9') => Some(Event::ToggleTree),
        KeyCode::Char('s') => Some(Event::Settings),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Event> {
    match key.code {
        KeyCode::Enter => Some(Event::Search),
        KeyCode::Backspace => Some(Event::FilterBackspace),
        KeyCode::Down => Some(Event::Move(1)),
        KeyCode::Up => Some(Event::Move(-1)),
        KeyCode::Char(c) if !c.is_control() => Some(Event::FilterChar(c)),
        _ => None,
    }
}

fn map_normal_key(key: KeyEvent) -> Option<Event> {
    match key.code {
        KeyCode::Char('q') => Some(Event::Quit),
        KeyCode::Char('s') => Some(Event::Settings),
        KeyCode::Char('g') => Some(Event::Glance),
        KeyCode::Char('w') => Some(Event::Work),
        KeyCode::Char('/') => Some(Event::Search),
        KeyCode::Char('j') | KeyCode::Down => Some(Event::Move(1)),
        KeyCode::Char('k') | KeyCode::Up => Some(Event::Move(-1)),
        KeyCode::Char('x') | KeyCode::Delete => Some(Event::Kill),
        KeyCode::Char('y') => Some(Event::ConfirmYes),
        KeyCode::Char('n') => Some(Event::ConfirmNo),
        KeyCode::Char('[' | ']') => Some(Event::CycleInterval),
        KeyCode::Char('f') => Some(Event::Freeze),
        KeyCode::Tab => Some(Event::NextPanel),
        KeyCode::BackTab => Some(Event::PrevPanel),
        KeyCode::Enter => Some(Event::Expand),
        KeyCode::Backspace => Some(Event::FilterBackspace),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    #[test]
    fn question_always_help() {
        assert_eq!(
            map_key_in_mode(key('?'), true, false, false, false),
            Some(Event::Help)
        );
    }

    #[test]
    fn q_in_search_is_a_letter() {
        assert_eq!(
            map_key_in_mode(key('q'), true, false, false, false),
            Some(Event::FilterChar('q'))
        );
        assert_eq!(
            map_key_in_mode(key('q'), false, false, false, false),
            Some(Event::Quit)
        );
    }

    #[test]
    fn x_kills() {
        assert_eq!(
            map_key_in_mode(key('x'), false, false, false, false),
            Some(Event::Kill)
        );
    }

    #[test]
    fn tab_enter_backtab() {
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Tab), false, false, false, false),
            Some(Event::NextPanel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::BackTab), false, false, false, false),
            Some(Event::PrevPanel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Enter), false, false, false, false),
            Some(Event::Expand)
        );
    }

    #[test]
    fn detail_mode_routes_actions() {
        assert_eq!(
            map_key_in_mode(key('t'), false, false, false, true),
            Some(Event::DetailTerm)
        );
        assert_eq!(
            map_key_in_mode(key('k'), false, false, false, true),
            Some(Event::DetailKill)
        );
        assert_eq!(
            map_key_in_mode(key('i'), false, false, false, true),
            Some(Event::DetailInterrupt)
        );
        assert_eq!(
            map_key_in_mode(key('x'), false, false, false, true),
            None,
            "x stays out of the popup"
        );
    }

    #[test]
    fn esc_is_always_cancel() {
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Esc), false, false, true, false),
            Some(Event::FilterCancel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Esc), false, false, false, false),
            Some(Event::FilterCancel)
        );
    }
}
