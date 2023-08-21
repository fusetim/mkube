use crate::library::{Library, LibraryFlavor, LibraryType};
use crate::util::{OwnedSpan, OwnedSpans};
use crate::views::widgets::{
    Button, ButtonState, Checkbox, Input, LabelledCheckbox, LabelledCheckboxState, LabelledInput,
    LabelledInputState,
};
use crate::{AppEvent, AppMessage, AppState, MultiFs, MESSAGE_SENDER};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Widget};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
};
use url::Url;

#[derive(Clone, Debug)]
pub struct SettingsPage {
    pub menu: SettingsMenu,
}

#[derive(Clone, Debug)]
pub enum SettingsState {
    Menu(SettingsMenuState),
    Edit(SettingsEditState),
}

#[derive(Clone, Debug)]
pub enum SettingsEvent {
    OpenMenu(Vec<Library>),
    EditNew(LibraryType),
    EditExisting(Library),
    ConnTestResult((bool, bool)),
}

#[derive(Clone, Debug)]
pub enum SettingsMessage {
    OpenMenu,
    EditExisting(Library),
    SaveLibrary(Library),
    TestLibrary(Library),
}

impl Default for SettingsState {
    fn default() -> SettingsState {
        SettingsState::Menu(SettingsMenuState::new(standard_actions()))
    }
}

impl SettingsState {
    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        match self {
            SettingsState::Menu(ref mut state) => {
                return state.press_key(kev);
            }
            SettingsState::Edit(ref mut state) => {
                return state.press_key(kev);
            }
        }
        false
    }

    pub fn input(&mut self, evt: AppEvent) -> bool {
        match evt {
            AppEvent::KeyEvent(kev) => self.press_key(kev),
            AppEvent::SettingsEvent(SettingsEvent::OpenMenu(libraries)) => {
                let mut items = standard_actions();
                for l in libraries {
                    items.push(MenuItem::from(l));
                }
                *self = SettingsState::Menu(SettingsMenuState::new(items));
                true
            }
            AppEvent::SettingsEvent(SettingsEvent::EditNew(fs_type)) => {
                let mut state = SettingsEditState::default();
                if fs_type != LibraryType::Local {
                    state.host = Some(LabelledInputState::default());
                    state.username = Some(LabelledInputState::default());
                    state.password = Some(LabelledInputState::default());
                }
                state.fs_type = fs_type;
                *self = SettingsState::Edit(state);
                true
            }
            AppEvent::SettingsEvent(SettingsEvent::EditExisting(lib)) => {
                let mut state = SettingsEditState::default();
                if lib.fs_type != LibraryType::Local {
                    state.host = Some(LabelledInputState::default());
                    state.username = Some(LabelledInputState::default());
                    state.password = Some(LabelledInputState::default());
                    if let Some(host) = lib.host {
                        state.host.as_mut().unwrap().set_value(&host);
                    }
                    if let Some(username) = lib.username {
                        state.username.as_mut().unwrap().set_value(&username);
                    }
                    if let Some(password) = lib.password {
                        state.password.as_mut().unwrap().set_value(&password);
                    }
                }
                state.name.set_value(lib.name);
                state.path.set_value(lib.path.display().to_string());
                if lib.flavor == LibraryFlavor::Movie {
                    state.movie.check(true);
                } else {
                    state.tv_show.check(true);
                }
                state.fs_type = lib.fs_type;
                *self = SettingsState::Edit(state);
                true
            }
            AppEvent::SettingsEvent(SettingsEvent::ConnTestResult(tests)) => {
                if let SettingsState::Edit(ref mut state) = self {
                    state.test_result = Some(tests);
                    state.test.click(false);
                    true
                } else {
                    false
                }
            }
            _ => match self {
                SettingsState::Menu(ref mut state) => state.input(evt),
                SettingsState::Edit(ref mut state) => state.input(evt),
            },
        }
    }
}

