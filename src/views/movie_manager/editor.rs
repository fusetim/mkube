use crossterm::event::KeyCode;
use std::path::PathBuf;
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::DOT,
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, Row, StatefulWidget, Table, TableState, Tabs, Widget,
    },
};

use crate::nfo::{Actor, CrewPerson, Movie, Thumb};
use crate::views::movie_manager::{MovieManagerEvent, MovieManagerMessage};
use crate::views::widgets::{Input, InputState};
use crate::MESSAGE_SENDER;
use crate::{AppEvent, AppMessage};

const FIELDS: [&'static str; 10] = [
    "Title",
    "Original Title",
    "Release Date",
    "Tagline",
    "Plot",
    "Genres",
    "Tags",
    "Studio",
    "Countries",
    "Source",
];

const TAB_NAMES: [&'static str; 6] = [
    "General",
    "Actors",
    "Productors",
    "Directors",
    "Save",
    "Cancel",
];

#[derive(Clone, Debug, Default)]
pub struct MovieEditor {}

#[derive(Clone, Debug, Default)]
pub struct MovieEditorState {
    pub movie_nfo: Movie,
    pub movie_fs_id: usize,
    pub movie_path: PathBuf,
    pub table_state: TableState,
    pub fields_value: [InputState; 10],
    pub actor_state: Vec<[InputState; 4]>,
    pub producer_state: Vec<[InputState; 3]>,
    pub director_state: Vec<[InputState; 3]>,
    pub open_tab: usize,
    pub selected_tab: Option<usize>,
    pub selected_column: usize,
}

impl StatefulWidget for MovieEditor {
    type State = MovieEditorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .title(" Movie Editor ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded);

        let inner = block.inner(area.clone());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(2), Constraint::Percentage(100)])
            .split(inner.clone());
        let tabs = Tabs::new(
            TAB_NAMES
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if let Some(s) = state.selected_tab {
                        if i == s {
                            Spans::from(Span::styled(*t, Style::default().fg(Color::LightRed)))
                        } else {
                            Spans::from(*t)
                        }
                    } else {
                        Spans::from(*t)
                    }
                })
                .collect(),
        )
        .select(state.open_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(DOT);

        match state.open_tab {
            1 => {
                self.render_cast_tab(chunks[1], buf, state);
            }
            2 => {
                self.render_crew_tab(
                    chunks[1],
                    buf,
                    &mut state.producer_state,
                    &mut state.table_state,
                    state.selected_column,
                );
            }
            3 => {
                self.render_crew_tab(
                    chunks[1],
                    buf,
                    &mut state.director_state,
                    &mut state.table_state,
                    state.selected_column,
                );
            }
            _ => {
                self.render_general_tab(chunks[1], buf, state);
            }
        }
        block.render(area, buf);
        tabs.render(inner, buf);
    }
}

impl MovieEditorState {
    pub fn with(mut self, movie_nfo: Movie, movie_fs_id: usize, movie_path: PathBuf) -> Self {
        self.fields_value[0].set_value(&movie_nfo.title);
        self.fields_value[1].set_value(movie_nfo.original_title.as_deref().unwrap_or(""));
        self.fields_value[2].set_value(movie_nfo.premiered.as_deref().unwrap_or(""));
        self.fields_value[3].set_value(movie_nfo.tagline.as_deref().unwrap_or(""));
        self.fields_value[4].set_value(movie_nfo.plot.as_deref().unwrap_or(""));
        self.fields_value[5].set_value(movie_nfo.genre.join(", "));
        self.fields_value[6].set_value(movie_nfo.tag.join(", "));
        self.fields_value[7].set_value(movie_nfo.studio.join(", "));
        self.fields_value[8].set_value(movie_nfo.country.join(", "));
        self.fields_value[9].set_value(movie_nfo.source.as_deref().unwrap_or(""));
        self.actor_state = movie_nfo
            .actor
            .iter()
            .map(|actor| {
                let mut inputs: [InputState; 4] = Default::default();
                inputs[0].set_value(&actor.name);
                inputs[1].set_value(actor.role.join(", "));
                if let Some(id) = actor.tmdbid {
                    inputs[2].set_value(format!("{}", id));
                }
                if let Some(thumb) = &actor.thumb {
                    inputs[3].set_value(&thumb.path);
                }
                inputs
            })
            .collect();
        self.producer_state = movie_nfo.producer.iter().map(crew_to_inputs).collect();
        self.director_state = movie_nfo.director.iter().map(crew_to_inputs).collect();
        Self {
            movie_nfo,
            movie_fs_id,
            movie_path,
            ..self
        }
    }

    pub fn table_len(&self) -> usize {
        match self.open_tab {
            1 => self.actor_state.len() + 1,
            2 => self.producer_state.len() + 1,
            3 => self.director_state.len() + 1,
            _ => FIELDS.len(),
        }
    }

