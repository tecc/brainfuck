pub use crate::runtime::context::*;

mod context;

#[derive(Copy, Clone)]
pub struct LoadedInstruction {
    pub instruction: Instruction,
    pub source_position: usize,
}

pub struct Script {
    pub source: String,
    pub instructions: Vec<LoadedInstruction>,
    pub instruction_pointer: usize,
    pub cycles: usize,
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
    pub fn new(source: String) -> Self {
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
            cycles: 0,
        }
    }

    pub fn jump_forwards<T: CellType>(&mut self, context: &RuntimeContext<T>) -> bool {
        let mut depth = 0usize;
        while self.instruction_pointer < self.instructions.len() {
            self.instruction_pointer += 1;
            context.refresh(self);
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

    pub fn jump_backwards<T: CellType>(&mut self, context: &RuntimeContext<T>) -> bool {
        let mut depth = 0usize;
        while self.instruction_pointer > 0 {
            self.instruction_pointer -= 1;
            context.refresh(self);
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

    pub fn execute_instruction<T: CellType>(&mut self, context: &mut RuntimeContext<T>) {
        let Some(instruction) = self.instruction() else {
            return;
        };
        let mut next_instr = true;
        match instruction {
            Instruction::IncrementDataPointer => context.data_pointer += 1,
            Instruction::DecrementDataPointer => context.data_pointer -= 1,
            Instruction::IncrementData => context.increment_cell(context.data_pointer),
            Instruction::DecrementData => context.decrement_cell(context.data_pointer),
            Instruction::OutputData => {
                context.write(context.read_cell(context.data_pointer));
            }
            Instruction::AcceptData => {
                *context.get_cell(context.data_pointer) = context.read();
            }
            Instruction::JumpForwardsIfZero => {
                if context.read_cell(context.data_pointer) == T::zero() {
                    self.jump_forwards(context);
                    next_instr = false;
                }
            }
            Instruction::JumpBackwardsIfNonzero => {
                if context.read_cell(context.data_pointer) != T::zero() {
                    self.jump_backwards(context);
                    next_instr = false;
                }
            }
        }
        if next_instr {
            self.instruction_pointer += 1;
        }
        context.refresh(self);
        self.cycles += 1;
    }
    pub fn instruction(&self) -> Option<Instruction> {
        self.instructions
            .get(self.instruction_pointer)
            .map(|v| v.instruction)
    }
    pub fn loaded_instruction(&self) -> Option<LoadedInstruction> {
        self.instructions.get(self.instruction_pointer).cloned()
    }

    pub fn has_remaining_instructions(&self) -> bool {
        self.instructions.len() > self.instruction_pointer
    }
}
