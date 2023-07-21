use tui::widgets::{StatefulWidget, Widget, Block, Tabs, Borders, BorderType};
use tui::{
    backend::{Backend},
    layout::{Rect, Constraint, Direction, Layout},
    Frame,
    symbols::DOT,
    text::{Span, Spans},
    style::{Style, Color},
    buffer::Buffer,
};
use std::pin::Pin;
use std::future::Future;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

pub mod home;
pub mod settings;
pub mod widgets;

use crate::multifs::{MultiFs};
use crate::library::Library;
use settings::{SettingsPage, SettingsState, SettingsMessage};

pub enum AppMessage {
    Future(Box<dyn FnOnce(&mut AppState) -> Pin<Box<dyn Future<Output=Option<AppEvent>>>> + Send + Sync>),
    TriggerEvent(AppEvent),
    SettingsMessage(SettingsMessage),
    Close,    
}

impl std::fmt::Debug for AppMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            AppMessage::Close => write!(f, "AppMessage::Close"),
            AppMessage::Future(_) => write!(f, "AppMessage::Future(<builder>)"),
            AppMessage::TriggerEvent(evt) => write!(f, "AppMessage::TriggerEvent({:?})", evt),
            AppMessage::SettingsMessage(msg) => write!(f, "AppMessage::SettingsMessage({:?})", msg),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    KeyEvent(KeyEvent),
    SettingsEvent(settings::SettingsEvent),
}

#[derive(Default)]
pub enum TabState {
    #[default]
    Home,
    Settings(SettingsState),
}

impl From<&TabState> for usize {
    fn from(v: &TabState) -> usize {
        match v {
            &TabState::Home => 0,
            &TabState::Settings(_) => 1,
        }
    }
}


#[derive(Default)]
pub struct AppState {
    pub tab: TabState,
    pub frame_number: usize,
    pub events: Vec<AppEvent>,
    pub settings_state: SettingsState,
    pub libraries: Vec<Library>, 
    pub conns: Vec<MultiFs>,
}

impl AppState {
    pub fn tick(&mut self) {
        self.frame_number += 1;
    } 

    pub fn register_event(&mut self, evt: AppEvent) -> bool {
        self.events.push(evt.clone());
        match evt {
            AppEvent::KeyEvent(kev) => {
                if kev.code == KeyCode::Char('s') && kev.modifiers == KeyModifiers::ALT {
                    self.tab = TabState::Settings(self.settings_state.clone());
                    use crate::MESSAGE_SENDER;
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender.send(crate::AppMessage::Future(Box::new(|appstate: &mut AppState| {
                        let libs = appstate.libraries.clone(); 
                        Box::pin(async move {
                            Some(AppEvent::SettingsEvent(settings::SettingsEvent::OpenMenu(libs)))
                    })}))).unwrap();
                    true
                } else if kev.code == KeyCode::Char('h') && kev.modifiers == KeyModifiers::ALT {
                    self.tab = TabState::Home;
                    true
                } else if let TabState::Settings(ref mut sstate) = self.tab {
                    sstate.press_key(kev.clone())
                } else {
                    false
                }
            }
            _ => {
                if let TabState::Settings(ref mut sstate) = self.tab {
                    sstate.input(evt)
                } else {
                    false
                }
            }
        }
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}

#[derive(Clone, Debug)]
pub struct App {
    pub settings_page: SettingsPage,
}

impl StatefulWidget for App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Percentage(90),
            ].as_ref()
        )
        .split(area.clone());

        let titles = ["Home (Alt+H)", "Settings (Alt+S)"].iter().cloned().map(Spans::from).collect();
        let tabs = Tabs::new(titles)
            .block(Block::default().title("MKube").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .select((&state.tab).into())
            .divider(DOT);
        if let TabState::Settings(ref mut sstate) = state.tab {
            self.settings_page.render(chunks[1], buf, sstate);
        } else {
            let child = Block::default()
                .title(format!("Child  / Frame: {} / Events: {:?} / Libraries: {}", state.frame_number, state.events, state.libraries.len()))
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(Color::White))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(Color::Black));
            child.render(chunks[1], buf);
        }
        tabs.render(chunks[0], buf);   
    }
}

