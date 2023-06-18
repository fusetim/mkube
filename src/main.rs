use std::{io, thread, time::Duration};
use tui::{
    text::{Span, Spans},
    style::{Style, Color, Modifier},
    backend::CrosstermBackend,
    widgets::{Widget, Wrap, Paragraph, Block, Borders},
    layout::{Layout, Constraint, Direction, Alignment},
    Terminal
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

fn main() -> Result<(), io::Error> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let text = vec![
        Spans::from(vec![
            Span::raw("First"),
            Span::styled("line",Style::default().add_modifier(Modifier::ITALIC)),
            Span::raw("."),
        ]),
        Spans::from(Span::styled("Second line", Style::default().fg(Color::Red))),
    ];

    terminal.draw(|f| {
        let size = f.size();
        let block = Paragraph::new(text)
            .block(Block::default().title("Paragraph").borders(Borders::ALL))
            .style(Style::default())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(block, size);
    })?;

    thread::sleep(Duration::from_millis(5000));

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}