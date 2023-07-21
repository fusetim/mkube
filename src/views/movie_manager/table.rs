use tui::{
    style::{Style, Color, Modifier},
    buffer::Buffer,
    layout::{Constraint, Rect, Direction},
    widgets::{StatefulWidget, Widget, Table, TableState, Row, Cell, Paragraph},
};
use crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

use crate::{AppEvent, AppState, AppMessage};
use crate::nfo::{Movie};
use crate::views::movie_manager::{MovieManagerEvent, MovieManagerMessage};
use crate::MESSAGE_SENDER;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MovieTable {}
#[derive(Clone, Debug, Default)]
pub struct MovieTableState {
    table_state: TableState,
    movies: Vec<(Movie, PathBuf)>,
    is_loading: bool,
}

impl StatefulWidget for MovieTable {
    type State = MovieTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if state.is_loading {
            Paragraph::new("Loading...").render(area, buf);
            return;
        }
        if state.movies.len() == 0 {
            Paragraph::new("No movie found. You might need to refresh the list? Ctrl+Shift+R").render(area, buf);
            return;
        }

        let rows : Vec<_> = state.movies.iter()
            .map(|(m, _)| {
                let title = m.title.clone();
                let year = m.premiered.as_deref().unwrap_or("".into());
                let source = m.source.as_deref().unwrap_or("".into());
                let res = m.fileinfo.as_ref().map(|fi| fi.streamdetails.video.get(0)).flatten().map(|vt| vt.height).flatten().map(|h| format!("{}p", h)).unwrap_or("".into());
                Row::new(vec![title, year.to_owned(), source.to_owned(), res])
            })
            .collect();
        
        let table = Table::new(rows)
        .style(Style::default().fg(Color::White))
        .header(
            Row::new(vec!["Title", "Year", "Source", "Res."])
                .style(Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD))
                .bottom_margin(1)
        )
        .widths(&[Constraint::Length(50), Constraint::Length(4), Constraint::Length(10), Constraint::Length(5)])
        .column_spacing(1)
        .highlight_style(Style::default().bg(Color::LightRed));

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

impl MovieTableState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match app_event {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Char('R') && kev.modifiers == (KeyModifiers::SHIFT | KeyModifiers::CONTROL) && (!self.is_loading) {
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender.send(AppMessage::MovieManagerMessage(MovieManagerMessage::RefreshMovies)).unwrap();
                    self.is_loading = true;
                    true
                } else if kev.code == KeyCode::Up && self.movies.len() > 0 {
                    self.table_state.select(self.table_state.selected().map(|c| (c + self.movies.len() - 1) % self.movies.len()));
                    true
                } else if kev.code == KeyCode::Down && self.movies.len() > 0 {
                    self.table_state.select(self.table_state.selected().map(|c| (c + 1) % self.movies.len()).or(Some(0)));
                    true
                } else if kev.code == KeyCode::Char('s') {
                    if let Some(s) = self.table_state.selected() {
                        let sender = MESSAGE_SENDER.get().unwrap();
                        sender.send(AppMessage::TriggerEvent(AppEvent::MovieManagerEvent(MovieManagerEvent::SearchMovie(self.movies[s].clone())))).unwrap();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList) => {
                self.table_state.select(None);
                self.movies.clear();
                true
            },
            AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(movie)) => {
                self.is_loading = false;
                self.movies.push(movie);
                true
            },
            AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((movie, path))) => {
                self.is_loading = false;
                if let Some((ind, _)) = self.movies.iter().enumerate().filter(|(_, (_,p))| p == &path).next() {
                    self.movies[ind] = (movie, path);
                } else {
                    self.movies.push((movie, path));
                }
                true
            },
            _ => { false },
        }
    }
}