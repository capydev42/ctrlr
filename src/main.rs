use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use ratatui::DefaultTerminal;

mod app;
mod cli;
mod config;
mod hash;
mod history;
mod input;
mod keymap;
mod storage;
mod ui;

use app::{Action, AppState};
use std::io;
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}

pub fn run_tui(output_file: Option<String>) -> color_eyre::Result<Option<String>> {
    let mut terminal = ratatui::init();
    // Best-effort: a terminal that refuses mouse reporting still runs ctrlr,
    // it just stays keyboard-only.
    //
    // This also turns on motion tracking (?1002 drag, ?1003 any motion), which
    // ctrlr has no use for. Resetting those two to save the redraws is not
    // worth it: xterm treats 1000/1002/1003 as independent, but a terminal
    // that keeps one tracking-mode variable can read `?1003l` as "tracking
    // off" and stop reporting clicks altogether — untested here, and the
    // failure looks exactly like mouse support never having been built. The
    // cost of leaving them on is one diffed frame per pointer move, which
    // writes nothing; `input::mouse::handle` drops the events.
    let _ = execute!(io::stdout(), EnableMouseCapture);
    // ratatui's own panic hook restores the screen but knows nothing about
    // mouse capture, so chain onto it — otherwise a panic leaves the terminal
    // emitting escape sequences on every pointer move.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        previous_hook(info);
    }));

    let result = app(&mut terminal, output_file.clone());

    let _ = execute!(io::stdout(), DisableMouseCapture);

    // Leave the alternate screen first, then restore cursor visibility on the
    // normal screen: ?1049 does not save/restore DECTCEM, and the draw loop
    // hides the cursor every frame because ctrlr draws its own glyph rather
    // than calling frame.set_cursor_position. Doing this here instead of
    // leaning on Terminal's Drop is what makes it total — the process::exit
    // calls below would skip Drop and leave the cursor hidden for good.
    ratatui::restore();
    let _ = terminal.show_cursor();
    drop(terminal);

    match result {
        Ok(Some(cmd)) => {
            if let Some(path) = output_file {
                match std::fs::write(&path, &cmd) {
                    Ok(()) => Ok(Some(cmd)),
                    Err(e) => {
                        eprintln!("Failed to write output file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                Ok(Some(cmd))
            }
        }
        Ok(None) => {
            if let Some(path) = output_file {
                if let Err(e) = std::fs::write(&path, "") {
                    eprintln!("Failed to write output file: {}", e);
                }
                std::process::exit(1);
            }
            Ok(None)
        }
        Err(e) => {
            if let Some(path) = output_file {
                let _ = std::fs::write(&path, "");
            }
            Err(color_eyre::Report::new(e))
        }
    }
}

fn app(terminal: &mut DefaultTerminal, _output_file: Option<String>) -> io::Result<Option<String>> {
    let mut state = AppState::bootstrap();
    // Only through --output-file can ctrlr hand the shell a command to run,
    // which is what lets the integration popup offer a reload.
    state.writes_to_output_file = _output_file.is_some();
    let mut result = Ok(None);

    loop {
        if let Some(ts) = state.status_timestamp {
            let should_clear =
                state.status_message.is_some() && ts.elapsed() > Duration::from_secs(2);
            if should_clear {
                state.status_message = None;
                state.status_timestamp = None;
            }
        }

        terminal.draw(|f| ui::render(f, &mut state))?;
        let action = match crossterm::event::read()? {
            // Esc and Ctrl+C are not special-cased here any more: they resolve
            // through `AppState::cancel_or_quit`, which owns the staging order,
            // and reach this loop as `Action::Exit`.
            Event::Key(key) => input::handle(&mut state, key),
            // Hit-tested against the rects the draw above just recorded.
            Event::Mouse(mouse) => input::mouse::handle(&mut state, mouse),
            _ => Action::None,
        };

        match action {
            Action::Execute(cmd) => {
                result = Ok(Some(cmd));
                break;
            }
            Action::Exit => {
                break;
            }
            // The editor owns the terminal while it runs, so this cannot live
            // in the input layer. Returning here keeps the user in the edit
            // line afterwards — readline does the same, and it means the text
            // is confirmed once before it leaves ctrlr.
            Action::OpenEditor(text) => {
                let outcome = app::editor::edit_in_external_editor(terminal, &text);
                if let Some(edited) = outcome.text {
                    state.edit_input.set_value(edited);
                }
                if let Some(message) = outcome.message {
                    state.status_message = Some(message);
                    state.status_timestamp = Some(std::time::Instant::now());
                }
            }
            Action::None => {}
        }
    }

    result
}
