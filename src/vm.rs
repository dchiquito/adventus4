use crate::source;
use std::{fmt::Write, ops::Range};

use crate::bytecode::{
    BlockId, ByteCode, ClosureId, DefId, LayoutId, LocalId, Op, OpCode, PropId, SourceRef,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorKind {
    EmptyStack,
    ValueEncoding(u64),
    TypeMismatch { expected: Value, actual: Value },
    NegativeNumber(i64),
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
    closures: Vec<DefId>,
    frame_with_locals: Option<usize>,
    closure_stack: Vec<ClosureFrame>,
}

pub struct VM<'a> {
    bytecode: &'a ByteCode,
    frame: StackFrame,
    stack: Vec<u64>,
    call_stack: Vec<StackFrame>,
    objects: Vec<Object>,
    arrays: Vec<Array>,
}

impl VM<'_> {
    fn next_word(&mut self) -> u64 {
        let word = self.bytecode.get_block(self.frame.block_id).data[self.frame.pc];
        self.frame.pc += 1;
        word
    }
}
impl Iterator for VM<'_> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if self.frame.pc >= self.bytecode.get_block(self.frame.block_id).data.len() {
            return None;
        }
        let opcode = OpCode::try_from(self.next_word()).expect("invalid opcode");
        let op = match opcode {
            OpCode::Integer => Op::Integer(self.next_word()),
            OpCode::Character => Op::Character(self.next_word() as u8),
            OpCode::Call => Op::Call(DefId::new(self.next_word())),
            OpCode::CallClosure => Op::CallClosure(ClosureId::new(self.next_word())),
            OpCode::Return => Op::Return,
            OpCode::ReturnClosure => Op::ReturnClosure,
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
        let frame = StackFrame {
            block_id,
            pc: 0,
            locals: bytecode.get_def(def_id).initialize_locals(),
            closures: vec![],
            frame_with_locals: None,
            closure_stack: vec![],
        };
        let stack = vec![];
        let call_stack = vec![];
        let objects = vec![];
        let arrays = vec![];
        Self {
            bytecode,
            frame,
            stack,
            call_stack,
            objects,
            arrays,
        }
    }
    pub fn run(&mut self) -> Result<()> {
        while let Some(op) = self.next() {
            self.step(&op)?;
        }
        Ok(())
    }
    fn step(&mut self, op: &Op) -> Result<()> {
        // eprintln!("EXEC {op:?}");
        match *op {
            Op::Integer(i) => self.op_integer(i),
            Op::Character(c) => self.op_character(c),
            Op::Call(def_id) => self.op_call(def_id)?,
            Op::CallClosure(closure_id) => self.op_call_closure(closure_id),
            Op::Return => self.op_return(),
            Op::ReturnClosure => self.op_return_closure(),
            Op::GoTo(block_id) => self.op_go_to(block_id),
            Op::GoToIf(block_id) => self.op_go_to_if(block_id)?,
            Op::Dup => self.op_dup(),
            Op::Swap => self.op_swap()?,
            Op::Pop => self.op_pop()?,
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
            Op::Layout(layout_id) => self.op_layout(layout_id),
            Op::Malloc(layout_id) => self.op_malloc(layout_id)?,
            Op::EmptyArray => self.op_empty_array()?,
            Op::ArrayGet => self.op_array_get()?,
            Op::ArraySet => self.op_array_set()?,
        }
        Ok(())
    }
}
impl VM<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        let source_ref = self
            .bytecode
            .source_map
            .get(self.frame.block_id, self.frame.pc.saturating_sub(1));
        Error { kind, source_ref }
    }
    pub fn pop_raw(&mut self) -> Result<u64> {
        self.stack.pop().ok_or(self.error(ErrorKind::EmptyStack))
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
    fn pop_char(&mut self) -> Result<u8> {
        match self.pop()? {
            Value::Char(c) => Ok(c),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::Char(0),
                actual: v,
            })),
        }
    }
    fn pop_integer(&mut self) -> Result<i64> {
        match self.pop()? {
            Value::Integer(i) => Ok(i),
            v => Err(self.error(ErrorKind::TypeMismatch {
                expected: Value::Integer(0),
                actual: v,
            })),
        }
    }
    fn pop_index(&mut self) -> Result<usize> {
        let index = self.pop_integer()?;
        if index < 0 {
            return Err(self.error(ErrorKind::NegativeNumber(index)));
        }
        Ok(index as usize)
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
impl VM<'_> {
    fn op_integer(&mut self, i: u64) {
        self.push(Value::Integer(i as i64));
    }
    fn op_character(&mut self, c: u8) {
        self.push(Value::Char(c));
    }
    fn op_call(&mut self, def_id: DefId) -> Result<()> {
        let def = self.bytecode.get_def(def_id);
        let mut closures = vec![];
        for _ in 0..def.closure_count {
            // TODO encode as something better
            let closure_def_id = DefId::new(self.pop_integer()? as u64);
            closures.push(closure_def_id);
        }
        let mut frame = StackFrame {
            block_id: def.block_id,
            pc: 0,
            locals: def.initialize_locals(),
            closures,
            frame_with_locals: None,
            closure_stack: vec![],
        };
        std::mem::swap(&mut self.frame, &mut frame);
        self.call_stack.push(frame);
        Ok(())
    }
    fn op_call_closure(&mut self, closure_id: ClosureId) {
        let def_id = self.frame.closures[closure_id.to_index()];
        let def = self.bytecode.get_def(def_id);
        let closure_frame = ClosureFrame {
            block_id: self.frame.block_id,
            pc: self.frame.pc,
        };
        self.frame.block_id = def.block_id;
        self.frame.pc = 0;
        self.frame.closure_stack.push(closure_frame);
        self.frame.frame_with_locals = Some(self.call_stack.len() - 1);
    }
    fn op_return(&mut self) {
        if let Some(frame) = self.call_stack.pop() {
            self.frame = frame;
        } else {
            panic!("Done");
        }
    }
    fn op_return_closure(&mut self) {
        let closure_frame = self
            .frame
            .closure_stack
            .pop()
            .expect("must be in a closure");
        self.frame.block_id = closure_frame.block_id;
        self.frame.pc = closure_frame.pc;
        self.frame.frame_with_locals = None;
    }
    fn op_go_to(&mut self, block_id: BlockId) {
        self.frame.block_id = block_id;
        self.frame.pc = 0;
    }
    fn op_go_to_if(&mut self, block_id: BlockId) -> Result<()> {
        let value = self.pop_bool()?;
        if value {
            self.frame.block_id = block_id;
            self.frame.pc = 0;
        }
        Ok(())
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
    fn op_pop(&mut self) -> Result<()> {
        self.pop()?;
        Ok(())
    }
    fn op_bind_local(&mut self, local_id: LocalId) -> Result<()> {
        let value = self.pop()?;
        let locals = if let Some(idx) = self.frame.frame_with_locals {
            &mut self.call_stack[idx].locals
        } else {
            &mut self.frame.locals
        };
        locals[local_id.to_index()] = u64::from(value);
        Ok(())
    }
    fn op_push_local(&mut self, local_id: LocalId) {
        let locals = if let Some(idx) = self.frame.frame_with_locals {
            &self.call_stack[idx].locals
        } else {
            &self.frame.locals
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
    fn _format_value(&self, w: &mut impl Write, value: Value) -> std::fmt::Result {
        match value {
            Value::Bool(b) => write!(w, "{b}")?,
            Value::Char(c) => write!(w, "{}", c as char)?,
            Value::Integer(i) => write!(w, "{i}")?,
            Value::ObjectRef(obj_ref) => {
                let obj = &self.objects[obj_ref];
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
            Value::LayoutId(id) => write!(w, "LayouId({id})")?,
        }
        Ok(())
    }
    fn op_print(&mut self) -> Result<()> {
        let value = self.pop()?;
        self.push(value.clone());
        let mut buf = String::new();
        self._format_value(&mut buf, value).unwrap();
        println!("{buf}");
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
        let len = self.pop_index()?;
        let array = Array {
            elements: vec![0; len],
        };
        let arr_id = Value::ArrayRef(self.arrays.len());
        self.arrays.push(array);
        self.push(arr_id);
        Ok(())
    }
    fn op_array_get(&mut self) -> Result<()> {
        let index = self.pop_index()?;
        let arr_id = self.pop_array_ref()?;
        let array = &self.arrays[arr_id];
        let element = array.elements[index];
        self.stack.push(element);
        Ok(())
    }
    fn op_array_set(&mut self) -> Result<()> {
        let value = self.pop()?;
        let index = self.pop_index()?;
        let arr_id = self.pop_array_ref()?;
        let array = &mut self.arrays[arr_id];
        array.elements[index] = u64::from(value);
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