    pub fn table_columns(&self) -> usize {
        match self.open_tab {
            1 => 4,
            2 | 3 => 3,
            _ => 1,
        }
    }

    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match app_event {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Enter {
                    if let Some(selected) = self.selected_tab {
                        let sender = MESSAGE_SENDER.get().unwrap();
                        if selected == 4 {
                            sender
                                .send(AppMessage::MovieManagerMessage(
                                    MovieManagerMessage::SaveNfo((
                                        self.get_nfo(),
                                        self.movie_fs_id,
                                        self.movie_path.clone(),
                                    )),
                                ))
                                .unwrap();
                        } else if selected == 5 {
                            sender
                                .send(AppMessage::TriggerEvent(AppEvent::MovieManagerEvent(
                                    MovieManagerEvent::OpenTable,
                                )))
                                .unwrap();
                        } else {
                            self.open_tab = selected;
                            self.selected_column = 0;
                        }
                    } else if self.table_state.selected().is_some() {
                        self.selected_column = (self.selected_column + 1) % self.table_columns();
                    } else {
                        return false;
                    }
                    true
                } else if kev.code == KeyCode::Tab {
                    if let Some(v) = self.table_state.selected() {
                        if v + 1 < self.table_len() {
                            self.table_state.select(Some(v + 1));
                        } else {
                            self.table_state.select(None);
                            self.selected_tab = Some(0);
                        }
                    } else if let Some(v) = self.selected_tab {
                        if v + 1 < TAB_NAMES.len() {
                            self.selected_tab = Some(v + 1);
                        } else {
                            self.table_state.select(Some(0));
                            self.selected_tab = None;
                        }
                    } else {
                        self.table_state.select(Some(0));
                        self.selected_tab = None;
                    }
                    true
                } else if kev.code == KeyCode::BackTab {
                    if let Some(v) = self.table_state.selected() {
                        let nv = (v + self.table_len() - 1) % self.table_len();
                        if v != 0 {
                            self.table_state.select(Some(nv));
                        } else {
                            self.table_state.select(None);
                            self.selected_tab = Some(TAB_NAMES.len() - 1);
                        }
                    } else if let Some(v) = self.selected_tab {
                        let nv = (v + TAB_NAMES.len() - 1) % TAB_NAMES.len();
                        if v != 0 {
                            self.selected_tab = Some(nv);
                        } else {
                            self.table_state.select(Some(self.table_len() - 1));
                            self.selected_tab = None;
                        }
                    } else {
                        self.table_state.select(Some(self.table_len() - 1));
                        self.selected_tab = None;
                    }
                    true
                } else if let Some(v) = self.table_state.selected() {
                    match self.open_tab {
                        1 => {
                            if v == self.actor_state.len() {
                                self.actor_state.push(Default::default());
                            }
                            self.actor_state[v][self.selected_column].input(kev)
                        }
                        2 => {
                            if v == self.producer_state.len() {
                                self.producer_state.push(Default::default());
                            }
                            self.producer_state[v][self.selected_column].input(kev)
                        }
                        3 => {
                            if v == self.director_state.len() {
                                self.director_state.push(Default::default());
                            }
                            self.director_state[v][self.selected_column].input(kev)
                        }
                        _ => self.fields_value[v].input(kev),
                    }
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
            Some(self.fields_value[1].get_value().to_owned())
        };
        nfo.premiered = if self.fields_value[2].is_empty() {
            None
        } else {
            Some(self.fields_value[2].get_value().to_owned())
        };
        nfo.tagline = if self.fields_value[3].is_empty() {
            None
        } else {
            Some(self.fields_value[3].get_value().to_owned())
        };
        nfo.plot = if self.fields_value[4].is_empty() {
            None
        } else {
            Some(self.fields_value[4].get_value().to_owned())
        };
        nfo.genre = self.fields_value[5]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.tag = self.fields_value[6]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.studio = self.fields_value[7]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.country = self.fields_value[8]
            .get_value()
            .split(",")
            .map(|s| s.trim().to_owned())
            .collect();
        nfo.source = if self.fields_value[9].is_empty() {
            None
        } else {
            Some(self.fields_value[9].get_value().to_owned())
        };
        nfo.actor = self
            .actor_state
            .iter()
            .filter(|inputs| !inputs[0].is_empty())
            .enumerate()
            .map(|(i, inputs)| Actor {
                name: inputs[0].get_value().to_owned(),
                role: inputs[1]
                    .get_value()
                    .split(",")
                    .map(|s| s.trim().to_string())
                    .collect(),
                order: Some(i as u64),
                tmdbid: inputs[2].get_value().parse().ok(),
                thumb: if inputs[3].is_empty() {
                    None
                } else {
                    Some(Thumb {
                        aspect: None,
                        path: inputs[3].get_value().to_owned(),
                    })
                },
            })
            .collect();
        nfo.director = self
            .director_state
            .iter()
            .filter(|inputs| !inputs[0].is_empty())
            .map(|inputs| CrewPerson {
                name: inputs[0].get_value().to_owned(),
                tmdbid: inputs[1].get_value().parse().ok(),
                thumb: if inputs[2].is_empty() {
                    None
                } else {
                    Some(Thumb {
                        aspect: None,
                        path: inputs[2].get_value().to_owned(),
                    })
                },
            })
            .collect();
        nfo.producer = self
            .producer_state
            .iter()
            .filter(|inputs| !inputs[0].is_empty())
            .map(|inputs| CrewPerson {
                name: inputs[0].get_value().to_owned(),
                tmdbid: inputs[1].get_value().parse().ok(),
                thumb: if inputs[2].is_empty() {
                    None
                } else {
                    Some(Thumb {
                        aspect: None,
                        path: inputs[2].get_value().to_owned(),
                    })
                },
            })
            .collect();
        nfo
    }
}

