# Brainf*ck Interpreter

Made using Rust. 

It has an interactive UI made using ratatui/crossterm.
The interactive UI takes up the bulk of the codebase (the runtime itself is only `src/runtime.rs`, about 200 lines).

## Usage

```
brainfuck [--mode default|dump|debug|interactive] [--stdin] [code]
```

### `--mode`

Sets the mode to execute code in.

#### `default`

Default mode. 
Will simply execute the code supplied, no funny business.

#### `dump` (in progress)

Dumping mode. 
Executes the code and prints a dump at the end.

#### `debug` (in progress)

Debug mode. Very verbose. 
Executes the code and prints a lot of lines of debug information - good luck getting the program's output out of there.
Prints a dump at the end as well.

#### `interactive` (in progress)

Interactive UI. The staple of this project.
Loads the code and will show execution in real time, but slowed down a _lot_.

### `--stdin`

Tells the interpreter to read all the code from stdin before running. 
If it's active, `[code]` is ignored.

### `[code]`

The code to execute. 
Should generally be supplied unless the code comes from another source.

## Licence

Licensed under the MIT License.
See [LICENCE](./LICENCE).