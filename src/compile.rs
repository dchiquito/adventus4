use std::collections::HashMap;

use tree_sitter::{Parser, Tree, TreeCursor};
use tree_sitter_adventus::LANGUAGE as ADVENTUS;

use crate::bytecode::{ByteCode, Op};

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

pub struct Compiler<'s> {
    source: &'s str,
    bytecode: ByteCode,
    def_map: HashMap<String, usize>,
}
impl<'s> Compiler<'s> {
    pub fn new(source: &'s str) -> Self {
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

    pub fn compile(mut self) -> ByteCode {
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
    fn compile_def<'a>(&'a mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, DEF, "def");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let name = &self.source[cursor.node().byte_range()];

        let block_id = self.bytecode.new_block();
        let def_id = self.bytecode.new_def(block_id);
        // Register the name of the definition now so that it can be referenced
        // recursively while compiling itself.
        self.def_map.insert(name.to_string(), block_id);
        if name == "main" {
            self.bytecode.main_id = Some(block_id);
        }

        {
            let mut def_compiler = DefCompiler::new(self, def_id);
            assert!(cursor.goto_next_sibling());
            def_compiler.compile_def(cursor, block_id);
        }
    }
}

struct DefCompiler<'a, 's> {
    compiler: &'a mut Compiler<'s>,
    def_id: usize,
    local_map: HashMap<String, usize>,
}
impl<'a, 's> DefCompiler<'a, 's> {
    fn new(compiler: &'a mut Compiler<'s>, def_id: usize) -> Self {
        let local_map = HashMap::default();
        Self {
            compiler,
            def_id,
            local_map,
        }
    }
    fn compile_def(&'a mut self, cursor: &mut TreeCursor, block_id: usize) {
        if cursor.node().grammar_id() != EXPRESSION {
            // TODO ingest the signature
            assert!(cursor.goto_next_sibling()); // :
        }
        let mut block_compiler = BlockCompiler::new(self, block_id);
        block_compiler.compile_expression(cursor);
        assert!(cursor.goto_parent());
    }
}
struct BlockCompiler<'d, 'c, 's> {
    def_compiler: &'d mut DefCompiler<'c, 's>,
    block_id: usize,
}
impl<'d, 'c, 's> BlockCompiler<'d, 'c, 's> {
    fn new(def_compiler: &'d mut DefCompiler<'c, 's>, block_id: usize) -> Self {
        Self {
            def_compiler,
            block_id,
        }
    }
    fn push(&mut self, op: Op) {
        self.def_compiler.compiler.bytecode.blocks[self.block_id].push(op);
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
            IF => self.compile_if(cursor),
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
        let string_repr = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("id {string_repr}");
        if let Some(&def_id) = self.def_compiler.compiler.def_map.get(string_repr) {
            eprintln!("Looked up {def_id}");
            self.push(Op::Call(def_id));
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
        let string_repr = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("int {:?}", string_repr);
        let int = string_repr
            .as_bytes()
            .iter()
            .filter(|&&b| b != b'_')
            .map(|b| (b - b'0') as i64)
            .fold(0_i64, |lhs, rhs| lhs * 10 + rhs);
        self.push(Op::Literal(int));
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
            DUP => self.compile_dup(cursor),
            SWAP => self.compile_swap(cursor),
            ADD => self.compile_add(cursor),
            SUB => self.compile_sub(cursor),
            MUL => self.compile_mul(cursor),
            DIV => self.compile_div(cursor),
            EQ => self.compile_eq(cursor),
            NE => self.compile_ne(cursor),
            GT => self.compile_gt(cursor),
            LT => self.compile_lt(cursor),
            GTE => self.compile_gte(cursor),
            LTE => self.compile_lte(cursor),
            NOT => self.compile_not(cursor),
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
        let local_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  lb {:?}", local_name);
        let local_id = if let Some(id) = self.def_compiler.local_map.get(local_name) {
            *id
        } else {
            let id =
                self.def_compiler.compiler.bytecode.defs[self.def_compiler.def_id].get_local_size();
            self.def_compiler
                .local_map
                .insert(local_name.to_string(), id);
            self.def_compiler.compiler.bytecode.defs[self.def_compiler.def_id].incr_local_size();
            id
        };
        self.push(Op::BindLocal(local_id));

        assert!(cursor.goto_parent());
    }
    fn compile_local_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOCAL_VAR, "local_var");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let local_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  lv {:?}", local_name);
        let local_id = self
            .def_compiler
            .local_map
            .get(local_name)
            .unwrap_or_else(|| panic!("local {local_name} is unbound"));
        self.push(Op::PushLocal(*local_id));
        assert!(cursor.goto_parent());
    }
    fn compile_if(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IF, "if");
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let then_block_id = self.def_compiler.compiler.bytecode.new_block();
        let finally_block_id = self.def_compiler.compiler.bytecode.new_block();
        {
            let mut then_block_compiler = BlockCompiler::new(self.def_compiler, then_block_id);
            then_block_compiler.compile_expression(cursor);
            then_block_compiler.push(Op::GoTo(finally_block_id));
        }
        self.push(Op::GoToIf(then_block_id));

        if cursor.goto_next_sibling() {
            // else
            assert!(cursor.goto_next_sibling());
            self.compile_expression(cursor);
        }

        self.push(Op::GoTo(finally_block_id));
        self.block_id = finally_block_id;
        assert!(cursor.goto_parent());
    }
}

macro_rules! compile_builtin_method {
    ($method:ident, $lower:ident, $pascal:ident, $upper:ident) => {
        fn $method(&mut self, cursor: &mut TreeCursor) {
            assert_node_id!(cursor, $upper, stringify!($lower));
            self.push(Op::$pascal);
        }
    };
}
impl BlockCompiler<'_, '_, '_> {
    compile_builtin_method!(compile_dup, dup, Dup, DUP);
    compile_builtin_method!(compile_swap, swap, Swap, SWAP);
    compile_builtin_method!(compile_add, add, Add, ADD);
    compile_builtin_method!(compile_sub, sub, Sub, SUB);
    compile_builtin_method!(compile_mul, mul, Mul, MUL);
    compile_builtin_method!(compile_div, div, Div, DIV);
    compile_builtin_method!(compile_eq, eq, Eq, EQ);
    compile_builtin_method!(compile_ne, ne, Ne, NE);
    compile_builtin_method!(compile_gt, gt, Gt, GT);
    compile_builtin_method!(compile_lt, lt, Lt, LT);
    compile_builtin_method!(compile_gte, gte, Gte, GTE);
    compile_builtin_method!(compile_lte, lte, Lte, LTE);
    compile_builtin_method!(compile_not, not, Not, NOT);
    compile_builtin_method!(compile_print, print, Print, PRINT);
}
