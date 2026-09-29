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
    /// jf: 7 a b
    ///   if <a> is zero, jump to <b>
    JF,
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

#[derive(Debug)]
enum Arg {
    Number(u16),
    Register(usize),
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

impl Arg {
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
}
impl Opcode {
    fn parse(value: u16) -> anyhow::Result<Opcode> {
        let bytes = value.to_le_bytes();
        let opcode = match bytes[0] {
            0 => Opcode::HALT,
            1 => Opcode::SET,
            6 => Opcode::JMP,
            7 => Opcode::JT,
            8 => Opcode::JF,
            19 => Opcode::OUT,
            21 => Opcode::NOOP,
            e => Err(anyhow::anyhow!("unimplemented: {e}"))?,
        };
        Ok(opcode)
    }
}

impl Memory {
    fn fetch(&self, addr: u16) -> u16 {
        self.0[addr as usize]
    }
}

impl Computer {
    fn step(&mut self) -> anyhow::Result<Opcode> {
        let instruction: u16 = self.memory.fetch(self.pc);
        let opcode = Opcode::parse(instruction)?;
        self.execute(opcode)
    }

    fn execute(&mut self, opcode: Opcode) -> anyhow::Result<Opcode> {
        match opcode {
            Opcode::HALT => {
                let next = self.memory.fetch(self.pc + 1);
                eprintln!("[{}] HALT: {} -> [END]", self.pc, next);
                Err(anyhow::anyhow!("halted"))
            }
            Opcode::SET => {
                let a = self.memory.fetch(self.pc + 1);
                let b = self.memory.fetch(self.pc + 2);

                let rvalue = Arg::parse(b)?;
                let value = match rvalue {
                    Arg::Number(value) => value,
                    Arg::Register(index) => self.registers.0[index],
                };

                self.registers.0[a as usize] = value;
                let next = self.pc + 3;

                eprintln!(
                    "[{}] SET: {} {} ({:?}) [{}] -> [{}]",
                    self.pc, a, b, rvalue, value, next
                );
                eprintln!(
                    "[{}] SET: {} {} ({:?}) [{}] -> [{}] | {:?}",
                    self.pc, a, b, rvalue, value, next, self.registers
                );
                self.pc = next;
                Ok(opcode)
            }
            Opcode::JMP => {
                let dest = self.memory.fetch(self.pc + 1);
                eprintln!("[{}] JMP: {} -> [{}]", self.pc, dest, dest);
                self.pc = dest;
                Ok(opcode)
            }
            Opcode::JT => {
                let a = self.memory.fetch(self.pc + 1);
                let b = self.memory.fetch(self.pc + 2);

                let arg = Arg::parse(a)?;
                let value = match arg {
                    Arg::Number(value) => value,
                    Arg::Register(index) => self.registers.0[index],
                };

                // jump if nonzero, otherwise skip 2 opcode args
                let next = if value == 0 { self.pc + 3 } else { b };
                eprintln!(
                    "[{}] JT: {} {} ({:?}) [{}] -> [{}]",
                    self.pc, a, b, arg, value, next
                );
                self.pc = next;
                Ok(opcode)
            }
            Opcode::JF => {
                let a = self.memory.fetch(self.pc + 1);
                let b = self.memory.fetch(self.pc + 2);

                let arg = Arg::parse(a)?;
                let value = match arg {
                    Arg::Number(value) => value,
                    Arg::Register(index) => self.registers.0[index],
                };

                // jump if zero, otherwise skip 2 opcode args
                let next = if value == 0 { b } else { self.pc + 3 };
                eprintln!(
                    "[{}] JF: {} {} ({:?}) [{}] -> [{}]",
                    self.pc, a, b, arg, value, next
                );
                self.pc = next;
                Ok(opcode)
            }
            Opcode::RMEM => todo!(),
            Opcode::NOOP => {
                let next = self.pc + 1;
                eprintln!("[{}] NOOP -> [{}]", self.pc, next);
                self.pc = next;
                Ok(opcode)
            }
            Opcode::OUT => {
                let (ch, next) = (self.memory.fetch(self.pc + 1), self.pc + 2);
                eprintln!("[{}] OUT: {} -> [{}]", self.pc, ch, next);
                print!("{}", char::from(ch as u8));
                self.pc = next;
                Ok(opcode)
            }
        }
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
