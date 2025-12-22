use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::app::{receive::TaskListener, task::{Task, TaskState}};

#[derive(Debug)]
pub struct Sender {
    sender: mpsc::Sender<Task>,
}

impl Sender {
    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        Sender { sender }
    }

    pub fn send_request(
        &self,
        request: DownloadRequest,
    ) -> Result<TaskListener, Box<mpsc::error::SendError<Task>>> {
        let state = Arc::new(Mutex::new(TaskState::new()));
        let (tx, rx) = oneshot::channel();
        let task = Task::new(state.clone(), request, tx);
        self.sender.blocking_send(task)?;
        let listener = TaskListener::new(state, rx);
        Ok(listener)
    }
}

#[derive(Debug)]
pub enum DownloadRequest {
    Normal { url: String },
}

impl DownloadRequest {
    pub fn new_normal(url: String) -> Self {
        DownloadRequest::Normal { url }
    }
}
