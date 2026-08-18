use crate::compile::{ByteCode, OpCode};

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
        loop {
            self.step();
        }
    }
    fn step(&mut self) {
        let opcode = self.next_opcode();
        match opcode {
            OpCode::Literal => self.op_literal(),
            OpCode::Call => self.op_call(),
            OpCode::Return => self.op_return(),
            OpCode::Dup => self.op_dup(),
            OpCode::Swap => self.op_swap(),
            OpCode::BindLocal => self.op_bind_local(),
            OpCode::PushLocal => self.op_push_local(),
            OpCode::Add => self.op_add(),
            OpCode::Sub => self.op_sub(),
            OpCode::Mul => self.op_mul(),
            OpCode::Div => self.op_div(),
            OpCode::Print => self.op_print(),
        }
    }
    fn next_opcode(&mut self) -> OpCode {
        let def = &self.bytecode.defs[self.def_id];
        if let Some(raw_opcode) = def.read_word(self.pc) {
            let opcode = OpCode::try_from(raw_opcode).expect("invalid opcode");
            self.pc += 1;
            opcode
        } else {
            OpCode::Return
        }
    }
    fn next_literal(&mut self) -> i64 {
        let def = &self.bytecode.defs[self.def_id];
        let literal = def.read_word(self.pc).unwrap() as i64;
        self.pc += 1;
        literal
    }
    fn next_id(&mut self) -> usize {
        let def = &self.bytecode.defs[self.def_id];
        let id = def.read_word(self.pc).unwrap() as usize;
        self.pc += 1;
        id
    }
}
impl VM {
    fn op_literal(&mut self) {
        let literal = self.next_literal();
        self.stack.push(literal);
    }
    fn op_call(&mut self) {
        let id = self.next_id();
        let mut locals = self.bytecode.defs[id].initialize_locals();
        std::mem::swap(&mut self.locals, &mut locals);
        let frame = StackFrame {
            def_id: self.def_id,
            pc: self.pc,
            locals,
        };
        self.call_stack.push(frame);
        self.def_id = id;
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
    fn op_bind_local(&mut self) {
        let local_id = self.next_id();
        let value = self.stack.pop().unwrap();
        self.locals[local_id] = value;
    }
    fn op_push_local(&mut self) {
        let local_id = self.next_id();
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
