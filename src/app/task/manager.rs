use tokio::{runtime::Runtime, sync::mpsc};

use crate::app::task::{Task, resolve};

/// 用于在另一个线程中管理异步任务的执行
///
/// 所有TUI的渲染都是同步的，为了不阻塞UI线程，所有IO任务都放在另一个线程中执行，
/// 该线程由[`TaskManager`]管理。
///
/// 该线程的主要任务就是轮询mpsc通道，接收来自用户端的任务请求，由于这些任务的操作
/// 暂时是由UI线程承担的，所以这些任务需要从UI线程发送。
#[derive(Debug)]
pub struct TaskManager {
    runtime: Runtime,
    receiver: mpsc::Receiver<Task>,
}

impl TaskManager {
    // -------------------- CONSTRUCT ---------------------

    pub fn new(runtime: Runtime, receiver: mpsc::Receiver<Task>) -> Self {
        TaskManager { runtime, receiver }
    }

    // -------------------- RUNNING -----------------------

    pub fn run(&mut self) {
        self.runtime.block_on(async {
            while let Some(task) = self.receiver.recv().await {
                tokio::spawn(async move {
                    resolve::handle_task(task).await;
                });
            }
        })
    }
}
