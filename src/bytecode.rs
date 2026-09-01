use std::collections::HashMap;

use crate::type_check::StackMutation;

#[derive(Copy, Clone, Debug)]
pub enum OpCode {
    Literal,
    Call,
    Return,
    GoTo,
    GoToIf,
    Dup,
    Swap,
    Pop,
    BindLocal,
    PushLocal,
    BindProp,
    PushProp,
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
    Not,
    Print,
    ObjectId,
    Malloc,
}
macro_rules! opcode_u64_conversions {
    ($($number:expr => $opcode:ident),*,) => {
        impl TryFrom<u64> for OpCode {
            type Error = u64;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                Ok(match value {
                    $($number => OpCode::$opcode),*,
                    _ => return Err(value),
                })
            }
        }
        impl From<OpCode> for u64 {
            fn from(value: OpCode) -> Self {
                match value {
                    $(OpCode::$opcode => $number),*
                }
            }
        }
    };
}
opcode_u64_conversions!(
    0x1 => Literal,
    0x2 => Call,
    0x3 => Return,
    0x4 => GoTo,
    0x5 => GoToIf,
    0x10 => Dup,
    0x11 => Swap,
    0x12 => Pop,
    0x20 => BindLocal,
    0x21 => PushLocal,
    0x22 => BindProp,
    0x23 => PushProp,
    0x30 => Add,
    0x31 => Sub,
    0x32 => Mul,
    0x33 => Div,
    0x40 => Eq,
    0x41 => Ne,
    0x42 => Gt,
    0x43 => Lt,
    0x44 => Gte,
    0x45 => Lte,
    0x46 => And,
    0x47 => Or,
    0x48 => Not,
    0x50 => Print,
    0x60 => ObjectId,
    0x61 => Malloc,
);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LocalId(u64);
impl LocalId {
    pub fn new(local_id: u64) -> Self {
        Self(local_id)
    }
    pub fn to_index(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PropId(u64);
impl PropId {
    pub fn new(prop_id: u64) -> Self {
        Self(prop_id)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LayoutId(u64);
impl LayoutId {
    pub fn new(prop_id: u64) -> Self {
        Self(prop_id)
    }
    pub fn to_value(&self) -> i64 {
        self.0 as i64
    }
    pub fn from_value(value: i64) -> Self {
        Self(value as u64)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockId(u64);
impl BlockId {
    pub fn new(block_id: u64) -> Self {
        Self(block_id)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct DefId(u64);
impl DefId {
    pub fn new(def_id: u64) -> Self {
        Self(def_id)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Op {
    Literal(i64),
    Call(DefId),
    Return,
    GoTo(BlockId),
    GoToIf(BlockId),
    Dup,
    Swap,
    Pop,
    BindLocal(LocalId),
    PushLocal(LocalId),
    BindProp(PropId),
    PushProp(PropId),
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
    Not,
    Print,
    Layout(LayoutId),
    Malloc(LayoutId),
}
impl From<Op> for OpCode {
    fn from(op: Op) -> Self {
        match op {
            Op::Literal(_) => OpCode::Literal,
            Op::Call(_) => OpCode::Call,
            Op::Return => OpCode::Return,
            Op::GoTo(_) => OpCode::GoTo,
            Op::GoToIf(_) => OpCode::GoToIf,
            Op::Dup => OpCode::Dup,
            Op::Swap => OpCode::Swap,
            Op::Pop => OpCode::Pop,
            Op::BindLocal(_) => OpCode::BindLocal,
            Op::PushLocal(_) => OpCode::PushLocal,
            Op::BindProp(_) => OpCode::BindProp,
            Op::PushProp(_) => OpCode::PushProp,
            Op::Add => OpCode::Add,
            Op::Sub => OpCode::Sub,
            Op::Mul => OpCode::Mul,
            Op::Div => OpCode::Div,
            Op::Eq => OpCode::Eq,
            Op::Ne => OpCode::Ne,
            Op::Gt => OpCode::Gt,
            Op::Lt => OpCode::Lt,
            Op::Gte => OpCode::Gte,
            Op::Lte => OpCode::Lte,
            Op::And => OpCode::And,
            Op::Or => OpCode::Or,
            Op::Not => OpCode::Not,
            Op::Print => OpCode::Print,
            Op::Layout(_) => OpCode::ObjectId,
            Op::Malloc(_) => OpCode::Malloc,
        }
    }
}

#[derive(Debug, Default)]
pub struct Block {
    pub data: Vec<u64>,
}
impl Block {
    pub fn push(&mut self, op: Op) {
        self.data.push(u64::from(OpCode::from(op)));
        if let Some(word) = match op {
            Op::Literal(literal) => Some(literal as u64),
            Op::Call(def_id) => Some(def_id.0),
            Op::GoTo(block_id) => Some(block_id.0),
            Op::GoToIf(block_id) => Some(block_id.0),
            Op::BindLocal(local_id) => Some(local_id.0),
            Op::PushLocal(local_id) => Some(local_id.0),
            Op::BindProp(prop_id) => Some(prop_id.0),
            Op::PushProp(prop_id) => Some(prop_id.0),
            Op::Layout(layout_id) => Some(layout_id.0),
            Op::Malloc(layout_id) => Some(layout_id.0),
            _ => None,
        } {
            self.data.push(word)
        }
    }
    pub fn iter(&self) -> BlockIterator<'_> {
        BlockIterator { block: self, pc: 0 }
    }
    pub fn pretty_print(&self) {
        for op in self.iter() {
            println!("  {op:?}",);
        }
    }
}
pub struct BlockIterator<'a> {
    block: &'a Block,
    pc: usize,
}
impl BlockIterator<'_> {
    fn next_word(&mut self) -> u64 {
        let word = self.block.data[self.pc];
        self.pc += 1;
        word
    }
}
impl Iterator for BlockIterator<'_> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pc >= self.block.data.len() {
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
            OpCode::BindLocal => Op::BindLocal(LocalId(self.next_word())),
            OpCode::PushLocal => Op::PushLocal(LocalId(self.next_word())),
            OpCode::BindProp => Op::BindProp(PropId(self.next_word())),
            OpCode::PushProp => Op::PushProp(PropId(self.next_word())),
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

#[derive(Debug)]
pub struct Definition {
    pub block_id: BlockId,
    local_size: usize,
    pub declared_type: Option<StackMutation>,
}
impl Definition {
    pub fn new(block_id: BlockId) -> Self {
        let local_size = 0;
        let declared_type = None;
        Self {
            block_id,
            local_size,
            declared_type,
        }
    }
    pub fn initialize_locals(&self) -> Vec<i64> {
        vec![0; self.local_size]
    }
    pub fn get_local_size(&self) -> usize {
        self.local_size
    }
    pub fn incr_local_size(&mut self) {
        self.local_size += 1;
    }
}

#[derive(Debug, Default)]
pub struct ByteCode {
    pub blocks: Vec<Block>,
    pub defs: Vec<Definition>,
    pub main_id: Option<DefId>,
    pub layouts: HashMap<LayoutId, Vec<PropId>>,
}

impl ByteCode {
    pub fn new_block(&mut self) -> BlockId {
        let block_id = BlockId::new(self.blocks.len() as u64);
        self.blocks.push(Block::default());
        block_id
    }
    pub fn get_block(&self, block_id: BlockId) -> &Block {
        &self.blocks[block_id.0 as usize]
    }
    pub fn get_block_mut(&mut self, block_id: BlockId) -> &mut Block {
        &mut self.blocks[block_id.0 as usize]
    }
    pub fn new_def(&mut self, block_id: BlockId) -> DefId {
        let def_id = DefId::new(self.defs.len() as u64);
        self.defs.push(Definition::new(block_id));
        def_id
    }
    pub fn get_def(&self, def_id: DefId) -> &Definition {
        &self.defs[def_id.0 as usize]
    }
    pub fn get_def_mut(&mut self, def_id: DefId) -> &mut Definition {
        &mut self.defs[def_id.0 as usize]
    }
    pub fn iter_def_ids(&self) -> impl Iterator<Item = DefId> {
        (0..self.defs.len()).map(|i| DefId::new(i as u64))
    }
    pub fn pretty_print(&self) {
        for (i, block) in self.blocks.iter().enumerate() {
            println!("Block {i}",);
            block.pretty_print();
        }
        for (i, def) in self.defs.iter().enumerate() {
            if Some(DefId::new(i as u64)) == self.main_id {
                println!("main:");
            }
            if let Some(sig) = &def.declared_type {
                println!(
                    "Definition {i} :({sig:?}) has {:?} ({} locals)",
                    def.block_id, def.local_size
                );
            } else {
                println!(
                    "Definition {i} has {:?} ({} locals)",
                    def.block_id, def.local_size
                );
            }
        }
    }
}

#[cfg(test)]
mod test_opcodes {
    use super::*;

    #[test]
    fn test_u64_to_opcode() {
        for i in 0..0xffff {
            if let Ok(op) = OpCode::try_from(i) {
                assert_eq!(i, u64::from(op));
            }
        }
    }
}
