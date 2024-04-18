use crate::interactive::block_widget;
use ratatui::layout::Alignment;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders};

pub enum Command {
    Pause,
    SetSpeed(),
}

pub struct CommandInput<'a> {
    block: Block<'a>,
}
block_widget!(CommandInput => block);
