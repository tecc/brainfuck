mod command;
mod command_input;
mod runtime_data;
mod simple_text_block;
mod source_code;

use crate::interactive::runtime_data::RuntimeDataWidget;
use crate::interactive::simple_text_block::SimpleTextBlock;
use crate::{Instruction, LoadedInstruction, RuntimeContext, RuntimeContextU64, Script};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::{event, execute};
use ratatui::layout::Constraint::{Length, Max, Min};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use spin::{Mutex, RwLock};
use std::borrow::Cow;
use std::fmt::Display;
use std::io;
use std::io::{Cursor, Read};
use std::ops::Div;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tui_input::backend::crossterm::EventHandler;

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
macro_rules! widget_setter {
    (impl $( < $($impl_bounds:tt $(: $impl_extra_bound:tt $(+ $impl_extra_bounds:tt)* )?),* > )? $type_name:ident $( < $($type_bounds:tt),* > )? { $( $f_name:ident : $f_type:ty $( = $f_value: expr )? ),* }) => {
        impl$(< $($impl_bounds $(: $impl_extra_bound $(+ $impl_extra_bounds)* )?),* >)? $type_name $(< $($type_bounds)* > )? {
            $(
            pub fn $f_name(mut self, $f_name: $f_type) -> Self {
                self.$f_name = widget_setter!(OR { $f_name } { $($f_value)? });
                self
            }
            )*
        }
    };
    (OR { $default:expr } { } ) => {
        $default
    };
    (OR { $_default:expr } { $value:expr } ) => {
        $value
    };
}
use crate::interactive::command::{parse_command, Command, CommandPartState, CommandResult};
use crate::interactive::command_input::{CommandInput, CommandInputState, OwnedCommandResult};
use crate::interactive::source_code::SourceCode;
pub(self) use {block_widget, widget_setter};

type Cell = u64;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Activity {
    Normal,
    Command,
}

pub struct InteractiveState {
    pub should_quit: bool,
    pub execution_paused: bool,
    pub execution_clock_speed: Duration,
    pub last_cycle_time: Instant,
    pub last_executed_instruction: Option<LoadedInstruction>,
    pub frame_count: u128,
    pub script: Script,
    pub runtime_context: RuntimeContext<Cell>,

    pub activity: Activity,
    pub command_input: CommandInputState<Cell>,
    pub command_output: Vec<CommandOutput>,
}
pub struct CommandOutput {
    style: Style,
    message: Cow<'static, str>,
}
impl InteractiveState {
    fn cmd_info(&mut self, message: impl Display) {
        self.command_output.push(CommandOutput {
            style: styles::COMMAND_OUTPUT_INFO,
            message: Cow::Owned(message.to_string()),
        })
    }
    fn cmd_error(&mut self, message: impl Display) {
        self.command_output.push(CommandOutput {
            style: styles::COMMAND_OUTPUT_ERROR,
            message: format!("Error: {}", message).into(),
        })
    }
}
impl InteractiveState {
    fn execute(&mut self) {
        if !self.script.has_remaining_instructions() {
            return;
        }
        self.last_executed_instruction = self
            .script
            .instructions
            .get(self.script.instruction_pointer)
            .cloned();
        self.script.execute_instruction(&mut self.runtime_context);
        self.last_cycle_time = Instant::now();
    }
}
#[derive(Default)]
struct InteractiveIo {
    input: Mutex<Cursor<Vec<u8>>>,
    output: RwLock<Vec<u8>>,
}

