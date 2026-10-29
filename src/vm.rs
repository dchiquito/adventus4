use log::trace;

use crate::{bytecode::ClosureId, source};
use std::{fmt::Write, io::BufRead, ops::Range, slice::Iter};

use crate::bytecode::{
    BlockId, ByteCode, ClosureVar, DefId, LayoutId, LocalId, Op, OpCode, PropId, SourceRef,
    Terminator,
};

#[derive(Debug)]
pub enum Step {
    Op(Op),
    Terminator(Terminator),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorKind {
    EmptyStack,
    ValueEncoding(u64),
    TypeMismatch { expected: Value, actual: Value },
    NegativeNumber(i64),
    IndexOutOfBounds { index: i64, len: usize },
    Unknown,
}
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source_ref: SourceRef,
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)?;
        Ok(())
    }
}
impl Error {
    pub fn format(&self, f: &mut impl std::io::Write, bytecode: &ByteCode) -> std::io::Result<()> {
        let SourceRef {
            source_id: id,
            range,
        } = self.source_ref.clone();
        let name = bytecode.source_map.get_source_name(id);
        let source = source::read_source_file(name);
        let line_number = self.get_line_number(&source);
        let line_range = self.get_source_line_extents(&source);
        let column_number = 1 + range.start - line_range.start;
        writeln!(f, "Run time error: {:?}", self.kind)?;
        writeln!(
            f,
            "- {name}:{line_number}:{column_number}: {}",
            &source[range]
        )?;
        writeln!(f, "{}", &source[line_range])?;
        Ok(())
    }
    fn get_source_line_extents(&self, source: &str) -> Range<usize> {
        let range = &self.source_ref.range;
        let start_offset = source.as_bytes()[..range.start]
            .iter()
            .rev()
            .position(|&c| c == b'\n')
            .unwrap_or(range.start);
        let end_offset = source.as_bytes()[range.end..]
            .iter()
            .position(|&c| c == b'\n')
            .unwrap_or(source.len() - range.end);
        (range.start - start_offset)..(range.end + end_offset)
    }
    fn get_line_number(&self, source: &str) -> usize {
        let mut lines = 1;
        for i in 0..self.source_ref.range.start {
            if source.as_bytes()[i] == b'\n' {
                lines += 1;
            }
        }
        lines
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Bool(bool),
    Char(u8),
    Integer(i64),
    ObjectRef(usize),
    ArrayRef(usize),
    LayoutId(u64),
    ClosureId(ClosureId),
}
impl TryFrom<u64> for Value {
    type Error = ErrorKind;
    fn try_from(value: u64) -> std::result::Result<Self, ErrorKind> {
        let stripped_value = value & 0x0fff_ffff_ffff_ffff;
        let control_bits = value >> 60;
        assert!(control_bits <= 0b1111);
        Ok(match control_bits {
            0b1111 | 0b0000 => Self::Integer(value as i64),
            0b0100 => match stripped_value {
                0 => Self::Bool(false),
                1 => Self::Bool(true),
                _ => return Err(ErrorKind::ValueEncoding(value)),
            },
            0b0101 => match stripped_value {
                0x0..0xff => Self::Char(stripped_value as u8),
                _ => return Err(ErrorKind::ValueEncoding(value)),
            },
            0b1000 => Self::ObjectRef(stripped_value as usize),
            0b1001 => Self::ArrayRef(stripped_value as usize),
            0b1010 => Self::LayoutId(stripped_value),
            0b1011 => Self::ClosureId(ClosureId::new(stripped_value)),
            0b10000..=u64::MAX => unreachable!(),
            _ => return Err(ErrorKind::ValueEncoding(value)),
        })
    }
}
impl From<Value> for u64 {
    fn from(value: Value) -> Self {
        match value {
            Value::Bool(b) => {
                if b {
                    0x4000_0000_0000_0001
                } else {
                    0x4000_0000_0000_0000
                }
            }
            Value::Char(c) => (c as u64) ^ (0b0101 << 60),
            Value::Integer(i) => {
                let i = i as u64;
                let control_bits = i >> 60;
                assert!(control_bits == 0b0000 || control_bits == 0b1111);
                i
            }
            Value::ObjectRef(idx) => {
                assert_eq!(idx >> 60, 0);
                (idx ^ (0b1000 << 60)) as u64
            }
            Value::ArrayRef(idx) => {
                assert_eq!(idx >> 60, 0);
                (idx ^ (0b1001 << 60)) as u64
            }
            Value::LayoutId(id) => {
                assert_eq!(id >> 60, 0);
                id ^ (0b1010 << 60)
            }
            Value::ClosureId(id) => {
                let id = id.to_value();
                assert_eq!(id >> 60, 0);
                id ^ (0b1011 << 60)
            }
        }
    }
}

struct Object {
    layout_id: LayoutId,
    props: Vec<u64>,
}
impl Object {
    fn _prop_idx(&self, bytecode: &ByteCode, prop_id: PropId) -> usize {
        bytecode
            .layouts
            .get(&self.layout_id)
            .unwrap()
            .iter()
            .position(|&pid| pid == prop_id)
            .unwrap()
    }
    fn get_prop(&self, bytecode: &ByteCode, prop_id: PropId) -> u64 {
        self.props[self._prop_idx(bytecode, prop_id)]
    }
    fn get_prop_mut(&mut self, bytecode: &ByteCode, prop_id: PropId) -> &mut u64 {
        let prop_idx = self._prop_idx(bytecode, prop_id);
        &mut self.props[prop_idx]
    }
}

struct Array {
    len: usize,
    elements: Vec<u64>,
}

struct ClosureFrame {
    block_id: BlockId,
    pc: usize,
}
struct StackFrame {
    block_id: BlockId,
    pc: usize,
    locals: Vec<u64>,
    closures: Vec<ClosureId>,
    frame_with_locals: Option<usize>,
    closure_stack: Vec<ClosureFrame>,
}

pub trait IO {
    fn read_line(&mut self) -> Vec<u64>;
    fn write(&mut self, data: &str);
}

#[derive(Default)]
pub struct StdIO {}
impl IO for StdIO {
    fn read_line(&mut self) -> Vec<u64> {
        let mut buf = String::new();
        {
            let mut stdin = std::io::stdin().lock();
            stdin.read_line(&mut buf).expect("error reading stdin");
        }
        buf.chars().map(|c| Value::Char(c as u8).into()).collect()
    }

