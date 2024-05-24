use crate::interactive::block_widget;
use crate::CellType;
use ratatui::layout::Alignment;
use ratatui::text::Text;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::fs::DirEntry;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use uncased::UncasedStr;

#[derive(Clone)]
pub enum Command<T: CellType> {
    Start,
    Pause,
    SetInstructionPointer { idx: usize },
    SetDataPointer { idx: usize },
    SetData { idx: Option<usize>, value: T },
    SetSpeed { speed: Duration },
    SetBounds { lower: T, upper: T },
    LoadScriptFromFile { path: PathBuf },
    Quit,
}
#[derive(Clone)]
pub enum CommandResult<'a, T: CellType> {
    Parsed {
        parts: Vec<CommandPart<'a>>,
        command: Command<T>,
    },
    CannotContinue {
        parts: Vec<CommandPart<'a>>,
    },
    TooShort {
        parts: Vec<CommandPart<'a>>,
        message: Option<&'a str>,
    },
}
#[derive(Clone)]
pub struct CommandPart<'a> {
    source: &'a str,
    pub start: usize,
    pub end: usize,
    pub state: CommandPartState<'a>,
}
impl<'a> CommandPart<'a> {
    pub fn ok(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            end: source.len(),
            state: CommandPartState::Ok,
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn content(&self) -> &'a str {
        &self.source[self.start..self.end]
    }
    pub fn content_uncased(&self) -> &'a UncasedStr {
        UncasedStr::new(self.content())
    }

    pub fn empty_at_end(&self) -> Self {
        Self {
            source: self.source,
            start: self.end,
            end: self.end,
            state: CommandPartState::Ignored,
        }
    }

    pub fn trim_start(&mut self) {
        if let Some(offset) = self.content().find(|v: char| !v.is_whitespace()) {
            self.start += offset;
        } else {
            self.start = self.end;
        }
    }

    pub fn split_whitespace(&self) -> (Self, Option<Self>) {
        let mut first = Self {
            source: self.source,
            start: self.start,
            end: self.end,
            state: CommandPartState::Ok,
        };
        if let Some(split_point) = self.content().find(char::is_whitespace) {
            first.end = first.start + split_point;
            let mut second = Self {
                source: self.source,
                start: self.start + split_point,
                end: self.end,
                state: CommandPartState::Ignored,
            };
            second.trim_start();
            if second.len() > 0 {
                return (first, Some(second));
            }
        }
        (first, None)
    }

    fn autocomplete_uncased(&mut self, choices: &[&'a str]) {
        for choice in choices {
            if UncasedStr::new(choice).starts_with(self.content()) {
                self.state = CommandPartState::Autocomplete {
                    suggestion: { *choice }.into(),
                };
                return;
            }
        }
        // self.state = CommandPartState::Invalid(None);
    }
}
impl<'a> Display for CommandPart<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.content(), f)
    }
}