pub fn interactive_runtime<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rt: Script,
) -> io::Result<()> {
    let mut io = Rc::new(InteractiveIo::default());

    let mut state = InteractiveState {
        should_quit: false,
        execution_paused: true,
        execution_clock_speed: Duration::from_millis(100),
        last_cycle_time: Instant::now(),
        last_executed_instruction: None,
        frame_count: 0,
        script: rt,
        runtime_context: RuntimeContext::new(
            {
                let io = io.clone();
                move || {
                    let mut buf = [0u8];
                    io.input
                        .lock()
                        .read_exact(&mut buf)
                        .expect("failed to read");
                    buf[0] as u64
                }
            },
            {
                let io = io.clone();
                move |value| {
                    io.output.write().push(value as u8);
                }
            },
        ),
        activity: Activity::Normal,
        command_input: CommandInputState::default(),
        command_output: Vec::new(),
    };

    // Since we use RuntimeContext<i128> for extended customisation,
    // we have to set these default values manually.
    // For executing code in a standard Brainfuck environment, just using RuntimeContext<u8> is fine.
    state.runtime_context.min_cell_value = 0;
    state.runtime_context.max_cell_value = u8::MAX as Cell;

    loop {
        let completed = terminal.draw(|frame| ui(frame, &mut state, &io));
        if event::poll(Duration::from_millis(20))? {
            let event = event::read()?;

            match state.activity {
                Activity::Normal => handle_event_normal(event, &mut state),
                Activity::Command => handle_event_command(event, &mut state),
            }
        }
        if !state.execution_paused && state.last_cycle_time.elapsed() > state.execution_clock_speed
        {
            state.execute();
        }
        if state.should_quit {
            return Ok(());
        }
    }
}

