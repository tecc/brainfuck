use crate::interactive::{block_widget, Cell};
use ratatui::prelude::*;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Table};

pub struct RuntimeDataWidget<'a> {
    block: Block<'a>,
}
impl<'a> RuntimeDataWidget<'a> {
    pub fn new() -> Self {
        Self {
            block: Block::new(),
        }
    }
}
block_widget!(RuntimeDataWidget => block);
impl<'a> StatefulWidget for RuntimeDataWidget<'a> {
    type State = super::InteractiveState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let content_area = self.block.inner(area);
        self.block.render(area, buf);

        "[This feature doesn't yet exist :(]".render(content_area, buf)
    }
}
