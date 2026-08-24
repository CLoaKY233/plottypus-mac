mod app;
mod event;
mod tui;

fn main() -> Result<(), i32> {
    tui::install_panic_hook();
    if let Err(err) = app::run() {
        tui::restore_best_effort();
        eprintln!("{err}");
        return Err(1);
    }
    Ok(())
}
