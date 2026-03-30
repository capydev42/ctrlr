use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, Paragraph},
};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());

    // search bar
    frame.render_widget(
        Paragraph::new("Search: ").block(Block::bordered().title("cltr")),
        chunks[0],
    );

    //History List
    frame.render_widget(
        //        List::new(commands.iter().map(|c| c.text.as_str())),
        Paragraph::new("History"),
        chunks[1],
    );

    //Footer
    frame.render_widget(
        Paragraph::new(
            " ↑ / ↓ : Navigate  | Enter : Select | f : Favorite | t : Tag | Esc : Exit ",
        ),
        chunks[2],
    );
}
