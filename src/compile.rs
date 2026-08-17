use std::collections::HashMap;

use tree_sitter::{Parser, Tree, TreeCursor};
use tree_sitter_adventus::LANGUAGE as ADVENTUS;

include!(concat!(env!("OUT_DIR"), "/grammar_ids.rs"));

macro_rules! assert_node_id {
    ($cursor:expr, $id:expr, $name:expr) => {
        if $cursor.node().grammar_id() != $id {
            assert_eq!($cursor.node().grammar_name(), $name);
            panic!(
                "grammar_id for {} has changed from {} to {}",
                $name,
                $id,
                $cursor.node().grammar_id()
            );
        } else {
            debug_assert_eq!($cursor.node().grammar_name(), $name);
        }
    };
}

pub enum OpCode {
    Literal,
    Call,
    Return,
    Dup,
    Swap,
    Add,
    Sub,
    Mul,
    Div,
    Print,
}
impl TryFrom<u64> for OpCode {
    type Error = u64;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(match value {
            0x1 => OpCode::Literal,
            0x2 => OpCode::Call,
            0x3 => OpCode::Return,
            0x10 => OpCode::Dup,
            0x11 => OpCode::Swap,
            0x20 => OpCode::Add,
            0x21 => OpCode::Sub,
            0x22 => OpCode::Mul,
            0x23 => OpCode::Div,
            0x30 => OpCode::Print,
            _ => return Err(value),
        })
    }
}
impl From<OpCode> for u64 {
    fn from(value: OpCode) -> Self {
        match value {
            OpCode::Literal => 0x1,
            OpCode::Call => 0x2,
            OpCode::Return => 0x3,
            OpCode::Dup => 0x10,
            OpCode::Swap => 0x11,
            OpCode::Add => 0x20,
            OpCode::Sub => 0x21,
            OpCode::Mul => 0x22,
            OpCode::Div => 0x23,
            OpCode::Print => 0x30,
        }
    }
}

#[derive(Debug, Default)]
pub struct Definition {
    data: Vec<u64>,
}
impl Definition {
    pub fn read_word(&self, pc: usize) -> Option<u64> {
        self.data.get(pc).copied()
    }
}

#[derive(Debug, Default)]
pub struct ByteCode {
    pub defs: Vec<Definition>,
    pub main_id: Option<usize>,
}
impl ByteCode {
    pub fn compile(source: &str) -> ByteCode {
        Compiler::new(source).compile()
    }
}
struct Compiler<'a> {
    source: &'a str,
    bytecode: ByteCode,
    current_def: Option<Definition>,
    def_map: HashMap<String, usize>,
}
macro_rules! compile_builtin_method {
    ($method:ident, $lower:ident, $pascal:ident, $upper:ident) => {
        fn $method(&mut self, cursor: &mut TreeCursor) {
            assert_node_id!(cursor, $upper, stringify!($lower));
            self.current_def
                .as_mut()
                .unwrap()
                .data
                .push(u64::from(OpCode::$pascal));
        }
    };
}
impl<'a> Compiler<'a> {
    fn new(source: &'a str) -> Self {
        let bytecode = ByteCode::default();
        let current_def = None;
        let def_map = HashMap::default();
        Self {
            source,
            bytecode,
            current_def,
            def_map,
        }
    }

