use std::sync::{Arc, Mutex};

use ratatui::widgets::StatefulWidget;
use ratatui::{prelude::*, widgets::Paragraph};
use tokio::sync::oneshot;

use crate::{
    app::task::{TaskFinalStage, TaskResult, TaskState},
    window::app::{FinishState, FinishedTask},
};

pub struct TaskListener {
    state: Arc<Mutex<TaskState>>,
    result_recv: oneshot::Receiver<TaskResult>,
    // Task的结果，一旦接收后就存储在这里。
    // 如果是None表明Task还没有完成
    task_result: Option<TaskResult>,
    // 一个标志，表示结果是否已经被处理过
    processed: bool,
}

impl TaskListener {
    pub fn new(state: Arc<Mutex<TaskState>>, result_recv: oneshot::Receiver<TaskResult>) -> Self {
        TaskListener {
            state,
            result_recv,
            task_result: None,
            processed: false,
        }
    }

    pub fn into_finished_task(&mut self) -> FinishedTask {
        self.processed = true;
        let finish_state = match &self.task_result {
            None => FinishState::Failure,
            Some(r) => match r.stage() {
                TaskFinalStage::Finished => FinishState::Success,
                _ => FinishState::Failure,
            },
        };
        let cloned_state = {
            let state_lock = self.state.lock().unwrap();
            state_lock.clone()
        };

        let content_length = match self.task_result.as_ref() {
            Some(result) => match result.final_stage {
                TaskFinalStage::Finished => cloned_state
                    .content_length()
                    .or(Some(cloned_state.downloaded())),
                _ => cloned_state.content_length(),
            },
            None => cloned_state.content_length(),
        };

        FinishedTask::new(
            finish_state,
            cloned_state.filepath().to_path_buf(),
            cloned_state.url().cloned(),
            content_length,
            cloned_state.downloaded(),
        )
    }

    pub fn try_receive(&mut self) -> Option<&TaskResult> {
        if self.task_result.is_some() || self.processed {
            // 已经接收过结果，直接返回
            return self.task_result.as_ref();
        }

        match self.result_recv.try_recv() {
            Ok(task_result) => {
                self.task_result = Some(task_result);
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(oneshot::error::TryRecvError::Closed) => {
                self.task_result = Some(TaskResult::new_unknown_error(String::from(
                    "Task result channel closed unexpectedly",
                )));
            }
        }

        self.task_result.as_ref()
    }

    pub fn get_state_handler(&self) -> Arc<Mutex<TaskState>> {
        self.state.clone()
    }

    pub fn task_result(&self) -> Option<&TaskResult> {
        self.task_result.as_ref()
    }

    pub fn processed(&self) -> bool {
        self.processed
    }

    pub fn mark_processed(&mut self) {
        self.processed = true;
    }
}

impl StatefulWidget for &mut TaskListener {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 渲染只占用3行
        let area = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area)[0];

        let mut cloned_state = {
            let mut state_lock = self.state.lock().unwrap();
            // 首先更新下载速度等状态信息
            state_lock.ui_update();
            // 然后克隆一份用于渲染
            state_lock.clone()
            // 此处unlock，这样在渲染时不会阻塞其他线程对state的访问
        };

        cloned_state.render(area, buf, state);

        let text_area =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area)[1];

        let text = match &self.task_result {
            Some(result) => result.final_stage.to_string(),
            None => String::from("Downloading..."),
        };
        Paragraph::new(text).left_aligned().render(text_area, buf);
    }
}
