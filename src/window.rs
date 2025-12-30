use ratatui::crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::Widget;

use crate::app::App;
use crate::window::download::DownloadInput;

pub mod app;
pub mod common;
pub mod download;

/// 代表所有可能的窗口类型
///
/// 所有列出的窗口必须使用Box包装，原因是每个窗口的大小差距非常大。
/// [`WidgetType`]是实现Widget trait的，因此其中的每个可能的窗口类型
/// 必须都实现Widget trait。
pub enum WidgetType {
    DownloadInput(Box<DownloadInput>),
}

impl Widget for &mut WidgetType {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            WidgetType::DownloadInput(w) => {
                let area = common::centered_rect(50, 50, area);
                w.render(area, buf);
            }
        }
    }
}

impl WidgetType {
    pub fn new_download_input() -> Self {
        WidgetType::DownloadInput(Box::default())
    }

    pub fn handle_key_event(self, key: KeyEvent, app: &mut App) {
        let vec: Vec<WidgetType> = match self {
            WidgetType::DownloadInput(w) => w.handle_key_event(key, app),
        };
        app.append_widgets(vec);
    }
}
