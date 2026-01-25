use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ratatui::widgets::{Paragraph, Widget};
use ratatui::{prelude::*, style::palette::tailwind, widgets::Gauge};
use url::Url;

use crate::window::common::{self, Fill};

/// 用于表示单个下载任务的状态
///
/// 这些状态主要用于UI线程的渲染使用。这个结构体应当尽量轻量化，原因是在UI线程
/// 渲染前，会将这个结构体进行复制，以避免UI线程阻塞锁。
#[derive(Debug, Clone)]
pub struct TaskState {
    pub filepath: PathBuf,
    pub url: Option<Url>,
    pub accept_ranges: bool,
    pub content_length: Option<u64>,
    pub downloaded: u64,

    // 用于UI显示侧修改的数据
    pub last_updated: Instant,
    pub last_downloaded: u64,
    pub last_speed: Option<u64>,
}

impl Default for TaskState {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskState {
    // ------------------- CONSTANT -----------------------

    const BAR_STYLE_WITH_TOTAL: Style =
        Style::new().fg(tailwind::BLUE.c400).bg(tailwind::GRAY.c500);
    const BAR_STYLE_NO_TOTAL: Style = Style::new()
        .fg(tailwind::YELLOW.c600)
        .bg(tailwind::GRAY.c500);
    const BAR_TEXT_STYLE: Style = Style::new().fg(Color::White);

    // 我们希望每隔500毫秒刷新一次下载速度显示
    const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

    const FOCUSED_HIGHTLIGHT_COLOR: Color = Color::LightBlue;
    const UNFOCUSED_HIGHTLIGHT_COLOR: Color = tailwind::GRAY.c500;

    pub const RENDER_HEIGHT: u16 = 3;

    // ----------------------- CONSTRUCT ------------------------

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

    // --------------------- MEMBER_ACCESS -----------------------

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

    // ---------------------- FUNCTION ------------------------

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

#[derive(Debug, Clone, Copy)]
pub struct TaskStateRenderState {
    pub page_focused: bool,
    pub selected: bool,
}

impl TaskStateRenderState {
    pub fn new(page_focused: bool, selected: bool) -> Self {
        TaskStateRenderState {
            page_focused,
            selected,
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
    type State = TaskStateRenderState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let highlight_color = if state.page_focused {
            TaskState::FOCUSED_HIGHTLIGHT_COLOR
        } else {
            TaskState::UNFOCUSED_HIGHTLIGHT_COLOR
        };

        let text_style = if state.selected {
            Style::new().bg(highlight_color).fg(Color::Black)
        } else {
            Style::new().fg(Color::White)
        };

        // 我们将区域垂直分为三部分：文件名、进度条、其他信息，每个信息占据一行。
        // 理论上在调用render时，area的高度正好为3，但为了保险起见，我们在此给出限制。
        let [area, _] = Layout::vertical([
            Constraint::Length(TaskState::RENDER_HEIGHT),
            Constraint::Min(0),
        ])
        .areas(area);

        if state.selected {
            Fill::new(Style::new().bg(highlight_color)).render(area, buf);
        }

        let [text, bar, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(area);

        let bar = Layout::horizontal([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(bar)[1];

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
