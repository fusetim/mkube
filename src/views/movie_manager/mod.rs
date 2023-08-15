use std::path::PathBuf;
use tui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

pub mod details;
pub mod editor;
pub mod search;
pub mod table;

use crate::views::widgets::InputState;
use crate::AppEvent;
use editor::{MovieEditor, MovieEditorState};
use search::{MovieSearch, MovieSearchState};
use table::{MovieTable, MovieTableState};

#[derive(Clone, Debug, Default)]
pub struct MovieManager {
    table: MovieTable,
    search: MovieSearch,
    editor: MovieEditor,
}

#[derive(Clone, Debug, Default)]
enum InnerState {
    #[default]
    Table,
    Search(MovieSearchState),
    Editor(MovieEditorState),
}

#[derive(Clone, Debug, Default)]
pub struct MovieManagerState {
    table_state: MovieTableState,
    inner: InnerState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerEvent {
    ClearMovieList,
    MovieDiscovered((crate::nfo::Movie, usize, PathBuf)),
    MovieUpdated((crate::nfo::Movie, usize, PathBuf)),
    SearchMovie((crate::nfo::Movie, usize, PathBuf)),
    EditMovie((crate::nfo::Movie, usize, PathBuf)),
    SearchResults(Vec<tmdb_api::movie::MovieShort>),
    OpenTable,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerMessage {
    RefreshMovies,
    SearchTitle(String),
    CreateNfo((u64, usize, PathBuf)), // tmdb_id, fs_id, movie_path
    SaveNfo((crate::nfo::Movie, usize, PathBuf)),
}

impl StatefulWidget for MovieManager {
    type State = MovieManagerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match state.inner {
            InnerState::Table => {
                StatefulWidget::render(self.table, area, buf, &mut state.table_state);
            }
            InnerState::Search(ref mut state) => {
                StatefulWidget::render(self.search, area, buf, state);
            }
            InnerState::Editor(ref mut state) => {
                StatefulWidget::render(self.editor, area, buf, state);
            }
            _ => {}
        }
    }
}

impl MovieManagerState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match self.inner {
            InnerState::Table => match app_event {
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
                    self.inner = InnerState::Search(new_state);
                    true
                }
                AppEvent::MovieManagerEvent(MovieManagerEvent::EditMovie((movie, fs_id, path))) => {
                    let state = MovieEditorState::default().with(movie, fs_id, path);
                    self.inner = InnerState::Editor(state);
                    true
                }
                _ => self.table_state.input(app_event),
            },
            InnerState::Search(ref mut state) => {
                if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated(..)) = app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable) = app_event
                {
                    self.inner = InnerState::Table;
                    true
                } else {
                    state.input(app_event)
                }
            }
            InnerState::Editor(ref mut state) => {
                if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated(..)) = app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable) = app_event
                {
                    self.inner = InnerState::Table;
                    true
                } else {
                    state.input(app_event)
                }
            }
            _ => false,
        }
    }
}
