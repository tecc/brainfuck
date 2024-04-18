mod command;
mod runtime_data;
mod simple_text_block;

use crate::interactive::runtime_data::RuntimeDataWidget;
use crate::interactive::simple_text_block::SimpleTextBlock;
use crate::{Instruction, LoadedInstruction, RuntimeContext, RuntimeOptions, Script};
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use spin::RwLock;
use std::io;
use std::ops::Div;
use std::rc::Rc;
use std::time::{Duration, Instant};

macro_rules! block_widget {
    ($ty:ident => $block:ident) => {
        impl<'a> $ty<'a> {
            pub fn title(mut self, title: impl Into<ratatui::widgets::block::Title<'a>>) -> Self {
                self.$block = self.$block.title(title);
                self
            }
            pub fn title_alignment(mut self, alignment: ratatui::layout::Alignment) -> Self {
                self.$block = self.$block.title_alignment(alignment);
                self
            }
            pub fn borders(mut self, borders: ratatui::widgets::Borders) -> Self {
                self.$block = self.$block.borders(borders);
                self
            }
            pub fn padding(mut self, padding: ratatui::widgets::Padding) -> Self {
                self.$block = self.$block.padding(padding);
                self
            }
        }
    };
}
pub(self) use block_widget;

#[derive(Copy, Clone)]
enum Activity {
    Normal,
    Command,
}

pub struct InteractiveState {
    pub execution_paused: bool,
    pub execution_clock_speed: Duration,
    pub last_cycle_time: Instant,
    pub last_executed_instruction: Option<LoadedInstruction>,
    pub frame_count: u128,
    pub script: Script,
    pub runtime_context: RuntimeContext,

    pub activity: Activity,
}

