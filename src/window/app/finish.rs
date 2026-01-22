use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::style::palette::tailwind;
use ratatui::widgets::{Gauge, Paragraph, Widget};
use url::Url;

use crate::app::App;
use crate::window::WidgetType;
use crate::window::common::{self, Fill, VerticalList, VerticalListItem};

#[derive(Debug, Clone, Copy)]
pub enum FinishState {
    Success,
    Failure,
}

pub struct FinishedTask {
    state: FinishState,
    filepath: PathBuf,
    url: Option<Url>,
    content_length: Option<u64>,
    downloaded: u64,
}

impl FinishedTask {
    // ------------------- CONSTANT -----------------------

    const BAR_STYLE_WITH_TOTAL: Style =
        Style::new().fg(tailwind::BLUE.c400).bg(tailwind::GRAY.c500);
    const BAR_STYLE_NO_TOTAL: Style = Style::new()
        .fg(tailwind::YELLOW.c600)
        .bg(tailwind::GRAY.c500);
    const BAR_TEXT_STYLE: Style = Style::new().fg(Color::White);

    const HIGHTLIGHT_COLOR: Color = Color::LightBlue;

    pub const RENDER_HEIGHT: u16 = 3;

    // -------------------- CONSTRUCT ----------------------

    pub fn new(
        state: FinishState,
        filepath: PathBuf,
        url: Option<Url>,
        content_length: Option<u64>,
        downloaded: u64,
    ) -> Self {
        FinishedTask {
            state,
            filepath,
            url,
            content_length,
            downloaded,
        }
    }
}

/// 对于完成的任务，渲染的内容也大致与正在进行的任务类似，见[`TaskState`]：
/// <filename>
/// <process bar> <percentage>%
///       <downloaded> / <size>
///
/// [`TaskState`]: crate::app::task::TaskState
impl StatefulWidget for &FinishedTask {
    type State = bool;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let text_style = if *state {
            Style::new()
                .bg(FinishedTask::HIGHTLIGHT_COLOR)
                .fg(Color::Black)
        } else {
            Style::new().fg(Color::White)
        };

        // 我们将区域垂直分为三部分：文件名、进度条、其他信息，每个信息占据一行。
        // 理论上在调用render时，area的高度正好为3，但为了保险起见，我们在此给出限制。
        let [area, _] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

        if *state {
            Fill::new(Style::new().bg(FinishedTask::HIGHTLIGHT_COLOR)).render(area, buf);
        }

