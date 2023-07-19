use tui::{
    style::{Style, Color, Modifier}, 
    widgets::{Paragraph, StatefulWidget, Widget, Wrap},
    layout::{Rect,Layout, Constraint, Direction},
    text::{Span, Spans},
    buffer::Buffer,
};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

#[derive(Clone, Debug)]
pub struct Checkbox {
    disabled_style: (Style, Style),
    focused_style: (Style, Style),
    normal_style: (Style, Style),
}

#[derive(Clone, Debug, Default)]
pub struct CheckboxState {
    pub checked: bool,
    pub focused: bool,
    pub disabled: bool,
}

impl Default for Checkbox {
    fn default() -> Checkbox {
        let check_style = Style::default().add_modifier(Modifier::BOLD);
        let focus_style = Style::default().fg(Color::LightRed);
        let normal_style = Style::default().fg(Color::White);
        let disabled_style = Style::default().fg(Color::Gray);
        Checkbox {
            disabled_style: (check_style.clone(), disabled_style),
            focused_style: (check_style.clone(), focus_style),
            normal_style: (check_style, normal_style),
        }
    }
}

impl Checkbox {
    pub fn with_style(mut self, check: Style, brackets: Style) -> Self {
        self.normal_style = (check,brackets);
        self
    }

    pub fn with_focus_style(mut self, check: Style, brackets: Style) -> Self {
        self.focused_style = (check,brackets);
        self
    }

    pub fn with_disabled_style(mut self, check: Style, brackets: Style) -> Self {
        self.disabled_style = (check,brackets);
        self
    }
}

impl CheckboxState {
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_enabled(&self) -> bool {
        !self.disabled
    }

    pub fn check(&mut self, state: bool) {
        self.checked = state;
    }

    pub fn toggle(&mut self, state: bool) {
        self.disabled = !state;
    }

    pub fn focus(&mut self, state: bool) {
        self.focused = state;
    }

    pub fn input(&mut self, kev: KeyEvent) -> bool {
        if kev.code == KeyCode::Char(' ') {
            self.checked = !self.checked;
            return true;
        }
        false
    }
}

impl StatefulWidget for Checkbox {
    type State = CheckboxState;

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
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(rows[0]);

        let (check_style, bracket_style) = if state.disabled {
            self.disabled_style
        } else if state.focused {
            self.focused_style
        } else {
            self.normal_style
        };

        let check = if state.checked {
            Span::styled("x", check_style.clone())
        } else {
            Span::styled(" ", check_style.clone())
        };

        let content = Spans::from(vec![
            Span::styled("[", bracket_style.clone()),
            check,
            Span::styled("]", bracket_style.clone()),
        ]);
        let par = Paragraph::new(content);

        Widget::render(par, chunks[0], buf);
    }
}