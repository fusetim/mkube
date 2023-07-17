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
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

pub mod home;
pub mod settings;
pub mod widgets;

use settings::{SettingsPage, SettingsState};

#[derive(Clone, Debug)]
pub enum Event {
    Key(KeyEvent),
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
    pub events: Vec<Event>,
    pub settings_state: SettingsState,
}

impl AppState {
    pub fn tick(&mut self) {
        self.frame_number += 1;
    } 

    pub fn register_event(&mut self, event: Event) {
        if let Event::Key(kev) = event {
            if kev.code == KeyCode::Char('s') && kev.modifiers == KeyModifiers::ALT {
                self.tab = TabState::Settings(self.settings_state.clone());
            }
            if kev.code == KeyCode::Char('h') && kev.modifiers == KeyModifiers::ALT {
                self.tab = TabState::Home;
            }
            if let TabState::Settings(ref mut sstate) = self.tab {
                sstate.press_key(kev.clone());
            }
        }
        self.events.push(event);
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
                .title(format!("Child  / Frame: {} / Events: {:?}", state.frame_number, state.events))
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(Color::White))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(Color::Black));
            child.render(chunks[1], buf);
        }
        tabs.render(chunks[0], buf);   
    }
}