    fn parse_tree(&self) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&ADVENTUS.into())
            .expect("Error loading Adventus grammar");
        parser.parse(self.source, None).unwrap()
    }

    fn compile(mut self) -> ByteCode {
        let tree = self.parse_tree();
        let root = tree.root_node();
        eprintln!("{root:?}");
        let mut cursor = root.walk();
        let mut walked = cursor.goto_first_child();
        eprintln!("{:?}", cursor.node());
        while walked {
            self.compile_def(&mut cursor);
            walked = cursor.goto_next_sibling();
        }
        self.bytecode
    }
    fn compile_def(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, DEF, "def");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let name = &self.source[cursor.node().byte_range()];

        // Register the name of the definition now so that it can be referenced
        // recursively while compiling itself.
        self.def_map
            .insert(name.to_string(), self.bytecode.defs.len());
        if name == "main" {
            self.bytecode.main_id = Some(self.bytecode.defs.len());
        }

        assert!(self.current_def.is_none());
        self.current_def = Some(Definition::default());

        eprintln!("Compiling def {name} ...");
        assert!(cursor.goto_next_sibling());
        self.compile_expression(cursor);
        assert!(cursor.goto_parent());

        self.bytecode.defs.push(self.current_def.take().unwrap());
    }
    fn compile_expression(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, EXPRESSION, "expression");
        assert!(cursor.goto_first_child());
        eprintln!("expression {:?}##", cursor.node().grammar_name());
        match cursor.node().grammar_id() {
            IDENTIFIER => self.compile_identifier(cursor),
            INT => self.compile_int(cursor),
            GROUPING => self.compile_grouping(cursor),
            BUILTIN => self.compile_builtin(cursor),
            _ => unreachable!("{}", cursor.node().grammar_id()),
        }
        assert!(cursor.goto_parent());
    }
    fn compile_identifier(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IDENTIFIER, "identifier");
        let string_repr = &self.source[cursor.node().byte_range()];
        eprintln!("id {string_repr}");
        if let Some(&def_id) = self.def_map.get(string_repr) {
            eprintln!("Looked up {def_id}");
            let def = self.current_def.as_mut().unwrap();
            def.data.push(u64::from(OpCode::Call));
            def.data.push(def_id as u64);
        } else {
            panic!("{string_repr} is undefined");
        }
    }
    fn compile_int(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, INT, "int");
        assert!(cursor.goto_first_child());
        eprintln!("{:?}##int", cursor.node().grammar_name());
        match cursor.node().grammar_id() {
            POSITIVE_INT => self.compile_positive_int(cursor),
            _ => unreachable!("{}", cursor.node().grammar_id()),
        }
        assert!(cursor.goto_parent());
    }
    fn compile_positive_int(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, POSITIVE_INT, "positive_int");
        let string_repr = &self.source[cursor.node().byte_range()];
        eprintln!("int {:?}", string_repr);
        let int = string_repr
            .as_bytes()
            .iter()
            .filter(|&&b| b != b'_')
            .map(|b| (b - b'0') as i64)
            .fold(0_i64, |lhs, rhs| lhs * 10 + rhs);
        let def = self.current_def.as_mut().unwrap();
        def.data.push(u64::from(OpCode::Literal));
        def.data.push(int as u64);
    }
    fn compile_grouping(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, GROUPING, "grouping");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        while cursor.node().grammar_id() == EXPRESSION {
            self.compile_expression(cursor);
            assert!(cursor.goto_next_sibling());
        }
        eprintln!("{:?}", cursor.node());
        assert!(cursor.goto_parent());
    }
    fn compile_builtin(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, BUILTIN, "builtin");
        assert!(cursor.goto_first_child());
        match cursor.node().grammar_id() {
            ADD => self.compile_add(cursor),
            SUB => self.compile_sub(cursor),
            MUL => self.compile_mul(cursor),
            DIV => self.compile_div(cursor),
            PRINT => self.compile_print(cursor),
            _ => unreachable!("{}", cursor.node().grammar_id()),
        }
        assert!(cursor.goto_parent());
    }
    compile_builtin_method!(compile_add, add, Add, ADD);
    compile_builtin_method!(compile_sub, sub, Sub, SUB);
    compile_builtin_method!(compile_mul, mul, Mul, MUL);
    compile_builtin_method!(compile_div, div, Div, DIV);
    compile_builtin_method!(compile_print, print, Print, PRINT);
}

#[cfg(test)]
mod test_opcodes {
    use super::*;

    #[test]
    fn test_u64_to_opcode() {
        for i in 0..255 {
            if let Ok(op) = OpCode::try_from(i) {
                assert_eq!(i, u64::from(op));
            }
        }
    }
}
