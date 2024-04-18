use crate::interactive::block_widget;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::Widget;
use ratatui::text::Text;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct SimpleTextBlock<'a> {
    block: Block<'a>,
    content: Text<'a>,
}
impl<'a> SimpleTextBlock<'a> {
    pub fn new(content: impl Into<Text<'a>>) -> Self {
        Self {
            content: content.into(),
            block: Block::new(),
        }
    }
}
impl<'a> Widget for SimpleTextBlock<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let inner_area = self.block.inner(area);
        self.block.render(area, buf);
        Paragraph::new(self.content).render(inner_area, buf);
    }
}
block_widget!(SimpleTextBlock => block);
