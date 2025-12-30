use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::{
    text::Text,
    widgets::{HighlightSpacing, List, ListItem, ListState, Widget},
};

/// PageList包含如下几个页面：
///
/// 0 -- 当前正在下载的任务列表
/// 1 -- 已经完成的任务列表
pub struct PageList {
    selected: ListState,
    items: [ListItem<'static>; PageList::PAGE_COUNT],
    enter: bool,
}

impl Default for PageList {
    fn default() -> Self {
        Self::new()
    }
}

impl PageList {
    // ------------------- CONSTANT -----------------------

    // FIXME: 目前暂时使用Ratatui自带的List，为此，需要使用换行符来保证一个项能够多行显示
    pub const PAGE_STR: [&'static str; PageList::PAGE_COUNT] =
        ["\nDownloading\n\n", "\nFinished\n\n"];

    pub const PAGE_COUNT: usize = 2;

    const SELECTED_STYLE: Style = Style::new().bg(Color::LightBlue).fg(Color::Black);

    // ----------------------- CONSTRUCT ------------------------

    pub fn new() -> Self {
        let mut selected = ListState::default();
        selected.select(Some(0));
        PageList {
            selected,
            items: [
                ListItem::new(Text::from(Self::PAGE_STR[0]).centered()),
                ListItem::new(Text::from(Self::PAGE_STR[1]).centered()),
            ],
            enter: false,
        }
    }

    // -------------------- MEMBER_ACCESS ---------------------

    pub fn selected(&self) -> Option<usize> {
        self.selected.selected()
    }

    pub fn entered(&self) -> bool {
        self.enter
    }

    // -------------------- MODIFIER -----------------------

    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected.select(index);
    }

    pub fn enter(&mut self) {
        self.enter = true;
    }

    pub fn exit(&mut self) {
        self.enter = false;
    }

    // -------------------- FUNCTION -----------------------

    pub fn select_next(&mut self) {
        match self.selected() {
            Some(i) => {
                self.selected.select(Some((i + 1) % Self::PAGE_COUNT));
            }
            None => {
                self.selected.select(Some(0));
            }
        }
    }

    pub fn select_previous(&mut self) {
        match self.selected() {
            Some(i) => {
                if i == 0 {
                    self.selected.select(Some(Self::PAGE_COUNT - 1));
                } else {
                    self.selected.select(Some(i - 1));
                }
            }
            None => {
                self.selected.select(Some(0));
            }
        }
    }

    // -------------------- HANDLE_MESSAGE -----------------------

    pub fn respond_to_message(&mut self, message: PageListMessage) -> Option<PageListMessage> {
        match message {
            PageListMessage::Exit => {
                self.exit();
                None
            }
            PageListMessage::Enter => {
                self.enter();
                None
            }
            PageListMessage::GoUp => {
                self.select_previous();
                None
            }
            PageListMessage::GoDown => {
                self.select_next();
                None
            }
            PageListMessage::Distribute(_) => None,
        }
    }

    fn get_key_message(&self, key: KeyEvent) -> Option<PageListMessage> {
        if self.entered() {
            match key.code {
                KeyCode::Left => Some(PageListMessage::Exit),
                _ => Some(PageListMessage::Distribute(key)),
            }
        } else {
            match key.code {
                KeyCode::Enter | KeyCode::Right => Some(PageListMessage::Enter),
                KeyCode::Left => Some(PageListMessage::Exit),
                KeyCode::Up | KeyCode::Char('k') => Some(PageListMessage::GoUp),
                KeyCode::Down | KeyCode::Char('j') => Some(PageListMessage::GoDown),
                _ => None,
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<KeyEvent> {
        let mut opt_message = self.get_key_message(key);
        if let Some(PageListMessage::Distribute(key)) = opt_message {
            return Some(key);
        }
        while let Some(message) = opt_message {
            opt_message = self.respond_to_message(message);
        }
        None
    }
}

impl Widget for &mut PageList {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let list = List::new(self.items.clone())
            .highlight_style(PageList::SELECTED_STYLE)
            .highlight_spacing(HighlightSpacing::Always);

        <List as StatefulWidget>::render(list, area, buf, &mut self.selected);
    }
}

pub enum PageListMessage {
    Enter,
    Exit,
    Distribute(KeyEvent),
    GoUp,
    GoDown,
}
