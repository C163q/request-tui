use ratatui::{
    layout::Flex,
    prelude::*,
    widgets::{Block, BorderType},
};
use unicode_width::UnicodeWidthStr;

pub fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout: [Rect; 3] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .areas(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .areas::<3>(popup_layout[1])[1] // Return the middle chunk
}

/// 使文本在给定区域内居中
///
/// `additional_x`和`additional_y`用于指定文本周围的额外空间。例如如果有边框的情况下，
/// 你需要设置这两个值为2。
pub fn centered_text(text: &str, area: Rect, additional_x: u16, additional_y: u16) -> Rect {
    let lines: Vec<_> = text.lines().collect();
    let lines_width: Vec<_> = lines
        .iter()
        .map(|&line| UnicodeWidthStr::width(line))
        .collect();

    let area_width = area.width.saturating_sub(additional_x).max(1);
    let line_height = lines_width
        .iter()
        .map(|&w| w.div_ceil(area_width as usize))
        .sum::<usize>() as u16;
    let line_width = lines_width.iter().max().cloned().unwrap_or(1) as u16;

    center(
        area,
        Constraint::Length(line_width + additional_x),
        Constraint::Length(line_height + additional_y),
    )
}

/// 渲染边框，并返回边框内的可用区域
pub fn render_border(
    top: Option<Line>,
    bottom: Option<Line>,
    style: Style,
    area: Rect,
    buf: &mut Buffer,
) -> Rect {
    let mut block = Block::bordered()
        .border_style(style)
        .border_type(BorderType::Rounded);

    if let Some(line) = top {
        block = block.title_top(line);
    }
    if let Some(line) = bottom {
        block = block.title_bottom(line);
    }

    block.render(area, buf);

    Layout::default()
        .margin(1)
        .constraints([Constraint::Min(0)])
        .split(area)[0]
}
