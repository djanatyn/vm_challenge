use std::{fs::File, io::Read};

const MEMORY_SIZE: usize = 1usize << 15;

#[derive(Debug)]
enum Opcode {
    /// halt: 0
    ///   stop execution and terminate the program
    HALT,

    /// set: 1 a b
    ///   set register <a> to the value of <b>
    SET,

    /// jmp: 6 a
    ///   jump to <a>
    JMP,

    /// jt: 7 a b
    ///   if <a> is nonzero, jump to <b>
    JT,

    /// jf: 8 a b
    ///   if <a> is zero, jump to <b>
    JF,

    /// add: 9 a b c
    ///   assign into <a> the sum of <b> and <c> (modulo 32768)
    ADD,

    /// rmem: 15 a b
    ///   read memory at address <b> and write it to <a>
    RMEM,

    /// out: 19 a
    ///   writethe character represented by ascii code <a> to the terminal
    OUT,

    // noop: 21
    //    no operation
    NOOP,
}

/// instruction argument: either a number, or a register reference
/// (how the register reference is resolved depends on the opcode)
#[derive(Debug)]
enum Arg {
    Number(u16),
    Register(usize),
}

#[derive(Debug)]
enum Instruction {
    HALT,
    SET((Arg, Arg)),
    JMP(Arg),
    JT((Arg, Arg)),
    JF((Arg, Arg)),
    ADD((Arg, Arg, Arg)),
    RMEM((Arg, Arg)),
    OUT(Arg),
    NOOP,
}

#[derive(Debug)]
struct Computer {
    memory: Memory,
    registers: Registers,
    stack: Stack,
    pc: u16,
}

/// eight registers
#[derive(Debug, Default)]
struct Registers([u16; 8]);

/// an unbounded stack which holds 16-bit values
#[derive(Debug, Default)]
struct Stack(Vec<u16>);

/// memory with 15-bit address space storing 16-bit values
#[derive(Debug)]
struct Memory(Vec<u16>);

impl Instruction {
    // offset of following instruction, relative to current instruction, skipping opcode arguments
    fn next_instruction(&self) -> u16 {
        match self {
            Instruction::HALT => 1,
            Instruction::SET(_) => 3,
            Instruction::JMP(_) => 2,
            Instruction::JT(_) => 3,
            Instruction::JF(_) => 3,
            Instruction::ADD(_) => 4,
            Instruction::RMEM(_) => 3,
            Instruction::OUT(_) => 2,
            Instruction::NOOP => 1,
        }
    }
}

impl Arg {
    /// parse according to the binary format:
    /// - numbers 0..32767 mean a literal value
    /// - numbers 32768..32775 instead mean registers 0..7
    fn parse(value: u16) -> anyhow::Result<Arg> {
        // check for register
        if value > 32767 {
            match value {
                32768 => Ok(Arg::Register(0)),
                32769 => Ok(Arg::Register(1)),
                32770 => Ok(Arg::Register(2)),
                32771 => Ok(Arg::Register(3)),
                32772 => Ok(Arg::Register(4)),
                32773 => Ok(Arg::Register(5)),
                32774 => Ok(Arg::Register(6)),
                32775 => Ok(Arg::Register(7)),
                e => Err(anyhow::anyhow!("invalid number: {}", e)),
            }
        } else {
            Ok(Arg::Number(value))
        }
    }

    /// extract literal value without memory lookup
    fn literal(&self) -> anyhow::Result<u16> {
        match self {
            Arg::Number(value) => Ok(*value),
            Arg::Register(index) => Ok(*index as u16),
        }
    }
}

impl Opcode {
    /// each opcode is one word, followed by some number of arguments
    fn parse(value: u16) -> anyhow::Result<Opcode> {
        let bytes = value.to_le_bytes();
        let opcode = match bytes[0] {
            0 => Opcode::HALT,
            1 => Opcode::SET,
            6 => Opcode::JMP,
            7 => Opcode::JT,
            8 => Opcode::JF,
            9 => Opcode::ADD,
            19 => Opcode::OUT,
            21 => Opcode::NOOP,
            e => Err(anyhow::anyhow!("unimplemented: {e}"))?,
        };
        Ok(opcode)
    }
}

impl Memory {
    /// fetch a cell from memory by address
    fn fetch(&self, addr: u16) -> u16 {
        self.0[addr as usize]
    }
}

