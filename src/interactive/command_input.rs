use crate::interactive::command::{parse_command, CommandPartState, CommandResult};
use crate::interactive::widget_setter;
use crate::CellType;
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::mem;
use std::pin::Pin;
use tui_input::Input;

pub struct CommandInput<T: CellType> {
    _data: PhantomData<T>,
    base_style: Style,
    ignored_style: Style,
    error_style: Style,
    error_comment_style: Style,
    suggestion_style: Style,
    cursor_style: Style,
    scroll: (u16, u16),
}
impl<T> CommandInput<T>
where
    T: CellType,
{
    pub fn new() -> Self {
        Self {
            _data: PhantomData::default(),
            base_style: Style::new(),
            ignored_style: Style::new(),
            error_style: Style::new(),
            error_comment_style: Style::new(),
            suggestion_style: Style::new(),
            cursor_style: Style::new(),
            scroll: (0, 0),
        }
    }
}
widget_setter! { impl<T: CellType> CommandInput<T> {
    base_style: Style,
    ignored_style: Style,
    error_style: Style,
    error_comment_style: Style,
    suggestion_style: Style,
    cursor_style: Style,
    scroll: (u16, u16)
} }
pub struct CommandInputState<T: CellType> {
    pub input: tui_input::Input,
    pub current: OwnedCommandResult<T>,
    pub history: VecDeque<String>,
    pub history_selected: Option<usize>,
}
impl<T> Default for CommandInputState<T>
where
    T: CellType,
{
    fn default() -> Self {
        Self {
            input: Default::default(),
            history: VecDeque::new(),
            history_selected: None,
            current: OwnedCommandResult::empty(),
        }
    }
}
impl<T> CommandInputState<T>
where
    T: CellType,
{
    pub fn set_input_value(&mut self, value: String) {
        self.input = Input::new(value.clone());
        self.current = OwnedCommandResult::parse(value, true);
    }
    pub fn set_input_value_to_history(&mut self) {
        if let Some(idx) = self.history_selected {
            let target = self.history.get(idx).unwrap();
            self.set_input_value(target.clone());
        } else {
            self.set_input_value(String::new())
        }
    }
}

pub struct OwnedCommandResult<T: CellType> {
    source: *mut Cow<'static, str>, // I really like Cows. Moo.
    // TODO: Make this only accessible through references.
    pub result: CommandResult<'static, T>,
}

impl<T: CellType> Drop for OwnedCommandResult<T> {
    fn drop(&mut self) {
        mem::drop(mem::replace(
            &mut self.result,
            CommandResult::TooShort {
                parts: Vec::new(),
                message: None,
            },
        ));
        unsafe {
            mem::drop(Box::from_raw(self.source));
        }
    }
}

impl<T: CellType> OwnedCommandResult<T> {
    pub fn empty() -> Self {
        Self {
            source: unsafe { Box::into_raw(Box::new(Cow::Borrowed(""))) },
            result: CommandResult::TooShort {
                parts: Vec::new(),
                message: None,
            },
        }
    }
    pub fn parse(data: String, autocomplete: bool) -> Self {
        let mut owned = Self {
            source: unsafe { Box::into_raw(Box::new(data.into())) },
            result: CommandResult::TooShort {
                parts: Vec::new(),
                message: None,
            },
        };
        owned.result = unsafe { parse_command(&*owned.source, autocomplete) };
        owned
    }
    pub fn source(&self) -> &Cow<'static, str> {
        unsafe { &*self.source }
    }
}

impl<T> StatefulWidget for CommandInput<T>
where
    T: CellType,
{
    type State = CommandInputState<T>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let input = state.input.value();

        let parts = match &state.current.result {
            CommandResult::Parsed { parts, .. } => parts,
            CommandResult::CannotContinue { parts } => parts,
            CommandResult::TooShort { parts, .. } => parts,
        };
        let mut main_line = Line::default();

        let mut previous_end = 0;
        let mut main_line_length = 0;

        struct Message<'a> {
            start: usize,
            style: Style,
            content: Cow<'a, str>,
        }
        let mut messages = Vec::<Message>::new();

        for part in parts {
            if previous_end < part.start {
                main_line.push_span(Span::styled(
                    &state.current.source()[previous_end..part.start],
                    self.ignored_style,
                ));
                main_line_length = main_line.width();
            }
            match &part.state {
                CommandPartState::Ok => {
                    main_line.push_span(Span::styled(part.content(), self.base_style))
                }
                CommandPartState::Ignored => {
                    main_line.push_span(Span::styled(part.content(), self.ignored_style))
                }
                CommandPartState::Invalid(reason) => {
                    let message = reason
                        .as_ref()
                        .unwrap_or(&Cow::Borrowed("invalid part (no reason provided)"));
                    messages.push(Message {
                        start: main_line_length,
                        style: self.error_comment_style,
                        content: message.clone(),
                    });
                    main_line.push_span(Span::styled(part.content(), self.error_style));
                }
                CommandPartState::Autocomplete { suggestion } => {
                    main_line.push_span(Span::styled(part.content(), self.base_style));
                    if suggestion.len() > part.len() {
                        main_line.push_span(Span::styled(
                            &suggestion[part.len()..suggestion.len()],
                            self.suggestion_style,
                        ));
                    }
                }
            }
            previous_end = part.end;
            main_line_length = main_line.width();
        }
        if previous_end < input.len() {
            main_line.push_span(Span::styled(
                &input[previous_end..input.len()],
                self.ignored_style,
            ));
            main_line_length = main_line.width();
        }
        match &state.current.result {
            CommandResult::TooShort {
                message: Some(message),
                ..
            } => messages.push(Message {
                start: main_line_length,
                style: self.error_comment_style,
                content: { *message }.into(),
            }),
            _ => {}
        }

        let mut temp_empty = String::with_capacity(main_line_length + 2);
        for _ in 0..main_line_length {
            temp_empty.push(' ');
        }
        temp_empty.push_str(" │");
        // Note: We iterate over the messages in reverse order so we get the latest messages last

        let mut message_lines = Vec::new();
        for message in messages.iter().rev() {
            let mut line = Line::default();
            let mut previous_prefix_start = None;
            for prefix in messages.iter() {
                if prefix.start >= message.start {
                    break;
                }
                // NOTE(tecc): I'm really not confident in this.
                line.push_span(Span::styled(
                    &temp_empty[temp_empty.len() - prefix.start - 3..],
                    prefix.style,
                ));
                previous_prefix_start = Some(prefix.start);
            }
            let diff = message.start - previous_prefix_start.map(|v| v + 1).unwrap_or(0);
            if diff != 0 {
                line.push_span(Span::styled(&temp_empty[..diff], message.style));
            }
            line.push_span(Span::styled("└ ", message.style));
            line.push_span(Span::styled(message.content.clone(), message.style));
            message_lines.push(line);
        }

        let mut text = Text::default();
        text.push_line(main_line);
        message_lines
            .into_iter()
            .for_each(|line| text.push_line(line));

        Paragraph::new(text).scroll(self.scroll).render(area, buf);
    }
}
