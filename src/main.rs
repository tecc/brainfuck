mod interactive;
pub mod runtime;

pub use runtime::*;

use crate::interactive::interactive_runtime;
use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::Constraint::Length;
use ratatui::prelude::Constraint::Min;
use ratatui::prelude::*;
use ratatui::widgets::Block;
use std::error::Error;
use std::io;
use std::io::{stdin, stdout, Read, Write};
use std::time::Duration;

#[derive(clap::Parser)]
pub struct Cli {
    #[arg(long, value_enum, default_value_t)]
    mode: Mode,
    #[arg(long)]
    stdin: bool,
    code: Option<String>,
}

#[derive(clap::ValueEnum, Copy, Clone, Default, Eq, PartialEq)]
enum Mode {
    #[default]
    Default,
    Dump,
    Debug,
    Interactive,
}

fn main() {
    let cli = Cli::parse();

    let mut code;
    if cli.mode == Mode::Interactive {
        code = String::new();
    } else if cli.stdin {
        code = String::new();
        stdin()
            .read_to_string(&mut code)
            .expect("Could not read from stdin");
    } else if let Some(code_opt) = cli.code {
        code = code_opt;
    } else {
        code = String::new();
        stdin()
            .read_line(&mut code)
            .expect("Could not read line from stdin");
    }

    if cli.mode == Mode::Interactive {
        interactive(Script::new(code)).expect("Failure");
        return;
    }

    let mut runtime = Script::new(code);
    let mut context = RuntimeContextU8::new_stdio();
    context.refresh_fn = Some(Box::new(|script, context| {
        let instruction = if let Some(instr) = script.instruction() {
            format!("{:?}", instr)
        } else {
            format!(
                "<end+{}>",
                script.instructions.len() - script.instruction_pointer
            )
        };
        println!(
            "{}: data(*{}={}) instr(*{}={})",
            script.cycles,
            context.data_pointer,
            context.read_cell(context.data_pointer),
            script.instruction_pointer,
            instruction
        )
    }));
    while runtime.has_remaining_instructions() {
        runtime.execute_instruction(&mut context);
        if runtime.cycles % 20 == 9 {
            stdout().flush().expect("Could not flush");
        }
    }
    stdout().flush().expect("Could not flush");

    match cli.mode {
        Mode::Dump | Mode::Debug => {
            println!(
                r#"
============
--- DATA ---
{:?}
"#,
                &context.data
            )
        }
        _ => {}
    }
}

fn interactive(runtime: Script) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.hide_cursor()?;

    let res = interactive_runtime(&mut terminal, runtime);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res?;
    Ok(())
}
