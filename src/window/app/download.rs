use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Widget;
use ratatui::widgets::{Block, Borders, Paragraph};
use tokio::sync::mpsc;

use crate::app::App;
use crate::app::receive::TaskListener;
use crate::app::send::{self, DownloadRequest};
use crate::app::task::{Task, TaskFinalStage};
use crate::window::WidgetType;
use crate::window::app::FinishList;
use crate::window::common::{self, Fill};

/// 此处我们不使用ratatui的list，因为我们此处需要渲染进度条等复杂组件。
///
/// 组件的每一项是一个下载任务的状态。其每一项大致结构如下：
///
/// <filename>
/// <process bar> <percentage>%
///               <speed> <eta>
/// --------------------------- (分隔线，如果不是最后一项并且有空间渲染时)
pub struct DownloadList {
    list: Vec<TaskListener>,
    selected: Option<usize>,
    sender: send::Sender,
}

impl DownloadList {
    pub const NOT_ENOUGH_SPACE_BG: Style = Style::new().bg(Color::DarkGray);

    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        DownloadList {
            list: Vec::new(),
            selected: None,
            sender: send::Sender::new(sender),
        }
    }

    #[inline]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    #[inline]
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

    pub fn append_task(&mut self, request: DownloadRequest) -> anyhow::Result<()> {
        let listener = self.sender.send_request(request)?;
        self.list.push(listener);
        Ok(())
    }

    #[inline]
    pub fn respond_to_message(
        app: &mut App,
        message: DownloadListMessage,
    ) -> Option<DownloadListMessage> {
        let (this_widget, widgets) = app.download_list_widgets();
        this_widget.respond_to_message_inner(message, widgets)
    }

    fn respond_to_message_inner(
        &mut self,
        message: DownloadListMessage,
        widgets: &mut Vec<WidgetType>,
    ) -> Option<DownloadListMessage> {
        match message {
            DownloadListMessage::GoUp => {
                self.select_previous();
                None
            }
            DownloadListMessage::GoDown => {
                self.select_next();
                None
            }
            DownloadListMessage::AppendTaskInput => {
                widgets.push(WidgetType::new_download_input());
                None
            }
            DownloadListMessage::AppendTask(request) => {
                self.append_task(request); // TODO: handle error
                None
            }
        }
    }

    fn get_key_message(&mut self, key: KeyEvent) -> Option<DownloadListMessage> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(DownloadListMessage::GoUp),
            KeyCode::Down | KeyCode::Char('j') => Some(DownloadListMessage::GoDown),
            KeyCode::Char('a') => Some(DownloadListMessage::AppendTaskInput),
            _ => None,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, widgets: &mut Vec<WidgetType>) {
        let mut opt_message = self.get_key_message(key);
        while let Some(message) = opt_message {
            opt_message = self.respond_to_message_inner(message, widgets);
        }
    }

    fn push_to_finish_list(
        listener: &mut TaskListener,
        finish_list: &mut FinishList,
    ) {
        finish_list.push_task(listener.into_finished_task());
    }

    pub fn handle_async(&mut self, finish_list: &mut FinishList) {
        for index in 0..self.list.len() {
            let listener = &mut self.list[index];
            if listener.processed() {
                continue;
            }

            let (remove_hint, mark) = if let Some(task_result) = listener.try_receive() {
                (
                    matches!(
                        task_result.final_stage,
                        TaskFinalStage::UnknownUrl
                            | TaskFinalStage::FailToConnection
                            | TaskFinalStage::FailToCreateFile
                            | TaskFinalStage::UnknownError
                            | TaskFinalStage::Finished
                    ),
                    true,
                )
            } else {
                (false, false)
            };

            if mark {
                listener.mark_processed();
            }
            if remove_hint {
                Self::push_to_finish_list(listener, finish_list);
                self.list.remove(index);
            }
        }
    }
}

impl StatefulWidget for &mut DownloadList {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let mut list_remain_area = area;
        let empty_text_style = if *state {
            Style::new().bg(Color::Gray).fg(Color::Black)
        } else {
            Style::new().fg(Color::White)
        };

        // 没有下载任务时，显示EMPTY
        if self.list.is_empty() {
            let text = "NO TASKS";
            let text_area = common::centered_text(text, area, 0, 0);
            Paragraph::new(text)
                .style(empty_text_style)
                .centered()
                .render(text_area, buf);
            return;
        }

        for (i, listener) in self.list.iter_mut().enumerate() {
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

            // 我们至少需要3行空间来渲染一个下载任务的状态
            if list_remain_area.height < 3 {
                Fill::new(DownloadList::NOT_ENOUGH_SPACE_BG).render(list_remain_area, buf);
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
            listener.render(state_area, buf, &mut is_selected);

            list_remain_area = area;
        }
    }
}

pub enum DownloadListMessage {
    GoUp,
    GoDown,
    AppendTaskInput,
    AppendTask(DownloadRequest),
}
