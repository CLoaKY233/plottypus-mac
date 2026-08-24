use std::io::{self, Stdout, stdout};
use std::sync::Once;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use plottypus_core::{Error, Result};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn install_panic_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_best_effort();
            original(info);
        }));
    });
}

pub fn install() -> Result<AppTerminal> {
    install_panic_hook();
    enable_raw_mode().map_err(|err| Error::terminal(err.to_string()))?;
    let mut out = stdout();
    if let Err(err) = execute!(out, EnterAlternateScreen, EnableMouseCapture) {
        let _ = disable_raw_mode();
        return Err(Error::terminal(err.to_string()));
    }
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(err) => {
            restore_best_effort();
            Err(Error::terminal(err.to_string()))
        }
    }
}

pub fn restore() -> Result<()> {
    let result = restore_inner();
    let _ = disable_raw_mode();
    result
}

fn restore_inner() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)
        .map_err(|err| Error::terminal(err.to_string()))?;
    disable_raw_mode().map_err(|err| Error::terminal(err.to_string()))?;
    Ok(())
}

pub fn restore_best_effort() {
    let _ = restore();
    let _ = io::Write::flush(&mut stdout());
}
