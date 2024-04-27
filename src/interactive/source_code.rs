use std::borrow::Cow;
use std::ops::Div;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::interactive::{styles, widget_setter, InteractiveState};
use crate::Instruction;

pub struct SourceCode<'a> {
    current_instruction_style: Style,
    next_instruction_style: Style,
    instruction_style: Style,
    comment_style: Style,

    code: Cow<'a, str>,
    current_instruction_pos: Option<usize>,
    next_instruction_pos: Option<usize>,
}
impl<'a> SourceCode<'a> {
    pub fn new(code: impl Into<Cow<'a, str>>) -> Self {
        Self {
            current_instruction_style: Style::default(),
            next_instruction_style: Style::default(),
            instruction_style: Style::default(),
            comment_style: Style::default(),
            code: code.into(),
            current_instruction_pos: None,
            next_instruction_pos: None,
        }
    }
}
widget_setter! { impl<'a> SourceCode<'a> {
    current_instruction_style: Style,
    next_instruction_style: Style,
    instruction_style: Style,
    comment_style: Style,
    current_instruction_pos: Option<usize>,
    next_instruction_pos: Option<usize>
} }

impl<'a> Widget for SourceCode<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![];
        let mut spans = vec![];
        let mut span_start = 0;
        let mut span_is_instruction = false;
        let mut line_offset = 0;
        let amount_of_lines = self.code.lines().count();
        let viewable_area_y_size = { area.height as f64 * 0.8 }.ceil() as usize;
        let viewable_areas_y = if viewable_area_y_size == 0 {
            1
        } else {
            amount_of_lines.div_ceil(viewable_area_y_size)
        };

        for (i, ch) in self.code.char_indices() {
            let current_span_style = if span_is_instruction {
                self.instruction_style
            } else {
                self.comment_style
            };
            if let Some(last_executed) = self.current_instruction_pos {
                if last_executed == i {
                    spans.push(Span::styled(&self.code[span_start..i], current_span_style));
                    spans.push(Span::styled(ch.to_string(), self.current_instruction_style));
                    span_start = i + 1;
                    if lines.len() >= viewable_area_y_size {
                        let area = lines.len().div(viewable_area_y_size);
                        line_offset += viewable_area_y_size * area;
                    }
                    continue;
                }
            }
            if let Some(instruction) = self.next_instruction_pos {
                if instruction == i {
                    spans.push(Span::styled(&self.code[span_start..i], current_span_style));
                    spans.push(Span::styled(ch.to_string(), self.next_instruction_style));
                    span_start = i + 1;
                    continue;
                }
            }
            if ch == '\n' {
                spans.push(Span::styled(&self.code[span_start..i], current_span_style));
                span_start = i + 1;
                let line = Line::default().spans(spans.drain(..));
                lines.push(line);
                continue;
            }
            let should_be_instruction = Instruction::from_char(ch) != None;
            if span_is_instruction != should_be_instruction {
                spans.push(Span::styled(&self.code[span_start..i], current_span_style));
                span_start = i;
                span_is_instruction = should_be_instruction;
            }
        }
        spans.push(Span::styled(
            &self.code[span_start..self.code.len()],
            if span_is_instruction {
                self.instruction_style
            } else {
                self.comment_style
            },
        ));
        lines.push(Line::default().spans(spans.drain(..)));

        Paragraph::new(lines).render(area, buf)
    }
}