        let [text, bar, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(area);
        let bar = Layout::horizontal([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(bar)[1];

        // 文件名
        Paragraph::new(self.filepath.to_string_lossy())
            .style(text_style)
            .left_aligned()
            .render(text, buf);

        // 进度条和其他信息
        match self.content_length {
            Some(total) => {
                let percentage = (if total == 0 {
                    100.0
                } else {
                    self.downloaded as f64 / total as f64 * 100.0
                } as u64)
                    .clamp(0, 100) as u16;

                Gauge::default()
                    .label(
                        Span::from(format!("{}%", percentage)).style(FinishedTask::BAR_TEXT_STYLE),
                    )
                    .gauge_style(FinishedTask::BAR_STYLE_WITH_TOTAL)
                    .style(FinishedTask::BAR_TEXT_STYLE)
                    .percent(percentage)
                    .use_unicode(true)
                    .render(bar, buf);

                Paragraph::new(format!(
                    "{} / {}",
                    common::get_human_readable_size(self.downloaded),
                    common::get_human_readable_size(total)
                ))
                .style(text_style)
                .right_aligned()
                .render(footer, buf);
            }
            None => {
                Gauge::default()
                    .label(
                        Span::from(common::get_human_readable_size(self.downloaded))
                            .style(FinishedTask::BAR_TEXT_STYLE),
                    )
                    .gauge_style(FinishedTask::BAR_STYLE_NO_TOTAL)
                    .style(FinishedTask::BAR_TEXT_STYLE)
                    .ratio(1.0)
                    .use_unicode(true)
                    .render(bar, buf);

                Paragraph::new(format!(
                    "{} / {}",
                    common::get_human_readable_size(self.downloaded),
                    "Unknown"
                ))
                .style(text_style)
                .right_aligned()
                .render(footer, buf);
            }
        }
    }
}

/// 对于完成的任务列表，渲染的内容也大致与正在进行的任务列表类似，见[`DownloadList`]：
/// <filename>
/// <process bar> <percentage>%
///       <downloaded> / <size>
/// ---------------------------
/// [`DownloadList`]: crate::window::app::download::DownloadList
pub struct FinishList {
    list: Vec<FinishedTask>,
    selected: Option<usize>,
    scroll: usize,
}

impl Default for FinishList {
    fn default() -> Self {
        FinishList::new()
    }
}

impl FinishList {
    // ------------------- CONSTANT -----------------------

    pub const NOT_ENOUGH_SPACE_BG: Style = Style::new().bg(Color::DarkGray);
    pub const RENDER_ITEM_HEIGHT: u16 = FinishedTask::RENDER_HEIGHT;

    // -------------------- CONSTRUCT ----------------------

    pub fn new() -> Self {
        FinishList {
            list: Vec::new(),
            selected: None,
            scroll: 0,
        }
    }

    // ------------------ MEMBER_ACCESS --------------------

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    // -------------------- MODIFIER -----------------------

    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    pub fn scroll_to(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    // --------------------- FUNCTION ----------------------

    pub fn select_next(&mut self) {
        if self.list.is_empty() {
            self.selected = None;
            return;
        }

        match self.selected {
            Some(i) => {
                self.selected = Some((i + 1) % self.list.len());
            }
            None => {
                self.selected = Some(0);
            }
        }
    }

    pub fn select_previous(&mut self) {
        if self.list.is_empty() {
            self.selected = None;
            return;
        }

        let len = self.list.len();

        match self.selected {
            Some(i) => {
                if i == 0 || i >= len {
                    self.selected = Some(len - 1);
                } else {
                    self.selected = Some(i - 1);
                }
            }
            None => {
                self.selected = Some(0);
            }
        }
    }

    pub fn push_task(&mut self, task: FinishedTask) {
        self.list.push(task);
    }

    fn fit_to_screen(&mut self, area_height: u16) {
        let total_height =
            (self.list.len() * (Self::RENDER_ITEM_HEIGHT as usize + 1)).saturating_sub(1);
        self.scroll_to(
            self.scroll()
                .min(total_height.saturating_sub(area_height as usize)),
        );
    }

    // ------------------- HANDLE_MESSAGE ----------------------

    pub fn respond_to_message(
        app: &mut App,
        message: FinishListMessage,
    ) -> Option<FinishListMessage> {
        let (_, widgets, this_widget) = app.destruct_data();
        this_widget.respond_to_message_inner(message, widgets)
    }

    fn respond_to_message_inner(
        &mut self,
        message: FinishListMessage,
        _widgets: &mut Vec<WidgetType>,
    ) -> Option<FinishListMessage> {
        match message {
            FinishListMessage::GoUp => {
                self.select_previous();
                None
            }
            FinishListMessage::GoDown => {
                self.select_next();
                None
            }
        }
    }

    fn get_key_message(&mut self, key: KeyEvent) -> Option<FinishListMessage> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(FinishListMessage::GoUp),
            KeyCode::Down | KeyCode::Char('j') => Some(FinishListMessage::GoDown),
            _ => None,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, widgets: &mut Vec<WidgetType>) {
        let mut opt_message = self.get_key_message(key);
        while let Some(message) = opt_message {
            opt_message = self.respond_to_message_inner(message, widgets);
        }
    }

    // ------------------- HANDLE_ASYNC ----------------------

    pub fn handle_async(&mut self) {
        if self.selected.is_none() && !self.list.is_empty() {
            self.selected = Some(0);
        }
    }
}

impl StatefulWidget for &mut FinishList {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let empty_text_style = if *state {
            Style::default().bg(Color::Gray).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };

        // 没有已经完成的任务时，显示EMPTY
        if self.list.is_empty() {
            let text = "NO TASKS";
            let text_area = common::centered_text(text, area, 0, 0);
            Paragraph::new(text)
                .style(empty_text_style)
                .centered()
                .render(text_area, buf);
            return;
        }

        self.fit_to_screen(area.height);
        match self.selected() {
            None => {}
            Some(idx) => {
                let focused_distance = idx * (FinishList::RENDER_ITEM_HEIGHT + 1) as usize;
                if focused_distance < self.scroll() {
                    self.scroll_to(focused_distance);
                }
                if focused_distance + FinishList::RENDER_ITEM_HEIGHT as usize
                    > self.scroll() + area.height as usize
                {
                    self.scroll_to(
                        focused_distance + FinishList::RENDER_ITEM_HEIGHT as usize
                            - area.height as usize,
                    );
                }
            }
        }

        let items: Vec<_> = self
            .list
            .iter()
            .map(|item| VerticalListItem::new(FinishList::RENDER_ITEM_HEIGHT, item))
            .collect();
        VerticalList::new(items)
            .with_selected(self.selected())
            .with_scroll(self.scroll())
            .render(area, buf);
    }
}

pub enum FinishListMessage {
    GoUp,
    GoDown,
}
