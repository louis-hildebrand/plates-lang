use anyhow::{anyhow, Error};
use rand::{rngs::ThreadRng, Rng};
use std::{collections::HashMap, fmt::Display, io::Write};

use crate::parser::Instruction;

#[derive(Clone)]
enum Word {
    Data(u32),
    Function(String),
}

impl Display for Word {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Word::Data(n) => write!(formatter, "{n}"),
            Word::Function(f) => write!(formatter, "function {f}"),
        }
    }
}

pub struct Runtime {
    value_stack: Vec<Word>,
    function_table: HashMap<String, Vec<Instruction>>,
    rng: ThreadRng,
    instruction_stack: Vec<Instruction>,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            value_stack: Vec::new(),
            function_table: HashMap::new(),
            rng: rand::thread_rng(),
            instruction_stack: Vec::new(),
        }
    }

    pub fn stack_to_string(&mut self) -> String {
        let words = self
            .value_stack
            .iter()
            .map(|w| w.to_string())
            .collect::<Vec<_>>();
        format!("[{}]  <-- top", words.join(", "))
    }

    /// Returns true iff the program should exit.
    pub fn run(&mut self, instruction: Instruction) -> Result<bool, Error> {
        self.instruction_stack.push(instruction);

        loop {
            match self.instruction_stack.pop() {
                None => return Ok(false),
                Some(instruction) => match self.run_instruction(instruction) {
                    Err(e) => {
                        self.instruction_stack.clear();
                        return Err(e);
                    }
                    Ok(true) => return Ok(true),
                    Ok(false) => continue,
                },
            };
        }
    }

    fn run_instruction(&mut self, instruction: Instruction) -> Result<bool, Error> {
        match instruction {
            Instruction::Exit => Ok(true),
            Instruction::PushData(n) => self.run_pushdata(n),
            Instruction::PushFunction(f) => self.run_pushfunction(f),
            Instruction::PushCopy => self.run_pushcopy(),
            Instruction::PushRandom => self.run_pushrandom(),
            Instruction::Define(f, body) => self.run_define(f, body),
            Instruction::CallIf => self.run_callif(),
        }
    }

    fn run_pushdata(&mut self, n: u32) -> Result<bool, Error> {
        self.value_stack.push(Word::Data(n));
        Ok(false)
    }

    fn run_pushfunction(&mut self, f: String) -> Result<bool, Error> {
        self.value_stack.push(Word::Function(f));
        Ok(false)
    }

    fn run_pushcopy(&mut self) -> Result<bool, Error> {
        match self.value_stack.last() {
            None => Err(anyhow!(
                "Runtime error: cannot push copy when the stack is empty."
            )),
            Some(w) => {
                self.value_stack.push(w.clone());
                Ok(false)
            }
        }
    }

    fn run_pushrandom(&mut self) -> Result<bool, Error> {
        let n = self.rng.gen();
        self.value_stack.push(Word::Data(n));
        Ok(false)
    }

    fn run_define(&mut self, f: String, body: Vec<Instruction>) -> Result<bool, Error> {
        self.function_table.insert(f, body);
        Ok(false)
    }

    fn run_callif(&mut self) -> Result<bool, Error> {
        let top_data = self.pop_data_from_stack()?;

        if top_data == 0 {
            // TODO: What if there's nothing to pop?
            // In general, be more clear about the state of the stack after a runtime error.
            self.value_stack.pop();
            return Ok(false);
        }

        let f = self.pop_function_from_stack()?;
        self.call_function(&f)
    }

    fn call_function(&mut self, f: &str) -> Result<bool, Error> {
        if f.starts_with("__") {
            return self.call_builtin(f);
        }

        let body = match self.function_table.get(f) {
            None => return Err(anyhow!("Runtime error: function '{}' is not defined.", f)),
            Some(body) => body,
        };

        for instruction in body.iter().rev() {
            self.instruction_stack.push(instruction.clone());
        }

        Ok(false)
    }

    pub fn call_builtin(&mut self, f: &str) -> Result<bool, Error> {
        match f {
            "__print__" => self.call_print(),
            "__input__" => self.call_input(),
            "__swap__" => self.call_swap(),
            "__nand__" => self.call_nand(),
            _ => Err(anyhow!(
                "Runtime error: unrecognized built-in function '{}'.",
                f
            )),
        }
    }

    fn call_print(&mut self) -> Result<bool, Error> {
        loop {
            let n = self.pop_data_from_stack()?;

            if n == 0 {
                match std::io::stdout().flush() {
                    Err(_) => return Err(anyhow!("Failed to flush stdout.")),
                    Ok(_) => {}
                };
                return Ok(false);
            }

            let c = match char::from_u32(n) {
                None => return Err(anyhow!("Runtime error: {n} is not a valid code point.")),
                Some(c) => c,
            };

            print!("{c}");
        }
    }

    fn call_input(&mut self) -> Result<bool, Error> {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Err(_) => return Err(anyhow!("Runtime error: failed to read from stdin.")),
            Ok(_) => {}
        };

        for c in line.chars().rev() {
            let n = c as u32;
            self.value_stack.push(Word::Data(n));
        }

        Ok(false)
    }

    fn call_swap(&mut self) -> Result<bool, Error> {
        let idx_u32 = self.pop_data_from_stack()?;

        let i: usize = match idx_u32.try_into() {
            Ok(i) => i,
            Err(_) => return Err(anyhow!("Runtime error: {idx_u32} is not a valid index.")),
        };

        let stack_size = self.value_stack.len();
        if i >= stack_size {
            return Err(anyhow!(
                "Runtime error: cannot swap to index {} in stack of size {}.",
                i,
                stack_size
            ));
        }

        let top_index = stack_size - 1;
        self.value_stack.swap(top_index, top_index - i);
        Ok(false)
    }

    fn call_nand(&mut self) -> Result<bool, Error> {
        // Use !(a & b)
        let a = self.pop_data_from_stack()?;
        let b = self.pop_data_from_stack()?;

        let result = !(a & b);
        self.value_stack.push(Word::Data(result));
        return Ok(false);
    }

    fn pop_data_from_stack(&mut self) -> Result<u32, Error> {
        match self.value_stack.pop() {
            None => Err(anyhow!("Runtime error: cannot pop from empty stack.")),
            Some(Word::Function(f)) => Err(anyhow!(
                "Runtime error: expected data but received function '{}'.",
                f
            )),
            Some(Word::Data(n)) => Ok(n),
        }
    }

    fn pop_function_from_stack(&mut self) -> Result<String, Error> {
        match self.value_stack.pop() {
            None => Err(anyhow!("Runtime error: cannot pop from empty stack.")),
            Some(Word::Data(n)) => Err(anyhow!(
                "Runtime error: expected function but received data '{}'.",
                n
            )),
            Some(Word::Function(f)) => Ok(f),
        }
    }
}