    fn write(&mut self, data: &str) {
        println!("{data}");
    }
}

struct StringReader<'a> {
    iter: Iter<'a, u8>,
}

impl<'a> StringReader<'a> {
    pub fn new(data: &'a str) -> Self {
        Self {
            iter: data.as_bytes().iter(),
        }
    }
    fn read_line(&mut self) -> Vec<u64> {
        let mut buf = vec![];
        for x in self.iter.by_ref() {
            buf.push(Value::Char(*x).into());
            if *x == b'\n' {
                return buf;
            }
        }
        buf
    }
}
pub struct TestIO<'a> {
    input_buffer: StringReader<'a>,
    output: String,
}
impl<'a> TestIO<'a> {
    pub fn new(input: &'a str) -> Self {
        let input_buffer = StringReader::new(input);
        Self {
            input_buffer,
            output: String::new(),
        }
    }
    pub fn get_output(&self) -> &str {
        &self.output
    }
}
impl IO for TestIO<'_> {
    fn read_line(&mut self) -> Vec<u64> {
        self.input_buffer.read_line()
    }

    fn write(&mut self, data: &str) {
        self.output.push_str(data);
        self.output.push('\n');
    }
}

pub struct VM<'a, T: IO> {
    io: T,
    bytecode: &'a ByteCode,
    stack: Vec<u64>,
    call_stack: Vec<StackFrame>,
    objects: Vec<Object>,
    arrays: Vec<Array>,
}

