use std::sync::{Arc, Mutex};

use ratatui::widgets::StatefulWidget;
use ratatui::{prelude::*, widgets::Paragraph};
use tokio::sync::{mpsc, oneshot};

use crate::app::sender::Sender;
use crate::app::task::TaskCommand;
use crate::{
    app::task::{TaskFinalStage, TaskResult, TaskState},
    window::app::{FinishState, FinishedTask},
};

pub struct TaskListener {
    state: Arc<Mutex<TaskState>>,
    channel: ListenerChannel,

    // Task的结果，一旦接收后就存储在这里。
    // 如果是None表明Task还没有完成
    task_result: Option<TaskResult>,
    // 一个标志，表示结果是否已经被处理过
    processed: bool,
    stopped: bool,
}

impl TaskListener {
    pub const RENDER_HEIGHT: u16 = TaskState::RENDER_HEIGHT;

    // -------------------- CONSTRUCT -----------------------

    pub fn new(
        state: Arc<Mutex<TaskState>>,
        result_recv: oneshot::Receiver<TaskResult>,
        command_sender: oneshot::Sender<TaskCommand>,
    ) -> Self {
        TaskListener {
            state,
            channel: ListenerChannel::new(result_recv, command_sender),
            task_result: None,
            processed: false,
            stopped: false,
        }
    }

    // -------------------- TYPE_CONVERSION -----------------------

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

    // -------------------- FUNCTION -----------------------

    pub fn resume_task(
        &mut self,
        sender: &mut Sender,
    ) -> Result<(), Box<mpsc::error::SendError<Arc<Mutex<TaskState>>>>> {
        if !self.stopped {
            return Ok(());
        }

        let ListenerChannel {
            result_recv,
            command_sender,
        } = sender
            .send_resume_request(self.state.clone())
            .map_err(|t| Box::new(mpsc::error::SendError(t.0.release_state())))?;

        self.stopped = false;
        self.channel.command_sender = command_sender;
        self.channel.result_recv = result_recv;
        self.processed = false;
        self.task_result = None;
        Ok(())
    }

    pub fn try_receive(&mut self) -> Option<&TaskResult> {
        if self.task_result.is_some() || self.processed || self.stopped {
            // 已经接收过结果，直接返回
            return self.task_result.as_ref();
        }

        match self.result_recv_channel().try_recv() {
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

    pub fn send_command(&mut self, command: TaskCommand) {
        let sender = self.command_sender_channel().take();
        if let Some(sender) = sender
            && !self.stopped
        {
            log::debug!("Sending command to task: {:?}", command);
            let _ = sender.send(command);
        }
    }

    // -------------------- MEMBER_ACCESS -----------------------

    pub fn result_recv_channel(&mut self) -> &mut oneshot::Receiver<TaskResult> {
        &mut self.channel.result_recv
    }

    pub fn command_sender_channel(&mut self) -> &mut Option<oneshot::Sender<TaskCommand>> {
        &mut self.channel.command_sender
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

    pub fn is_stopped(&mut self) -> bool {
        self.stopped
    }

    // -------------------- MODIFIER -----------------------

    pub fn mark_processed(&mut self) {
        self.processed = true;
    }

    pub fn mark_stopped(&mut self) {
        self.stopped = true;
    }
}

impl StatefulWidget for &TaskListener {
    type State = bool; // focused
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // 渲染只占用3行
        let area = Layout::vertical([
            Constraint::Length(TaskListener::RENDER_HEIGHT),
            Constraint::Min(0),
        ])
        .split(area)[0];

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

pub struct ListenerChannel {
    pub result_recv: oneshot::Receiver<TaskResult>,

    // 用于给Task发送指令
    pub command_sender: Option<oneshot::Sender<TaskCommand>>,
}

impl ListenerChannel {
    pub fn new(
        result_recv: oneshot::Receiver<TaskResult>,
        command_sender: oneshot::Sender<TaskCommand>,
    ) -> Self {
        ListenerChannel {
            result_recv,
            command_sender: Some(command_sender),
        }
    }
}
