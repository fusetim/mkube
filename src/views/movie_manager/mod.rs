use tui::{
    style::{Style, Color},
    buffer::Buffer,
    layout::{Constraint, Rect, Direction},
    widgets::{StatefulWidget, Widget, Block, Borders, BorderType},
};

use std::path::PathBuf;
use crossterm::event::KeyCode;


pub mod details;
pub mod table;
pub mod search;

use table::{MovieTable, MovieTableState};
use search::{MovieSearch, MovieSearchState};
use crate::{AppEvent, AppState, AppMessage};
use crate::views::widgets::{Input, InputState};

#[derive(Clone, Debug, Default)]
pub struct MovieManager {
    table: MovieTable,
    search: MovieSearch,
}
#[derive(Clone, Debug)]
pub enum MovieManagerState {
    Table(MovieTableState),
    Search(MovieSearchState),
}

impl Default for MovieManagerState {
    fn default() -> MovieManagerState {
        MovieManagerState::Table(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerEvent {
    ClearMovieList,
    MovieDiscovered((crate::nfo::Movie, usize, PathBuf)),
    MovieUpdated((crate::nfo::Movie, usize, PathBuf)),
    SearchMovie((crate::nfo::Movie, usize, PathBuf)),
    SearchResults(Vec<tmdb_api::movie::MovieShort>),
}
#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerMessage {
    RefreshMovies,
    SearchTitle(String),
    SaveNfo((u64, usize, PathBuf)), // tmdb_id, movie_path
}

impl StatefulWidget for MovieManager {
    type State = MovieManagerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded); 
        let inner = block.inner(area.clone());   
        match state {
            MovieManagerState::Table(ref mut state) => {
                block = block.title(" Movies ");
                StatefulWidget::render(self.table, inner, buf, state);
            },
            MovieManagerState::Search(ref mut state) => {
                block = block.title(" Search ");
                StatefulWidget::render(self.search, inner, buf, state);
            },
            _ => { },
        }
        Widget::render(block, area, buf);
    }
}

impl MovieManagerState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match self {
            MovieManagerState::Table(ref mut state) => {
                match app_event {
                    AppEvent::MovieManagerEvent(MovieManagerEvent::SearchMovie((movie, fs_id, path))) => {
                        let mut query_state = InputState::default();
                        query_state.set_value(&movie.title);
                        let new_state = MovieSearchState {
                            movie_path: path,
                            movie_fs_id: fs_id,
                            query_state,
                            ..Default::default()
                        };
                        *self = MovieManagerState::Search(new_state);
                        true
                    }
                    _ => state.input(app_event),
                }
            },
            MovieManagerState::Search(ref mut state) => {
                if let AppEvent::KeyEvent(kev) = app_event {
                    if kev.code == KeyCode::Esc {
                        *self = Default::default();
                        true
                    } else {
                        state.input(app_event)
                    }
                } else {
                    state.input(app_event)
                }
            },
            _ => { false },
        }
    }
}