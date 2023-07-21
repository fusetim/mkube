use tui::{
    style::{Style, Color, Modifier},
    widgets::{Widget, StatefulWidget, Paragraph, Wrap},
    text::{Text, Span, Spans},
    buffer::Buffer,
    layout::{Rect, Constraint, Direction, Layout},
};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

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
            style: Style::default().bg(Color::Gray),
            focus_style: Style::default().bg(Color::LightRed),
            disable_style: Style::default().add_modifier(Modifier::UNDERLINED),
            placeholder: None,
            placeholder_style: Style::default().add_modifier(Modifier::ITALIC),
            horiz_constraint: Constraint::Percentage(100),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    value: String,
    focused: bool,
    disabled: bool,
    cursor: usize,
}

impl InputState {
    pub fn input(&mut self, kev: KeyEvent) -> bool {
        match kev.code {
            KeyCode::Char(c) => {
                let prev = self.value.split_at(self.cursor);
                self.value = format!("{}{}{}", prev.0, c, prev.1);
                self.cursor += 1;
            },
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    if self.cursor == self.value.len() {
                        self.value.pop();
                    } else {
                        let prev = self.value.split_at(self.cursor-1);
                        let follow = prev.1[1..].to_owned();
                        self.value = format!("{}{}", prev.0, follow);
                    }
                    self.cursor -= 1;
                }
            },
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    let prev = self.value.split_at(self.cursor);
                    let follow = prev.1[1..].to_owned();
                    self.value = format!("{}{}", prev.0, follow);
                }
            },
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            },
            KeyCode::Right => {
                self.cursor = Ord::min(self.cursor + 1, self.value.len())
            },
            KeyCode::Up => {
                self.cursor = 0;
            },
            KeyCode::Down | KeyCode::End => {
                self.cursor = self.value.len();
            },
            _ => { return false; },
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
        self.value = val.into();
    }

    pub fn get_value<'a>(&'a self) -> &'a str {
        &self.value
    }
}

impl StatefulWidget for Input {
    type State = InputState;

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
                self.horiz_constraint,
            ].as_ref()
        )
        .split(rows[0]);

        let style = if state.disabled {
            self.disable_style
        } else if state.focused {
            self.focus_style
        } else {
            self.style
        };
        let par = if area.width < 10 {
            let error_style = Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD).add_modifier(Modifier::SLOW_BLINK);
            Paragraph::new(Text::raw("TOO SMALL"))
                .style(error_style)
        } else { 
            if state.value.len() == 0 { 
                if let Some(placeholder) = self.placeholder.clone() {
                    Paragraph::new(Text::raw(placeholder))
                        .style(style.patch(self.placeholder_style.clone()))
                        .wrap(Wrap { trim: true })
                } else {
                    Paragraph::new(Text::raw(&state.value))
                        .style(style)
                }
            } else {
                let width = area.width as usize;
                let text_col = state.cursor / width;
                let text_start = (text_col * width).saturating_sub(10);
                let cursor_pos = state.cursor - text_start;
                let text_end = Ord::min(text_start + (width as usize), state.value.len());
                let content = get_within_char_boundaries(&state.value, text_start..text_end).unwrap();
                if state.focused {
                    if state.value.len() == state.cursor {
                        Paragraph::new(Spans::from(vec![
                            Span::raw(content),
                            Span::styled(tui::symbols::block::FULL, Style::default().bg(Color::Red)),
                        ])).style(style)
                    } else {
                        let parts = state.value.split_at(cursor_pos);
                        let parts_ = parts.1.split_at(1);
                        Paragraph::new(Spans::from(vec![
                            Span::raw(parts.0),
                            Span::styled(parts_.0, Style::default().bg(Color::White)),
                            Span::raw(parts_.1),
                        ])).style(style)
                    }
                } else {
                    Paragraph::new(Text::raw(content))
                        .style(style)
                }
            }
        };
        par.render(chunks[0], buf);
    }
}

#[inline]
fn floor_char_boundary<'a>(text: &'a str, index: usize) -> usize {
    if index >= text.len() {
        text.len()
    } else {
        let lower_bound = index.saturating_sub(3);
        let new_index = (lower_bound..(index+1))
            .rposition(|c| text.is_char_boundary(c));

        // SAFETY: we know that the character boundary will be within four bytes
        unsafe { lower_bound + new_index.unwrap_unchecked() }
    }
}

#[inline]
fn ceil_char_boundary<'a>(text: &'a str, index: usize) -> usize {
    if index > text.len() {
        text.len()
    } else {
        let upper_bound = Ord::min(index + 4, text.len());
        (index..upper_bound)
            .position(|c| text.is_char_boundary(c))
            .map_or(upper_bound, |pos| pos + index)
    }
}

fn get_within_char_boundaries<'a, I: std::ops::RangeBounds<usize>>(text: &'a str, i: I) -> Option<&'a str> {
    use std::ops::Bound;
    let start = match i.start_bound() {
        Bound::Included(u) => Bound::Included(ceil_char_boundary(text, *u)),
        Bound::Excluded(u) => Bound::Included(ceil_char_boundary(text, u.saturating_add(1))),
        Bound::Unbounded => Bound::Unbounded
    };
    let end = match i.end_bound() {
        Bound::Included(u) => Bound::Included(floor_char_boundary(text, *u)),
        Bound::Excluded(u) => Bound::Included(floor_char_boundary(text, u.saturating_sub(1))),
        Bound::Unbounded => Bound::Unbounded
    };
    match (start, end) {
        (Bound::Unbounded,Bound::Unbounded) => text.get(..),
        (Bound::Included(u),Bound::Unbounded) => text.get(u..),
        (Bound::Included(u),Bound::Included(v)) => text.get(u..=v),
        (Bound::Unbounded,Bound::Included(v)) => text.get(..=v),
        _ => unimplemented!(),
    }
}