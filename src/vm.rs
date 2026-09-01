use std::fmt::Write;

use crate::bytecode::{BlockId, ByteCode, DefId, LayoutId, LocalId, Op, OpCode, PropId};

const REF_ID_MASK: usize = 0x8000_0000_0000_0000;
const ARR_ID_MASK: usize = 0xc000_0000_0000_0000;

struct Object {
    layout_id: LayoutId,
    props: Vec<i64>,
}
impl Object {
    fn is_id(ref_id: i64) -> bool {
        (ref_id as usize) >> 60 == 0b1000
    }
    fn id_to_index(ref_id: i64) -> usize {
        assert_eq!(ref_id as usize >> 60, 0b1000);
        (ref_id as usize) ^ REF_ID_MASK
    }
    fn index_to_id(index: usize) -> i64 {
        let id = index ^ REF_ID_MASK;
        assert_eq!(id >> 60, 0b1000);
        id as i64
    }

    fn _prop_idx(&self, bytecode: &ByteCode, prop_id: PropId) -> usize {
        bytecode
            .layouts
            .get(&self.layout_id)
            .unwrap()
            .iter()
            .position(|&pid| pid == prop_id)
            .unwrap()
    }
    fn get_prop(&self, bytecode: &ByteCode, prop_id: PropId) -> i64 {
        self.props[self._prop_idx(bytecode, prop_id)]
    }
    fn get_prop_mut(&mut self, bytecode: &ByteCode, prop_id: PropId) -> &mut i64 {
        let prop_idx = self._prop_idx(bytecode, prop_id);
        &mut self.props[prop_idx]
    }
}

struct Array {
    elements: Vec<i64>,
}
impl Array {
    fn is_id(ref_id: i64) -> bool {
        (ref_id as usize) >> 60 == 0b1100
    }
    fn id_to_index(arr_id: i64) -> usize {
        assert_eq!(arr_id as usize >> 60, 0b1100);
        (arr_id as usize) ^ ARR_ID_MASK
    }
    fn index_to_id(index: usize) -> i64 {
        let id = index ^ ARR_ID_MASK;
        assert_eq!(id >> 60, 0b1100);
        id as i64
    }
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
    arrays: Vec<Array>,
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
            OpCode::BindLocal => Op::BindLocal(LocalId::new(self.next_word())),
            OpCode::PushLocal => Op::PushLocal(LocalId::new(self.next_word())),
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
            OpCode::EmptyArray => Op::EmptyArray,
            OpCode::ArrayGet => Op::ArrayGet,
            OpCode::ArraySet => Op::ArraySet,
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
        let arrays = vec![];
        Self {
            bytecode,
            block_id,
            pc,
            locals,
            stack,
            call_stack,
            objects,
            arrays,
        }
    }
    pub fn run(&mut self) {
        while let Some(op) = self.next() {
            self.step(&op);
        }
    }
    fn step(&mut self, op: &Op) {
        // eprintln!("EXEC {op:?}");
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
            Op::EmptyArray => self.op_empty_array(),
            Op::ArrayGet => self.op_array_get(),
            Op::ArraySet => self.op_array_set(),
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
    fn op_bind_local(&mut self, local_id: LocalId) {
        let value = self.stack.pop().unwrap();
        self.locals[local_id.to_index()] = value;
    }
    fn op_push_local(&mut self, local_id: LocalId) {
        let value = self.locals[local_id.to_index()];
        self.stack.push(value);
    }
    fn op_bind_prop(&mut self, prop_id: PropId) {
        let ref_id = Object::id_to_index(self.stack.pop().unwrap());
        let obj = &mut self.objects[ref_id];
        let value = self.stack.pop().unwrap();
        *obj.get_prop_mut(self.bytecode, prop_id) = value;
    }
    fn op_push_prop(&mut self, prop_id: PropId) {
        let ref_id = Object::id_to_index(self.stack.pop().unwrap());
        let obj = &self.objects[ref_id];
        let prop = obj.get_prop(self.bytecode, prop_id);
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
    fn _format_value(&self, w: &mut impl Write, value: i64) -> std::fmt::Result {
        if Object::is_id(value) {
            let obj = &self.objects[Object::id_to_index(value)];
            let layout = self.bytecode.layouts.get(&obj.layout_id).expect("layout");
            let mut layout_iter = layout.iter();
            write!(w, "{{")?;
            if let Some(first_prop_id) = layout_iter.next() {
                let prop_name = self.bytecode.prop_names.get(first_prop_id).unwrap();
                write!(w, "{prop_name}: ")?;
                self._format_value(w, obj.get_prop(self.bytecode, *first_prop_id))?;
                for prop_id in layout_iter {
                    let prop_name = self.bytecode.prop_names.get(prop_id).unwrap();
                    write!(w, ", {prop_name}: ")?;
                    self._format_value(w, obj.get_prop(self.bytecode, *prop_id))?;
                }
            }
            write!(w, "}}")?;
        } else if Array::is_id(value) {
            let array = &self.arrays[Array::id_to_index(value)];
            let mut elements = array.elements.iter();
            write!(w, "[")?;
            if let Some(first) = elements.next() {
                self._format_value(w, *first)?;
                for element in elements {
                    write!(w, ", ",)?;
                    self._format_value(w, *element)?;
                }
            }
            write!(w, "]")?;
        } else {
            write!(w, "{value}")?;
        }
        Ok(())
    }
    fn op_print(&mut self) {
        let value = *self.stack.last().expect("value on stack");
        let mut buf = String::new();
        self._format_value(&mut buf, value).unwrap();
        println!("{buf}");
    }
    fn op_layout(&mut self, layout_id: LayoutId) {
        self.stack.push(layout_id.to_value());
    }
    fn op_malloc(&mut self, layout_id: LayoutId) {
        let prop_ids = self.bytecode.layouts.get(&layout_id).unwrap();
        let ref_id = Object::index_to_id(self.objects.len());
        let mut props = vec![0; prop_ids.len()];
        for i in (0..prop_ids.len()).rev() {
            props[i] = self.stack.pop().unwrap();
        }
        self.objects.push(Object { layout_id, props });
        self.stack.push(ref_id);
    }
    fn op_empty_array(&mut self) {
        let len = self.stack.pop().unwrap() as usize;
        let array = Array {
            elements: vec![0; len],
        };
        let arr_id = Array::index_to_id(self.arrays.len());
        self.arrays.push(array);
        self.stack.push(arr_id);
    }
    fn op_array_get(&mut self) {
        let index = self.stack.pop().unwrap() as usize;
        let arr_id = Array::id_to_index(self.stack.pop().unwrap());
        let array = &self.arrays[arr_id];
        let element = array.elements[index];
        self.stack.push(element);
    }
    fn op_array_set(&mut self) {
        let value = self.stack.pop().unwrap();
        let index = self.stack.pop().unwrap() as usize;
        let arr_id = Array::id_to_index(self.stack.pop().unwrap());
        let array = &mut self.arrays[arr_id];
        array.elements[index] = value;
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
