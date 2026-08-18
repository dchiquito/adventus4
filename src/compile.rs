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
    BindLocal,
    PushLocal,
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
            0x20 => OpCode::BindLocal,
            0x21 => OpCode::PushLocal,
            0x30 => OpCode::Add,
            0x31 => OpCode::Sub,
            0x32 => OpCode::Mul,
            0x33 => OpCode::Div,
            0x40 => OpCode::Print,
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
            OpCode::BindLocal => 0x20,
            OpCode::PushLocal => 0x21,
            OpCode::Add => 0x30,
            OpCode::Sub => 0x31,
            OpCode::Mul => 0x32,
            OpCode::Div => 0x33,
            OpCode::Print => 0x40,
        }
    }
}

#[derive(Debug, Default)]
pub struct Definition {
    data: Vec<u64>,
    local_size: usize,
}
impl Definition {
    pub fn read_word(&self, pc: usize) -> Option<u64> {
        self.data.get(pc).copied()
    }
    pub fn initialize_locals(&self) -> Vec<i64> {
        vec![0; self.local_size]
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
    def_map: HashMap<String, usize>,
}
impl<'a> Compiler<'a> {
    fn new(source: &'a str) -> Self {
        let bytecode = ByteCode::default();
        let def_map = HashMap::default();
        Self {
            source,
            bytecode,
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
        assert!(cursor.goto_parent());
        assert_eq!(cursor.node(), root);
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

        let mut def_compiler = DefCompiler::new(self);
        assert!(cursor.goto_next_sibling());
        def_compiler.compile_def(cursor);

        self.bytecode.defs.push(def_compiler.def);
    }
}

struct DefCompiler<'a> {
    compiler: &'a Compiler<'a>,
    def: Definition,
    local_map: HashMap<String, usize>,
}
impl<'a> DefCompiler<'a> {
    fn new(compiler: &'a Compiler) -> Self {
        let def = Definition::default();
        let local_map = HashMap::default();
        Self {
            compiler,
            def,
            local_map,
        }
    }
    fn compile_def(&mut self, cursor: &mut TreeCursor) {
        if cursor.node().grammar_id() != EXPRESSION {
            // TODO ingest the signature
            assert!(cursor.goto_next_sibling()); // :
        }
        self.compile_expression(cursor);
        assert!(cursor.goto_parent());
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
            LOCAL_BIND => self.compile_local_bind(cursor),
            LOCAL_VAR => self.compile_local_var(cursor),
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        assert!(cursor.goto_parent());
    }
    fn compile_identifier(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IDENTIFIER, "identifier");
        let string_repr = &self.compiler.source[cursor.node().byte_range()];
        eprintln!("id {string_repr}");
        if let Some(&def_id) = self.compiler.def_map.get(string_repr) {
            eprintln!("Looked up {def_id}");
            self.def.data.push(u64::from(OpCode::Call));
            self.def.data.push(def_id as u64);
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
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        assert!(cursor.goto_parent());
    }
    fn compile_positive_int(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, POSITIVE_INT, "positive_int");
        let string_repr = &self.compiler.source[cursor.node().byte_range()];
        eprintln!("int {:?}", string_repr);
        let int = string_repr
            .as_bytes()
            .iter()
            .filter(|&&b| b != b'_')
            .map(|b| (b - b'0') as i64)
            .fold(0_i64, |lhs, rhs| lhs * 10 + rhs);
        self.def.data.push(u64::from(OpCode::Literal));
        self.def.data.push(int as u64);
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
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        assert!(cursor.goto_parent());
    }
    fn compile_local_bind(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOCAL_BIND, "local_bind");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let local_name = &self.compiler.source[cursor.node().byte_range()];
        eprintln!("  lb {:?}", local_name);
        let local_id = if let Some(id) = self.local_map.get(local_name) {
            *id
        } else {
            let id = self.def.local_size;
            self.local_map.insert(local_name.to_string(), id);
            self.def.local_size += 1;
            id
        };
        self.def.data.push(u64::from(OpCode::BindLocal));
        self.def.data.push(local_id as u64);

        assert!(cursor.goto_parent());
    }
    fn compile_local_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOCAL_VAR, "local_var");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let local_name = &self.compiler.source[cursor.node().byte_range()];
        eprintln!("  lv {:?}", local_name);
        let local_id = self
            .local_map
            .get(local_name)
            .unwrap_or_else(|| panic!("local {local_name} is unbound"));
        self.def.data.push(u64::from(OpCode::PushLocal));
        self.def.data.push(*local_id as u64);
        assert!(cursor.goto_parent());
    }
}

macro_rules! compile_builtin_method {
    ($method:ident, $lower:ident, $pascal:ident, $upper:ident) => {
        fn $method(&mut self, cursor: &mut TreeCursor) {
            assert_node_id!(cursor, $upper, stringify!($lower));
            self.def.data.push(u64::from(OpCode::$pascal));
        }
    };
}
impl<'a> DefCompiler<'a> {
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
