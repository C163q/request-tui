use std::sync::{Arc, Mutex};

use tokio::{fs::File, sync::oneshot};

use crate::app::sender::DownloadRequest;

mod manager;
pub mod resolve;
mod result;
mod state;

pub use manager::*;
pub use result::*;
pub use state::*;

/// 由于UI线程需要频繁地了解任务的执行状态，并在UI界面显示，因此需要将任务状态
/// 与UI线程共享，这些状态通过[`TaskState`]结构体表示。
///
/// 任何无须与UI线程交互的任务逻辑都不应放置在`TaskState`中。
#[derive(Debug)]
pub struct Task {
    request: DownloadRequest,
    inner: TaskInner,
    handler: SignalHandler,
}

impl Task {
    // -------------------- CONSTRUCT -----------------------

    pub fn new(
        state: Arc<Mutex<TaskState>>,
        request: DownloadRequest,
        reporter: oneshot::Sender<TaskResult>,
        command_recv: oneshot::Receiver<TaskCommand>,
    ) -> Self {
        Task {
            request,
            inner: TaskInner::new(state),
            handler: SignalHandler::new(reporter, command_recv),
        }
    }

    // -------------------- MEMBER_ACCESS -----------------------

    pub fn request(&self) -> &DownloadRequest {
        &self.request
    }

    pub fn file(&self) -> Option<&File> {
        self.inner.file()
    }

    pub fn release_state(self) -> Arc<Mutex<TaskState>> {
        self.inner.state
    }
}

#[derive(Debug)]
struct TaskInner {
    pub state: Arc<Mutex<TaskState>>,
    pub file: Option<File>,
}

impl TaskInner {
    // -------------------- CONSTRUCT -----------------------

    pub fn new(state: Arc<Mutex<TaskState>>) -> Self {
        TaskInner { state, file: None }
    }

    // -------------------- MEMBER_ACCESS -----------------------

    pub fn file(&self) -> Option<&File> {
        self.file.as_ref()
    }
}

#[derive(Debug)]
struct SignalHandler {
    pub reporter: oneshot::Sender<TaskResult>,
    pub receiver: oneshot::Receiver<TaskCommand>,
}

impl SignalHandler {
    pub fn new(
        reporter: oneshot::Sender<TaskResult>,
        receiver: oneshot::Receiver<TaskCommand>,
    ) -> Self {
        SignalHandler { reporter, receiver }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskCommand {
    Stop,
    Abort,
}
