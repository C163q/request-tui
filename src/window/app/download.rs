use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use tokio::sync::mpsc;

use crate::app::App;
use crate::app::listener::{TaskListener, TaskListenerRanderState};
use crate::app::sender;
use crate::app::task::{Task, TaskCommand, TaskFinalStage};
use crate::window::WidgetType;
use crate::window::app::FinishList;
use crate::window::common::{self, VerticalList, VerticalListItem};

pub struct DownloadListInner {
    list: Vec<TaskListener>,
    selected: Option<usize>,
    scroll: usize,
}

impl Default for DownloadListInner {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadListInner {
    pub const RENDER_ITEM_HEIGHT: u16 = TaskListener::RENDER_HEIGHT;

    // -------------------- CONSTRUCT -----------------------

    pub fn new() -> Self {
        DownloadListInner {
            list: Vec::new(),
            selected: None,
            scroll: 0,
        }
    }

    // -------------------- MEMBER_ACCESS ---------------------

    #[inline]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    #[inline]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    #[inline]
    pub fn list(&self) -> &Vec<TaskListener> {
        &self.list
    }

    #[inline]
    pub fn get_item(&self, index: usize) -> Option<&TaskListener> {
        self.list.get(index)
    }

    #[inline]
    pub fn get_item_mut(&mut self, index: usize) -> Option<&mut TaskListener> {
        self.list.get_mut(index)
    }

    // -------------------- MODIFIER -----------------------

    #[inline]
    pub fn set_selected(&mut self, selected: Option<usize>) {
        self.selected = selected;
    }

    #[inline]
    pub fn scroll_to(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    // -------------------- FUNCTION -----------------------

    fn fit_to_screen(&mut self, area_height: u16) {
        let total_height =
            (self.list.len() * (Self::RENDER_ITEM_HEIGHT as usize + 1)).saturating_sub(1);
        self.scroll_to(
            self.scroll()
                .min(total_height.saturating_sub(area_height as usize)),
        );
    }

    #[inline]
    pub fn push_task(&mut self, listener: TaskListener) {
        self.list.push(listener);
    }

    #[inline]
    pub fn remove_task(&mut self, index: usize) {
        self.list.remove(index);
    }
}

impl StatefulWidget for &mut DownloadListInner {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.fit_to_screen(area.height);
        match self.selected() {
            None => {}
            Some(idx) => {
                let focused_distance = idx * (DownloadListInner::RENDER_ITEM_HEIGHT + 1) as usize;
                if focused_distance < self.scroll() {
                    self.scroll_to(focused_distance);
                }
                if focused_distance + DownloadListInner::RENDER_ITEM_HEIGHT as usize
                    > self.scroll() + area.height as usize
                {
                    self.scroll_to(
                        focused_distance + DownloadListInner::RENDER_ITEM_HEIGHT as usize
                            - area.height as usize,
                    );
                }
            }
        }

        let items: Vec<_> = self
            .list
            .iter()
            .map(|item| VerticalListItem::new(DownloadListInner::RENDER_ITEM_HEIGHT, item))
            .collect();
        VerticalList::new(items, TaskListenerRanderState::new(*state, false))
            .with_selected_state(TaskListenerRanderState::new(*state, true))
            .with_selected(self.selected())
            .with_scroll(self.scroll())
            .render(area, buf);
    }
}

/// 此处我们不使用ratatui的list，因为我们此处需要渲染进度条等复杂组件。
///
/// 组件的每一项是一个下载任务的状态。其每一项大致结构如下：
///
/// <filename>
/// <process bar> <percentage>%
///               <speed> <eta>
/// --------------------------- (分隔线，如果不是最后一项并且有空间渲染时)
pub struct DownloadList {
    inner: DownloadListInner,
    sender: sender::Sender,
}

impl DownloadList {
    // -------------------- CONSTANT -----------------------

    pub const NOT_ENOUGH_SPACE_BG: Style = Style::new().bg(Color::DarkGray);

