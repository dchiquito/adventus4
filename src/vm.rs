use crate::bytecode::{BlockId, ByteCode, DefId, LayoutId, Op, OpCode, PropId};

struct Object {
    layout_id: LayoutId,
    props: Vec<i64>,
}

struct StackFrame {
    block_id: BlockId,
    pc: usize,
    locals: Vec<i64>,
}

pub struct VM<'a> {
    bytecode: &'a ByteCode,
    block_id: BlockId,
    pc: usize,
    locals: Vec<i64>,
    stack: Vec<i64>,
    call_stack: Vec<StackFrame>,
    objects: Vec<Object>,
}

impl VM<'_> {
    fn next_word(&mut self) -> u64 {
        let word = self.bytecode.get_block(self.block_id).data[self.pc];
        self.pc += 1;
        word
    }
}
impl Iterator for VM<'_> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pc >= self.bytecode.get_block(self.block_id).data.len() {
            return None;
        }
        let opcode = OpCode::try_from(self.next_word()).expect("invalid opcode");
        let op = match opcode {
            OpCode::Literal => Op::Literal(self.next_word() as i64),
            OpCode::Call => Op::Call(DefId::new(self.next_word())),
            OpCode::Return => Op::Return,
            OpCode::GoTo => Op::GoTo(BlockId::new(self.next_word())),
            OpCode::GoToIf => Op::GoToIf(BlockId::new(self.next_word())),
            OpCode::Dup => Op::Dup,
            OpCode::Swap => Op::Swap,
            OpCode::Pop => Op::Pop,
            OpCode::BindLocal => Op::BindLocal(self.next_word() as usize),
            OpCode::PushLocal => Op::PushLocal(self.next_word() as usize),
            OpCode::BindProp => Op::BindProp(PropId::new(self.next_word())),
            OpCode::PushProp => Op::PushProp(PropId::new(self.next_word())),
            OpCode::Add => Op::Add,
            OpCode::Sub => Op::Sub,
            OpCode::Mul => Op::Mul,
            OpCode::Div => Op::Div,
            OpCode::Eq => Op::Eq,
            OpCode::Ne => Op::Ne,
            OpCode::Gt => Op::Gt,
            OpCode::Lt => Op::Lt,
            OpCode::Gte => Op::Gte,
            OpCode::Lte => Op::Lte,
            OpCode::And => Op::And,
            OpCode::Or => Op::Or,
            OpCode::Not => Op::Not,
            OpCode::Print => Op::Print,
            OpCode::ObjectId => Op::Layout(LayoutId::new(self.next_word())),
            OpCode::Malloc => Op::Malloc(LayoutId::new(self.next_word())),
        };
        Some(op)
    }
}
impl<'a> VM<'a> {
    pub fn new_main(bytecode: &'a ByteCode) -> Self {
        let def_id = bytecode.main_id.expect("no main definition");
        let block_id = bytecode.get_def(def_id).block_id;
        Self::new(bytecode, def_id, block_id)
    }
    pub fn new(bytecode: &'a ByteCode, def_id: DefId, block_id: BlockId) -> Self {
        let pc = 0;
        let locals = bytecode.get_def(def_id).initialize_locals();
        let stack = vec![];
        let call_stack = vec![];
        let objects = vec![];
        Self {
            bytecode,
            block_id,
            pc,
            locals,
            stack,
            call_stack,
            objects,
        }
    }
    pub fn run(&mut self) {
        while let Some(op) = self.next() {
            self.step(&op);
        }
    }
    fn step(&mut self, op: &Op) {
        eprintln!("EXEC {op:?}");
        match *op {
            Op::Literal(literal) => self.op_literal(literal),
            Op::Call(def_id) => self.op_call(def_id),
            Op::Return => self.op_return(),
            Op::GoTo(block_id) => self.op_go_to(block_id),
            Op::GoToIf(block_id) => self.op_go_to_if(block_id),
            Op::Dup => self.op_dup(),
            Op::Swap => self.op_swap(),
            Op::Pop => self.op_pop(),
            Op::BindLocal(local_id) => self.op_bind_local(local_id),
            Op::PushLocal(local_id) => self.op_push_local(local_id),
            Op::BindProp(prop_id) => self.op_bind_prop(prop_id),
            Op::PushProp(prop_id) => self.op_push_prop(prop_id),
            Op::Add => self.op_add(),
            Op::Sub => self.op_sub(),
            Op::Mul => self.op_mul(),
            Op::Div => self.op_div(),
            Op::Eq => self.op_eq(),
            Op::Ne => self.op_ne(),
            Op::Gt => self.op_gt(),
            Op::Lt => self.op_lt(),
            Op::Gte => self.op_gte(),
            Op::Lte => self.op_lte(),
            Op::And => self.op_and(),
            Op::Or => self.op_or(),
            Op::Not => self.op_not(),
            Op::Print => self.op_print(),
            Op::Layout(layout_id) => self.op_layout(layout_id),
            Op::Malloc(layout_id) => self.op_malloc(layout_id),
        }
    }
}
impl VM<'_> {
    fn op_literal(&mut self, literal: i64) {
        self.stack.push(literal);
    }
    fn op_call(&mut self, def_id: DefId) {
        let mut locals = self.bytecode.get_def(def_id).initialize_locals();
        std::mem::swap(&mut self.locals, &mut locals);
        let frame = StackFrame {
            block_id: self.block_id,
            pc: self.pc,
            locals,
        };
        self.call_stack.push(frame);
        self.block_id = self.bytecode.get_def(def_id).block_id;
        self.pc = 0;
    }
    fn op_return(&mut self) {
        if let Some(StackFrame {
            block_id,
            pc,
            locals,
        }) = self.call_stack.pop()
        {
            self.block_id = block_id;
            self.pc = pc;
            self.locals = locals;
        } else {
            panic!("Done");
        }
    }
    fn op_go_to(&mut self, block_id: BlockId) {
        self.block_id = block_id;
        self.pc = 0;
    }
    fn op_go_to_if(&mut self, block_id: BlockId) {
        let value = self.stack.pop().unwrap();
        if value == 1 {
            self.block_id = block_id;
            self.pc = 0;
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
    fn op_pop(&mut self) {
        self.stack.pop().unwrap();
    }
    fn op_bind_local(&mut self, local_id: usize) {
        let value = self.stack.pop().unwrap();
        self.locals[local_id] = value;
    }
    fn op_push_local(&mut self, local_id: usize) {
        let value = self.locals[local_id];
        self.stack.push(value);
    }
    fn op_bind_prop(&mut self, prop_id: PropId) {
        let ref_id = self.stack.pop().unwrap() as usize;
        let obj = &mut self.objects[ref_id];
        let prop_idx = self
            .bytecode
            .layouts
            .get(&obj.layout_id)
            .unwrap()
            .iter()
            .position(|&pid| pid == prop_id)
            .unwrap();

        let value = self.stack.pop().unwrap();
        obj.props[prop_idx] = value;
    }
    fn op_push_prop(&mut self, prop_id: PropId) {
        let ref_id = self.stack.pop().unwrap() as usize;
        let obj = &self.objects[ref_id];
        let prop_idx = self
            .bytecode
            .layouts
            .get(&obj.layout_id)
            .unwrap()
            .iter()
            .position(|&pid| pid == prop_id)
            .unwrap();
        let prop = obj.props[prop_idx];
        self.stack.push(prop);
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
    fn op_eq(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a == b) as i64)
    }
    fn op_ne(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a != b) as i64)
    }
    fn op_gt(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a > b) as i64)
    }
    fn op_lt(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a < b) as i64)
    }
    fn op_gte(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a >= b) as i64)
    }
    fn op_lte(&mut self) {
        let b = self.stack.pop().expect("value on stack");
        let a = self.stack.pop().expect("value on stack");
        self.stack.push((a <= b) as i64)
    }
    fn op_and(&mut self) {
        let b = self.stack.pop().expect("value on stack") == 1;
        let a = self.stack.pop().expect("value on stack") == 1;
        self.stack.push((a && b) as i64)
    }
    fn op_or(&mut self) {
        let b = self.stack.pop().expect("value on stack") == 1;
        let a = self.stack.pop().expect("value on stack") == 1;
        self.stack.push((a || b) as i64)
    }
    fn op_not(&mut self) {
        let a = self.stack.pop().expect("value on stack") == 1;
        self.stack.push((!a) as i64)
    }
    fn op_print(&mut self) {
        let value = self.stack.last().expect("value on stack");
        println!("{value}");
    }
    fn op_layout(&mut self, layout_id: LayoutId) {
        self.stack.push(layout_id.to_value());
    }
    fn op_malloc(&mut self, layout_id: LayoutId) {
        let prop_ids = self.bytecode.layouts.get(&layout_id).unwrap();
        let ref_id = self.objects.len();
        let mut props = vec![0; prop_ids.len()];
        for i in (0..prop_ids.len()).rev() {
            props[i] = self.stack.pop().unwrap();
        }
        self.objects.push(Object { layout_id, props });
        self.stack.push(ref_id as i64);
    }
}
impl VM<'_> {
    pub fn pop_from_stack(&mut self) -> i64 {
        self.stack.pop().unwrap()
    }
    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }
}
