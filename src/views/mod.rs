use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::future::Future;
use std::pin::Pin;
use tui::widgets::{Block, Borders, StatefulWidget, Tabs, Widget};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::DOT,
    text::Spans,
};

pub mod movie_manager;
pub mod settings;
pub mod widgets;

use crate::library::Library;
use movie_manager::{MovieManager, MovieManagerEvent, MovieManagerMessage, MovieManagerState};
use settings::{SettingsMessage, SettingsPage, SettingsState};

pub enum AppMessage {
    Future(
        Box<
            dyn FnOnce(&mut AppState) -> Pin<Box<dyn Future<Output = Option<AppEvent>>>>
                + Send
                + Sync,
        >,
    ),
    TriggerEvent(AppEvent),
    SettingsMessage(SettingsMessage),
    MovieManagerMessage(MovieManagerMessage),
    Close,
}

impl std::fmt::Debug for AppMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            AppMessage::Close => write!(f, "AppMessage::Close"),
            AppMessage::Future(_) => write!(f, "AppMessage::Future(<builder>)"),
            AppMessage::TriggerEvent(evt) => write!(f, "AppMessage::TriggerEvent({:?})", evt),
            AppMessage::SettingsMessage(msg) => write!(f, "AppMessage::SettingsMessage({:?})", msg),
            AppMessage::MovieManagerMessage(msg) => {
                write!(f, "AppMessage::MovieManagerMessage({:?})", msg)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    KeyEvent(KeyEvent),
    SettingsEvent(settings::SettingsEvent),
    MovieManagerEvent(MovieManagerEvent),
}

pub enum TabState {
    MovieManager(MovieManagerState),
    Settings(SettingsState),
}

impl From<&TabState> for usize {
    fn from(v: &TabState) -> usize {
        match v {
            &TabState::MovieManager(_) => 0,
            &TabState::Settings(_) => 1,
        }
    }
}

impl Default for TabState {
    fn default() -> TabState {
        TabState::MovieManager(Default::default())
    }
}

#[derive(Default)]
pub struct AppState {
    pub tab: TabState,
    pub saved_movie_state: Option<MovieManagerState>,
    pub libraries: Vec<Option<Library>>,
}

impl AppState {
    pub fn register_event(&mut self, evt: AppEvent) -> bool {
        match evt {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Char('s') && kev.modifiers == KeyModifiers::ALT {
                    if let TabState::MovieManager(state) = &self.tab {
                        self.saved_movie_state = Some(state.clone());
                    }
                    self.tab = TabState::Settings(Default::default());
                    use crate::MESSAGE_SENDER;
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender
                        .send(crate::AppMessage::Future(Box::new(
                            |appstate: &mut AppState| {
                                let libs = appstate.libraries.iter().flatten().cloned().collect();
                                Box::pin(async move {
                                    Some(AppEvent::SettingsEvent(
                                        settings::SettingsEvent::OpenMenu(libs),
                                    ))
                                })
                            },
                        )))
                        .unwrap();
                    true
                } else if kev.code == KeyCode::Char('h') && kev.modifiers == KeyModifiers::ALT {
                    if let TabState::MovieManager(ref mut mstate) = self.tab {
                        mstate.input(AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable))
                    } else {
                        self.tab = TabState::MovieManager(
                            self.saved_movie_state.clone().unwrap_or_default(),
                        );
                        true
                    }
                } else if let TabState::Settings(ref mut state) = self.tab {
                    state.press_key(kev.clone())
                } else if let TabState::MovieManager(ref mut state) = self.tab {
                    state.input(evt.clone())
                } else {
                    false
                }
            }
            _ => {
                if let TabState::Settings(ref mut sstate) = self.tab {
                    sstate.input(evt)
                } else if let TabState::MovieManager(ref mut state) = self.tab {
                    state.input(evt)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct App {
    pub settings_page: SettingsPage,
    pub movie_manager: MovieManager,
}

impl StatefulWidget for App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(3), Constraint::Percentage(90)].as_ref())
            .split(area.clone());

        let titles = ["Home (Alt+H)", "Settings (Alt+S)"]
            .iter()
            .cloned()
            .map(Spans::from)
            .collect();
        let tabs = Tabs::new(titles)
            .block(Block::default().title("MKube").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .select((&state.tab).into())
            .divider(DOT);
        /*if let TabState::Settings(ref mut sstate) = state.tab {
            self.settings_page.render(chunks[1], buf, sstate);
        } else if let {
            let child = Block::default()
                .title(format!("Child  / Frame: {} / Events: {:?} / Libraries: {}", state.frame_number, state.events, state.libraries.len()))
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(Color::White))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(Color::Black));
            child.render(chunks[1], buf);
        }*/
        match state.tab {
            TabState::Settings(ref mut state) => {
                self.settings_page.render(chunks[1], buf, state);
            }
            TabState::MovieManager(ref mut state) => {
                self.movie_manager.render(chunks[1], buf, state);
            }
        }
        tabs.render(chunks[0], buf);
    }
}
