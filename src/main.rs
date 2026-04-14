use crossterm::event::Event;
use ratatui::DefaultTerminal;

mod app;
mod cli;
mod history;
mod input;
mod storage;
mod ui;

use app::{Action, AppState, InputMode};
use std::io;
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}

pub fn run_tui(output_file: Option<String>) -> color_eyre::Result<Option<String>> {
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stdout) {
        eprintln!("Error: ctrlr must be run from a terminal. Stdin or stdout is not a TTY.");
        return Ok(None);
    }
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
    let mut db = match storage::init_db() {
        Ok(conn) => Some(conn),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            None
        }
    };
    let mut commands = history::load_history();
    commands = history::deduplicate(commands);
    commands.reverse();

    if let Some(ref mut conn) = db {
        let cmd_refs: Vec<(&str, String)> = commands
            .iter()
            .map(|c| (c.text.as_str(), c.id.clone()))
            .collect();
        if let Err(e) = storage::commands::ensure_commands_exist(conn, &cmd_refs) {
            eprintln!("Failed to save commands: {}", e);
        }

        for cmd in &mut commands {
            if let Some(meta) = storage::load_metadata(conn, &cmd.text) {
                cmd.favorite = meta.favorite;
                cmd.use_count = meta.use_count;
                cmd.last_used = meta.last_used;
            }
            let tags = storage::load_tags(conn, &cmd.text);
            if !tags.is_empty() {
                cmd.tags = tags;
            }
            let collections = storage::collections::get_collections_for_command(conn, &cmd.text)
                .unwrap_or_default();
            if !collections.is_empty() {
                cmd.collection_ids = collections;
            }
        }
    }

    let mut state = AppState::new(commands, db);
    state.load_collections();

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
                && state.input_mode != InputMode::TagInput
                && state.input_mode != InputMode::CollectionInput
                && state.handle_esc()
            {
                break Ok(None);
            }
            match input::handle(&mut state, key) {
                Action::Execute(cmd) => break Ok(Some(cmd)),
                Action::Exit => break Ok(None),
                Action::None => {}
            }
        }
    }
}
