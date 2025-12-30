use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::app::{
    listener::{ListenerChannel, TaskListener},
    task::{Task, TaskState},
};

#[derive(Debug)]
pub struct Sender {
    sender: mpsc::Sender<Task>,
}

impl Sender {
    // -------------------- CONSTRUCT -----------------------

    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        Sender { sender }
    }

    // -------------------- FUNCTION -----------------------

    pub fn send_normal_request(
        &self,
        url: String,
    ) -> Result<TaskListener, Box<mpsc::error::SendError<Task>>> {
        let state = Arc::new(Mutex::new(TaskState::new()));
        let (res_tx, res_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = oneshot::channel();
        let task = Task::new(
            state.clone(),
            DownloadRequest::new_normal(url),
            res_tx,
            cmd_rx,
        );
        self.sender.blocking_send(task)?;
        let listener = TaskListener::new(state, res_rx, cmd_tx);
        Ok(listener)
    }

    pub fn send_resume_request(
        &self,
        task_state: Arc<Mutex<TaskState>>,
    ) -> Result<ListenerChannel, Box<mpsc::error::SendError<Task>>> {
        let (res_tx, res_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = oneshot::channel();
        let task = Task::new(task_state, DownloadRequest::Resume, res_tx, cmd_rx);
        self.sender.blocking_send(task)?;
        Ok(ListenerChannel::new(res_rx, cmd_tx))
    }
}

#[derive(Debug)]
pub enum DownloadRequest {
    Normal { url: String },
    Resume,
}

impl DownloadRequest {
    pub fn new_normal(url: String) -> Self {
        DownloadRequest::Normal { url }
    }
}
