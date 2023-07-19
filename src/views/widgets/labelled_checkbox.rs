
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
use crate::views::widgets::checkbox::{Checkbox, CheckboxState};

#[derive(Clone, Debug)]
pub struct LabelledCheckbox {
    pub checkbox: Checkbox,
    pub label: OwnedSpans,
    pub label_constraint: Constraint,
}

impl LabelledCheckbox {
    pub fn new<T>(label: T, checkbox: Checkbox) -> Self 
    where T: Into<OwnedSpans>
    {
        let label = label.into();
        let width = label.width();
        Self {
            checkbox,
            label,
            label_constraint: Constraint::Length(width as u16),
        }
    }

    pub fn with_checkbox(&mut self, checkbox: Checkbox) {
        self.checkbox = checkbox;
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
pub struct LabelledCheckboxState {
    checkbox_state: CheckboxState,
}

impl LabelledCheckboxState {
    pub fn input(&mut self, kev: KeyEvent) -> bool {
        self.checkbox_state.input(kev)
    }

    pub fn focus(&mut self, f: bool) {
        self.checkbox_state.focus(f);
    }

    pub fn toggle(&mut self, d: bool) {
        self.checkbox_state.toggle(d);
    }

    pub fn is_focused(&self) -> bool {
        self.checkbox_state.is_focused()
    }

    pub fn is_enabled(&self) -> bool {
        self.checkbox_state.is_enabled()
    }

    pub fn check(&mut self, state: bool) {
        self.checkbox_state.check(state);
    } 

    pub fn is_checked(&self) -> bool {
        self.checkbox_state.is_checked()
    }

}

impl StatefulWidget for LabelledCheckbox {
    type State = LabelledCheckboxState;

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
                Constraint::Length(3),
                Constraint::Length(1),
                self.label_constraint,
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(rows[0]);

        let label = Paragraph::new(self.label).wrap(Wrap { trim: true});
        Widget::render(label, chunks[2], buf);
        StatefulWidget::render(self.checkbox, chunks[0], buf, &mut state.checkbox_state);
    }
}