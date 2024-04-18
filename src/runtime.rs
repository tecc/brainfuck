use std::io::{stdin, stdout, Read, Write};
use std::time::Duration;

#[derive(Copy, Clone)]
pub struct LoadedInstruction {
    pub instruction: Instruction,
    pub source_position: usize,
}

pub struct RuntimeContext {
    pub data: Vec<u8>,
    pub data_pointer: usize,
}

impl RuntimeContext {
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(30000), // Minimum capacity according to Wikipedia
            data_pointer: 0,
        }
    }
    pub fn get_cell(&mut self) -> &mut u8 {
        if self.data.len() <= self.data_pointer {
            self.data.resize(self.data_pointer + 1, 0);
        }
        &mut self.data[self.data_pointer]
    }
    pub fn read_cell(&self) -> u8 {
        if self.data.len() <= self.data_pointer {
            return 0;
        }
        self.data[self.data_pointer]
    }
    pub fn process_cell(&mut self, f: impl FnOnce(u8) -> u8) {
        let cell = self.get_cell();
        *cell = f(*cell);
        *cell;
    }
}

pub struct Script {
    pub source: String,
    pub instructions: Vec<LoadedInstruction>,
    pub instruction_pointer: usize,

    pub options: RuntimeOptions,
    pub cycles: usize,
}

pub struct RuntimeOptions {
    pub refresh: Option<Box<dyn Fn(&Script, &RuntimeContext)>>,
    pub write: Box<dyn FnMut(u8)>,
    pub read: Box<dyn FnMut() -> u8>,
}
impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            refresh: None,
            write: Box::new(|value| {
                stdout().write(&[value]).expect("Could not write");
            }),
            read: Box::new(|| {
                let mut value = [0u8];
                stdin().read_exact(&mut value).expect("Could not read");
                value[0]
            }),
        }
    }
}
impl RuntimeOptions {
    pub fn write(&mut self, value: u8) {
        (self.write)(value)
    }
    pub fn read(&mut self) -> u8 {
        (self.read)()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Instruction {
    IncrementDataPointer,
    DecrementDataPointer,
    IncrementData,
    DecrementData,
    OutputData,
    AcceptData,
    JumpForwardsIfZero,
    JumpBackwardsIfNonzero,
}
impl Instruction {
    pub fn from_char(ch: char) -> Option<Self> {
        Some(match ch {
            '>' => Instruction::IncrementDataPointer,
            '<' => Instruction::DecrementDataPointer,
            '+' => Instruction::IncrementData,
            '-' => Instruction::DecrementData,
            '.' => Instruction::OutputData,
            ',' => Instruction::AcceptData,
            '[' => Instruction::JumpForwardsIfZero,
            ']' => Instruction::JumpBackwardsIfNonzero,
            _ => return None,
        })
    }
}

impl Script {
    pub fn new(source: String, options: RuntimeOptions) -> Self {
        let mut instructions = Vec::new();
        for (u, ch) in source.chars().enumerate() {
            let Some(instruction) = Instruction::from_char(ch) else {
                continue;
            };
            instructions.push(LoadedInstruction {
                instruction,
                source_position: u,
            });
        }
        Self {
            source,
            instructions,
            instruction_pointer: 0,
            options,
            cycles: 0,
        }
    }

    pub fn refresh(&self, context: &RuntimeContext) {
        if let Some(refresh_fn) = self.options.refresh.as_ref() {
            refresh_fn(&self, context);
        }
    }
    pub fn clock(&mut self, context: &RuntimeContext) {
        self.refresh(context);
    }

    pub fn jump_forwards(&mut self, context: &RuntimeContext) -> bool {
        let mut depth = 0usize;
        while self.instruction_pointer < self.instructions.len() {
            self.instruction_pointer += 1;
            self.clock(context);
            let Some(instruction) = self.instruction() else {
                break;
            };
            if instruction == Instruction::JumpBackwardsIfNonzero {
                if depth == 0 {
                    return true;
                }
                depth -= 1;
            }
            if instruction == Instruction::JumpForwardsIfZero {
                depth += 1;
            }
        }
        return false;
    }

    pub fn jump_backwards(&mut self, context: &RuntimeContext) -> bool {
        let mut depth = 0usize;
        while self.instruction_pointer >= 0 {
            self.instruction_pointer -= 1;
            self.clock(context);
            let Some(instruction) = self.instruction() else {
                break;
            };
            if instruction == Instruction::JumpForwardsIfZero {
                if depth == 0 {
                    self.instruction_pointer += 1;
                    return true;
                }
                depth -= 1;
            }
            if instruction == Instruction::JumpBackwardsIfNonzero {
                depth += 1;
            }
        }
        return false;
    }

    pub fn execute_instruction(&mut self, context: &mut RuntimeContext) {
        let Some(instruction) = self.instruction() else {
            return;
        };
        let mut next_instr = true;
        match instruction {
            Instruction::IncrementDataPointer => context.data_pointer += 1,
            Instruction::DecrementDataPointer => context.data_pointer -= 1,
            Instruction::IncrementData => context.process_cell(|v| v.overflowing_add(1).0),
            Instruction::DecrementData => context.process_cell(|v| v.overflowing_sub(1).0),
            Instruction::OutputData => {
                self.options.write(context.read_cell());
            }
            Instruction::AcceptData => {
                *context.get_cell() = self.options.read();
            }
            Instruction::JumpForwardsIfZero => {
                if context.read_cell() == 0 {
                    self.jump_forwards(context);
                    next_instr = false;
                }
            }
            Instruction::JumpBackwardsIfNonzero => {
                if context.read_cell() != 0 {
                    self.jump_backwards(context);
                    next_instr = false;
                }
            }
        }
        if next_instr {
            self.instruction_pointer += 1;
        }
        self.clock(context);
        self.cycles += 1;
    }
    pub fn instruction(&self) -> Option<Instruction> {
        self.instructions
            .get(self.instruction_pointer)
            .map(|v| v.instruction)
    }

    pub fn has_remaining_instructions(&self) -> bool {
        self.instructions.len() > self.instruction_pointer
    }
}