#[derive(Clone)]
pub enum CommandPartState<'a> {
    Ok,
    Ignored,
    Autocomplete { suggestion: Cow<'a, str> },
    Invalid(Option<Cow<'a, str>>),
}

pub enum TargetVariable {
    InstructionPointer,
    DataPointer,
    Data,
    Speed,
    Bound,
}
impl TargetVariable {
    fn from_str(s: impl for<'a> PartialEq<&'a str>) -> Option<Self> {
        if s == "instruction pointer" || s == "ip" {
            return Some(Self::InstructionPointer);
        }
        if s == "data pointer" || s == "dp" {
            return Some(Self::DataPointer);
        }
        if s == "data" || s == "d" {
            return Some(Self::Data);
        }
        if s == "speed" {
            return Some(Self::Speed);
        }
        if s == "bound" {
            return Some(Self::Bound);
        }

        None
    }
}
const AUTOCOMPLETE_COMMAND: &[&str] = &["start", "pause", "set", "load", "quit", "execute"];
const AUTOCOMPLETE_SET_VARIABLE: &[&str] = &[
    "instruction pointer",
    "ip",
    "data pointer",
    "dp",
    "data",
    "d",
    "speed",
    "bound",
];
const EQUALS: &str = "=";
const AUTOCOMPLETE_EQUAL: &[&str] = &[EQUALS];
pub fn parse_command<T: CellType>(cmd_str: &str, autocomplete: bool) -> CommandResult<T> {
    if cmd_str.len() < 1 {
        return CommandResult::TooShort {
            parts: Vec::new(),
            message: None,
        };
    }
    let mut parts = Vec::new();

    let main_part = CommandPart::ok(cmd_str);
    let (mut command_part, remaining) = main_part.split_whitespace();

    if command_part.content_uncased() == "start" {
        parts.push(command_part);

        return CommandResult::Parsed {
            command: Command::Start,
            parts,
        };
    }
    if command_part.content_uncased() == "pause" {
        parts.push(command_part);

        return CommandResult::Parsed {
            command: Command::Pause,
            parts,
        };
    }

    if command_part.content_uncased() == "set" {
        parts.push(command_part);

        let Some(remaining) = remaining else {
            return CommandResult::TooShort {
                parts,
                message: Some("variable name required".into()),
            };
        };

        let (mut variable_part, remaining) = remaining.split_whitespace();

        let Some(variable) = TargetVariable::from_str(variable_part.content_uncased()) else {
            variable_part.state = CommandPartState::Invalid(Some(
                format!("unknown variable '{}'", variable_part).into(),
            ));
            if autocomplete {
                variable_part.autocomplete_uncased(AUTOCOMPLETE_SET_VARIABLE);
            }
            parts.push(variable_part);
            if let Some(remaining) = remaining {
                parts.push(remaining);
            }
            return CommandResult::CannotContinue { parts };
        };

        parts.push(variable_part);

        let Some(mut remaining) = remaining else {
            return CommandResult::TooShort {
                parts,
                message: Some(match variable {
                    TargetVariable::Data => "expecting index or =",
                    _ => "expecting =",
                }),
            };
        };

        match variable {
            TargetVariable::Data => {
                let (mut idx_part, Some(mut remaining)) = remaining.split_whitespace() else {
                    remaining.autocomplete_uncased(AUTOCOMPLETE_EQUAL);
                    parts.push(remaining);
                    return CommandResult::CannotContinue { parts };
                };
                let idx: Option<usize> = if idx_part.content() == EQUALS {
                    parts.push(idx_part);
                    None
                } else {
                    let Ok(num) = parse_number(&mut idx_part) else {
                        parts.push(idx_part);
                        return CommandResult::CannotContinue { parts };
                    };

                    parts.push(idx_part);

                    let is_correct = expect_equals_part(&mut parts, remaining, autocomplete);
                    if !is_correct.0 {
                        return CommandResult::CannotContinue { parts };
                    }
                    if let Some(remaining_inner) = is_correct.1 {
                        remaining = remaining_inner;
                    } else {
                        return CommandResult::TooShort {
                            parts,
                            message: Some("expecting value"),
                        };
                    }

                    Some(num)
                };

                let (mut value_part, remaining) = remaining.split_whitespace();
                let Ok(value) = parse_number::<T>(&mut value_part) else {
                    parts.push(value_part);
                    if let Some(remaining) = remaining {
                        parts.push(remaining)
                    }
                    return CommandResult::CannotContinue { parts };
                };
                parts.push(value_part);

                if let Some(remaining) = remaining {
                    parts.push(remaining);
                }
                return CommandResult::Parsed {
                    command: Command::SetData { idx, value },
                    parts,
                };
            }
            variable => {
                let (is_correct, mut remaining) =
                    expect_equals_part(&mut parts, remaining, autocomplete);
                if !is_correct {
                    return CommandResult::CannotContinue { parts };
                }
                let Some(mut remaining) = remaining else {
                    return CommandResult::TooShort {
                        parts,
                        message: Some("expecting value"),
                    };
                };

                match variable {
                    TargetVariable::InstructionPointer => {
                        let (mut value_part, remaining) = remaining.split_whitespace();
                        let Ok(idx) = parse_number::<usize>(&mut value_part) else {
                            parts.push(value_part);
                            return CommandResult::CannotContinue { parts };
                        };
                        if let Some(remaining) = remaining {
                            parts.push(remaining);
                        }
                        return CommandResult::Parsed {
                            parts,
                            command: Command::SetInstructionPointer { idx },
                        };
                    }
                    TargetVariable::DataPointer => {
                        let (mut value_part, remaining) = remaining.split_whitespace();
                        let Ok(idx) = parse_number::<usize>(&mut value_part) else {
                            parts.push(value_part);
                            return CommandResult::CannotContinue { parts };
                        };
                        if let Some(remaining) = remaining {
                            parts.push(remaining);
                        }
                        return CommandResult::Parsed {
                            parts,
                            command: Command::SetDataPointer { idx },
                        };
                    }
                    TargetVariable::Data => {
                        unreachable!("`variable != Data` should be guaranteed by outer match")
                    }
                    TargetVariable::Speed => {
                        remaining.state = CommandPartState::Ok;
                        let Ok(speed) = parse_duration(&mut remaining) else {
                            parts.push(remaining);
                            return CommandResult::CannotContinue { parts };
                        };
                        parts.push(remaining);
                        return CommandResult::Parsed {
                            parts,
                            command: Command::SetSpeed { speed },
                        };
                    }
                    TargetVariable::Bound => {
                        remaining.state = CommandPartState::Ok;
                        todo!("Parsing of bounds");
                        /*return CommandResult::Parsed {
                            parts,
                            command: Command::SetBounds { ..cargo run },
                        };*/
                    }
                }
            }
        }
    }

    if command_part.content_uncased() == "load" {
        command_part.state = CommandPartState::Ok;
        parts.push(command_part);
        let Some(mut file_part) = remaining else {
            return CommandResult::TooShort {
                parts,
                message: Some("expected file name"),
            };
        };

        file_part.state = CommandPartState::Ok;
        let file_path = std::env::current_dir()
            .map(|v| v.join(file_part.content()))
            .unwrap_or_else(|_| PathBuf::from(file_part.content()));

        fn try_autocomplete(
            path: &Path,
            query_direct: bool,
        ) -> io::Result<Option<Cow<'static, str>>> {
            let Some(file_name) = path.file_name() else {
                return if let Some(parent) = path.parent() {
                    let mut read_dir = parent.read_dir()?;
                    let Some(entry) = read_dir.next() else {
                        return Ok(None);
                    };
                    let entry = entry?;
                    let entry = entry.file_name().to_string_lossy().into_owned();
                    Ok(Some(entry.into()))
                } else {
                    Ok(None)
                };
            };
            let file_name = file_name.to_string_lossy();

            let (target, start_with) = if query_direct {
                (Some(path), "")
            } else {
                (path.parent(), file_name.as_ref())
            };
            if let Some(parent) = target {
                let dir = parent.read_dir()?;

                let mut found_suggestion: Option<(DirEntry, String)> = None;
                for entry in dir {
                    let entry = entry?;
                    let entry_name = entry.file_name().to_string_lossy().into_owned();
                    if entry_name.starts_with(start_with) {
                        if let Some((previous_entry, previous_name)) = &mut found_suggestion {
                            if entry_name.as_str() < previous_name.as_str() {
                                *previous_entry = entry;
                                *previous_name = entry_name;
                            }
                        } else {
                            found_suggestion = Some((entry, entry_name))
                        }
                    }
                }
                if let Some((suggestion, mut suggestion_str)) = found_suggestion {
                    if file_name == suggestion_str && suggestion.path().is_dir() {
                        suggestion_str.push(std::path::MAIN_SEPARATOR);
                    }

                    return Ok(Some(suggestion_str.into()));
                }
            }

            Ok(None)
        }

        if !file_path.exists() {
            file_part.state = CommandPartState::Invalid(Some("file not found".into()))
        } else if !file_path.is_file() {
            file_part.state =
                CommandPartState::Invalid(Some("path does not refer to a file".into()))
        }
        if autocomplete {
            // TODO: Fix the try_autocomplete function so that the parent path is
            //       the input instead of it guessing the parent.
            //       The result of not doing that is this unrefined salad.
            let at_end = file_part.content().ends_with(std::path::is_separator);

            if let Ok(Some(suggestion)) = try_autocomplete(&file_path, at_end) {
                let base = PathBuf::from(file_part.content());
                let full_suggestion = base.parent();
                file_part.state = CommandPartState::Autocomplete {
                    suggestion: if at_end {
                        base.join(suggestion.as_ref())
                            .to_string_lossy()
                            .into_owned()
                            .into()
                    } else if let Some(parent) = full_suggestion {
                        parent
                            .join(suggestion.as_ref())
                            .to_string_lossy()
                            .into_owned()
                            .into()
                    } else {
                        suggestion
                    },
                };
            }
        }
        let path = PathBuf::from(file_part.content());
        parts.push(file_part);

        return CommandResult::Parsed {
            parts,
            command: Command::LoadScriptFromFile { path },
        };
    }

    if command_part.content_uncased() == "quit" {
        command_part.state = CommandPartState::Ok;
        parts.push(command_part);
        return CommandResult::Parsed {
            parts,
            command: Command::Quit,
        };
    }

    command_part.state = CommandPartState::Invalid(Some(
        format!("unrecognised command '{}'", command_part.content()).into(),
    ));
    if autocomplete {
        command_part.autocomplete_uncased(AUTOCOMPLETE_COMMAND);
    }
    parts.push(command_part);
    CommandResult::CannotContinue { parts }
}

