use tui::{
    style::{Style, Color},
    buffer::Buffer,
    layout::{Constraint, Rect, Direction},
    widgets::{StatefulWidget, Widget, Block, Borders, BorderType},
};

pub mod details;
pub mod table;

use table::{MovieTable, MovieTableState, MovieTableMessage, MovieTableEvent};
use crate::{AppEvent, AppState, AppMessage};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MovieManager {
    table: MovieTable,
}
#[derive(Clone, Debug)]
pub enum MovieManagerState {
    Table(MovieTableState),
}

impl Default for MovieManagerState {
    fn default() -> MovieManagerState {
        MovieManagerState::Table(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerEvent {
    ClearMovieList,
    MovieDiscovered(crate::nfo::Movie),
}
#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerMessage {
    RefreshMovies,
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
            _ => { },
        }
        Widget::render(block, area, buf);
    }
}

impl MovieManagerState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match self {
            MovieManagerState::Table(ref mut state) => {
                state.input(app_event)
            },
            _ => { false },
        }
    }
}