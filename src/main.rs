use crossterm::event::Event;
use ratatui::DefaultTerminal;

mod app;
mod cli;
mod history;
mod input;
mod storage;
mod ui;

use app::{Action, AppState, InputMode};
use input::help;
use std::io;
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}

pub fn run_tui(output_file: Option<String>) -> color_eyre::Result<Option<String>> {
    let mut terminal = ratatui::init();
    let result = app(&mut terminal, output_file.clone());
    ratatui::restore();

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
        if let Event::Key(key) = crossterm::event::read()? {
            if key.code == crossterm::event::KeyCode::Esc
                && !state.help_open
                && state.input_mode != InputMode::TagInput
                && state.input_mode != InputMode::CollectionInput
                && state.handle_esc()
            {
                break;
            }
            match input::handle(&mut state, key) {
                Action::Execute(cmd) => {
                    result = Ok(Some(cmd));
                    break;
                }
                Action::Exit => {
                    break;
                }
                Action::CloseHelp => {
                    state.help_open = false;
                    state.help_search_query.clear();
                }
                Action::ExecuteHelpShortcut(action_id) => {
                    match help::execute_help_action(&mut state, &action_id) {
                        Action::Execute(cmd) => {
                            result = Ok(Some(cmd));
                            break;
                        }
                        Action::Exit => {
                            break;
                        }
                        _ => {}
                    }
                }
                Action::None => {}
            }
        }
    }

    result
}