impl Computer {
    /// given an address with an instruction, parse the opcode and arguments
    fn parse_instruction(&self, addr: u16) -> anyhow::Result<Instruction> {
        let opcode = Opcode::parse(self.memory.fetch(addr))?;
        Ok(match opcode {
            Opcode::HALT => Instruction::HALT,
            Opcode::SET => Instruction::SET((
                Arg::parse(self.memory.fetch(addr + 1))?,
                Arg::parse(self.memory.fetch(addr + 2))?,
            )),
            Opcode::JMP => Instruction::JMP(Arg::parse(self.memory.fetch(addr + 1))?),
            Opcode::JT => Instruction::JT((
                Arg::parse(self.memory.fetch(addr + 1))?,
                Arg::parse(self.memory.fetch(addr + 2))?,
            )),
            Opcode::JF => Instruction::JF((
                Arg::parse(self.memory.fetch(addr + 1))?,
                Arg::parse(self.memory.fetch(addr + 2))?,
            )),
            Opcode::ADD => Instruction::ADD((
                Arg::parse(self.memory.fetch(addr + 1))?,
                Arg::parse(self.memory.fetch(addr + 2))?,
                Arg::parse(self.memory.fetch(addr + 3))?,
            )),
            Opcode::RMEM => Instruction::RMEM((
                Arg::parse(self.memory.fetch(addr + 1))?,
                Arg::parse(self.memory.fetch(addr + 2))?,
            )),
            Opcode::OUT => Instruction::OUT(Arg::parse(self.memory.fetch(addr + 1))?),
            Opcode::NOOP => Instruction::NOOP,
        })
    }

    /// run the instruction at the current program counter
    fn step(&mut self) -> anyhow::Result<()> {
        self.execute(self.parse_instruction(self.pc)?)
    }

    /// resolve an opcode argument according to the binary format:
    /// - numbers 0..32767 mean a literal value
    /// - numbers 32768..32775 instead mean registers 0..7
    fn lookup(&self, arg: &Arg) -> u16 {
        match arg {
            Arg::Number(value) => *value,
            Arg::Register(index) => self.registers.0[*index],
        }
    }

    /// execute an instruction and update the program counter
    fn execute(&mut self, instruction: Instruction) -> anyhow::Result<()> {
        match instruction {
            Instruction::HALT => {
                eprintln!("[{}] HALT -> [END]", self.pc);
                Err(anyhow::anyhow!("halted"))?
            }
            Instruction::SET((ref a, ref b)) => {
                let register_index = a.literal()? as usize;
                let value = self.lookup(&b);

                self.registers.0[register_index] = value;
                let next = self.pc + instruction.next_instruction();

                eprintln!(
                    "[{}] SET: a={:?} [{}] b={:?} [{}] -> [{}] | {:?}",
                    self.pc, a, register_index, b, value, next, self.registers
                );

                self.pc = next;
            }
            Instruction::JMP(a) => {
                let dest = self.lookup(&a);

                eprintln!("[{}] JMP: a={:?} [{}]-> [{}]", self.pc, a, dest, dest);

                self.pc = dest; // unconditional jump
            }
            Instruction::JT((ref a, ref b)) => {
                let value = self.lookup(&a);
                let dest = self.lookup(&b);

                // jump if nonzero, otherwise skip 2 opcode args
                let next = if value == 0 {
                    self.pc + instruction.next_instruction()
                } else {
                    dest
                };
                eprintln!(
                    "[{}] JT: a={:?} [{}] b={:?} [{}] -> [{}]",
                    self.pc, a, value, b, dest, next
                );

                self.pc = next;
            }
            Instruction::JF((ref a, ref b)) => {
                let value = self.lookup(&a);
                let dest = self.lookup(&b);

                // jump if zero
                let next = if value == 0 {
                    dest
                } else {
                    self.pc + instruction.next_instruction()
                };
                eprintln!(
                    "[{}] JF: a={:?} [{}] b={:?} [{}] -> [{}]",
                    self.pc, a, value, b, dest, next
                );

                self.pc = next;
            }
            Instruction::RMEM(_) => todo!(),
            Instruction::NOOP => {
                let next = self.pc + instruction.next_instruction();
                eprintln!("[{}] NOOP -> [{}]", self.pc, next);
                self.pc = next;
            }
            Instruction::OUT(ref a) => {
                let ch = self.lookup(&a);
                let next = self.pc + instruction.next_instruction();

                eprintln!("[{}] OUT: {} -> [{}]", self.pc, ch, next);
                print!("{}", char::from(ch as u8));

                self.pc = next;
            }
            Instruction::ADD((a, b, c)) => {
                todo!()
            }
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let mut binary = File::open("challenge.bin")?;

    // read file as u8
    let mut bytes = vec![];
    binary.read_to_end(&mut bytes)?;

    // values in addressable memories are u16, convert with chunking
    let mut memory: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<u16>>();

    // add padding to match addressable size
    memory.resize(MEMORY_SIZE, 0);

    let mut computer = Computer {
        registers: Default::default(),
        memory: Memory(memory),
        stack: Default::default(),
        pc: 0,
    };

    loop {
        let _ = computer.step()?;
    }
}