impl MovieEditor {
    pub fn render_general_tab(self, area: Rect, buf: &mut Buffer, state: &mut MovieEditorState) {
        let row_constraints = vec![Constraint::Min(16), Constraint::Percentage(100)];
        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(row_constraints.as_slice())
            .split(area.clone());

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
                    .bottom_margin(0),
            )
            .widths(&row_constraints)
            .column_spacing(0);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }

    pub fn render_cast_tab(&self, area: Rect, buf: &mut Buffer, state: &mut MovieEditorState) {
        let row_constraints = vec![
            Constraint::Min(30),
            Constraint::Min(30),
            Constraint::Min(10),
            Constraint::Percentage(100),
        ];
        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(row_constraints.as_slice())
            .split(area.clone());

        let rows: Vec<Row> = state
            .actor_state
            .iter_mut()
            .enumerate()
            .map(|(ind, inputs)| {
                for input in inputs.iter_mut() {
                    input.set_focus(false);
                }
                if let Some(s) = state.table_state.selected() {
                    if ind == s {
                        inputs[state.selected_column].set_focus(true);
                    }
                }
                Row::new(
                    inputs
                        .iter_mut()
                        .enumerate()
                        .map(|(i, state)| {
                            Input::render_text(Input::default(), row_chunks[i], state)
                        })
                        .map(|(text, style)| Cell::from(text).style(style))
                        .collect::<Vec<_>>(),
                )
            })
            .chain(std::iter::once(Row::new(vec!["Add an actor", "", "", ""])))
            .collect();

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(
                Row::new(vec!["Name", "Role", "TMDB ID", "Thumbnail URL"])
                    .style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(0),
            )
            .widths(&row_constraints)
            .column_spacing(0);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }

    pub fn render_crew_tab(
        &self,
        area: Rect,
        buf: &mut Buffer,
        field_state: &mut Vec<[InputState; 3]>,
        table_state: &mut TableState,
        selected_column: usize,
    ) {
        let row_constraints = vec![
            Constraint::Min(30),
            Constraint::Min(10),
            Constraint::Percentage(100),
        ];
        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(row_constraints.as_slice())
            .split(area.clone());

        let rows: Vec<Row> = field_state
            .iter_mut()
            .enumerate()
            .map(|(ind, inputs)| {
                for input in inputs.iter_mut() {
                    input.set_focus(false);
                }
                if let Some(s) = table_state.selected() {
                    if ind == s {
                        inputs[selected_column].set_focus(true);
                    }
                }
                Row::new(
                    inputs
                        .iter_mut()
                        .enumerate()
                        .map(|(i, state)| {
                            Input::render_text(Input::default(), row_chunks[i], state)
                        })
                        .map(|(text, style)| Cell::from(text).style(style))
                        .collect::<Vec<_>>(),
                )
            })
            .chain(std::iter::once(Row::new(vec!["Add a line", "", ""])))
            .collect();

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(
                Row::new(vec!["Name", "TMDB ID", "Thumbnail URL"])
                    .style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(0),
            )
            .widths(&row_constraints)
            .column_spacing(0);

        StatefulWidget::render(table, area, buf, table_state);
    }
}

fn crew_to_inputs(person: &CrewPerson) -> [InputState; 3] {
    let mut inputs: [InputState; 3] = Default::default();
    inputs[0].set_value(&person.name);
    if let Some(id) = person.tmdbid {
        inputs[1].set_value(format!("{}", id));
    }
    if let Some(thumb) = &person.thumb {
        inputs[2].set_value(&thumb.path);
    }
    inputs
}
