# Interactive Mode: Commands

*Not all of these commands are implemented.*

## `set <variable> = <value>`: Set variables

Set various variables in a simple manner.

Variables:

| Variable name (`[optional parameter]`) | Description                                                                                                                                                                      |
|----------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `instruction pointer`, `ip`            | Set the instruction pointer. `value` should be a number indicating the instruction to go to.                                                                                     |
| `data pointer`, `dp`                   | Set the data pointer. `value` should be a number indicating the cell to go to.                                                                                                   |
| `data [idx]`, `d [idx]`                | Set the data at the specified cell `idx`. `value` should be a number indicating the value to set the cell to. If `idx` is not specified it defaults to the current data pointer. |
| `speed`                                | Set the speed to execute instructions at. `value` should be a number indicating the speed.                                                                                       |

## `clear [specifier]`: Clear... things

### `clear data`
Sets all data cells to 0.

### `clear io`
Clears all input/output buffers.

## `restart`: Restart the program

Restarts the currently running program:
- Sets the instruction pointer to 0.
- Sets the data pointer to 0.

## `reset`: Reset state

Resets the runtime context, meaning the 

## `quit`: Exit interactive mode