    // -------------------- CONSTRUCT -----------------------

    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        DownloadList {
            inner: DownloadListInner::new(),
            sender: sender::Sender::new(sender),
        }
    }

    // -------------------- MEMBER_ACCESS -----------------------

    #[inline]
    pub fn selected(&self) -> Option<usize> {
        self.inner.selected()
    }

    #[inline]
    pub fn list(&self) -> &Vec<TaskListener> {
        self.inner.list()
    }

    // -------------------- MODIFIER -----------------------

    #[inline]
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.inner.set_selected(index);
    }

    // -------------------- FUNCTION -----------------------

    pub fn select_next(&mut self) {
        if self.list().is_empty() {
            self.set_selected(None);
            return;
        }

        match self.selected() {
            Some(i) => {
                self.set_selected(Some((i + 1) % self.list().len()));
            }
            None => {
                self.set_selected(Some(0));
            }
        }
    }

    pub fn select_previous(&mut self) {
        if self.list().is_empty() {
            self.set_selected(None);
            return;
        }

        let len = self.list().len();

        match self.selected() {
            Some(i) => {
                if i == 0 || i >= len {
                    self.set_selected(Some(len - 1));
                } else {
                    self.set_selected(Some(i - 1));
                }
            }
            None => {
                self.set_selected(Some(0));
            }
        }
    }

    pub fn append_normal_task(&mut self, url: String) -> anyhow::Result<()> {
        let listener = self.sender.send_normal_request(url)?;
        self.inner.push_task(listener);
        Ok(())
    }

    pub fn stop_task(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.list().len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }

        let listener = self.inner.get_item_mut(index).unwrap();
        if listener.is_stopped() {
            return Ok(());
        }

        listener.send_command(TaskCommand::Stop);
        Ok(())
    }

    pub fn abort_task(&mut self, index: usize, finish_list: &mut FinishList) -> anyhow::Result<()> {
        if index >= self.list().len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }

        let listener = self.inner.get_item_mut(index).unwrap();
        if listener.is_stopped() {
            self.move_to_finish_list(index, finish_list);
            return Ok(());
        }

        listener.send_command(TaskCommand::Abort);
        Ok(())
    }

    pub fn resume_task(
        &mut self,
        index: usize,
        finish_list: &mut FinishList,
    ) -> anyhow::Result<()> {
        if index >= self.list().len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }

        if self
            .inner
            .get_item_mut(index)
            .unwrap()
            .resume_task(&mut self.sender)
            .is_err()
        {
            self.move_to_finish_list(index, finish_list);
        }
        Ok(())
    }

    fn push_to_finish_list(listener: &mut TaskListener, finish_list: &mut FinishList) {
        finish_list.push_task(listener.into_finished_task());
    }

    fn move_to_finish_list(&mut self, index: usize, finish_list: &mut FinishList) {
        Self::push_to_finish_list(self.inner.get_item_mut(index).unwrap(), finish_list);
        self.inner.remove_task(index);
        if let Some(selected) = self.selected() {
            if selected >= index && selected > 0 {
                self.set_selected(Some(selected - 1));
            } else if self.list().is_empty() {
                self.set_selected(None);
            }
        }
    }

    // ------------------- HANDLE_MESSAGE ----------------------

    #[inline]
    pub fn respond_to_message(
        app: &mut App,
        message: DownloadListMessage,
    ) -> Option<DownloadListMessage> {
        let (this_widget, widgets, finish_list) = app.destruct_data();
        this_widget.respond_to_message_inner(message, widgets, finish_list)
    }

    fn respond_to_message_inner(
        &mut self,
        message: DownloadListMessage,
        widgets: &mut Vec<WidgetType>,
        finish_list: &mut FinishList,
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
            DownloadListMessage::AppendNewTask(request) => {
                // FIXME: 应该之后会专门制作一个弹窗
                self.append_normal_task(request); // TODO: handle error
                None
            }
            DownloadListMessage::StopTask => {
                if let Some(index) = self.selected() {
                    if index >= self.list().len() {
                        self.set_selected(None);
                        return None;
                    }
                    self.stop_task(index).unwrap();
                }
                None
            }
            DownloadListMessage::CancelTask => {
                if let Some(index) = self.selected() {
                    if index >= self.list().len() {
                        self.set_selected(None);
                        return None;
                    }
                    self.abort_task(index, finish_list).unwrap();
                }
                None
            }
            DownloadListMessage::ContinueTask => {
                if let Some(index) = self.selected() {
                    if index >= self.list().len() {
                        self.set_selected(None);
                        return None;
                    }
                    self.resume_task(index, finish_list).unwrap();
                }
                None
            }
        }
    }

    fn get_key_message(&mut self, key: KeyEvent) -> Option<DownloadListMessage> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(DownloadListMessage::GoUp),
            KeyCode::Down | KeyCode::Char('j') => Some(DownloadListMessage::GoDown),
            KeyCode::Char('a') => Some(DownloadListMessage::AppendTaskInput),
            KeyCode::Char('s') => Some(DownloadListMessage::StopTask),
            KeyCode::Char('c') => Some(DownloadListMessage::ContinueTask),
            KeyCode::Char('x') => Some(DownloadListMessage::CancelTask),
            _ => None,
        }
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
        widgets: &mut Vec<WidgetType>,
        finish_list: &mut FinishList,
    ) {
        let mut opt_message = self.get_key_message(key);
        while let Some(message) = opt_message {
            opt_message = self.respond_to_message_inner(message, widgets, finish_list);
        }
    }

    // ------------------- HANDLE_ASYNC ----------------------

    pub fn handle_async(&mut self, finish_list: &mut FinishList) {
        if self.selected().is_none() && !self.list().is_empty() {
            self.set_selected(Some(0));
        }

        let mut idx = 0;
        while idx < self.list().len() {
            // 不应该写成这样：
            // for idx in 0..self.list().len() { ... }
            // 因为我们会在循环内删除元素，上面的写法不会及时反应Vec的长度变化，
            // 导致panic。

            let listener = self.inner.get_item_mut(idx).unwrap();

            if listener.processed() {
                idx += 1;
                continue;
            }

            let (remove_hint, mark_processed, mark_stopped) =
                if let Some(task_result) = listener.try_receive() {
                    (
                        matches!(
                            task_result.final_stage,
                            TaskFinalStage::UnknownUrl
                                | TaskFinalStage::FailToConnection
                                | TaskFinalStage::FailToCreateFile
                                | TaskFinalStage::FileCorrupted
                                | TaskFinalStage::Abort
                                | TaskFinalStage::Finished
                                | TaskFinalStage::UnknownError
                        ),
                        true,
                        !matches!(task_result.final_stage, TaskFinalStage::Finished),
                    )
                } else {
                    (false, false, false)
                };

            if mark_processed {
                listener.mark_processed();
            }
            if mark_stopped {
                listener.mark_stopped();
            }
            if remove_hint {
                self.move_to_finish_list(idx, finish_list);
                // 直接continue，因为当前idx已经被移除，下一项已经移动到当前位置
                continue;
            }

            idx += 1;
        }
    }
}

impl StatefulWidget for &mut DownloadList {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let empty_text_style = if *state {
            Style::new().bg(Color::Gray).fg(Color::Black)
        } else {
            Style::new().fg(Color::White)
        };

        // 没有下载任务时，显示EMPTY
        if self.list().is_empty() {
            let text = "NO TASKS";
            let text_area = common::centered_text(text, area, 0, 0);
            Paragraph::new(text)
                .style(empty_text_style)
                .centered()
                .render(text_area, buf);
            return;
        }

        self.inner.render(area, buf, state);
    }
}

pub enum DownloadListMessage {
    GoUp,
    GoDown,
    AppendTaskInput,
    AppendNewTask(String),
    StopTask,
    ContinueTask,
    CancelTask,
}
