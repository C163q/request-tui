use std::io::{self, Stdout};
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::style::palette::tailwind;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::{Terminal, widgets::Widget};
use tokio::sync::mpsc;

use crate::app::task::Task;
use crate::window::app::{DownloadList, FinishList, PageList};
use crate::window::common::Fill;
use crate::window::{WidgetType, common};

pub mod listener;
pub mod sender;
pub mod task;

/// 目前的设计如下：
///
/// Downloading |
/// Finished    | <Content>
/// Config      |
///
/// PageList管理左侧的页面选择部分，而AppData管理右侧内容区。
/// 在渲染右侧内容时，根据PageList的选择，选择不同的AppData进行渲染。
///
/// 一般来说不会存在PageList选择为[`None`]的情况，但为了避免这种情况发生，
/// 需要特地设计一个空页面。
pub struct App {
    // 左侧的页面选择
    list: PageList,
    data: Box<AppData>,
    // 由不同的WidgetType组成的窗口列表，尾部是最上层窗口
    widgets: Vec<WidgetType>,
    running: bool,
}

impl App {
    // --------------- CONSTRUCT ---------------

    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        App {
            list: PageList::new(),
            data: Box::new(AppData::new(sender)),
            widgets: vec![],
            running: true,
        }
    }

    // ---------------- RUNNING ----------------

    pub fn run(mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
        while self.running {
            self.handle_async();
            terminal.draw(|f| {
                f.render_widget(&mut self, f.area());
            })?;
            self.handle_event()?;
        }
        Ok(())
    }

    // ------------------ MEMBER_ACCESS --------------------

    #[inline]
    pub fn append_widget(&mut self, widget: WidgetType) {
        self.widgets.push(widget);
    }

    #[inline]
    pub fn append_widgets<I>(&mut self, widgets: I)
    where
        I: IntoIterator<Item = WidgetType>,
    {
        self.widgets.extend(widgets);
    }

    #[inline]
    pub fn download_list(&self) -> &DownloadList {
        self.data.downloading()
    }

    #[inline]
    pub fn download_list_mut(&mut self) -> &mut DownloadList {
        self.data.downloading_mut()
    }

    #[inline]
    pub fn finish_list(&self) -> &FinishList {
        self.data.finished()
    }

    #[inline]
    pub fn finish_list_mut(&mut self) -> &mut FinishList {
        self.data.finished_mut()
    }

    #[inline]
    pub(crate) fn destruct_data(
        &mut self,
    ) -> (&mut DownloadList, &mut Vec<WidgetType>, &mut FinishList) {
        (
            &mut self.data.downloading,
            &mut self.widgets,
            &mut self.data.finished,
        )
    }

    // -------------------- RENDER -----------------------

    /// 渲染整个程序的边框部分
    pub fn render_structure(&mut self, area: Rect, buf: &mut Buffer) -> (Rect, Rect) {
        let title = Line::from(" REQUEST ").bold().centered();

        // 外部边框
        let area = common::render_border(Some(title), None, Style::new(), area, buf);

        let [left, bar, right] = Layout::horizontal([
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);

        let [bar_top, bar_bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(bar);
        const HIGHLIGHT_BAR_STYLE: Style = Style::new().fg(tailwind::AMBER.c300);
        if self.list.entered() {
            Fill::new(HIGHLIGHT_BAR_STYLE).render(bar_bottom, buf);
        } else {
            Fill::new(HIGHLIGHT_BAR_STYLE).render(bar_top, buf);
        }
        Block::new()
            .borders(Borders::LEFT)
            .border_type(BorderType::Thick)
            .render(bar, buf);

        (left, right)
    }

    /// 仅仅是在屏幕中间显示一个EMPTY文本
    fn render_empty_page(&mut self, area: Rect, buf: &mut Buffer) {
        let text = "EMPTY";
        let text_area = common::centered_text(text, area, 0, 0);
        Paragraph::new(text).centered().render(text_area, buf);
    }

    /// 渲染整个程序右半屏的内容
    ///
    /// 需要根据页面左侧选择的不同而渲染不同的内容
    fn render_page(&mut self, area: Rect, buf: &mut Buffer, selected: usize) {
        match selected {
            0 => {
                self.data
                    .downloading_mut()
                    .render(area, buf, &mut self.list.entered());
            }
            1 => {
                self.data
                    .finished_mut()
                    .render(area, buf, &mut self.list.entered());
            }
            _ => self.list.set_selected(None),
        }
    }

    // --------------------- HANDLE_EVENT -----------------------

    // 我们让页面至少以10FPS的频率进行刷新，而不会因为没有事件而阻塞
    pub fn handle_event(&mut self) -> io::Result<()> {
        let timeout = Duration::from_secs_f64(1.0 / 10.0);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => self.distribute_key_event(key),
                Event::Mouse(_) => {} // TODO: handle mouse events
                _ => {}
            }
        }
        Ok(())
    }

    pub fn respond_to_message(&mut self, message: AppMessage) -> Option<AppMessage> {
        match message {
            // App只负责处理推出按键
            AppMessage::Quit => {
                self.running = false;
                None
            }
            // 其他的就交给各个子组件去处理
            AppMessage::Distribute(key) => {
                if let Some(key) = self.list.handle_key_event(key)
                    && let Some(i) = self.list.selected()
                {
                    self.distribute_to_content(key, i);
                }
                None
            }
        }
    }

    #[inline]
    fn handle_key_event(&mut self, key: KeyEvent) {
        let mut opt_message = Self::get_key_message(key);
        while let Some(message) = opt_message {
            opt_message = self.respond_to_message(message);
        }
    }

    // App只处理推出的逻辑，其他的就交给各个子组件去处理
    fn get_key_message(key: KeyEvent) -> Option<AppMessage> {
        if key.kind == KeyEventKind::Press {
            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, code) => match code {
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        return Some(AppMessage::Quit);
                    }
                    _ => {}
                },
                (_, code) => match code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        return Some(AppMessage::Quit);
                    }
                    _ => {}
                },
            }
        }
        Some(AppMessage::Distribute(key))
    }

    // 需要根据页面左侧选择的不同，将KeyEvent分发给不同的内容
    fn distribute_to_content(&mut self, key: KeyEvent, selected: usize) {
        match selected {
            0 => {
                self.data.downloading.handle_key_event(
                    key,
                    &mut self.widgets,
                    &mut self.data.finished,
                );
            }
            1 => {
                self.data
                    .finished_mut()
                    .handle_key_event(key, &mut self.widgets);
            }
            _ => self.list.set_selected(None),
        }
    }

    // 我们将KeyEvent分发给最上层的Widget处理，如果没有Widget，则交给App处理
    fn distribute_key_event(&mut self, key: KeyEvent) {
        match self.widgets.pop() {
            Some(widget) => {
                widget.handle_key_event(key, self);
            }
            None => {
                self.handle_key_event(key);
            }
        }
    }

    // ------------------- HANDLE_ASYNC ----------------------

    #[inline]
    pub fn handle_async(&mut self) {
        self.data.handle_async();
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let (left, right) = self.render_structure(area, buf);
        self.list.render(left, buf);
        match self.list.selected() {
            None => {
                self.render_empty_page(right, buf);
            }
            Some(i) if i < PageList::PAGE_COUNT => {
                self.render_page(right, buf, i);
            }
            _ => {
                self.list.set_selected(None);
                self.render_empty_page(right, buf);
            }
        }

        for widget in &mut self.widgets {
            widget.render(area, buf);
        }
    }
}

pub enum AppMessage {
    Quit,
    Distribute(KeyEvent),
}

pub struct AppData {
    downloading: DownloadList,
    finished: FinishList,
}

impl AppData {
    // ------------------ CONSTRUCT --------------------

    pub fn new(sender: mpsc::Sender<Task>) -> Self {
        AppData {
            downloading: DownloadList::new(sender),
            finished: FinishList::new(),
        }
    }

    // ----------------- MEMBER_ACCESS ------------------

    #[inline]
    pub fn downloading_mut(&mut self) -> &mut DownloadList {
        &mut self.downloading
    }

    #[inline]
    pub fn downloading(&self) -> &DownloadList {
        &self.downloading
    }

    #[inline]
    pub fn finished_mut(&mut self) -> &mut FinishList {
        &mut self.finished
    }

    #[inline]
    pub fn finished(&self) -> &FinishList {
        &self.finished
    }

    // ------------------- HANDLE_ASYNC ---------------------

    #[inline]
    pub fn handle_async(&mut self) {
        self.downloading.handle_async(&mut self.finished);
        self.finished.handle_async();
    }
}
