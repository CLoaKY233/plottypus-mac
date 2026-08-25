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
    pub detail: bool,
    pub confirm: bool,
}

pub fn poll(timeout: Duration, modes: Modes) -> Result<Option<Event>> {
    if !event::poll(timeout).map_err(|err| Error::terminal(err.to_string()))? {
        return Ok(Some(Event::Tick));
    }
    match event::read().map_err(|err| Error::terminal(err.to_string()))? {
        TermEvent::Resize(_, _) => Ok(Some(Event::Resize)),
        TermEvent::Key(key) if key.kind == KeyEventKind::Press => Ok(map_key_in_mode(key, modes)),
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

pub(crate) fn map_key_in_mode(key: KeyEvent, modes: Modes) -> Option<Event> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Some(Event::Quit);
    }
    if matches!(key.code, KeyCode::Esc) {
        return Some(Event::FilterCancel);
    }
    if matches!(key.code, KeyCode::Char('?')) {
        return Some(Event::Help);
    }
    if modes.confirm {
        return match key.code {
            KeyCode::Char('y') => Some(Event::ConfirmYes),
            KeyCode::Char('n' | 'x') => Some(Event::ConfirmNo),
            KeyCode::Char('q') => Some(Event::Quit),
            _ => None,
        };
    }
    if modes.settings {
        return map_settings_key(key);
    }
    if modes.searching {
        return map_search_key(key);
    }
    if modes.detail {
        return match key.code {
            KeyCode::Char('t') => Some(Event::DetailTerm),
            KeyCode::Char('k') => Some(Event::DetailKill),
            KeyCode::Char('i') => Some(Event::DetailInterrupt),
            KeyCode::Char('q') => Some(Event::Quit),
            KeyCode::Char('j') | KeyCode::Down => Some(Event::Move(1)),
            KeyCode::Up => Some(Event::Move(-1)),
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

    fn modes(searching: bool, detail: bool) -> Modes {
        Modes {
            searching,
            settings: false,
            detail,
            confirm: false,
        }
    }

    #[test]
    fn question_always_help() {
        assert_eq!(
            map_key_in_mode(key('?'), modes(true, false)),
            Some(Event::Help)
        );
    }

    #[test]
    fn q_in_search_is_a_letter() {
        assert_eq!(
            map_key_in_mode(key('q'), modes(true, false)),
            Some(Event::FilterChar('q'))
        );
        assert_eq!(
            map_key_in_mode(key('q'), modes(false, false)),
            Some(Event::Quit)
        );
    }

    #[test]
    fn x_kills() {
        assert_eq!(
            map_key_in_mode(key('x'), modes(false, false)),
            Some(Event::Kill)
        );
    }

    #[test]
    fn tab_enter_backtab() {
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Tab), modes(false, false)),
            Some(Event::NextPanel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::BackTab), modes(false, false)),
            Some(Event::PrevPanel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Enter), modes(false, false)),
            Some(Event::Expand)
        );
    }

    #[test]
    fn detail_mode_routes_actions() {
        assert_eq!(
            map_key_in_mode(key('t'), modes(false, true)),
            Some(Event::DetailTerm)
        );
        assert_eq!(
            map_key_in_mode(key('k'), modes(false, true)),
            Some(Event::DetailKill)
        );
        assert_eq!(
            map_key_in_mode(key('i'), modes(false, true)),
            Some(Event::DetailInterrupt)
        );
        assert_eq!(
            map_key_in_mode(key('x'), modes(false, true)),
            None,
            "x stays out of the popup"
        );
    }

    #[test]
    fn confirm_wins_over_detail() {
        let confirm = Modes {
            searching: false,
            settings: false,
            detail: true,
            confirm: true,
        };
        assert_eq!(map_key_in_mode(key('y'), confirm), Some(Event::ConfirmYes));
        assert_eq!(map_key_in_mode(key('n'), confirm), Some(Event::ConfirmNo));
        assert_eq!(map_key_in_mode(key('k'), confirm), None);
    }

    #[test]
    fn esc_is_always_cancel() {
        let expanded = Modes {
            searching: false,
            settings: false,
            detail: false,
            confirm: false,
        };
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Esc), expanded),
            Some(Event::FilterCancel)
        );
        assert_eq!(
            map_key_in_mode(KeyEvent::from(KeyCode::Esc), modes(false, false)),
            Some(Event::FilterCancel)
        );
    }
}
