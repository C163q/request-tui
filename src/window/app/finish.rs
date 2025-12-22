use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::style::palette::tailwind;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Widget};
use url::Url;

use crate::app::App;
use crate::window::common::{self, Fill};
use crate::window::WidgetType;

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
    const BAR_STYLE_WITH_TOTAL: Style =
        Style::new().fg(tailwind::BLUE.c400).bg(tailwind::GRAY.c500);
    const BAR_STYLE_NO_TOTAL: Style = Style::new()
        .fg(tailwind::YELLOW.c600)
        .bg(tailwind::GRAY.c500);
    const BAR_TEXT_STYLE: Style = Style::new().fg(Color::White);

    const HIGHTLIGHT_COLOR: Color = Color::LightBlue;

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
impl StatefulWidget for &mut FinishedTask {
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
}

impl Default for FinishList {
    fn default() -> Self {
        FinishList::new()
    }
}

impl FinishList {
    pub const NOT_ENOUGH_SPACE_BG: Style = Style::new().bg(Color::DarkGray);

    pub fn new() -> Self {
        FinishList {
            list: Vec::new(),
            selected: None,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }

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
                if i == 0 && i >= len {
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

    pub fn respond_to_message(
        app: &mut App,
        message: FinishListMessage,
    ) -> Option<FinishListMessage> {
        let (this_widget, widgets) = app.finish_list_widgets();
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
}

impl StatefulWidget for &mut FinishList {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut list_remain_area = area;
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

        for (i, state) in self.list.iter_mut().enumerate() {
            if i != 0 {
                // 必须保证分隔符有空间渲染
                if list_remain_area.height == 0 {
                    break;
                }

                let [bar, area] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
                    .areas(list_remain_area);
                list_remain_area = area;

                Block::new().borders(Borders::BOTTOM).render(bar, buf);
            }

            // 我们至少需要3行空间来渲染一个完成任务的状态
            if list_remain_area.height < 3 {
                Fill::new(FinishList::NOT_ENOUGH_SPACE_BG).render(list_remain_area, buf);
                break;
            }

            let [state_area, area] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)])
                .areas(list_remain_area);
            let mut is_selected = if let Some(selected) = self.selected
                && selected == i
            {
                true
            } else {
                false
            };
            state.render(state_area, buf, &mut is_selected);

            list_remain_area = area;
        }
    }
}

pub enum FinishListMessage {
    GoUp,
    GoDown,
}
