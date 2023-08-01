use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Paragraph, StatefulWidget, Widget, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug)]
pub struct Input {
    pub style: Style,
    pub focus_style: Style,
    pub disable_style: Style,
    pub placeholder: Option<String>,
    pub placeholder_style: Style,
    pub horiz_constraint: Constraint,
}

impl Default for Input {
    fn default() -> Input {
        Input {
            style: Style::default().fg(Color::Black).bg(Color::Gray),
            focus_style: Style::default().fg(Color::White).bg(Color::LightRed),
            disable_style: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::UNDERLINED),
            placeholder: None,
            placeholder_style: Style::default().add_modifier(Modifier::ITALIC),
            horiz_constraint: Constraint::Percentage(100),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    value: Vec<String>,
    focused: bool,
    disabled: bool,
    cursor: usize,
}

impl InputState {
    pub fn input(&mut self, kev: KeyEvent) -> bool {
        match kev.code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor, format!("{c}"));
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.value.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => self.cursor = Ord::min(self.cursor + 1, self.value.len()),
            KeyCode::Up => {
                self.cursor = 0;
            }
            KeyCode::Down | KeyCode::End => {
                self.cursor = self.value.len();
            }
            _ => {
                return false;
            }
        };
        true
    }

    pub fn set_focus(&mut self, f: bool) {
        self.focused = f;
    }

    pub fn toggle(&mut self, t: bool) {
        self.disabled = !t;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn set_value<T: Into<String>>(&mut self, val: T) {
        self.value = val
            .into()
            .graphemes(false)
            .into_iter()
            .map(|s| s.to_owned())
            .collect();
    }

    pub fn get_value<'a>(&'a self) -> String {
        String::from_iter(self.value.iter().map(|s| s.as_str()))
    }
}

impl StatefulWidget for Input {
    type State = InputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Percentage(100)].as_ref())
            .split(area.clone());
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([self.horiz_constraint].as_ref())
            .split(rows[0]);

        let style = if state.disabled {
            self.disable_style
        } else if state.focused {
            self.focus_style
        } else {
            self.style
        };
        let par = if area.width < 10 {
            let error_style = Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::SLOW_BLINK);
            Paragraph::new(Text::raw("TOO SMALL")).style(error_style)
        } else {
            if state.value.len() == 0 {
                if let Some(placeholder) = self.placeholder.clone() {
                    Paragraph::new(Text::raw(placeholder))
                        .style(style.patch(self.placeholder_style.clone()))
                        .wrap(Wrap { trim: true })
                } else {
                    Paragraph::new(Text::raw(state.get_value())).style(style)
                }
            } else {
                let width = area.width as usize;
                let text_col = state.cursor / width;
                let text_start = (text_col * width).saturating_sub(10);
                let cursor_pos = state.cursor - text_start;
                let text_end = Ord::min(text_start + (width as usize), state.value.len());
                let content: Vec<_> = state
                    .value
                    .iter()
                    .skip(text_start)
                    .take(text_end.saturating_sub(text_start))
                    .map(|s| s.as_str())
                    .collect();
                if state.focused {
                    if state.value.len() <= state.cursor {
                        Paragraph::new(Spans::from(vec![
                            Span::raw(String::from_iter(content)),
                            Span::styled(
                                tui::symbols::block::FULL,
                                Style::default().bg(Color::Red),
                            ),
                        ]))
                        .style(style)
                    } else {
                        Paragraph::new(Spans::from(vec![
                            Span::raw(String::from_iter(content[..(cursor_pos)].to_owned())),
                            Span::styled(
                                content[cursor_pos],
                                Style::default().fg(Color::Black).bg(Color::White),
                            ),
                            Span::raw(String::from_iter(content[(cursor_pos + 1)..].to_owned())),
                        ]))
                        .style(style)
                    }
                } else {
                    Paragraph::new(Text::raw(String::from_iter(content))).style(style)
                }
            }
        };
        par.render(chunks[0], buf);
    }
}
