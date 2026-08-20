#[derive(Copy, Clone, Debug)]
pub enum OpCode {
    Literal,
    Call,
    Return,
    GoTo,
    GoToIf,
    Dup,
    Swap,
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
    With,
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
    0x62 => With,
);

#[derive(Copy, Clone, Debug)]
pub enum Op {
    Literal(i64),
    Call(usize),
    Return,
    GoTo(usize),
    GoToIf(usize),
    Dup,
    Swap,
    BindLocal(usize),
    PushLocal(usize),
    BindProp(usize),
    PushProp(usize),
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
    ObjectId(u64),
    Malloc,
    With,
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
            Op::ObjectId(_) => OpCode::ObjectId,
            Op::Malloc => OpCode::Malloc,
            Op::With => OpCode::With,
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
            Op::Call(def_id) => Some(def_id as u64),
            Op::GoTo(block_id) => Some(block_id as u64),
            Op::GoToIf(block_id) => Some(block_id as u64),
            Op::BindLocal(local_id) => Some(local_id as u64),
            Op::PushLocal(local_id) => Some(local_id as u64),
            Op::ObjectId(obj_id) => Some(obj_id),
            _ => None,
        } {
            self.data.push(word)
        }
    }
    pub fn iter(&self) -> BlockIterator<'_> {
        BlockIterator { block: self, pc: 0 }
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
            OpCode::Call => Op::Call(self.next_word() as usize),
            OpCode::Return => Op::Return,
            OpCode::GoTo => Op::GoTo(self.next_word() as usize),
            OpCode::GoToIf => Op::GoToIf(self.next_word() as usize),
            OpCode::Dup => Op::Dup,
            OpCode::Swap => Op::Swap,
            OpCode::BindLocal => Op::BindLocal(self.next_word() as usize),
            OpCode::PushLocal => Op::PushLocal(self.next_word() as usize),
            OpCode::BindProp => Op::BindProp(self.next_word() as usize),
            OpCode::PushProp => Op::PushProp(self.next_word() as usize),
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
            OpCode::ObjectId => Op::ObjectId(self.next_word()),
            OpCode::Malloc => Op::Malloc,
            OpCode::With => Op::With,
        };
        Some(op)
    }
}

#[derive(Debug, Default)]
pub struct Definition {
    pub block_id: usize,
    local_size: usize,
}
impl Definition {
    pub fn new(block_id: usize) -> Self {
        let local_size = 0;
        Self {
            block_id,
            local_size,
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
    pub main_id: Option<usize>,
}

impl ByteCode {
    pub fn new_block(&mut self) -> usize {
        let block_id = self.blocks.len();
        self.blocks.push(Block::default());
        block_id
    }
    pub fn new_def(&mut self, block_id: usize) -> usize {
        let def_id = self.defs.len();
        self.defs.push(Definition::new(block_id));
        def_id
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
