use std::num::NonZeroU16;

use ratatui::{
    prelude::*,
    symbols::scrollbar,
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

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

pub struct VerticalListItem<W: StatefulWidget<State = bool> + Clone> {
    // vertical_size should not be 0
    vertical_size: NonZeroU16,
    widget: W,
}

impl<W: StatefulWidget<State = bool> + Clone> VerticalListItem<W> {
    // ----------------- CONSTRUCT ------------------

    pub fn new(vertical_size: u16, widget: W) -> Self {
        let vertical_size = NonZeroU16::new(vertical_size).expect("vertical_size should not be 0");
        VerticalListItem {
            vertical_size,
            widget,
        }
    }

    // ----------------- MEMBER_ACCESS -----------------

    pub fn vertical_size(&self) -> u16 {
        self.vertical_size.get()
    }

    pub fn widget(&self) -> &W {
        &self.widget
    }

    // ------------------ MODIFIER ------------------

    pub fn set_vertical_size(&mut self, vertical_size: u16) {
        self.vertical_size = NonZeroU16::new(vertical_size).expect("vertical_size should not be 0");
    }
}

impl<W: StatefulWidget<State = bool> + Clone> StatefulWidget for &VerticalListItem<W> {
    type State = bool;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = Rect {
            height: self.vertical_size.get(),
            ..area
        };
        self.widget.clone().render(area, buf, state);
    }
}

pub struct VerticalList<W: StatefulWidget<State = bool> + Clone> {
    list: Vec<VerticalListItem<W>>,
    selected: Option<usize>,

    /// 距离顶部的滚动距离
    scroll: usize,
}

impl<W: StatefulWidget<State = bool> + Clone> VerticalList<W> {
    pub const NOT_ENOUGH_SPACE_BG: Style = Style::new().bg(Color::DarkGray);

    // ----------------- CONSTRUCT ------------------

    pub fn new(list: Vec<VerticalListItem<W>>) -> Self {
        VerticalList {
            scroll: 0,
            list,
            selected: None,
        }
    }

    #[inline]
    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.set_selected(selected);
        self
    }

    #[inline]
    pub fn with_scroll(mut self, scroll: usize) -> Self {
        self.scroll_to(scroll);
        self
    }

    // ----------------- MEMBER_ACCESS -----------------

    pub fn list(&self) -> &Vec<VerticalListItem<W>> {
        &self.list
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    // ------------------ MODIFIER ------------------

    pub fn set_selected(&mut self, selected: Option<usize>) {
        self.selected = selected;
    }

    pub fn scroll_to(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    pub fn scroll_top(&mut self) {
        self.scroll = 0;
    }
}

impl<W: StatefulWidget<State = bool> + Clone> Widget for &VerticalList<W> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut distance: usize = 0;
        const DIVIDER_HEIGHT: usize = 1;

        fn render_divider(area: Rect, buf: &mut Buffer) {
            Block::new().borders(Borders::BOTTOM).render(area, buf);
        }

        // TODO: 优化，现在最多要遍历三次list，应该可以只遍历一次
        let left_item: Vec<_> = self
            .list
            .iter()
            .enumerate()
            .skip_while(|&(_, item)| {
                // 我希望在此处实现已经滚动过的元素不进行渲染
                //
                // 对于distace，他应当是在不滚动的情况下，当前元素顶部距离区域顶部的距离。
                //
                // 例如：
                // ```
                // +------------------+ <- distance = 0
                // |                  |
                // +------------------+
                // -------------------- <- divider
                // +------------------+ <- distance = 4
                // |                  |
                // +------------------+
                // ```
                //
                // 如果scroll = 0，则第一个元素会被渲染，
                // 如果scroll = 2，则第一个元素会被部分渲染，但我们依然跳过这个元素，并将这部分
                // 使用灰色背景填充，divider仍然会渲染。
                // 如果scroll = 4，则第一个元素不会被渲染，divider也不会，第二个元素会被渲染。
                if distance >= self.scroll {
                    return false;
                }
                distance += item.vertical_size() as usize;
                distance += DIVIDER_HEIGHT;
                true
            })
            .collect();

        let mut total_height: usize = self.list.iter().fold(0, |acc, item| {
            item.vertical_size() as usize + DIVIDER_HEIGHT + acc
        });
        // 注意，最后一个元素后面不需要divider
        total_height = total_height.saturating_sub(DIVIDER_HEIGHT);
        // TODO: 单独迭代一次，就为了计算total_height，实在是太浪费性能了

        // 如果总高度大于等于区域高度，则说明不需要渲染滚动条，否则需要单独留出渲染滚动条的空间
        let area = if total_height > area.height as usize {
            let [main_area, scroll_area] =
                Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(area);

            // 渲染滚动条
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(scrollbar::VERTICAL)
                .render(
                    scroll_area,
                    buf,
                    &mut ScrollbarState::new(total_height)
                        .viewport_content_length(main_area.height as usize)
                        .position(self.scroll()),
                );

            main_area
        } else {
            area
        };

        // scroll = 2, distance = 4
        // ```
        // +--------+ <-+
        // |        | <-+- scrolled area
        // +--------+ <- gap = 2，使用灰色背景填充
        // ---------- <- divider
        // +--------+
        // |        |
        // ```
        let gap: u16 = (distance.saturating_sub(self.scroll()) as u16).min(area.height);
        let mut area = if gap > 0 {
            let [fill, divider_area, remain] = Layout::vertical([
                Constraint::Length(gap - 1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .areas(area);
            Fill::new(VerticalList::<W>::NOT_ENOUGH_SPACE_BG).render(fill, buf);
            // render divider
            render_divider(divider_area, buf);
            remain
        } else {
            area
        };

        left_item
            .into_iter()
            .take_while(|&(idx, item)| {
                area = if item.vertical_size() + DIVIDER_HEIGHT as u16 <= area.height {
                    // fits divider and item
                    let [render_area, divider_area, remain] = Layout::vertical([
                        Constraint::Length(item.vertical_size()),
                        Constraint::Length(DIVIDER_HEIGHT as u16),
                        Constraint::Min(0),
                    ])
                    .areas(area);
                    item.render(render_area, buf, &mut (self.selected() == Some(idx)));
                    if idx + 1 < self.list.len() {
                        // render divider
                        render_divider(divider_area, buf);
                    }
                    remain
                } else if item.vertical_size() <= area.height {
                    // area.height == item.vertical_size()
                    // only fits item
                    let [render_area, _] = Layout::vertical([
                        Constraint::Length(item.vertical_size()),
                        Constraint::Min(0),
                    ])
                    .areas(area);
                    item.render(render_area, buf, &mut (self.selected() == Some(idx)));
                    return false;
                } else {
                    // cannot fits, fill the rest area with gray background
                    Fill::new(VerticalList::<W>::NOT_ENOUGH_SPACE_BG).render(area, buf);
                    return false;
                };
                true
            })
            .for_each(|_| ());
    }
}