impl SettingsPage {
    pub fn new() -> Self {
        SettingsPage {
            menu: SettingsMenu {},
        }
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
                StatefulWidget::render(SettingsEdit::default(), area, buf, estate);
            }
        }
    }
}

impl From<SettingsMessage> for AppMessage {
    fn from(value: SettingsMessage) -> AppMessage {
        match value {
            SettingsMessage::OpenMenu => {
                AppMessage::Closure(Box::new(|app_state: &mut AppState| {
                    Some(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(
                        app_state.libraries.iter().flatten().cloned().collect(),
                    )))
                }))
            }
            SettingsMessage::EditExisting(_) | SettingsMessage::SaveLibrary(_) => {
                AppMessage::SettingsMessage(value)
            }
            SettingsMessage::TestLibrary(lib) => AppMessage::Future(Box::new(|_| {
                Box::pin(async move {
                    let rst = match MultiFs::try_from(&lib) {
                        Ok(mut conn) => {
                            let _ = conn.as_mut_rfs().connect();
                            (
                                conn.as_mut_rfs().is_connected(),
                                conn.as_mut_rfs()
                                    .exists(&lib.path.as_path())
                                    .unwrap_or(false),
                            )
                        }
                        Err(err) => {
                            log::warn!(
                                "Connection to library `{}` failed due to:\n{:?}",
                                Url::try_from(&lib)
                                    .as_ref()
                                    .map(Url::as_ref)
                                    .unwrap_or("N/A"),
                                err
                            );
                            (false, false)
                        }
                    };
                    Some(AppEvent::SettingsEvent(SettingsEvent::ConnTestResult(rst)))
                })
            })),
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
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(area.clone());

        let items: Vec<_> = state.items.iter().map(|i| i.clone().into()).collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Manage your libraries ")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol("> ");

        StatefulWidget::render(list, chunks[0], buf, &mut state.list_state);
    }
}

