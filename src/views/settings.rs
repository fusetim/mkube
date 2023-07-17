use tui::widgets::{StatefulWidget, Widget, Block, Tabs, Borders, BorderType, List, ListItem, ListState};
use tui::{
    backend::{Backend},
    layout::{Rect, Constraint, Direction, Layout},
    Frame,
    symbols::DOT,
    text::{Span, Spans, Text},
    style::{Style, Color, Modifier},
    buffer::Buffer,
};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

use crate::multifs::MultiFs;
use crate::views::widgets::input::{Input, InputState};
use crate::views::widgets::labelled_input::{LabelledInput, LabelledInputState};

#[derive(Clone, Debug)]
pub struct SettingsPage {
    pub menu: SettingsMenu,
}

#[derive(Clone, Debug)]
pub enum SettingsState {
    Menu(SettingsMenuState),
    Edit(SettingsEditState),
}

impl Default for SettingsState {
    fn default() -> SettingsState {
        SettingsState::Menu(SettingsMenuState::default())
    }
}

impl SettingsState {
    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        match self {
            SettingsState::Menu(ref mut state) => {
                if state.press_key(kev) {
                    return true;
                }
                if kev.code == KeyCode::Enter {
                    if let Some(select) = state.selected() {
                        let mut estate = SettingsEditState::default();
                        estate.name.set_focus(true);
                        *self = SettingsState::Edit(estate);
                        return true
                    }
                    return true;
                }
            },
            SettingsState::Edit(ref mut state) => {
                return state.press_key(kev)
            },
        }
        false
    }
}

impl SettingsPage {
    pub fn new() -> Self {
        SettingsPage { menu: SettingsMenu {} }
    }
}

impl StatefulWidget for SettingsPage {
    type State = SettingsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match state {
            SettingsState::Menu(ref mut mstate) => {
                StatefulWidget::render(self.menu, area, buf, mstate);
            }
            SettingsState::Edit(ref mut estate) => {
                let input = Input { placeholder: Some("Placeholder for a name".into()), ..Default::default() };
                let labelled_input = LabelledInput::new("Name: ", input);
                StatefulWidget::render(SettingsEdit { name: labelled_input}, area, buf, estate);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SettingsMenu {}

#[derive(Clone, Debug, Default)]
pub struct SettingsMenuState {
    pub items: Vec<MenuItem>,
    pub list_state: ListState,
}

impl StatefulWidget for SettingsMenu {
    type State = SettingsMenuState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(100),
            ].as_ref()
        )
        .split(area.clone());

        let mut items : Vec<_> = state.items.iter().map(|i| i.clone().into()).collect();
        let list = List::new(items)
            .block(Block::default().title("List").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol("> ");

        StatefulWidget::render(list, chunks[0], buf, &mut state.list_state);
    }
}

impl SettingsMenuState {
    pub fn new(menu: &SettingsMenu, items: Vec<MenuItem>) -> Self {
        Self {
            list_state: ListState::default(),
            items
        }
    }

    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        let opt_len = self.items.len();
        if kev.code == KeyCode::Up {
            let select = Some(self.list_state.selected().map(|i| (i+opt_len-1) % opt_len).unwrap_or(0));
            self.list_state.select(select);
            true 
        } else if kev.code == KeyCode::Down {
            let select = Some(self.list_state.selected().map(|i| (i+1) % opt_len).unwrap_or(0));
            self.list_state.select(select);
            true
        } else {
            false
        }
    }

    pub fn selected(&self) -> Option<MenuItem> {
        self.list_state.selected().and_then(|i| self.items.get(i)).cloned()
    }
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub selectable: bool,
    pub text: String,
    pub style: Style,
    pub item_type: MenuItemType,
}

impl<'a> From<MenuItem> for ListItem<'a> {
    fn from(v: MenuItem) -> ListItem<'a> {
        ListItem::new(v.text.clone())
    }
}

impl MenuItem {
    pub fn new<T>(text: T) -> MenuItem
    where T: Into<String>
    {
        MenuItem {
            selectable: true,
            text: text.into(),
            item_type: MenuItemType::None,
            style: Style::default(),
        }
    }

    pub fn set_type(mut self, type_: MenuItemType) -> MenuItem {
        self.item_type = type_;
        self
    }

    pub fn selectable(mut self, selectable: bool) -> MenuItem{
        self.selectable = selectable;
        self
    }

    pub fn style(mut self, style: Style) -> MenuItem{
        self.style = style;
        self
    }
}

pub fn standard_actions() -> Vec<MenuItem> {
    let mut items = Vec::new();
    items.push(MenuItem::new("Add a local library").set_type(MenuItemType::NewLocalLibrary));
    items.push(MenuItem::new("Add a FTP library").set_type(MenuItemType::NewFtpLibrary));
    items.push(MenuItem::new("Add a SMB library").set_type(MenuItemType::NewSmbLibrary));
    items.push(MenuItem::new(" - Existing libraries -").set_type(MenuItemType::None).selectable(false));
    items
}
 
#[derive(Debug, Clone, Default)]
pub enum MenuItemType {
    #[default]
    None,
    NewLocalLibrary,
    NewSmbLibrary,
    NewFtpLibrary,
    ExistingLibrary(String),
}


#[derive(Clone, Debug)]
pub struct SettingsEdit {
    pub name: LabelledInput,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsEditState {
    pub focused: usize,
    pub name: LabelledInputState,
}

impl Default for SettingsEdit {
    fn default() -> SettingsEdit {
        SettingsEdit {
            name: LabelledInput::new("Name", Input::default())
        } 
    }
}

impl StatefulWidget for SettingsEdit {
    type State = SettingsEditState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(self.name, area, buf, &mut state.name);
    }
}

impl SettingsEditState {
    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        //self.name.input_without_shortcuts(kev)
        self.name.input(kev)
    }
}
