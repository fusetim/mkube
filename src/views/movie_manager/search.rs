use crossterm::event::KeyCode;
use std::path::PathBuf;
use tmdb_api::movie::MovieShort;
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, BorderType, Borders, Paragraph, Row, StatefulWidget, Table, TableState, Widget,
    },
};

use crate::views::movie_manager::{
    details::MovieSearchDetails, MovieManagerEvent, MovieManagerMessage,
};
use crate::views::widgets::{Button, ButtonState, Input, InputState};
use crate::MESSAGE_SENDER;
use crate::{AppEvent, AppMessage};

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
    pub table_state: TableState,
    pub results: Vec<MovieShort>,
    pub is_loading: bool,
    pub query_state: InputState,
    pub send_state: ButtonState,
    pub selected: usize,
    pub movie_path: PathBuf,
    pub movie_fs_id: usize,
}

impl StatefulWidget for MovieSearch {
    type State = MovieSearchState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded)
            .title(" Search ");
        let mut search_chunk = area.clone();
        if area.height > 14 {
            if let Some(movie) = state.table_state.selected() {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![
                        Constraint::Min(area.height - 8),
                        Constraint::Percentage(100),
                    ])
                    .split(area.clone());
                search_chunk = chunks[0];
                MovieSearchDetails {
                    movie: &state.results[movie],
                }
                .render(chunks[1], buf);
            }
        }
        let inner = block.inner(search_chunk.clone());
        block.render(search_chunk, buf);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1), Constraint::Percentage(100)])
            .split(inner);
        let search_bar = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(chunks[0].width - 10),
                Constraint::Min(2),
                Constraint::Min(8),
            ])
            .split(chunks[0]);
        let search_block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(if state.selected == 2 {
                Color::LightRed
            } else {
                Color::Gray
            }))
            .border_type(BorderType::Rounded);
        let inner = search_block.inner(chunks[1].clone());

        state.query_state.set_focus(state.selected == 0);
        state.send_state.focus(state.selected == 1);
        StatefulWidget::render(self.query, search_bar[0], buf, &mut state.query_state);
        StatefulWidget::render(self.send, search_bar[2], buf, &mut state.send_state);
        search_block.render(chunks[1], buf);
        if state.is_loading {
            Paragraph::new("Searching...").render(inner, buf);
        } else if state.results.len() == 0 {
            Paragraph::new("No result found.").render(inner, buf);
        } else {
            let rows: Vec<_> = state
                .results
                .iter()
                .map(|m| {
                    let yr = m
                        .inner
                        .release_date
                        .map(|rd| rd.format("%Y").to_string())
                        .unwrap_or("".into());
                    Row::new(vec![m.inner.title.clone(), yr, m.inner.overview.clone()])
                })
                .collect();

            let table = Table::new(rows)
                .style(Style::default().fg(Color::White))
                .header(
                    Row::new(vec!["Title", "Year", "Overview"])
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
                    Constraint::Percentage(100),
                ])
                .column_spacing(1)
                .highlight_style(Style::default().bg(if state.selected == 2 {
                    Color::LightRed
                } else {
                    Color::Gray
                }));
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
                        sender
                            .send(AppMessage::MovieManagerMessage(
                                MovieManagerMessage::SearchTitle(
                                    self.query_state.get_value().to_owned(),
                                ),
                            ))
                            .unwrap();
                        self.is_loading = true;
                        true
                    } else if self.selected == 2 {
                        if let Some(index) = self.table_state.selected() {
                            let sender = MESSAGE_SENDER.get().unwrap();
                            sender
                                .send(AppMessage::MovieManagerMessage(
                                    MovieManagerMessage::SaveNfo((
                                        self.results[index].inner.id,
                                        self.movie_fs_id,
                                        self.movie_path.clone(),
                                    )),
                                ))
                                .unwrap();
                            return true;
                        }
                        false
                    } else {
                        false
                    }
                } else if self.selected == 2 && kev.code == KeyCode::Up && self.results.len() > 0 {
                    self.table_state.select(
                        self.table_state
                            .selected()
                            .map(|c| (c + self.results.len() - 1) % self.results.len()),
                    );
                    true
                } else if self.selected == 2 && kev.code == KeyCode::Down && self.results.len() > 0
                {
                    self.table_state.select(
                        self.table_state
                            .selected()
                            .map(|c| (c + 1) % self.results.len())
                            .or(Some(0)),
                    );
                    true
                } else if kev.code == KeyCode::Tab {
                    self.selected = (self.selected + 1) % 3;
                    true
                } else if kev.code == KeyCode::BackTab {
                    self.selected = (self.selected + 2) % 3;
                    true
                } else {
                    if self.selected == 0 {
                        self.query_state.input(kev)
                    } else if self.selected == 1 {
                        self.send_state.input(kev)
                    } else {
                        false
                    }
                }
            }
            AppEvent::MovieManagerEvent(MovieManagerEvent::SearchResults(results)) => {
                self.results = results;
                self.table_state.select(None);
                self.is_loading = false;
                true
            }
            _ => false,
        }
    }
}
