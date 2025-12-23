use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget};
use tui_textarea::TextArea;

use crate::app::App;
use crate::window::WidgetType;
use crate::window::app::{DownloadList, DownloadListMessage};
use crate::window::common::{self, InputMode, MessageTransfer, WidgetExt};

/// 一个输入下载链接的窗口
///
/// TODO: 未来再扩展更多功能。
///
/// TODO: 现在先使用tui_input库的输入框，不过这个输入框对于自动换行的文本的支持较差，
/// 具体表现为光标显示错误，因此未来使用自定义的输入框替代。
pub struct DownloadInput {
    input: TextArea<'static>,
    mode: InputMode,
}

impl Default for DownloadInput {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadInput {
    const INPUT_BOARDER_HIGHLIGHT_STYLE: Style = Style::new().fg(Color::LightYellow);

    pub fn new() -> Self {
        DownloadInput {
            input: TextArea::default(),
            mode: InputMode::Normal,
        }
    }

    pub fn input(&self) -> &TextArea<'_> {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.input
    }

    pub fn mode(&self) -> &InputMode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }

    fn comfirm_inner(self: Box<Self>, app: &mut App) {
        let lines = self.input.into_lines();
        for line in lines {
            DownloadList::respond_to_message(
                app,
                DownloadListMessage::AppendNewTask(line),
            );
        }
    }

    pub fn handle_key_event(self: Box<Self>, key: KeyEvent, app: &mut App) -> Vec<WidgetType> {
        self.key_event_handler(key, app, Self::get_key_message, WidgetType::DownloadInput)
    }

    fn get_key_message(&mut self, key: KeyEvent) -> Option<DownloadInputMessage> {
        match self.mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('e') | KeyCode::Char('a') | KeyCode::Char('i') => {
                    Some(DownloadInputMessage::StartEditing)
                }
                KeyCode::Enter => Some(DownloadInputMessage::Confirm),
                KeyCode::Char('q') => Some(DownloadInputMessage::Quit),
                _ => None,
            },
            InputMode::Editing => match key.code {
                KeyCode::Esc => Some(DownloadInputMessage::StopEditing),
                _ => Some(DownloadInputMessage::Input(key)),
            },
        }
    }
}

impl Widget for &mut DownloadInput {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let area =
            common::render_border(Some(Line::from("Download")), None, Style::new(), area, buf);

        let [hint_area, input_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        Paragraph::new("URL:")
            .left_aligned()
            .bold()
            .render(hint_area, buf);

        let border_text = Line::from("input");
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let block = match self.mode {
            InputMode::Normal => block.title(border_text.reset_style()),
            InputMode::Editing => block
                .title(border_text.italic())
                .border_style(DownloadInput::INPUT_BOARDER_HIGHLIGHT_STYLE),
        };

        self.input.set_block(block);
        self.input.render(input_area, buf);
    }
}

impl WidgetExt for DownloadInput {
    type Message = DownloadInputMessage;

    fn respond_to_message(
        mut self: Box<Self>,
        message: DownloadInputMessage,
        app: &mut App,
    ) -> MessageTransfer<Self> {
        match message {
            DownloadInputMessage::StartEditing => {
                self.as_mut().set_mode(InputMode::Editing);
                MessageTransfer::keep(self)
            }
            DownloadInputMessage::StopEditing => {
                self.set_mode(InputMode::Normal);
                MessageTransfer::keep(self)
            }
            DownloadInputMessage::Confirm => {
                self.comfirm_inner(app);
                MessageTransfer::new()
            }
            DownloadInputMessage::Input(key) => {
                self.input.input(key);
                MessageTransfer::keep(self)
            }
            DownloadInputMessage::Quit => {
                MessageTransfer::new()
            }
        }
    }
}

pub enum DownloadInputMessage {
    StartEditing,
    StopEditing,
    Confirm,
    Input(KeyEvent),
    Quit,
}
