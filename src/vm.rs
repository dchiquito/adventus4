use std::fmt::Write;

use crate::bytecode::{BlockId, ByteCode, DefId, LayoutId, LocalId, Op, OpCode, PropId};

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
    range: std::ops::Range<usize>,
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)?;
        Ok(())
    }
}
impl Error {
    pub fn get_source_string<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range.clone()]
    }
    pub fn get_line_number(&self, source: &str) -> usize {
        let mut lines = 1;
        for i in 0..self.range.start {
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

struct StackFrame {
    block_id: BlockId,
    pc: usize,
    locals: Vec<u64>,
}

pub struct VM<'a> {
    bytecode: &'a ByteCode,
    block_id: BlockId,
    pc: usize,
    locals: Vec<u64>,
    stack: Vec<u64>,
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
            OpCode::Literal => Op::Literal(self.next_word()),
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
    pub fn run(&mut self) -> Result<()> {
        while let Some(op) = self.next() {
            self.step(&op)?;
        }
        Ok(())
    }
    fn step(&mut self, op: &Op) -> Result<()> {
        // eprintln!("EXEC {op:?}");
        match *op {
            Op::Literal(literal) => self.op_literal(literal),
            Op::Call(def_id) => self.op_call(def_id),
            Op::Return => self.op_return(),
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
        let range = self.bytecode.source_map.get(self.block_id, self.pc - 1);
        Error { kind, range }
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
    fn op_literal(&mut self, literal: u64) {
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
    fn op_go_to_if(&mut self, block_id: BlockId) -> Result<()> {
        let value = self.pop_bool()?;
        if value {
            self.block_id = block_id;
            self.pc = 0;
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
        self.locals[local_id.to_index()] = u64::from(value);
        Ok(())
    }
    fn op_push_local(&mut self, local_id: LocalId) {
        let value = self.locals[local_id.to_index()];
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
            Value::Char(c) => write!(w, "{c}")?,
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
