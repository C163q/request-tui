use ratatui::crossterm::event::KeyEvent;

use crate::{app::App, window::WidgetType};
pub enum InputMode {
    Normal,
    Editing,
}

/// WidgetExt表示该类型能够接收某种消息类型，并根据消息进行响应。
pub trait WidgetExt: Sized {
    /// 消息类型
    type Message;

    fn respond_to_message(
        self: Box<Self>,
        message: Self::Message,
        app: &mut App,
    ) -> MessageTransfer<Self>;

    /// 需要给出具体的按键处理逻辑给handler参数，其返回的消息会被不断传递给
    /// [`respond_to_message`]函数直到没有消息为止。
    ///
    /// wrapper用于将最终的Widget还原成WidgetType。
    ///
    /// 最后得到的Vec<WidgetType>会被添加到App的widget列表中。
    ///
    /// [`respond_to_message`]: WidgetExt::respond_to_message
    fn key_event_handler(
        mut self: Box<Self>,
        key: KeyEvent,
        app: &mut App,
        handler: impl FnOnce(&mut Self, KeyEvent) -> Option<Self::Message>,
        wrapper: impl FnOnce(Box<Self>) -> WidgetType,
    ) -> Vec<WidgetType> {
        let mut opt_message = handler(self.as_mut(), key);
        let mut self_widget = Some(self);
        let mut res = vec![];
        while let Some(message) = opt_message
            && let Some(widget) = self_widget
        {
            let MessageTransfer {
                response,
                boxed_widget,
                new_widget,
            } = widget.respond_to_message(message, app);
            opt_message = response;
            self_widget = boxed_widget;
            res.extend(new_widget.into_iter());
        }
        if let Some(widget) = self_widget {
            res.insert(0, wrapper(widget));
        }
        res
    }
}

/// 用于在Widget处理消息后传递结果。
///
/// response字段会被反复传递给[`WidgetExt`]的[`respond_to_message`]函数，
/// 直到其为None为止。
///
/// 由于消息传递时widget会以所有权的形式传递，因此需要通过boxed_widget字段
/// 传回处理后的widget。如果为[`None`]，视为窗口关闭。
///
/// new_widget字段用于传回需要添加到App的widget列表中的新widget。值得注意
/// 的是，如果此时response字段不为[`None`]，则不会处理new_widget字段。
///
/// [`respond_to_message`]: WidgetExt::respond_to_message
pub struct MessageTransfer<T: WidgetExt> {
    pub response: Option<T::Message>,
    pub boxed_widget: Option<Box<T>>,
    pub new_widget: Option<WidgetType>,
}

impl<T: WidgetExt> Default for MessageTransfer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: WidgetExt> MessageTransfer<T> {
    pub fn new() -> Self {
        MessageTransfer {
            response: None,
            boxed_widget: None,
            new_widget: None,
        }
    }

    pub fn keep(boxed_widget: Box<T>) -> Self {
        MessageTransfer {
            response: None,
            boxed_widget: Some(boxed_widget),
            new_widget: None,
        }
    }
}

/// <size> Bytes -> B/KB/MB/GB
pub fn get_human_readable_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