impl<T: IO> VM<'_, T> {
    fn next_word(&mut self) -> Option<u64> {
        let frame = self.call_stack.last_mut()?;
        let word = self.bytecode.get_block(frame.block_id).data[frame.pc];
        frame.pc += 1;
        Some(word)
    }
}
impl<T: IO> Iterator for VM<'_, T> {
    type Item = Step;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.call_stack.last_mut()?;
        let block = self.bytecode.get_block(frame.block_id);
        if frame.pc >= block.data.len() {
            return Some(Step::Terminator(block.terminator.clone()));
        }
        let opcode = OpCode::try_from(self.next_word()?).expect("invalid opcode");
        let op = match opcode {
            OpCode::Integer => Op::Integer(self.next_word()?),
            OpCode::Character => Op::Character(self.next_word()? as u8),
            OpCode::Call => Op::Call(DefId::new(self.next_word()?)),
            OpCode::CallClosure => Op::CallClosure(ClosureVar::new(self.next_word()?)),
            OpCode::ClosureDefId => Op::ClosureId(ClosureId::new(self.next_word()?)),
            OpCode::Dup => Op::Dup,
            OpCode::Swap => Op::Swap,
            OpCode::Drop => Op::Drop,
            OpCode::BindLocal => Op::BindLocal(LocalId::new(self.next_word()?)),
            OpCode::PushLocal => Op::PushLocal(LocalId::new(self.next_word()?)),
            OpCode::BindProp => Op::BindProp(PropId::new(self.next_word()?)),
            OpCode::PushProp => Op::PushProp(PropId::new(self.next_word()?)),
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
            OpCode::PrintStr => Op::PrintStr,
            OpCode::ReadLine => Op::ReadLine,
            OpCode::ObjectId => Op::Layout(LayoutId::new(self.next_word()?)),
            OpCode::Malloc => Op::Malloc(LayoutId::new(self.next_word()?)),
            OpCode::EmptyArray => Op::EmptyArray,
            OpCode::ArrayGet => Op::ArrayGet,
            OpCode::ArraySet => Op::ArraySet,
        };
        Some(Step::Op(op))
    }
}
impl<'a, T: IO> VM<'a, T> {
    pub fn new(bytecode: &'a ByteCode, io: T) -> Self {
        let stack = vec![];
        let call_stack = vec![];
        let objects = vec![];
        let arrays = vec![];
        Self {
            io,
            bytecode,
            stack,
            call_stack,
            objects,
            arrays,
        }
    }
    pub fn push_main(&mut self) {
        let def_id = self.bytecode.main_id.expect("no main definition");
        self.push_def(def_id);
    }
    pub fn push_def(&mut self, def_id: DefId) {
        let def = self.bytecode.get_def(def_id);
        let block_id = def.block_id;
        let frame = StackFrame {
            block_id,
            pc: 0,
            locals: def.initialize_locals(),
            closures: vec![],
            frame_with_locals: None,
            closure_stack: vec![],
        };
        self.call_stack.push(frame);
    }
    pub fn push_block(&mut self, block_id: BlockId) {
        let frame = StackFrame {
            block_id,
            pc: 0,
            locals: vec![],
            closures: vec![],
            frame_with_locals: None,
            closure_stack: vec![],
        };
        self.call_stack.push(frame);
    }
    pub fn push_string(&mut self, string: &str) -> Result<()> {
        let list_layout_id = self.bytecode.get_list_layout_id();
        let cap = Value::Integer(string.len() as i64);
        let len = Value::Integer(string.len() as i64);
        // Construct a new array of the correcct length
        let arr_id = {
            self.push(len.clone());
            self.op_empty_array()?;
            self.pop_array_ref()?
        };
        // Fill the array
        let arr = &mut self.arrays[arr_id];
        for (i, &b) in string.as_bytes().iter().enumerate() {
            arr.elements[i] = u64::from(Value::Char(b));
        }
        // Construct a new list
        self.push(Value::ArrayRef(arr_id));
        self.push(cap);
        self.push(len);
        self.op_malloc(list_layout_id)?;
        Ok(())
    }
    pub fn get_io(&self) -> &T {
        &self.io
    }
    pub fn run(&mut self) -> Result<()> {
        while let Some(step) = self.next() {
            self.step(&step)?;
        }
        Ok(())
    }
    fn step(&mut self, step: &Step) -> Result<()> {
        match step {
            Step::Op(op) => self.step_op(op),
            Step::Terminator(terminator) => self.step_terminator(terminator),
        }
    }
    fn step_op(&mut self, op: &Op) -> Result<()> {
        trace!("Step: {op:?}");
        match *op {
            Op::Integer(i) => self.op_integer(i),
            Op::Character(c) => self.op_character(c),
            Op::Call(def_id) => self.op_call(def_id)?,
            Op::CallClosure(closure_var) => self.op_call_closure(closure_var),
            Op::ClosureId(closure_id) => self.op_closure_id(closure_id),
            Op::Dup => self.op_dup(),
            Op::Swap => self.op_swap()?,
            Op::Drop => self.op_drop()?,
            Op::BindLocal(local_id) => self.op_bind_local(local_id)?,
            Op::PushLocal(local_id) => self.op_push_local(local_id),
            Op::BindProp(prop_id) => self.op_bind_prop(prop_id)?,
            Op::PushProp(prop_id) => self.op_push_prop(prop_id)?,
            Op::Add => self.op_add()?,
            Op::Sub => self.op_sub()?,
            Op::Mul => self.op_mul()?,
            Op::Div => self.op_div()?,
            Op::Eq => self.op_eq()?,
            Op::Ne => self.op_ne()?,
            Op::Gt => self.op_gt()?,
            Op::Lt => self.op_lt()?,
            Op::Gte => self.op_gte()?,
            Op::Lte => self.op_lte()?,
            Op::And => self.op_and()?,
            Op::Or => self.op_or()?,
            Op::Not => self.op_not()?,
            Op::Print => self.op_print()?,
            Op::PrintStr => self.op_print_str()?,
            Op::ReadLine => self.op_read_line()?,
            Op::Layout(layout_id) => self.op_layout(layout_id),
            Op::Malloc(layout_id) => self.op_malloc(layout_id)?,
            Op::EmptyArray => self.op_empty_array()?,
            Op::ArrayGet => self.op_array_get()?,
            Op::ArraySet => self.op_array_set()?,
        }
        Ok(())
    }
    fn step_terminator(&mut self, op: &Terminator) -> Result<()> {
        match *op {
            Terminator::Return => self.terminator_return(),
            Terminator::ReturnClosure => self.terminator_return_closure(),
            Terminator::GoTo(block_id) => self.terminator_go_to(block_id),
            Terminator::GoToIf {
                then: then_block_id,
                or_else: else_block_id,
            } => self.terminator_go_to_if(then_block_id, else_block_id)?,
        }
        Ok(())
    }
}
impl<T: IO> VM<'_, T> {
    fn error(&self, kind: ErrorKind) -> Error {
        let frame = self.call_stack.last().expect("nothing on call stack");
        let source_ref = self
            .bytecode
            .source_map
            .get(frame.block_id, frame.pc.saturating_sub(1));
        Error { kind, source_ref }
    }
    pub fn pop_raw(&mut self) -> Result<u64> {
        self.stack
            .pop()
            .ok_or_else(|| self.error(ErrorKind::EmptyStack))
    }
    fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or(self.error(ErrorKind::EmptyStack))
            .and_then(|v| Value::try_from(v).map_err(|kind| self.error(kind)))
    }
    fn pop_bool(&mut self) -> Result<bool> {
        match self.pop()? {
            Value::Bool(b) => Ok(b),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::Bool(false),
                actual: v,
            })),
        }
    }
    // fn pop_char(&mut self) -> Result<u8> {
    //     match self.pop()? {
    //         Value::Char(c) => Ok(c),
    //         v => Err(self.error(ErrorKind::TypeMismatch {
    //             expected: Value::Char(0),
    //             actual: v,
    //         })),
    //     }
    // }
    fn pop_integer(&mut self) -> Result<i64> {
        match self.pop()? {
            Value::Integer(i) => Ok(i),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::Integer(0),
                actual: v,
            })),
        }
    }
    fn pop_closure_id(&mut self) -> Result<ClosureId> {
        match self.pop()? {
            Value::ClosureId(closure_id) => Ok(closure_id),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::ClosureId(ClosureId::new(0)),
                actual: v,
            })),
        }
    }
    fn pop_object_ref(&mut self) -> Result<usize> {
        match self.pop()? {
            Value::ObjectRef(idx) => Ok(idx),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::ObjectRef(0),
                actual: v,
            })),
        }
    }
    fn pop_array_ref(&mut self) -> Result<usize> {
        match self.pop()? {
            Value::ArrayRef(idx) => Ok(idx),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::ArrayRef(0),
                actual: v,
            })),
        }
    }
    fn push(&mut self, value: Value) {
        self.stack.push(u64::from(value));
    }
    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }
}
impl<T: IO> VM<'_, T> {
    fn op_integer(&mut self, i: u64) {
        self.push(Value::Integer(i as i64));
    }
    fn op_character(&mut self, c: u8) {
        self.push(Value::Char(c));
    }
    fn op_call(&mut self, def_id: DefId) -> Result<()> {
        let def = self.bytecode.get_def(def_id);
        let mut closures = Vec::with_capacity(def.closure_count);
        for _ in 0..def.closure_count {
            let closure_def_id = self.pop_closure_id()?;
            closures.push(closure_def_id);
        }
        // Consider `def foo @a @b @c [...]`
        // The stack will contain [..., @a, @b, @c], so the def_ids will be popped
        // in reverse order. Reverse them again to get the correct order back.
        closures.reverse();
        let frame = StackFrame {
            block_id: def.block_id,
            pc: 0,
            locals: def.initialize_locals(),
            closures,
            frame_with_locals: None,
            closure_stack: vec![],
        };
        self.call_stack.push(frame);
        Ok(())
    }
    fn op_call_closure(&mut self, closure_var: ClosureVar) {
        let call_stack_len = self.call_stack.len();
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        let closure_id = frame.closures[closure_var.to_index()];
        let closure = self.bytecode.get_closure(closure_id);
        let closure_frame = ClosureFrame {
            block_id: frame.block_id,
            pc: frame.pc,
        };
        frame.block_id = closure.block_id;
        frame.pc = 0;
        frame.closure_stack.push(closure_frame);
        frame.frame_with_locals = Some(call_stack_len - 1);
    }
    fn op_closure_id(&mut self, closure_id: ClosureId) {
        self.push(Value::ClosureId(closure_id));
    }
    fn op_dup(&mut self) {
        let peek = *self.stack.last().unwrap();
        self.stack.push(peek);
    }
    fn op_swap(&mut self) -> Result<()> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(b);
        self.push(a);
        Ok(())
    }
    fn op_drop(&mut self) -> Result<()> {
        self.pop()?;
        Ok(())
    }
    fn op_bind_local(&mut self, local_id: LocalId) -> Result<()> {
        // TODO Result
        let value = self.pop()?;
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        let locals = if let Some(idx) = frame.frame_with_locals {
            &mut self.call_stack[idx].locals
        } else {
            &mut frame.locals
        };
        locals[local_id.to_index()] = u64::from(value);
        Ok(())
    }
    fn op_push_local(&mut self, local_id: LocalId) {
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        let locals = if let Some(idx) = frame.frame_with_locals {
            &self.call_stack[idx].locals
        } else {
            &frame.locals
        };
        let value = locals[local_id.to_index()];
        self.stack.push(value);
    }
    fn op_bind_prop(&mut self, prop_id: PropId) -> Result<()> {
        let ref_id = self.pop_object_ref()?;
        let value = self.pop()?;
        let obj = &mut self.objects[ref_id];
        *obj.get_prop_mut(self.bytecode, prop_id) = u64::from(value);
        Ok(())
    }
    fn op_push_prop(&mut self, prop_id: PropId) -> Result<()> {
        let ref_id = self.pop_object_ref()?;
        let obj = &self.objects[ref_id];
        let prop = obj.get_prop(self.bytecode, prop_id);
        self.stack.push(prop);
        Ok(())
    }
    fn op_add(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Integer(a + b));
        Ok(())
    }
    fn op_sub(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Integer(a - b));
        Ok(())
    }
    fn op_mul(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Integer(a * b));
        Ok(())
    }
    fn op_div(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Integer(a / b));
        Ok(())
    }
    fn op_eq(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a == b));
        Ok(())
    }
    fn op_ne(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a != b));
        Ok(())
    }
    fn op_gt(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a > b));
        Ok(())
    }
    fn op_lt(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a < b));
        Ok(())
    }
    fn op_gte(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a >= b));
        Ok(())
    }
    fn op_lte(&mut self) -> Result<()> {
        let b = self.pop_integer()?;
        let a = self.pop_integer()?;
        self.push(Value::Bool(a <= b));
        Ok(())
    }
    fn op_and(&mut self) -> Result<()> {
        let b = self.pop_bool()?;
        let a = self.pop_bool()?;
        self.push(Value::Bool(a && b));
        Ok(())
    }
    fn op_or(&mut self) -> Result<()> {
        let b = self.pop_bool()?;
        let a = self.pop_bool()?;
        self.push(Value::Bool(a || b));
        Ok(())
    }
    fn op_not(&mut self) -> Result<()> {
        let a = self.pop_bool()?;
        self.push(Value::Bool(!a));
        Ok(())
    }
    fn _format_string(&self, w: &mut impl Write, value: Value) -> std::fmt::Result {
        if let Value::ObjectRef(obj_ref) = value {
            let obj = &self.objects[obj_ref];
            // Look up the layout_id for List
            let list_layout_id = self.bytecode.get_list_layout_id();
            // If the object is a list, print only the internal arr
            if obj.layout_id == list_layout_id {
                let arr = Value::try_from(obj.props[0]).expect("invalid value");
                let len = Value::try_from(obj.props[2]).expect("invalid value");
                if let Value::ArrayRef(array_ref) = arr
                    && let Value::Integer(len) = len
                {
                    let len = len as usize;
                    let array = &self.arrays[array_ref];
                    for c in array
                        .elements
                        .iter()
                        .take(len)
                        .copied()
                        .map(Value::try_from)
                        .map(|v| {
                            if let Ok(Value::Char(c)) = v {
                                c
                            } else {
                                unreachable!()
                            }
                        })
                    {
                        write!(w, "{}", c as char)?
                    }
                    return Ok(());
                }
            }
        }
        panic!("not a string: {value:?}")
    }
    fn _format_value(&self, w: &mut impl Write, value: Value) -> std::fmt::Result {
        match value {
            Value::Bool(b) => write!(w, "{b}")?,
            Value::Char(c) => write!(w, "{}", c as char)?,
            Value::Integer(i) => write!(w, "{i}")?,
            Value::ObjectRef(obj_ref) => {
                let obj = &self.objects[obj_ref];
                // Look up the layout_id for List
                let list_layout_id = self.bytecode.get_list_layout_id();
                // If the object is a list, print only the internal arr
                if obj.layout_id == list_layout_id {
                    let arr = Value::try_from(obj.props[0]).expect("invalid value");
                    let len = Value::try_from(obj.props[2]).expect("invalid value");
                    if let Value::ArrayRef(array_ref) = arr
                        && let Value::Integer(len) = len
                    {
                        let len = len as usize;
                        let array = &self.arrays[array_ref];
                        let mut elements = array.elements.iter().take(len);
                        write!(w, "[")?;
                        if let Some(first) = elements.next() {
                            self._format_value(w, Value::try_from(*first).expect("invalid value"))?;
                            for element in elements {
                                write!(w, ", ",)?;
                                self._format_value(
                                    w,
                                    Value::try_from(*element).expect("invalid value"),
                                )?;
                            }
                        }
                        write!(w, "]")?;
                        return Ok(());
                    } else {
                        unreachable!("list prop types are incorrect")
                    }
                }
                let layout = self.bytecode.layouts.get(&obj.layout_id).expect("layout");
                let mut layout_iter = layout.iter();
                write!(w, "{{")?;
                if let Some(first_prop_id) = layout_iter.next() {
                    let prop_name = self.bytecode.prop_names.get(first_prop_id).unwrap();
                    write!(w, "{prop_name}: ")?;
                    self._format_value(
                        w,
                        Value::try_from(obj.get_prop(self.bytecode, *first_prop_id))
                            .expect("invalid value"),
                    )?;
                    for prop_id in layout_iter {
                        let prop_name = self.bytecode.prop_names.get(prop_id).unwrap();
                        write!(w, ", {prop_name}: ")?;
                        self._format_value(
                            w,
                            Value::try_from(obj.get_prop(self.bytecode, *prop_id))
                                .expect("invalid value"),
                        )?;
                    }
                }
                write!(w, "}}")?;
            }
            Value::ArrayRef(arr_ref) => {
                let array = &self.arrays[arr_ref];
                let mut elements = array.elements.iter();
                write!(w, "[")?;
                if let Some(first) = elements.next() {
                    self._format_value(w, Value::try_from(*first).expect("invalid value"))?;
                    for element in elements {
                        write!(w, ", ",)?;
                        self._format_value(w, Value::try_from(*element).expect("invalid value"))?;
                    }
                }
                write!(w, "]")?;
            }
            Value::LayoutId(id) => write!(w, "LayoutId({id})")?,
            Value::ClosureId(id) => write!(w, "ClosureDefId({id:?})")?,
        }
        Ok(())
    }
    fn op_print(&mut self) -> Result<()> {
        let value = self.pop()?;
        let mut buf = String::new();
        self._format_value(&mut buf, value).unwrap();
        self.io.write(&buf);
        Ok(())
    }
    fn op_print_str(&mut self) -> Result<()> {
        let value = self.pop()?;
        let mut buf = String::new();
        self._format_string(&mut buf, value).unwrap();
        self.io.write(&buf);
        Ok(())
    }
    fn op_read_line(&mut self) -> Result<()> {
        let line = self.io.read_line();
        let len = line.len();
        let array = Array {
            len,
            elements: line,
        };
        let arr_id = Value::ArrayRef(self.arrays.len());
        self.arrays.push(array);
        self.push(arr_id);
        self.push(Value::Integer(len as i64));
        self.push(Value::Integer(len as i64));
        let layout_id = self.bytecode.get_list_layout_id();
        self.op_malloc(layout_id)?;
        Ok(())
    }
    fn op_layout(&mut self, layout_id: LayoutId) {
        self.stack.push(layout_id.to_value());
    }
    fn op_malloc(&mut self, layout_id: LayoutId) -> Result<()> {
        let prop_ids = self.bytecode.layouts.get(&layout_id).unwrap();
        let ref_id = Value::ObjectRef(self.objects.len());
        let mut props = vec![0; prop_ids.len()];
        for i in (0..prop_ids.len()).rev() {
            props[i] = u64::from(self.pop()?);
        }
        self.objects.push(Object { layout_id, props });
        self.push(ref_id);
        Ok(())
    }
    fn op_empty_array(&mut self) -> Result<()> {
        let len = self.pop_integer()?;
        if len < 0 {
            return Err(self.error(ErrorKind::NegativeNumber(len)));
        }
        let len = len as usize;
        let array = Array {
            len,
            elements: vec![0; len],
        };
        let arr_id = Value::ArrayRef(self.arrays.len());
        self.arrays.push(array);
        self.push(arr_id);
        Ok(())
    }
    fn op_array_get(&mut self) -> Result<()> {
        let index = self.pop_integer()?;
        let arr_id = self.pop_array_ref()?;
        let array = &self.arrays[arr_id];
        let len = array.len;
        if index < 0 || len as i64 <= index {
            return Err(self.error(ErrorKind::IndexOutOfBounds { index, len }));
        }
        let index = index as usize;
        let element = array.elements[index];
        self.stack.push(element);
        Ok(())
    }
    fn op_array_set(&mut self) -> Result<()> {
        let value = self.pop()?;
        let index = self.pop_integer()?;
        let arr_id = self.pop_array_ref()?;
        let array = &mut self.arrays[arr_id];
        let len = array.len;
        if index < 0 || len as i64 <= index {
            return Err(self.error(ErrorKind::IndexOutOfBounds { index, len }));
        }
        let index = index as usize;
        array.elements[index] = u64::from(value);
        Ok(())
    }
    fn terminator_return(&mut self) {
        if self.call_stack.pop().is_none() {
            panic!("Done");
        }
    }
    fn terminator_return_closure(&mut self) {
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        let closure_frame = frame.closure_stack.pop().expect("must be in a closure");
        frame.block_id = closure_frame.block_id;
        frame.pc = closure_frame.pc;
        frame.frame_with_locals = None;
    }
    fn terminator_go_to(&mut self, block_id: BlockId) {
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        frame.block_id = block_id;
        frame.pc = 0;
    }
    fn terminator_go_to_if(
        &mut self,
        then_block_id: BlockId,
        else_block_id: BlockId,
    ) -> Result<()> {
        let value = self.pop_bool()?;
        let frame = self.call_stack.last_mut().expect("nothing on call stack");
        frame.block_id = if value { then_block_id } else { else_block_id };
        frame.pc = 0;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_value_encoding() {
        for high_bits in 0_u64..255_u64 {
            for low_bits in 0_u64..1023_u64 {
                let encoded = (high_bits << 56) + low_bits;
                if let Ok(value) = Value::try_from(encoded) {
                    assert_eq!(encoded, u64::from(value));
                }
            }
        }
    }
    macro_rules! assert_encoding_of {
        ($value:expr) => {
            assert_eq!(Ok($value), Value::try_from(u64::from($value)));
        };
    }
    #[test]
    fn test_value_decode_bool() {
        assert_encoding_of!(Value::Bool(true));
        assert_encoding_of!(Value::Bool(false));
    }
    #[test]
    fn test_value_decode_char() {
        for c in 0..255 {
            assert_encoding_of!(Value::Char(c));
        }
    }
    #[test]
    fn test_value_decode_int() {
        for i in -1024..1024 {
            assert_encoding_of!(Value::Integer(i));
            // Integers are conveniently encoded as themselves
            assert_eq!(i as u64, u64::from(Value::Integer(i)));
        }
    }
    #[test]
    fn test_value_decode_int_max() {
        let max_int = 0x0fff_ffff_ffff_ffff;
        assert_encoding_of!(Value::Integer(max_int));
        assert_eq!(max_int as u64, u64::from(Value::Integer(max_int)));
        assert_eq!(
            Err(ErrorKind::ValueEncoding((max_int + 1) as u64)),
            Value::try_from((max_int + 1) as u64)
        );
    }
    #[test]
    fn test_value_decode_int_min() {
        let min_int = 0xf000_0000_0000_0000_u64 as i64;
        assert_encoding_of!(Value::Integer(min_int));
        assert_eq!(min_int as u64, u64::from(Value::Integer(min_int)));
        assert_eq!(
            Err(ErrorKind::ValueEncoding((min_int - 1) as u64)),
            Value::try_from((min_int - 1) as u64)
        );
    }
    #[test]
    fn test_value_decode_object_ref() {
        for i in 0..1024 {
            assert_encoding_of!(Value::ObjectRef(i));
        }
    }
    #[test]
    fn test_value_decode_array_ref() {
        for i in 0..1024 {
            assert_encoding_of!(Value::ArrayRef(i));
        }
    }
    #[test]
    fn test_value_decode_layout_id() {
        for i in 0..1024 {
            assert_encoding_of!(Value::LayoutId(i));
        }
    }
}
