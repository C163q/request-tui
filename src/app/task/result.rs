use std::fmt::{self, Display, Formatter};

/// 通过channel发送给UI线程的内容，用于显示错误信息或者设置任务最终状态。
#[derive(Debug)]
pub struct TaskResult {
    pub final_stage: TaskFinalStage,
    pub message: Option<String>,
}

impl TaskResult {
    // -------------------- CONSTRUCT -----------------------

    pub fn new(final_stage: TaskFinalStage, message: Option<String>) -> Self {
        TaskResult {
            final_stage,
            message,
        }
    }

    pub fn new_unknown_url(message: String) -> Self {
        TaskResult::new(TaskFinalStage::UnknownUrl, Some(message))
    }

    pub fn new_failed_to_connection(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToConnection, Some(message))
    }

    pub fn new_failed_to_create_file(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToCreateFile, Some(message))
    }

    pub fn new_failed_to_download(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToDownload, Some(message))
    }

    pub fn new_failed_to_write(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToWrite, Some(message))
    }

    pub fn new_failed_to_resume_file(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToResumeFile, Some(message))
    }

    pub fn new_file_corrupted(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FileCorrupted, Some(message))
    }

    pub fn new_failed_to_resume_connection(message: String) -> Self {
        TaskResult::new(TaskFinalStage::FailToResumeConnection, Some(message))
    }

    pub fn new_interrupted() -> Self {
        TaskResult::new(TaskFinalStage::Interrupted, None)
    }

    pub fn new_abort() -> Self {
        TaskResult::new(TaskFinalStage::Abort, None)
    }

    pub fn new_finished() -> Self {
        TaskResult::new(TaskFinalStage::Finished, None)
    }

    pub fn new_unknown_error(message: String) -> Self {
        TaskResult::new(TaskFinalStage::UnknownError, Some(message))
    }

    // -------------------- MEMBER_ACCESS -----------------------

    pub fn stage(&self) -> TaskFinalStage {
        self.final_stage
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// 我们希望这些错误能够通过channel发送给UI线程，以便UI线程能够显示错误信息。
///
/// 一下是不同阶段可能的错误类型，不同阶段的错误处理方式应该不同。
///
/// 对于FailToConnection，可以直接将任务标记为失败，并以失败状态放置到完成列表。
/// 对于FailToCreateFile，同样可以将任务标记为失败，并以失败状态放置到完成列表。
/// 对于Interrupted，则需要将任务标记为暂停状态，用户仍然有机会重新开始该任务。
/// 对于Finished，则将任务标记为成功，放置到完成列表。
#[derive(Debug, Clone, Copy)]
pub enum TaskFinalStage {
    UnknownUrl,
    FailToConnection,
    FailToCreateFile,
    FailToDownload,
    FailToWrite,
    FailToResumeFile,
    FileCorrupted,
    FailToResumeConnection,
    Interrupted,
    Abort,
    Finished,
    UnknownError,
}

impl Display for TaskFinalStage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TaskFinalStage::UnknownUrl => write!(f, "Unknown URL"),
            TaskFinalStage::FailToConnection => write!(f, "Connection Failed"),
            TaskFinalStage::FailToCreateFile => write!(f, "Failed to create file"),
            TaskFinalStage::FailToDownload => write!(f, "Failed to download"),
            TaskFinalStage::FailToWrite => write!(f, "Failed to write to file"),
            TaskFinalStage::FailToResumeFile => write!(f, "Cannot open file"),
            TaskFinalStage::FileCorrupted => write!(f, "File corrupted"),
            TaskFinalStage::FailToResumeConnection => write!(f, "Connection failed"),
            TaskFinalStage::Interrupted => write!(f, "Stopped"),
            TaskFinalStage::Abort => write!(f, "Abort"),
            TaskFinalStage::Finished => write!(f, "Finished"),
            TaskFinalStage::UnknownError => write!(f, "Unknown error"),
        }
    }
}
