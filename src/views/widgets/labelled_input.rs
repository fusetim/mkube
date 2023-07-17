
use tui::{
    style::{Style},
    widgets::{Widget, StatefulWidget, Paragraph, Wrap},
    text::{Text},
    buffer::Buffer,
    layout::{Rect, Constraint, Direction, Layout},
};
use std::io::stdout;
use crossterm::event::{KeyEvent};

use crate::util::{OwnedSpan, OwnedSpans};
use crate::views::widgets::input::{Input, InputState};

#[derive(Clone, Debug)]
pub struct LabelledInput {
    pub input: Input,
    pub label: OwnedSpans,
    pub label_constraint: Constraint,
}

impl LabelledInput {
    pub fn new<T>(label: T, input: Input) -> Self 
    where T: Into<OwnedSpans>
    {
        let label = label.into();
        let width = label.width();
        Self {
            input,
            label,
            label_constraint: Constraint::Length(width as u16),
        }
    }

    pub fn with_input(&mut self, input: Input) {
        self.input = input;
    }

    pub fn with_label<T>(&mut self, label: T) 
    where T: Into<OwnedSpans>
    {
        self.label = label.into();
    }

    pub fn with_label_constraint(&mut self, constraint: Constraint) {
        self.label_constraint= constraint;
    }
}

#[derive(Clone, Debug, Default)]
pub struct LabelledInputState {
    input_state: InputState,
}

impl LabelledInputState {
    pub fn input(&mut self, kev: KeyEvent) -> bool {
        self.input_state.input(kev)
    }

    pub fn set_focus(&mut self, f: bool) {
        self.input_state.set_focus(f);
    }

    pub fn toggle(&mut self, d: bool) {
        self.input_state.toggle(d);
    }

    pub fn is_focused(&self) -> bool {
        self.input_state.is_focused()
    }

    pub fn is_disabled(&self) -> bool {
        self.input_state.is_disabled()
    }

    pub fn get_value<'a>(&'a self) -> &'a str {
        self.input_state.get_value()
    }
}

impl StatefulWidget for LabelledInput {
    type State = LabelledInputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
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
                self.label_constraint,
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(rows[0]);

        let label = Paragraph::new(self.label).wrap(Wrap { trim: true});
        Widget::render(label, chunks[0], buf);
        StatefulWidget::render(self.input, chunks[1], buf, &mut state.input_state);
    }
}