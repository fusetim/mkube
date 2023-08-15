use crossterm::event::KeyCode;
use std::path::PathBuf;
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::nfo::Movie;
use crate::views::movie_manager::MovieManagerMessage;
use crate::views::widgets::{Button, ButtonState, Input, InputState};
use crate::MESSAGE_SENDER;
use crate::{AppEvent, AppMessage};

const FIELDS: [&'static str; 9] = [
    "Title",
    "Original Title",
    "Plot",
    "Genres",
    "Tags",
    "Countries",
    "Release Date",
    "Tagline",
    "Source",
];

#[derive(Clone, Debug)]
pub struct MovieEditor {
    pub save: Button,
}

impl Default for MovieEditor {
    fn default() -> MovieEditor {
        let mut input = Input::default();
        input.placeholder = Some("Movie title".into());
        MovieEditor {
            save: Button::new("Save"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MovieEditorState {
    pub movie_nfo: Movie,
    pub movie_fs_id: usize,
    pub movie_path: PathBuf,
    pub table_state: TableState,
    pub fields_value: [InputState; 9],
    pub save_state: ButtonState,
}

impl StatefulWidget for MovieEditor {
    type State = MovieEditorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .title(" Movie Editor ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(12), Constraint::Min(1)])
            .split(block.inner(area.clone()));

        let row_constraints = vec![Constraint::Min(16), Constraint::Percentage(100)];
        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(row_constraints.as_slice())
            .split(chunks[0].clone());

        let rows: Vec<Row> = FIELDS
            .iter()
            .zip(state.fields_value.iter_mut())
            .enumerate()
            .map(|(ind, (name, input))| {
                input.set_focus(false);
                if let Some(s) = state.table_state.selected() {
                    if ind == s {
                        input.set_focus(true);
                    }
                }
                let (content, style) = Input::default().render_text(row_chunks[1], input);
                Row::new(vec![(*name).into(), Cell::from(content).style(style)])
            })
            .collect();

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(
                Row::new(vec!["Name", "Value"])
                    .style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(1),
            )
            .widths(&row_constraints)
            .column_spacing(0);

        StatefulWidget::render(table, chunks[0], buf, &mut state.table_state);
        block.render(area, buf);
        self.save.render(chunks[1], buf, &mut state.save_state);
    }
}

impl MovieEditorState {
    pub fn with(mut self, movie_nfo: Movie, movie_fs_id: usize, movie_path: PathBuf) -> Self {
        self.fields_value[0].set_value(&movie_nfo.title);
        self.fields_value[1].set_value(movie_nfo.original_title.as_deref().unwrap_or(""));
        self.fields_value[2].set_value(movie_nfo.plot.as_deref().unwrap_or(""));
        self.fields_value[3].set_value(movie_nfo.genre.join(", "));
        self.fields_value[4].set_value(movie_nfo.tag.join(", "));
        self.fields_value[5].set_value(movie_nfo.country.join(", "));
        self.fields_value[6].set_value(movie_nfo.premiered.as_deref().unwrap_or(""));
        self.fields_value[7].set_value(movie_nfo.tagline.as_deref().unwrap_or(""));
        self.fields_value[8].set_value(movie_nfo.source.as_deref().unwrap_or(""));
        Self {
            movie_nfo,
            movie_fs_id,
            movie_path,
            ..self
        }
    }

    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match app_event {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Enter && self.save_state.is_focused() {
                    self.save_state.click(true);
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender
                        .send(AppMessage::MovieManagerMessage(
                            MovieManagerMessage::SaveNfo((
                                self.get_nfo(),
                                self.movie_fs_id,
                                self.movie_path.clone(),
                            )),
                        ))
                        .unwrap();
                    true
                } else if kev.code == KeyCode::Tab {
                    if let Some(v) = self.table_state.selected() {
                        if v + 1 < FIELDS.len() {
                            self.table_state.select(Some(v + 1));
                        } else {
                            self.table_state.select(None);
                            self.save_state.focus(true);
                        }
                    } else {
                        self.table_state.select(Some(0));
                        self.save_state.focus(false);
                    }
                    true
                } else if kev.code == KeyCode::BackTab {
                    if let Some(v) = self.table_state.selected() {
                        let nv = (v + FIELDS.len() - 1) % FIELDS.len();
                        if v != 0 {
                            self.table_state.select(Some(nv));
                        } else {
                            self.table_state.select(None);
                            self.save_state.focus(true);
                        }
                    } else {
                        self.table_state.select(Some(FIELDS.len() - 1));
                        self.save_state.focus(false);
                    }
                    true
                } else if let Some(v) = self.table_state.selected() {
                    self.fields_value[v].input(kev)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn get_nfo(&mut self) -> Movie {
        let mut nfo = self.movie_nfo.clone();
        nfo.title = self.fields_value[0].get_value().to_owned();
        nfo.original_title = if self.fields_value[1].is_empty() {
            None
        } else {
            Some(self.fields_value[6].get_value().to_owned())
        };
        nfo.plot = if self.fields_value[2].is_empty() {
            None
        } else {
            Some(self.fields_value[2].get_value().to_owned())
        };
        nfo.genre = self.fields_value[3]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.tag = self.fields_value[4]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.country = self.fields_value[5]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.premiered = if self.fields_value[6].is_empty() {
            None
        } else {
            Some(self.fields_value[6].get_value().to_owned())
        };
        nfo.tagline = if self.fields_value[7].is_empty() {
            None
        } else {
            Some(self.fields_value[7].get_value().to_owned())
        };
        nfo.source = if self.fields_value[8].is_empty() {
            None
        } else {
            Some(self.fields_value[8].get_value().to_owned())
        };
        nfo
    }
}
