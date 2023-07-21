use tui::{
    style::{Style, Color, Modifier},
    buffer::Buffer,
    layout::{Constraint, Rect, Direction, Layout},
    widgets::{StatefulWidget, Widget, Table, TableState, Row, Cell, Paragraph, Block, Borders, BorderType},
};
use crossterm::event::KeyCode;
use tmdb_api::movie::MovieShort;
use std::path::PathBuf;

use crate::{AppEvent, AppState, AppMessage};
use crate::nfo::{Movie};
use crate::views::movie_manager::{MovieManagerEvent, MovieManagerMessage};
use crate::views::widgets::{Input, Button, InputState, ButtonState};
use crate::MESSAGE_SENDER;

#[derive(Clone, Debug)]
pub struct MovieSearch {
    query: Input,
    send: Button,
}

impl Default for MovieSearch {
    fn default() -> MovieSearch {
        let mut input = Input::default();
        input.placeholder = Some("Movie title".into());
        MovieSearch {
            query: input,
            send: Button::new("Search"),
        }
    }
}


#[derive(Clone, Debug, Default)]
pub struct MovieSearchState {
    table_state: TableState,
    results: Vec<MovieShort>,
    is_loading: bool,
    query_state: InputState,
    send_state: ButtonState,
    selected: usize,
    movie_path: PathBuf,
}

impl StatefulWidget for MovieSearch {
    type State = MovieSearchState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Min(1),
                Constraint::Percentage(100),
            ]).split(area);
        let search_bar = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(100),
                Constraint::Min(2),
                Constraint::Min(8),
            ]).split(chunks[0]);
        let mut search_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(if state.selected == 2 { Color::LightRed } else { Color::Gray }))
            .border_type(BorderType::Rounded); 
        let inner = search_block.inner(chunks[1].clone());  

        state.query_state.set_focus(state.selected == 0);
        state.send_state.focus(state.selected == 1);
        StatefulWidget::render(self.query, search_bar[0], buf, &mut state.query_state);
        StatefulWidget::render(self.send, search_bar[1], buf, &mut state.send_state);
        search_block.render(chunks[1], buf);
        if state.is_loading {
            Paragraph::new("Searching...").render(inner, buf);
        } else if state.results.len() == 0 {
            Paragraph::new("No result found.").render(inner, buf);
        } else {
            let rows : Vec<_> = state.results.iter()
                .map(|m| {
                    let yr = m.inner.release_date.map(|rd| rd.format("%Y").to_string()).unwrap_or("".into());
                    Row::new(vec![m.inner.title.clone(), yr, m.inner.overview.clone()])
                })
                .collect();
            
            let table = Table::new(rows)
                .style(Style::default().fg(Color::White))
                .header(
                    Row::new(vec!["Title", "Year", "Overview"])
                        .style(Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD))
                        .bottom_margin(1)
                )
                .widths(&[Constraint::Length(50), Constraint::Length(4), Constraint::Percentage(100)])
                .column_spacing(1)
                .highlight_style(Style::default().bg(Color::LightRed));
            StatefulWidget::render(table, inner, buf, &mut state.table_state);
        }
    }
}

impl MovieSearchState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match app_event {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Enter {
                    if self.selected == 0 || self.selected == 1 {
                        let sender = MESSAGE_SENDER.get().unwrap();
                        sender.send(AppMessage::MovieManagerMessage(MovieManagerMessage::SearchTitle(self.query_state.get_value().to_owned()))).unwrap();
                        true
                    } else if self.selected == 2 {
                        // TODO: Apply suggestion
                        true
                    } else {
                        false
                    }
                } else if kev.code == KeyCode::Up && self.results.len() > 0 {
                    self.table_state.select(self.table_state.selected().map(|c| (c + self.results.len() - 1) % self.results.len()));
                    true
                } else if kev.code == KeyCode::Down && self.results.len() > 0 {
                    self.table_state.select(self.table_state.selected().map(|c| (c + 1) % self.results.len()).or(Some(0)));
                    true
                } else if kev.code == KeyCode::Tab {
                    self.selected = (self.selected + 1) % 3;
                    true
                } else if kev.code == KeyCode::BackTab {
                    self.selected = (self.selected + 2) % 3;
                    true
                } else {
                    false
                }
            },
            _ => { false },
        }
    }
}