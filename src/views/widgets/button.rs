use tui::{
    style::{Style, Color, Modifier}, 
    widgets::{Paragraph, StatefulWidget, Widget, Wrap},
    layout::{Rect,Layout, Constraint, Direction},
    text::{Span, Spans},
    buffer::Buffer,
};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

use crate::util::{OwnedSpan, OwnedSpans};

#[derive(Clone, Debug)]
pub struct Button {
    disabled_style: Style,
    focused_style: Style,
    normal_style: Style,
    clicked_style: Style,
    text: OwnedSpans,
}

impl Default for Button {
    fn default() -> Button {
        let focused_style = Style::default().bg(Color::LightRed);
        let normal_style = Style::default().bg(Color::White);
        let clicked_style = Style::default().add_modifier(Modifier::BOLD);
        let disabled_style = Style::default().bg(Color::Gray);
        let text = OwnedSpans::from("Button");
        Button {
            disabled_style,
            focused_style,
            normal_style,
            clicked_style,
            text,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ButtonState {
    pub clicked: bool,
    pub focused: bool,
    pub disabled: bool,
}


impl Button {
    pub fn with_style(mut self, text: Style) -> Self {
        self.normal_style = text;
        self
    }

    pub fn with_focus_style(mut self, text: Style) -> Self {
        self.focused_style = text;
        self
    }

    pub fn with_disabled_style(mut self, text: Style) -> Self {
        self.disabled_style = text;
        self
    }

    pub fn with_text<T>(mut self, text: T) -> Self 
    where T: Into<OwnedSpans> {
        self.text = text.into();
        self
    }
}

impl ButtonState {
    pub fn is_clicked(&self) -> bool {
        self.clicked
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_enabled(&self) -> bool {
        !self.disabled
    }

    pub fn clicked(&mut self, state: bool) {
        self.clicked = state;
    }

    pub fn toggle(&mut self, state: bool) {
        self.disabled = !state;
    }

    pub fn focus(&mut self, state: bool) {
        self.focused = state;
    }

    pub fn input(&mut self, kev: KeyEvent) -> bool {
        if kev.code == KeyCode::Enter {
            self.clicked = true;
            return true;
        }
        false
    }
}

impl StatefulWidget for Button {
    type State = ButtonState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut spans = self.text.0.clone();
        spans.insert(0, OwnedSpan::raw(" "));
        spans.push(OwnedSpan::raw(" "));
        let content = OwnedSpans::from(spans);

        let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(1),
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(area.clone());
        let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(content.width() as u16),
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(rows[0]);

        let style = if state.disabled {
            self.disabled_style
        } else if state.focused {
            self.focused_style
        } else {
            self.normal_style
        };

        let style = if state.clicked {
            style.patch(self.clicked_style)
        } else { style };

        let par = Paragraph::new(content).style(style);

        Widget::render(par, chunks[0], buf);
    }
}