pub fn interactive_runtime<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rt: Script,
) -> io::Result<()> {
    let mut state = InteractiveState {
        execution_paused: true,
        execution_clock_speed: Duration::from_millis(100),
        last_cycle_time: Instant::now(),
        last_executed_instruction: None,
        frame_count: 0,
        script: rt,
        runtime_context: RuntimeContext::new(),
        activity: Activity::Normal,
    };

    let mut output = Rc::new(RwLock::new(Vec::new()));
    state.script.options.write = Box::new({
        let output = output.clone();
        move |value| output.write().push(value)
    });

    loop {
        let completed = terminal.draw(|frame| {
            let vertical = Layout::vertical([Min(3), Length(3), Length(5), Length(3)]);

            let [major_area, misc_area, output_area, command_line] = vertical.areas(frame.size());

            let major_layout = Layout::vertical([Min(0), Length(5), Min(0)]);
            let [instruction_area, output_area, data_area] = major_layout.areas(major_area);

            let instruction_block = Block::default()
                .title(" Source code ")
                .padding(Padding::horizontal(1))
                .borders(Borders::ALL);
            let instruction_text_area = instruction_block.inner(instruction_area);
            frame.render_widget(instruction_block, instruction_area);

            let mut lines = vec![];
            let mut spans = vec![];
            let mut span_start = 0;
            let mut span_is_instruction = false;
            let mut line_offset = 0;
            let amount_of_lines = state.script.source.lines().count();
            let viewable_area_size = { instruction_area.height as f64 * 0.8 }.ceil() as usize;
            let viewable_areas = amount_of_lines.div_ceil(viewable_area_size);
            for (i, ch) in state.script.source.char_indices() {
                let current_span_style = if span_is_instruction {
                    styles::IRRELEVANT_INSTRUCTION
                } else {
                    styles::COMMENT
                };
                if let Some(last_executed) = state.last_executed_instruction.as_ref() {
                    if last_executed.source_position == i {
                        spans.push(Span::styled(
                            &state.script.source[span_start..i],
                            current_span_style,
                        ));
                        spans.push(Span::styled(ch.to_string(), styles::EXECUTED_INSTRUCTION));
                        span_start = i + 1;
                        if lines.len() >= viewable_area_size {
                            let area = lines.len().div(viewable_area_size);
                            line_offset += viewable_area_size * area;
                        }
                        continue;
                    }
                }
                if let Some(instruction) = state
                    .script
                    .instructions
                    .get(state.script.instruction_pointer)
                {
                    if instruction.source_position == i {
                        spans.push(Span::styled(
                            &state.script.source[span_start..i],
                            current_span_style,
                        ));
                        spans.push(Span::styled(ch.to_string(), styles::NEXT_INSTRUCTION));
                        span_start = i + 1;
                        continue;
                    }
                }
                if ch == '\n' {
                    spans.push(Span::styled(
                        &state.script.source[span_start..i],
                        current_span_style,
                    ));
                    span_start = i + 1;
                    let line = Line::default().spans(spans.drain(..));
                    lines.push(line);
                    continue;
                }
                let should_be_instruction = Instruction::from_char(ch) != None;
                if span_is_instruction != should_be_instruction {
                    spans.push(Span::styled(
                        &state.script.source[span_start..i],
                        current_span_style,
                    ));
                    span_start = i;
                    span_is_instruction = should_be_instruction;
                }
            }
            spans.push(Span::styled(
                &state.script.source[span_start..state.script.source.len()],
                if span_is_instruction {
                    styles::IRRELEVANT_INSTRUCTION
                } else {
                    styles::COMMENT
                },
            ));
            lines.push(Line::default().spans(spans.drain(..)));

            frame.render_widget(
                Paragraph::new(lines).scroll((line_offset as u16, 0)),
                instruction_text_area,
            );

            let output_data = output.read();
            let output_block = SimpleTextBlock::new(String::from_utf8_lossy(&output_data))
                .title(" Output ")
                .padding(Padding::horizontal(1))
                .borders(Borders::ALL);
            frame.render_widget(output_block, output_area);

            let data = RuntimeDataWidget::new()
                .title(" Data ")
                .padding(Padding::horizontal(1))
                .borders(Borders::ALL);
            frame.render_stateful_widget(data, data_area, &mut state);

            let misc_layout =
                Layout::horizontal([Min(10), Length(16), Length(16), Length(16), Length(32)]);
            let [input_area, state_area, frame_counter_area, cycle_counter_area, speed_area] =
                misc_layout.areas(misc_area);

            let state_text = {
                if !state.script.has_remaining_instructions() {
                    Span::styled("Finished", Style::new().fg(Color::LightRed).bold())
                } else if state.execution_paused {
                    Span::styled("Paused", Style::new().fg(Color::LightCyan))
                } else {
                    Span::styled("Running", Style::new().fg(Color::LightGreen))
                }
            };

            let state_block = SimpleTextBlock::new(state_text)
                .title("State")
                .borders(Borders::ALL);
            frame.render_widget(state_block, state_area);

            let frame_counter =
                SimpleTextBlock::new(Span::styled(state.frame_count.to_string(), styles::VALUE))
                    .title("Frame")
                    .borders(Borders::ALL);
            frame.render_widget(frame_counter, frame_counter_area);

            let mut line = Line::default();
            line.push_span(Span::styled(state.script.cycles.to_string(), styles::VALUE));

            let cycle_counter = SimpleTextBlock::new(line)
                .title("Cycle")
                .borders(Borders::ALL);
            frame.render_widget(cycle_counter, cycle_counter_area);

            let speed = humantime::format_duration(state.execution_clock_speed).to_string();
            let mut line = Line::default();
            line.push_span(Span::styled(speed, styles::VALUE));
            line.push_span(Span::styled(" per instruction", styles::VALUE_EXTRA));
            let speed_block = SimpleTextBlock::new(line)
                .title("Speed")
                .borders(Borders::ALL);
            frame.render_widget(speed_block, speed_area);

            state.frame_count += 1;
        });
        let mut execute = |state: &mut InteractiveState| {
            if !state.script.has_remaining_instructions() {
                return;
            }
            state.last_executed_instruction = state
                .script
                .instructions
                .get(state.script.instruction_pointer)
                .cloned();
            state.script.execute_instruction(&mut state.runtime_context);
            state.last_cycle_time = Instant::now();
        };
        if event::poll(Duration::from_millis(20))? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                let keydown = key.kind != KeyEventKind::Release;
                match key.code {
                    KeyCode::Char(ch) => match ch {
                        'q' => return Ok(()),
                        'n' if keydown => execute(&mut state),
                        ' ' if keydown => {
                            state.execution_paused = !state.execution_paused;
                        }
                        ':' if keydown => {
                            state.activity = Activity::Command;
                        }
                        _ => {}
                    },
                    KeyCode::Up if keydown => {
                        if let Some(speed) = state
                            .execution_clock_speed
                            .checked_add(speed_diff(key.modifiers))
                        {
                            state.execution_clock_speed = speed;
                        }
                    }
                    KeyCode::Down if keydown => {
                        if let Some(speed) = state
                            .execution_clock_speed
                            .checked_sub(speed_diff(key.modifiers))
                        {
                            state.execution_clock_speed = speed;
                        }
                    }
                    _ => {}
                }
            }
        }
        if !state.execution_paused && state.last_cycle_time.elapsed() > state.execution_clock_speed
        {
            execute(&mut state);
        }
    }
}

fn speed_diff(key_modifiers: KeyModifiers) -> Duration {
    let shift = key_modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key_modifiers.contains(KeyModifiers::CONTROL);
    let alt = key_modifiers.contains(KeyModifiers::ALT);

    Duration::from_millis(if alt {
        25
    } else if shift {
        50
    } else if ctrl {
        200
    } else {
        100
    })
}

mod styles {
    use ratatui::style::{Color, Modifier, Style};

    pub const VALUE: Style = Style::new().fg(Color::LightYellow);
    pub const VALUE_EXTRA: Style = Style::new();
    pub const VALUE_MODIFIER: Style = Style::new().add_modifier(Modifier::ITALIC);
    pub const NEXT_INSTRUCTION: Style = Style::new().fg(Color::LightCyan);
    pub const EXECUTED_INSTRUCTION: Style = Style::new()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    pub const IRRELEVANT_INSTRUCTION: Style = Style::new();
    pub const COMMENT: Style = Style::new()
        .add_modifier(Modifier::DIM)
        .add_modifier(Modifier::ITALIC);
}
