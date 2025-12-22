use std::{
    fmt::{self, Display, Formatter}, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}
};

use ratatui::widgets::{Paragraph, Widget};
use ratatui::{prelude::*, style::palette::tailwind, widgets::Gauge};
use tokio::{fs::File, runtime::Runtime, sync::{mpsc, oneshot}};
use url::Url;

use crate::{app::send::DownloadRequest, window::common::{self, Fill}};

pub mod resolve;

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
    pub fn new(runtime: Runtime, receiver: mpsc::Receiver<Task>) -> Self {
        TaskManager { runtime, receiver }
    }

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

/// 用于表示单个下载任务的状态
///
/// 这些状态主要用于UI线程的渲染使用。这个结构体应当尽量轻量化，原因是在UI线程
/// 渲染前，会将这个结构体进行复制，以避免UI线程阻塞锁。
#[derive(Debug, Clone)]
pub struct TaskState {
    filepath: PathBuf,
    url: Option<Url>,
    accept_ranges: bool,
    content_length: Option<u64>,
    downloaded: u64,

    // 用于UI显示侧修改的数据
    last_updated: Instant,
    last_downloaded: u64,
    last_speed: Option<u64>,
}

impl Default for TaskState {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskState {
    const BAR_STYLE_WITH_TOTAL: Style =
        Style::new().fg(tailwind::BLUE.c400).bg(tailwind::GRAY.c500);
    const BAR_STYLE_NO_TOTAL: Style = Style::new()
        .fg(tailwind::YELLOW.c600)
        .bg(tailwind::GRAY.c500);
    const BAR_TEXT_STYLE: Style = Style::new().fg(Color::White);

    // 我们希望每隔500毫秒刷新一次下载速度显示
    const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

    const HIGHTLIGHT_COLOR: Color = Color::LightBlue;

    pub fn new() -> Self {
        TaskState {
            filepath: PathBuf::new(),
            url: None,
            accept_ranges: false,
            content_length: None,
            downloaded: 0,
            last_updated: Instant::now(),
            last_downloaded: 0,
            last_speed: None,
        }
    }

    pub fn filepath(&self) -> &Path {
        &self.filepath
    }

    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub fn accept_ranges(&self) -> bool {
        self.accept_ranges
    }

    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    pub fn downloaded(&self) -> u64 {
        self.downloaded
    }

    fn get_speed_string(&self) -> String {
        match self.last_speed {
            None => String::from("-- B/s"),
            Some(speed) => {
                format!("{}/s", common::get_human_readable_size(speed))
            }
        }
    }

    fn get_downloaded_string(&self) -> String {
        if let Some(total) = self.content_length {
            format!(
                "{}/{}",
                common::get_human_readable_size(self.downloaded),
                common::get_human_readable_size(total)
            )
        } else {
            format!("{} / --", common::get_human_readable_size(self.downloaded))
        }
    }

    /// 更新下载速度信息
    pub fn ui_update(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_updated);

        // 我们已经限制了刷新间隔，因此只有当距离上次刷新时间超过该间隔时，才更新速度信息
        if elapsed >= TaskState::REFRESH_INTERVAL {
            let downloaded_since_last = self.downloaded - self.last_downloaded;
            let speed = downloaded_since_last as f64 / elapsed.as_secs_f64();

            self.last_speed = Some(speed as u64);
            self.last_updated = now;
            self.last_downloaded = self.downloaded;
        }
    }
}

/// 给TaskState实现[`StatefulWidget`] trait，以便在UI线程中渲染任务状态。
/// 具体的渲染的样子可以见[`DownloadList`]的文档。
///
/// 请注意，下载速度信息的更新并不在这里进行，你需要调用[`TaskState::ui_update`]
/// 方法来更新速度信息。
///
/// 由于TaskState渲染的内容较多，不建议使用[`Mutex::lock`]的方式在UI线程中直接调用，
/// 推荐将TaskState进行复制，然后在UI线程中渲染复制的内容。这不会带来太大的性能损失，
/// 原因是UI线程的渲染频率并不高。
///
/// [`DownloadList`]: crate::window::app::DownloadList
impl StatefulWidget for &mut TaskState {
    type State = bool; // 是否选中
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let text_style = if *state {
            Style::new()
                .bg(TaskState::HIGHTLIGHT_COLOR)
                .fg(Color::Black)
        } else {
            Style::new().fg(Color::White)
        };

