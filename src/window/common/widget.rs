use ratatui::prelude::*;

pub struct Fill {
    bg: Style,
}

/// 用于给某个区域填充某个style内容的组件
impl Fill {
    pub fn new(bg: Style) -> Self {
        Fill { bg }
    }
}

impl Widget for Fill {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for x in area.left()..area.right() {
            for y in area.top()..area.bottom() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(self.bg);
                }
            }
        }
    }
}
