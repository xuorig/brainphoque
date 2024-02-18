use std::{
    env,
    io::{Error, Read, Write},
    ptr,
};

use libc::pthread_jit_write_protect_np;

#[derive(Debug)]
enum Op {
    /// +
    /// Increment the data pointer by one (to point to the next cell to the right).
    Inc,
    /// -
    /// Decrement the data pointer by one (to point to the next cell to the left).
    Dec,
    /// <
    /// Increment the byte at the data pointer by one.
    Left,
    /// >
    /// Decrement the byte at the data pointer by one.
    Right,
    /// .
    /// Output the byte at the data pointer.
    Output,
    /// ,
    /// Accept one byte of input, storing its value in the byte at the data pointer.
    Input,
    /// [
    /// If the byte at the data pointer is zero, then instead of moving the instruction pointer forward to the next command,
    /// jump it forward to the command after the matching ] command.
    JumpIfZero(usize),
    /// ]
    /// If the byte at the data pointer is nonzero, then instead of moving the instruction pointer forward to the next command,
    /// jump it back to the command after the matching [ command.[a]
    JumpIfNonZero(usize),
}

fn main() -> Result<(), &'static str> {
    let args: Vec<String> = env::args().collect();
    let file_path = &args[1];

    let program = std::fs::read_to_string(file_path).unwrap();

    let mut operations = vec![];
    let mut jump_op_stack = vec![];

    for (i, char) in program.chars().enumerate() {
        match char {
            '+' => operations.push(Op::Inc),
            '-' => operations.push(Op::Dec),
            '<' => operations.push(Op::Left),
            '>' => operations.push(Op::Right),
            '.' => operations.push(Op::Output),
            ',' => operations.push(Op::Input),
            '[' => {
                operations.push(Op::JumpIfZero(0));
                jump_op_stack.push(i);
            }
            ']' => {
                match jump_op_stack.pop() {
                    Some(addr) => {
                        operations.push(Op::JumpIfNonZero(addr + 1));

                        // Back patch the matching `[`
                        match operations[addr] {
                            Op::JumpIfZero(ref mut addr) => *addr = i + 1,
                            _ => unreachable!(),
                        };
                    }
                    None => return Err("Unbalanced jumps"),
                }
            }
            _ => {
                // Brainfuck ignores all other chars
            }
        }
    }

    // let mut interpreter = Interpreter::new(operations, std::io::stdin(), std::io::stdout());
    // interpreter.run();

    let mut jit_compiler = JitCompiler::new(operations);
    let jit_memory = [0; 1024];
    println!("mem addr {:p}",&jit_memory);

    let func = jit_compiler.compile();
    let result = func(jit_memory.as_ptr());

    println!("RESULT: {:#08x}", result);

    Ok(())
}

struct Interpreter<R, W> {
    ops: Vec<Op>,
    cells: [u8; 1000],
    reader: R,
    writer: W,
}

impl<R, W> Interpreter<R, W>
where
    R: Read,
    W: Write,
{
    fn new(ops: Vec<Op>, reader: R, writer: W) -> Self {
        Self {
            ops,
            cells: [0; 1000],
            writer,
            reader,
        }
    }

    fn run(&mut self) {
        let mut ip = 0;
        let mut dp = 0;

        while ip < self.ops.len() {
            match self.ops[ip] {
                Op::Inc => {
                    self.cells[dp] += 1;
                }
                Op::Dec => {
                    self.cells[dp] -= 1;
                }
                Op::Left => {
                    if dp > 0 {
                        dp -= 1;
                    } else {
                        panic!("Tried to move left when dp was 0");
                    }
                }
                Op::Right => {
                    dp += 1;
                }
                Op::Output => {
                    self.writer.write(&[self.cells[dp]]).unwrap();
                }
                Op::Input => {
                    let mut read = [0; 1];
                    self.reader.read(&mut read).unwrap();
                    self.cells[dp] = read[0];
                }
                Op::JumpIfZero(addr) => {
                    if self.cells[dp] == 0 {
                        ip = addr;
                        continue;
                    }
                }
                Op::JumpIfNonZero(addr) => {
                    if self.cells[dp] != 0 {
                        ip = addr;
                        continue;
                    }
                }
            }

            ip += 1;
        }
    }
}

struct JitCompiler {
    ops: Vec<Op>,
}

impl JitCompiler {
    fn new(ops: Vec<Op>) -> Self {
        Self {
            ops,
        }
    }

    fn compile(&mut self) -> extern "C" fn(memory: *const u8) -> i64 {
        let mut code = vec![];

        let mut ip = 0;
        let mut dp = 0;

        while ip < self.ops.len() {
            match self.ops[ip] {
                Op::Inc => {
                    code.push(0x00);
                }
                Op::Dec => {
                }
                Op::Left => {
                    if dp > 0 {
                        dp -= 1;
                    } else {
                        panic!("Tried to move left when dp was 0");
                    }
                }
                Op::Right => {
                    dp += 1;
                }
                Op::Output => {
                }
                Op::Input => {
                }
                Op::JumpIfZero(addr) => {
                }
                Op::JumpIfNonZero(addr) => {
                }
            }

            ip += 1;
        }

        let size = 1024;

        unsafe {
            pthread_jit_write_protect_np(0);
        }

        let mem = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_ANON | libc::MAP_PRIVATE | libc::MAP_JIT,
                -1,
                0,
            )
        };

        if mem == libc::MAP_FAILED {
            let err = Error::last_os_error();
            println!("Error code: {:?}", err.raw_os_error());
            panic!("Failed to allocate executable memory");
        }

        let code: Vec<u8> = vec![
            0xC0, 0x03, 0x5F, 0xD6, // RET
        ];

        unsafe {
            ptr::copy_nonoverlapping(code.as_ptr(), mem as *mut u8, code.len());
            pthread_jit_write_protect_np(1);
        }

        let func: extern "C" fn(memory: *const u8) -> i64 = unsafe { std::mem::transmute(mem) };

        func
    }
}