impl SettingsMenuState {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            list_state: ListState::default(),
            items,
        }
    }

    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        let opt_len = self.items.len();
        if kev.code == KeyCode::Up {
            let select = Some(
                self.list_state
                    .selected()
                    .map(|i| (i + opt_len - 1) % opt_len)
                    .unwrap_or(0),
            );
            self.list_state.select(select);
            true
        } else if kev.code == KeyCode::Down {
            let select = Some(
                self.list_state
                    .selected()
                    .map(|i| (i + 1) % opt_len)
                    .unwrap_or(0),
            );
            self.list_state.select(select);
            true
        } else if kev.code == KeyCode::Enter {
            if let Some(s) = self.list_state.selected() {
                if let Some(item) = self.items.get(s) {
                    if item.selectable {
                        let sender = MESSAGE_SENDER.get().unwrap();
                        match &item.item_type {
                            MenuItemType::None => {
                                return false;
                            }
                            MenuItemType::NewLocalLibrary => {
                                sender
                                    .send(AppMessage::TriggerEvent(AppEvent::SettingsEvent(
                                        SettingsEvent::EditNew(LibraryType::Local),
                                    )))
                                    .unwrap();
                            }
                            #[cfg(feature = "ftp")]
                            MenuItemType::NewFtpLibrary => {
                                sender
                                    .send(AppMessage::TriggerEvent(AppEvent::SettingsEvent(
                                        SettingsEvent::EditNew(LibraryType::Ftp),
                                    )))
                                    .unwrap();
                            }
                            #[cfg(feature = "smb")]
                            MenuItemType::NewSmbLibrary => {
                                sender
                                    .send(AppMessage::TriggerEvent(AppEvent::SettingsEvent(
                                        SettingsEvent::EditNew(LibraryType::Smb),
                                    )))
                                    .unwrap();
                            }
                            MenuItemType::ExistingLibrary(l) => {
                                sender
                                    .send(SettingsMessage::EditExisting(l.clone()).into())
                                    .unwrap();
                            }
                        }
                        return true;
                    }
                }
            }
            false
        } else {
            false
        }
    }

    pub fn input(&mut self, evt: AppEvent) -> bool {
        match evt {
            AppEvent::KeyEvent(kev) => self.press_key(kev),
            _ => false,
        }
    }

    pub fn selected(&self) -> Option<MenuItem> {
        self.list_state
            .selected()
            .and_then(|i| self.items.get(i))
            .cloned()
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

impl From<Library> for MenuItem {
    fn from(l: Library) -> MenuItem {
        let url = url::Url::try_from(&l)
            .map(|mut u| {
                let _ = u.set_password(None);
                return u.to_string();
            })
            .unwrap_or(l.to_string());
        MenuItem::new(format!("{} ({})", &l.name, url))
            .selectable(true)
            .set_type(MenuItemType::ExistingLibrary(l))
    }
}

impl MenuItem {
    pub fn new<T>(text: T) -> MenuItem
    where
        T: Into<String>,
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

    pub fn selectable(mut self, selectable: bool) -> MenuItem {
        self.selectable = selectable;
        self
    }

    pub fn style(mut self, style: Style) -> MenuItem {
        self.style = style;
        self
    }
}

pub fn standard_actions() -> Vec<MenuItem> {
    let mut items = Vec::new();
    items.push(MenuItem::new("Add a local library").set_type(MenuItemType::NewLocalLibrary));
    #[cfg(feature = "ftp")]
    items.push(MenuItem::new("Add a FTP library").set_type(MenuItemType::NewFtpLibrary));
    #[cfg(feature = "smb")]
    items.push(MenuItem::new("Add a SMB library").set_type(MenuItemType::NewSmbLibrary));
    items.push(
        MenuItem::new(" - Existing libraries -")
            .set_type(MenuItemType::None)
            .selectable(false),
    );
    items
}

#[derive(Debug, Clone, Default)]
pub enum MenuItemType {
    #[default]
    None,
    NewLocalLibrary,
    #[cfg(feature = "smb")]
    NewSmbLibrary,
    #[cfg(feature = "ftp")]
    NewFtpLibrary,
    ExistingLibrary(Library),
}

#[derive(Clone, Debug)]
pub struct SettingsEdit {
    pub name: LabelledInput,
    pub host: LabelledInput,
    pub username: LabelledInput,
    pub password: LabelledInput,
    pub path: LabelledInput,
    pub movie: LabelledCheckbox,
    pub tv_show: LabelledCheckbox,
    pub test: Button,
    pub save: Button,
    pub cancel: Button,
}

#[derive(Clone, Debug)]
pub struct SettingsEditState {
    pub focused: usize,
    pub fs_type: LibraryType,
    pub name: LabelledInputState,
    pub host: Option<LabelledInputState>,
    pub username: Option<LabelledInputState>,
    pub password: Option<LabelledInputState>,
    pub path: LabelledInputState,
    pub movie: LabelledCheckboxState,
    pub tv_show: LabelledCheckboxState,
    pub test: ButtonState,
    pub save: ButtonState,
    pub cancel: ButtonState,
    pub test_result: Option<(bool, bool)>,
}

impl Default for SettingsEdit {
    fn default() -> SettingsEdit {
        SettingsEdit {
            name: LabelledInput::new("Name: ", Input::default()),
            host: LabelledInput::new("Host: ", Input::default()),
            username: LabelledInput::new("Username: ", Input::default()),
            password: LabelledInput::new("Password: ", Input::default()),
            path: LabelledInput::new("Path: ", Input::default()),
            movie: LabelledCheckbox::new("Movie", Checkbox::default()),
            tv_show: LabelledCheckbox::new("TV Show", Checkbox::default()),
            test: Button::default().with_text("Test"),
            save: Button::default().with_text("Save"),
            cancel: Button::default().with_text("Delete"),
        }
    }
}

impl Default for SettingsEditState {
    fn default() -> SettingsEditState {
        SettingsEditState {
            focused: 0,
            fs_type: LibraryType::Local,
            name: LabelledInputState::default(),
            host: None,
            username: None,
            password: None,
            path: LabelledInputState::default(),
            movie: LabelledCheckboxState::default(),
            tv_show: LabelledCheckboxState::default(),
            test: ButtonState::default(),
            save: ButtonState::default(),
            cancel: ButtonState::default(),
            test_result: None,
        }
    }
}

impl StatefulWidget for SettingsEdit {
    type State = SettingsEditState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Min(1),
                    Constraint::Percentage(100),
                ]
                .as_ref(),
            )
            .split(area.clone());
        let type_selector_cells = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Min(14),
                    Constraint::Min(10),
                    Constraint::Min(2),
                    Constraint::Min(12),
                    Constraint::Percentage(100),
                ]
                .as_ref(),
            )
            .split(rows[5]);
        let buttons_cells = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Min(6),
                    Constraint::Min(2),
                    Constraint::Min(6),
                    Constraint::Min(2),
                    Constraint::Min(8),
                    Constraint::Min(2),
                    Constraint::Percentage(100),
                ]
                .as_ref(),
            )
            .split(rows[7]);

        StatefulWidget::render(self.name, rows[0], buf, &mut state.name);
        if let Some(ref mut istate) = state.host {
            StatefulWidget::render(self.host, rows[1], buf, istate);
        }
        if let Some(ref mut istate) = state.username {
            StatefulWidget::render(self.username, rows[2], buf, istate);
        }
        if let Some(ref mut istate) = state.password {
            StatefulWidget::render(self.password, rows[3], buf, istate);
        }
        StatefulWidget::render(self.path, rows[4], buf, &mut state.path);

        let type_label = Paragraph::new(Span::raw("Library type: "));
        Widget::render(type_label, type_selector_cells[0], buf);
        StatefulWidget::render(self.movie, type_selector_cells[1], buf, &mut state.movie);
        StatefulWidget::render(
            self.tv_show,
            type_selector_cells[3],
            buf,
            &mut state.tv_show,
        );
        StatefulWidget::render(self.test, buttons_cells[0], buf, &mut state.test);
        StatefulWidget::render(self.save, buttons_cells[2], buf, &mut state.save);
        StatefulWidget::render(self.cancel, buttons_cells[4], buf, &mut state.cancel);
        let conn_status = if let Some((conn, path)) = state.test_result {
            let mut spans = Vec::new();
            spans.push(OwnedSpan::raw("Connection: "));
            spans.push(if conn {
                OwnedSpan::styled("OK", Style::default().fg(Color::Green))
            } else {
                OwnedSpan::styled("Error", Style::default().fg(Color::LightRed))
            });
            spans.push(OwnedSpan::raw(" / Path: "));
            spans.push(if path {
                OwnedSpan::styled("OK", Style::default().fg(Color::Green))
            } else {
                OwnedSpan::styled("Error", Style::default().fg(Color::LightRed))
            });
            Paragraph::new(OwnedSpans::from(spans))
        } else {
            Paragraph::new("Connection: Untested / Path: Untested")
        };
        Widget::render(conn_status, buttons_cells[6], buf);
    }
}