fn handle_event_normal(event: Event, state: &mut InteractiveState) {
    if let Event::Key(key) = event {
        let keydown = key.kind != KeyEventKind::Release;
        {
            match key.code {
                KeyCode::Char(ch) => match ch {
                    'q' => {
                        state.should_quit = true;
                    }
                    'n' if keydown => state.execute(),
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
}
fn execute_command(command: &Command<Cell>, state: &mut InteractiveState) {
    match command {
        Command::Start => {
            state.execution_paused = false;
        }
        Command::Pause => {
            state.execution_paused = true;
        }
        Command::SetInstructionPointer { idx } => {
            state.script.instruction_pointer = *idx;
        }
        Command::SetDataPointer { idx } => {
            state.runtime_context.data_pointer = *idx;
        }
        Command::SetData { idx, value } => {
            let idx = idx.unwrap_or(state.runtime_context.data_pointer);
            *state.runtime_context.get_cell(idx) = *value;
            state.runtime_context.fix_cell(idx);
        }
        Command::SetSpeed { speed } => {
            state.execution_clock_speed = *speed;
            state.cmd_info(format_args!(
                "Set speed to {}",
                humantime::format_duration(*speed)
            ))
        }
        Command::SetBounds { lower, upper } => {
            state.runtime_context.min_cell_value = lower.clone();
            state.runtime_context.max_cell_value = upper.clone();
        }
        Command::LoadScriptFromFile { path } => {
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => {
                    state.cmd_error(e);
                    return;
                }
            };
            state.script = Script::new(content);
            state.cmd_info(format_args!("Loaded file {}", path.display()));
        }
        Command::Quit => {
            state.should_quit = true;
        }
    }
}
fn handle_event_command(event: Event, state: &mut InteractiveState) {
    if let Event::Key(key) = event {
        let is_down = key.kind != KeyEventKind::Release;
        match key.code {
            KeyCode::Enter if is_down => {
                let command_string = state.command_input.input.value().to_string();
                state.command_input.input.reset();
                state
                    .command_input
                    .history
                    .push_front(command_string.clone());
                state.command_input.history_selected = None;
                state.command_input.current = OwnedCommandResult::empty();
                // We parse it without allowing autocompletes here
                // It may be wasteful architecturally (autocompletes and errors could be separate)
                // but I can't be bothered to implement that change.
                match parse_command(&command_string, false) {
                    CommandResult::Parsed { command, .. } => {
                        execute_command(&command, state);
                    }
                    CommandResult::CannotContinue { parts } => {
                        let mut errors = Vec::new();
                        parts.iter().for_each(|part| match &part.state {
                            CommandPartState::Invalid(Some(reason)) => {
                                errors.push(reason);
                            }
                            _ => {}
                        });

                        state.cmd_error(format_args!(
                            "could not parse command ({})",
                            itertools::join(errors.iter().map(|c| -> &str { c.as_ref() }), ", ")
                        ));
                    }
                    CommandResult::TooShort { .. } => {
                        state.cmd_error("command is not complete (and maybe has errors)");
                    }
                }
                state.activity = Activity::Normal;
            }
            KeyCode::Esc if is_down => {
                state.activity = Activity::Normal;
            }
            KeyCode::Up if is_down => {
                let history_len = state.command_input.history.len();
                if history_len == 0 {
                    return;
                }

                if let Some(value) = &mut state.command_input.history_selected {
                    *value += 1;
                    if *value >= history_len {
                        *value = history_len - 1;
                    }
                } else {
                    state.command_input.history_selected = Some(0);
                }

                if let Some(idx) = state.command_input.history_selected {
                    let target = state.command_input.history.get(idx).unwrap();
                    state.command_input.set_input_value(target.clone());
                }

                state.command_input.set_input_value_to_history()
            }
            KeyCode::Down if is_down => {
                let history_len = state.command_input.history.len();
                if history_len == 0 {
                    return;
                }

                if let Some(value) = &mut state.command_input.history_selected {
                    if *value == 0 {
                        state.command_input.history_selected = None;
                    } else {
                        *value -= 1;
                    }
                }

                state.command_input.set_input_value_to_history()
            }
            KeyCode::Tab if is_down => match &state.command_input.current.result {
                CommandResult::TooShort { parts, .. } | CommandResult::Parsed { parts, .. } => {
                    if let Some(last_part) = parts.last() {
                        match &last_part.state {
                            CommandPartState::Autocomplete { suggestion } => {
                                let str = state.command_input.input.value();
                                if suggestion.len() < str.len() {
                                    return;
                                }
                                let mut string = String::with_capacity(
                                    str.len() - last_part.len() + suggestion.len(),
                                );
                                string.push_str(str);
                                string.push_str(
                                    &suggestion[last_part.content().len()..suggestion.len()],
                                );
                                state.command_input.set_input_value(string);
                            }
                            _ => state.command_output.push(CommandOutput {
                                style: styles::COMMAND_OUTPUT_INFO,
                                message: Cow::from("cannot autocomplete :/"),
                            }),
                        }
                    }
                }
                _ => {}
            },
            _ => {
                if let Some(change) = state.command_input.input.handle_event(&event) {
                    if change.value {
                        state.command_input.current = OwnedCommandResult::parse(
                            state.command_input.input.value().to_string(),
                            true,
                        );
                    }
                }
                return;
            }
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

fn ui(frame: &mut Frame, state: &mut InteractiveState, io: &Rc<InteractiveIo>) {
    let vertical = Layout::vertical([Min(10), Length(3), Max(6)]);

    let [major_area, misc_area, command_area] = vertical.areas(frame.size());

    let major_layout = Layout::vertical([Min(3), Length(3), Min(0)]);
    let [instruction_area, output_area, data_area] = major_layout.areas(major_area);

    let instruction_block = Block::default()
        .title(" Source code ")
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL);
    let instruction_text_area = instruction_block.inner(instruction_area);
    frame.render_widget(instruction_block, instruction_area);

    frame.render_widget(
        SourceCode::new(&state.script.source)
            .current_instruction_pos(state.last_executed_instruction.map(|v| v.source_position))
            .current_instruction_style(styles::CURRENT_INSTRUCTION)
            .next_instruction_pos(state.script.loaded_instruction().map(|v| v.source_position))
            .next_instruction_style(styles::NEXT_INSTRUCTION)
            .instruction_style(styles::INSTRUCTION)
            .comment_style(styles::COMMENT),
        instruction_text_area,
    );

    let output_data = io.output.read();
    let output_block = SimpleTextBlock::new(String::from_utf8_lossy(&output_data))
        .title(" Output ")
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL);
    frame.render_widget(output_block, output_area);

    let data = RuntimeDataWidget::new()
        .title(" Data ")
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL);
    frame.render_stateful_widget(data, data_area, state);

    let misc_layout = Layout::horizontal([Min(10), Length(16), Length(16), Length(16), Length(32)]);
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

    let command_layout = Layout::horizontal([Min(0), Min(0)]);
    let [command_line_area, command_output_area] = command_layout.areas(command_area);

    let mut command_line_block = Block::new().title(" Command line ").borders(Borders::ALL);
    let command_input_area = command_line_block.inner(command_line_area);
    if state.activity == Activity::Command {
        command_line_block = command_line_block
            .border_style(styles::ACTIVE_BLOCK)
            .title_style(styles::ACTIVE_BLOCK);
    }
    frame.render_widget(command_line_block, command_line_area);

    let visual_scroll = state
        .command_input
        .input
        .visual_scroll(command_input_area.width as usize);
    let command_input: CommandInput<Cell> = CommandInput::new()
        .base_style(styles::COMMAND_BASE)
        .ignored_style(styles::COMMAND_IGNORED)
        .error_style(styles::COMMAND_ERROR)
        .error_comment_style(styles::COMMAND_ERROR_COMMENT)
        .suggestion_style(styles::COMMAND_SUGGESTION)
        .scroll((0, visual_scroll as u16));

    if state.activity == Activity::Command {
        let visual_cursor = state.command_input.input.visual_cursor() - visual_scroll;
        frame.set_cursor(
            command_input_area.x + visual_cursor as u16,
            command_input_area.y,
        );
    }
    frame.render_stateful_widget(command_input, command_input_area, &mut state.command_input);

    let command_output_block = Block::new().title(" Command output ").borders(Borders::ALL);
    let command_output_text_area = command_output_block.inner(command_output_area);
    frame.render_widget(command_output_block, command_output_area);

    let mut command_output = Text::default();
    for message in state.command_output.iter().rev() {
        command_output.push_line(
            Line::default().spans([Span::styled(message.message.clone(), message.style)]),
        )
    }
    frame.render_widget(Paragraph::new(command_output), command_output_text_area);

    state.frame_count += 1;
}

mod styles {
    use ratatui::style::{Color, Modifier, Style};

    pub const VALUE: Style = Style::new().fg(Color::LightYellow);
    pub const VALUE_EXTRA: Style = Style::new();
    pub const VALUE_MODIFIER: Style = Style::new().add_modifier(Modifier::ITALIC);
    pub const NEXT_INSTRUCTION: Style = Style::new().fg(Color::LightCyan);
    pub const CURRENT_INSTRUCTION: Style = Style::new()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    pub const INSTRUCTION: Style = Style::new();
    pub const COMMENT: Style = Style::new()
        .add_modifier(Modifier::DIM)
        .add_modifier(Modifier::ITALIC);

    pub const COMMAND_OUTPUT_INFO: Style = Style::new().fg(Color::LightBlue);
    pub const COMMAND_OUTPUT_ERROR: Style = Style::new().fg(Color::LightRed);

    pub const COMMAND_BASE: Style = Style::new();
    pub const COMMAND_IGNORED: Style = Style::new().add_modifier(Modifier::DIM);
    pub const COMMAND_ERROR: Style = Style::new()
        .fg(Color::LightRed)
        .add_modifier(Modifier::BOLD);
    pub const COMMAND_ERROR_COMMENT: Style = Style::new().fg(Color::LightRed);
    pub const COMMAND_SUGGESTION: Style = Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    pub const ACTIVE_BLOCK: Style = Style::new().fg(Color::Yellow);
}
