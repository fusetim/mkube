use crossterm::event::KeyCode;
use std::path::PathBuf;
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, BorderType, Borders, Paragraph, Row, StatefulWidget, Table, TableState, Widget,
    },
};

use crate::nfo::Movie;
use crate::views::movie_manager::{details::MovieDetails, MovieManagerEvent, MovieManagerMessage};
use crate::MESSAGE_SENDER;
use crate::{AppEvent, AppMessage};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MovieTable {}
#[derive(Clone, Debug, Default)]
pub struct MovieTableState {
    table_state: TableState,
    movies: Vec<(Movie, usize, PathBuf)>,
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
            Paragraph::new("No movie found. You might need to refresh the list? Ctrl+Shift+R")
                .render(area, buf);
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded)
            .title(" Movies ");

        let mut movie_chunk = area.clone();
        if area.height > 18 {
            if let Some(movie) = state.table_state.selected() {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![
                        Constraint::Min(area.height - 10),
                        Constraint::Percentage(100),
                    ])
                    .split(area.clone());
                movie_chunk = chunks[0];
                MovieDetails {
                    movie: &state.movies[movie].0,
                }
                .render(chunks[1], buf);
            }
        }

        let inner = block.inner(movie_chunk.clone());

        let rows: Vec<_> = state
            .movies
            .iter()
            .map(|(m, _, _)| {
                let title = m.title.clone();
                let year = m.premiered.as_deref().unwrap_or("".into());
                let source = m.source.as_deref().unwrap_or("".into());
                let res = m
                    .fileinfo
                    .as_ref()
                    .map(|fi| fi.streamdetails.video.get(0))
                    .flatten()
                    .map(|vt| vt.height)
                    .flatten()
                    .map(|h| format!("{}p", h))
                    .unwrap_or("".into());
                Row::new(vec![title, year.to_owned(), source.to_owned(), res])
            })
            .collect();

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(
                Row::new(vec!["Title", "Year", "Source", "Res."])
                    .style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(1),
            )
            .widths(&[
                Constraint::Length(50),
                Constraint::Length(4),
                Constraint::Length(10),
                Constraint::Length(5),
            ])
            .column_spacing(1)
            .highlight_style(Style::default().bg(Color::LightRed));

        block.render(movie_chunk, buf);
        StatefulWidget::render(table, inner, buf, &mut state.table_state);
    }
}

impl MovieTableState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match app_event {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Char('r') && (!self.is_loading) {
                    self.is_loading = true;
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender
                        .send(AppMessage::MovieManagerMessage(
                            MovieManagerMessage::RefreshMovies,
                        ))
                        .unwrap();
                    true
                } else if kev.code == KeyCode::Up && self.movies.len() > 0 {
                    self.table_state.select(
                        self.table_state
                            .selected()
                            .map(|c| (c + self.movies.len() - 1) % self.movies.len()),
                    );
                    true
                } else if kev.code == KeyCode::Down && self.movies.len() > 0 {
                    self.table_state.select(
                        self.table_state
                            .selected()
                            .map(|c| (c + 1) % self.movies.len())
                            .or(Some(0)),
                    );
                    true
                } else if let Some(s) = self.table_state.selected() {
                    let sender = MESSAGE_SENDER.get().unwrap();
                    let msg = match kev.code {
                        KeyCode::Char('s') => {
                            AppMessage::TriggerEvent(AppEvent::MovieManagerEvent(
                                MovieManagerEvent::SearchMovie(self.movies[s].clone()),
                            ))
                        }
                        KeyCode::Char('e') => {
                            AppMessage::TriggerEvent(AppEvent::MovieManagerEvent(
                                MovieManagerEvent::EditMovie(self.movies[s].clone()),
                            ))
                        }
                        KeyCode::Char('t') => {
                            let (mut movie, fs_id, path) = self.movies[s].clone();
                            movie.source = Some("TV".into());
                            AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((
                                movie, fs_id, path,
                            )))
                        }
                        KeyCode::Char('b') => {
                            let (mut movie, fs_id, path) = self.movies[s].clone();
                            movie.source = Some("Bluray".into());
                            AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((
                                movie, fs_id, path,
                            )))
                        }
                        KeyCode::Char('d') => {
                            let (mut movie, fs_id, path) = self.movies[s].clone();
                            movie.source = Some("DVD".into());
                            AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((
                                movie, fs_id, path,
                            )))
                        }
                        KeyCode::Char('w') => {
                            let (mut movie, fs_id, path) = self.movies[s].clone();
                            movie.source = Some("WEB".into());
                            AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((
                                movie, fs_id, path,
                            )))
                        }
                        KeyCode::Char('u') => {
                            let (mut movie, fs_id, path) = self.movies[s].clone();
                            movie.source = Some("UHD Bluray".into());
                            AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((
                                movie, fs_id, path,
                            )))
                        }
                        KeyCode::Char('a') => AppMessage::MovieManagerMessage(
                            MovieManagerMessage::RetrieveArtworks(self.movies[s].clone()),
                        ),
                        _ => return false,
                    };
                    sender.send(msg).unwrap();
                    true
                } else {
                    false
                }
            }
            AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList) => {
                self.table_state.select(None);
                self.movies.clear();
                true
            }
            AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(movie)) => {
                self.is_loading = false;
                self.movies.push(movie);
                true
            }
            AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((movie, fs_id, path))) => {
                self.is_loading = false;
                if let Some((ind, _)) = self
                    .movies
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, fi, p))| p == &path && fi == &fs_id)
                    .next()
                {
                    self.movies[ind] = (movie, fs_id, path);
                } else {
                    self.movies.push((movie, fs_id, path));
                }
                true
            }
            _ => false,
        }
    }
}