        // 我们将区域垂直分为三部分：文件名、进度条、其他信息，每个信息占据一行。
        // 理论上在调用render时，area的高度正好为3，但为了保险起见，我们在此给出限制。
        let [area, _] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

        if *state {
            Fill::new(Style::new().bg(TaskState::HIGHTLIGHT_COLOR)).render(area, buf);
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

        // 进度条
        match self.content_length {
            Some(total) => {
                let percentage = (if total == 0 {
                    0.0
                } else {
                    self.downloaded as f64 / total as f64 * 100.0
                } as u64)
                    .clamp(0, 100) as u16;

                Gauge::default()
                    .label(Span::from(format!("{}%", percentage)).style(TaskState::BAR_TEXT_STYLE))
                    .gauge_style(TaskState::BAR_STYLE_WITH_TOTAL)
                    .style(TaskState::BAR_TEXT_STYLE)
                    .percent(percentage)
                    .use_unicode(true)
                    .render(bar, buf);
            }
            None => {
                Gauge::default()
                    .label(
                        Span::from(common::get_human_readable_size(self.downloaded))
                            .style(TaskState::BAR_TEXT_STYLE),
                    )
                    .gauge_style(TaskState::BAR_STYLE_NO_TOTAL)
                    .style(TaskState::BAR_TEXT_STYLE)
                    .ratio(1.0)
                    .use_unicode(true)
                    .render(bar, buf);
            }
        }

        // 其他信息
        Paragraph::new(format!(
            "{} | {}",
            self.get_downloaded_string(),
            self.get_speed_string()
        ))
        .style(text_style)
        .right_aligned()
        .render(footer, buf);
    }
}

/// 由于UI线程需要频繁地了解任务的执行状态，并在UI界面显示，因此需要将任务状态
/// 与UI线程共享，这些状态通过[`TaskState`]结构体表示。
///
/// 任何无须与UI线程交互的任务逻辑都不应放置在`TaskState`中。
#[derive(Debug)]
pub struct Task {
    request: DownloadRequest,
    inner: TaskInner,
    reporter: oneshot::Sender<TaskResult>,
}

impl Task {
    pub fn new(state: Arc<Mutex<TaskState>>, request: DownloadRequest, reporter: oneshot::Sender<TaskResult>) -> Self {
        Task {
            request,
            inner: TaskInner::new(state),
            reporter,
        }
    }

    pub fn request(&self) -> &DownloadRequest {
        &self.request
    }

    pub fn file(&self) -> Option<&File> {
        self.inner.file()
    }
}

#[derive(Debug)]
struct TaskInner {
    state: Arc<Mutex<TaskState>>,
    file: Option<File>,
}

impl TaskInner {
    pub fn new(state: Arc<Mutex<TaskState>>) -> Self {
        TaskInner { state, file: None }
    }

    pub fn file(&self) -> Option<&File> {
        self.file.as_ref()
    }
}

/// 通过channel发送给UI线程的内容，用于显示错误信息或者设置任务最终状态。
#[derive(Debug)]
pub struct TaskResult {
    pub final_stage: TaskFinalStage,
    pub message: Option<String>,
}

impl TaskResult {
    pub fn new(final_stage: TaskFinalStage, message: Option<String>) -> Self {
        TaskResult {
            final_stage,
            message,
        }
    }

    pub fn stage(&self) -> TaskFinalStage {
        self.final_stage
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
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

    pub fn new_finished() -> Self {
        TaskResult::new(TaskFinalStage::Finished, None)
    }

    pub fn new_unknown_error(message: String) -> Self {
        TaskResult::new(TaskFinalStage::UnknownError, Some(message))
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
    Interrupted,
    Finished,
    UnknownError,
}

impl Display for TaskFinalStage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TaskFinalStage::UnknownUrl => write!(f, "Unknown URL"),
            TaskFinalStage::FailToConnection => write!(f, "Failed to connect"),
            TaskFinalStage::FailToCreateFile => write!(f, "Failed to create file"),
            TaskFinalStage::FailToDownload => write!(f, "Failed to download"),
            TaskFinalStage::FailToWrite => write!(f, "Failed to write to file"),
            TaskFinalStage::Interrupted => write!(f, "Interrupted"),
            TaskFinalStage::Finished => write!(f, "Finished"),
            TaskFinalStage::UnknownError => write!(f, "Unknown error"),
        }
    }
}
