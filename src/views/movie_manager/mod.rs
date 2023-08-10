use crossterm::event::KeyCode;
use std::path::PathBuf;
use tui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

pub mod details;
pub mod search;
pub mod table;

use crate::views::widgets::InputState;
use crate::AppEvent;
use search::{MovieSearch, MovieSearchState};
use table::{MovieTable, MovieTableState};

#[derive(Clone, Debug, Default)]
pub struct MovieManager {
    table: MovieTable,
    search: MovieSearch,
}
#[derive(Clone, Debug)]
pub enum MovieManagerState {
    Table(MovieTableState),
    Search(MovieTableState, MovieSearchState),
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
    OpenTable,
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
        match state {
            MovieManagerState::Table(ref mut state) => {
                StatefulWidget::render(self.table, area, buf, state);
            }
            MovieManagerState::Search(_, ref mut state) => {
                StatefulWidget::render(self.search, area, buf, state);
            }
            _ => {}
        }
    }
}

impl MovieManagerState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match self {
            MovieManagerState::Table(ref mut state) => match app_event {
                AppEvent::MovieManagerEvent(MovieManagerEvent::SearchMovie((
                    movie,
                    fs_id,
                    path,
                ))) => {
                    let mut query_state = InputState::default();
                    query_state.set_value(&movie.title);
                    let new_state = MovieSearchState {
                        movie_path: path,
                        movie_fs_id: fs_id,
                        query_state,
                        ..Default::default()
                    };
                    *self = MovieManagerState::Search(state.clone(), new_state);
                    true
                }
                _ => state.input(app_event),
            },
            MovieManagerState::Search(ref mut table_state, ref mut state) => {
                if let AppEvent::KeyEvent(kev) = app_event {
                    if kev.code == KeyCode::Esc {
                        *self = MovieManagerState::Table(table_state.clone());
                        true
                    } else {
                        state.input(app_event)
                    }
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated(..)) = app_event {
                    table_state.input(app_event)  
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(..)) = app_event {
                    table_state.input(app_event)  
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable) = app_event {
                    *self = MovieManagerState::Table(table_state.clone());
                    true
                } else {
                    state.input(app_event)
                }
            }
            _ => false,
        }
    }
}