fn expect_equals_part<'a>(
    parts: &mut Vec<CommandPart<'a>>,
    mut remaining: CommandPart<'a>,
    autocomplete: bool,
) -> (bool, Option<CommandPart<'a>>) {
    let (mut equals_part, remaining) = remaining.split_whitespace() else {
        if autocomplete {
            remaining.autocomplete_uncased(AUTOCOMPLETE_EQUAL);
        }

        return (false, Some(remaining));
    };
    equals_part.state = CommandPartState::Ok;
    if equals_part.content() != EQUALS {
        equals_part.state =
            CommandPartState::Invalid(Some(format!("expected '=', got '{}'", equals_part).into()));
        if autocomplete {
            equals_part.autocomplete_uncased(AUTOCOMPLETE_EQUAL);
        }
    }
    parts.push(equals_part);
    (true, remaining)
}

// Really this should only require something like T: FromStrRadix but I can't be bothered
fn parse_number<T: CellType>(current: &mut CommandPart) -> Result<T, ()> {
    let mut str = current.content();
    let mut ustr = UncasedStr::new(str);
    let mut radix = 10;

    let first_two = &ustr[0..2.min(ustr.len())];
    let last_one = &ustr[ustr.len().checked_sub(1).unwrap_or(0)..];
    if first_two == "0b" {
        radix = 2;
        str = &str[2..];
    } else if first_two == "0o" {
        radix = 8;
        str = &str[2..];
    } else if first_two == "0x" {
        radix = 16;
        str = &str[2..];
    } else if last_one == "h" {
        radix = 16;
        str = &str[..str.len() - 1];
    }

    let result = T::from_str_radix(str, radix);
    if result.is_err() {
        current.state = CommandPartState::Invalid(Some("not a valid number".into()));
    }
    result
}
fn parse_duration(current: &mut CommandPart) -> Result<Duration, ()> {
    match humantime::parse_duration(current.content()) {
        Ok(duration) => Ok(duration),
        Err(e) => {
            current.state = CommandPartState::Invalid(Some(e.to_string().into()));
            Err(())
        }
    }
}