const SETTINGS_EDIT_SELECTABLES: usize = 10;

impl SettingsEditState {
    pub fn press_key(&mut self, kev: KeyEvent) -> bool {
        if kev.code == KeyCode::Tab {
            self.focus_child(self.focused, false);
            self.focused = (self.focused + 1) % SETTINGS_EDIT_SELECTABLES;
            while !self.focus_child(self.focused, true) {
                self.focused = (self.focused + 1) % SETTINGS_EDIT_SELECTABLES;
            }
            true
        } else if kev.code == KeyCode::BackTab {
            self.focus_child(self.focused, false);
            self.focused =
                (self.focused + SETTINGS_EDIT_SELECTABLES - 1) % SETTINGS_EDIT_SELECTABLES;
            while !self.focus_child(self.focused, true) {
                self.focused =
                    (self.focused + SETTINGS_EDIT_SELECTABLES - 1) % SETTINGS_EDIT_SELECTABLES;
            }
            true
        } else {
            if self.input_child(self.focused, kev) {
                if self.focused == 5 {
                    self.tv_show.check(!self.movie.is_checked());
                } else if self.focused == 6 {
                    self.movie.check(!self.tv_show.is_checked());
                } else if self.cancel.is_clicked() {
                    let sender = MESSAGE_SENDER.get().unwrap();
                    sender
                        .send(crate::AppMessage::Future(Box::new(
                            |appstate: &mut AppState| {
                                let libs = appstate.libraries.iter().flatten().cloned().collect();
                                Box::pin(async move {
                                    Some(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(libs)))
                                })
                            },
                        )))
                        .unwrap();
                } else if self.save.is_clicked() {
                    let sender = MESSAGE_SENDER.get().unwrap();
                    let library = Library {
                        name: self.name.get_value().to_owned(),
                        path: PathBuf::from(self.path.get_value()),
                        host: self.host.as_ref().map(|c| c.get_value().to_owned()),
                        username: self.username.as_ref().map(|c| c.get_value().to_owned()),
                        password: self.password.as_ref().map(|c| c.get_value().to_owned()),
                        fs_type: self.fs_type.clone(),
                        flavor: if self.movie.is_checked() {
                            LibraryFlavor::Movie
                        } else {
                            LibraryFlavor::TvShow
                        },
                    };
                    sender
                        .send(SettingsMessage::SaveLibrary(library).into())
                        .unwrap();
                } else if self.test.is_clicked() {
                    let sender = MESSAGE_SENDER.get().unwrap();
                    let library = Library {
                        name: self.name.get_value().to_owned(),
                        path: PathBuf::from(self.path.get_value()),
                        host: self.host.as_ref().map(|c| c.get_value().to_owned()),
                        username: self.username.as_ref().map(|c| c.get_value().to_owned()),
                        password: self.password.as_ref().map(|c| c.get_value().to_owned()),
                        fs_type: self.fs_type.clone(),
                        flavor: if self.movie.is_checked() {
                            LibraryFlavor::Movie
                        } else {
                            LibraryFlavor::TvShow
                        },
                    };
                    sender
                        .send(SettingsMessage::TestLibrary(library).into())
                        .unwrap();
                }
                true
            } else {
                false
            }
        }
    }

    pub fn input(&mut self, evt: AppEvent) -> bool {
        match evt {
            AppEvent::KeyEvent(kev) => self.press_key(kev),
            _ => false,
        }
    }

    fn focus_child(&mut self, index: usize, state: bool) -> bool {
        match index {
            0 => {
                self.name.focus(state);
                true
            }
            1 => self.host.as_mut().map(|u| u.focus(state)).is_some(),
            2 => self.username.as_mut().map(|u| u.focus(state)).is_some(),
            3 => self.password.as_mut().map(|u| u.focus(state)).is_some(),
            4 => {
                self.path.focus(state);
                true
            }
            5 => {
                self.movie.focus(state);
                true
            }
            6 => {
                self.tv_show.focus(state);
                true
            }
            7 => {
                self.test.focus(state);
                true
            }
            8 => {
                self.save.focus(state);
                true
            }
            9 => {
                self.cancel.focus(state);
                true
            }
            _ => true,
        }
    }

    fn input_child(&mut self, index: usize, kev: KeyEvent) -> bool {
        match index {
            0 => self.name.input(kev),
            1 => {
                let r = self.host.as_mut().map(|u| u.input(kev));
                return r.is_some() && r.unwrap();
            }
            2 => {
                let r = self.username.as_mut().map(|u| u.input(kev));
                return r.is_some() && r.unwrap();
            }
            3 => {
                let r = self.password.as_mut().map(|u| u.input(kev));
                return r.is_some() && r.unwrap();
            }
            4 => self.path.input(kev),
            5 => self.movie.input(kev),
            6 => self.tv_show.input(kev),
            7 => self.test.input(kev),
            8 => self.save.input(kev),
            9 => self.cancel.input(kev),
            _ => false,
        }
    }
}
