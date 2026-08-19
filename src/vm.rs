use crate::bytecode::{ByteCode, Op, OpCode};

struct StackFrame {
    def_id: usize,
    pc: usize,
    locals: Vec<i64>,
}

pub struct VM {
    bytecode: ByteCode,
    def_id: usize,
    pc: usize,
    locals: Vec<i64>,
    stack: Vec<i64>,
    call_stack: Vec<StackFrame>,
}

impl VM {
    fn next_word(&mut self) -> u64 {
        let word = self.bytecode.defs[self.def_id].data[self.pc];
        self.pc += 1;
        word
    }
}
impl Iterator for VM {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pc >= self.bytecode.defs[self.def_id].data.len() {
            return None;
        }
        let opcode = OpCode::try_from(self.next_word()).expect("invalid opcode");
        let op = match opcode {
            OpCode::Literal => Op::Literal(self.next_word() as i64),
            OpCode::Call => Op::Call(self.next_word() as usize),
            OpCode::Return => Op::Return,
            OpCode::Dup => Op::Dup,
            OpCode::Swap => Op::Swap,
            OpCode::BindLocal => Op::BindLocal(self.next_word() as usize),
            OpCode::PushLocal => Op::PushLocal(self.next_word() as usize),
            OpCode::Add => Op::Add,
            OpCode::Sub => Op::Sub,
            OpCode::Mul => Op::Mul,
            OpCode::Div => Op::Div,
            OpCode::Print => Op::Print,
        };
        Some(op)
    }
}
impl VM {
    pub fn new(bytecode: ByteCode) -> Self {
        let def_id = bytecode.main_id.expect("no main definition");
        let pc = 0;
        let locals = bytecode.defs[def_id].initialize_locals();
        let stack = vec![];
        let call_stack = vec![];
        Self {
            bytecode,
            def_id,
            pc,
            locals,
            stack,
            call_stack,
        }
    }
    pub fn run(&mut self) {
        while let Some(op) = self.next() {
            self.step(&op);
        }
    }
    fn step(&mut self, op: &Op) {
        match *op {
            Op::Literal(literal) => self.op_literal(literal),
            Op::Call(def_id) => self.op_call(def_id),
            Op::Return => self.op_return(),
            Op::Dup => self.op_dup(),
            Op::Swap => self.op_swap(),
            Op::BindLocal(local_id) => self.op_bind_local(local_id),
            Op::PushLocal(local_id) => self.op_push_local(local_id),
            Op::Add => self.op_add(),
            Op::Sub => self.op_sub(),
            Op::Mul => self.op_mul(),
            Op::Div => self.op_div(),
            Op::Print => self.op_print(),
        }
    }
}
impl VM {
    fn op_literal(&mut self, literal: i64) {
        self.stack.push(literal);
    }
    fn op_call(&mut self, def_id: usize) {
        let mut locals = self.bytecode.defs[def_id].initialize_locals();
        std::mem::swap(&mut self.locals, &mut locals);
        let frame = StackFrame {
            def_id: self.def_id,
            pc: self.pc,
            locals,
        };
        self.call_stack.push(frame);
        self.def_id = def_id;
        self.pc = 0;
    }
    fn op_return(&mut self) {
        if let Some(frame) = self.call_stack.pop() {
            self.def_id = frame.def_id;
            self.pc = frame.pc;
            self.locals = frame.locals;
        } else {
            panic!("Done");
        }
    }
    fn op_dup(&mut self) {
        let peek = *self.stack.last().unwrap();
        self.stack.push(peek);
    }
    fn op_swap(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(b);
        self.stack.push(a);
    }
    fn op_bind_local(&mut self, local_id: usize) {
        let value = self.stack.pop().unwrap();
        self.locals[local_id] = value;
    }
    fn op_push_local(&mut self, local_id: usize) {
        let value = self.locals[local_id];
        self.stack.push(value);
    }
    fn op_add(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push(a + b)
    }
    fn op_sub(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push(a - b)
    }
    fn op_mul(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push(a * b)
    }
    fn op_div(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push(a / b)
    }
    fn op_print(&mut self) {
        let value = self.stack.last().expect("value on stack");
        println!("{value}");
    }
}
