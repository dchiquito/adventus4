use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use tree_sitter::{Parser, Tree, TreeCursor};
use tree_sitter_adventus::LANGUAGE as ADVENTUS;

use crate::{
    bytecode::{BlockId, ByteCode, DefId, LayoutId, LocalId, Op, PropId},
    type_check::{BuiltinType, StackMutation, Type},
    vm::VM,
};

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
macro_rules! skip_comments {
    ($cursor:expr) => {
        while let id = $cursor.node().grammar_id()
            && (id == DOC_COMMENT || id == CODE_COMMENT)
        {
            assert!($cursor.goto_next_sibling());
        }
    };
}
macro_rules! goto_next_sibling {
    ($cursor:expr) => {
        assert!($cursor.goto_next_sibling());
        skip_comments!($cursor);
    };
}
macro_rules! goto_first_child {
    ($cursor:expr) => {
        assert!($cursor.goto_first_child());
        skip_comments!($cursor);
    };
}
macro_rules! goto_parent {
    ($cursor:expr) => {
        assert!($cursor.goto_parent());
        skip_comments!($cursor);
    };
}

fn evaluate_at_compile_time(bytecode: &ByteCode, block_id: BlockId) -> u64 {
    // TODO type check before running the VM
    let def_id = DefId::new(0); // TODO this is wrong
    let mut vm = VM::new(bytecode, def_id, block_id);
    vm.run();
    let obj_id = vm.pop_from_stack() as u64;
    assert_eq!(vm.stack_len(), 0);
    obj_id
}

enum Builtin {
    Dup,
    Swap,
    Pop,
    And,
    Or,
    Not,
    Print,
    Break,
}

pub struct Compiler<'s> {
    source: &'s str,
    bytecode: ByteCode,
    def_map: HashMap<String, DefId>,
    prop_ids: HashMap<String, PropId>,
}
impl<'s> Compiler<'s> {
    pub fn new(source: &'s str) -> Self {
        let bytecode = ByteCode::default();
        let def_map = HashMap::default();
        let prop_ids = HashMap::default();
        Self {
            source,
            bytecode,
            def_map,
            prop_ids,
        }
    }
    fn prop_id_for(&mut self, prop_name: &str) -> PropId {
        if let Some(prop_id) = self.prop_ids.get(prop_name) {
            *prop_id
        } else {
            let prop_id = PropId::new(self.prop_ids.len() as u64);
            self.prop_ids.insert(prop_name.to_string(), prop_id);
            prop_id
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
            skip_comments!(cursor);
            self.compile_def(&mut cursor);
            skip_comments!(cursor);
            walked = cursor.goto_next_sibling();
        }
        goto_parent!(cursor);
        assert_eq!(cursor.node(), root);
        self.bytecode
    }
    fn compile_def(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, DEF, "def");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let name = &self.source[cursor.node().byte_range()];
        goto_next_sibling!(cursor);

        let block_id = self.bytecode.new_block();
        let def_id = self.bytecode.new_def(block_id);
        // Register the name of the definition now so that it can be referenced
        // recursively while compiling itself.
        self.def_map.insert(name.to_string(), def_id);
        if name == "main" {
            self.bytecode.main_id = Some(def_id);
        }

        // Handle the type signature, if present
        if cursor.node().grammar_id() != EXPRESSION {
            assert_node_id!(cursor, MANUAL_SIGNATURE, "manual_signature");
            goto_first_child!(cursor);
            goto_next_sibling!(cursor); // :
            let signature = self.compile_signature(cursor);
            eprintln!("  {signature:?}");
            self.bytecode.get_def_mut(def_id).declared_type = Some(signature);
            goto_parent!(cursor);
            goto_next_sibling!(cursor);
        }

        {
            let mut def_compiler = DefCompiler::new(self, def_id);
            def_compiler.compile_def(cursor, block_id);
        }
    }
    fn compile_signature(&mut self, cursor: &mut TreeCursor) -> StackMutation {
        assert_node_id!(cursor, SIGNATURE, "signature");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor); // (
        let mut before = vec![];
        while cursor.node().grammar_id() != SYMBOL_ARROW {
            before.push(self.compile_type(cursor));
            goto_next_sibling!(cursor);
        }
        goto_next_sibling!(cursor); // ->
        let mut after = vec![];
        while cursor.node().grammar_id() != SYMBOL_RPAREN {
            after.push(self.compile_type(cursor));
            goto_next_sibling!(cursor);
        }
        goto_parent!(cursor);
        StackMutation::new(before, after)
    }
    fn compile_type(&mut self, cursor: &mut TreeCursor) -> Type {
        assert_node_id!(cursor, TYPE, "type");
        goto_first_child!(cursor);
        let t = match cursor.node().grammar_id() {
            BUILTIN_TYPE => Type::Builtin(self.compile_builtin_type(cursor)),
            EXPRESSION => {
                let block_id = self.bytecode.new_block();
                let dummy_def_id = DefId::new(0);
                let mut def_compiler = DefCompiler::new(self, dummy_def_id);
                let mut block_compiler = BlockCompiler::new(&mut def_compiler, block_id);
                block_compiler.compile_expression(cursor);
                eprintln!("bytecode {:?}", self.bytecode);
                eprintln!("block_id {block_id:?}");

                let raw_layout_id = evaluate_at_compile_time(&self.bytecode, block_id);
                let layout_id = LayoutId::new(raw_layout_id);
                Type::Object(layout_id)
            }
            _ => unreachable!(),
        };
        goto_parent!(cursor);
        t
    }
    fn compile_builtin_type(&self, cursor: &mut TreeCursor) -> BuiltinType {
        assert_node_id!(cursor, BUILTIN_TYPE, "builtin_type");
        goto_first_child!(cursor);
        let t = match cursor.node().grammar_id() {
            BUILTIN_TYPE_TYPE => BuiltinType::Type,
            BUILTIN_TYPE_INT => BuiltinType::Int,
            BUILTIN_TYPE_CHAR => BuiltinType::Char,
            BUILTIN_TYPE_BOOL => BuiltinType::Bool,
            BUILTIN_TYPE_NONE => BuiltinType::None,
            _ => unreachable!(),
        };
        goto_parent!(cursor);
        t
    }
}

struct DefCompiler<'a, 's> {
    compiler: &'a mut Compiler<'s>,
    def_id: DefId,
    local_map: HashMap<String, usize>,
    loop_stack: Vec<BlockId>,
}
impl<'a, 's> DefCompiler<'a, 's> {
    fn new(compiler: &'a mut Compiler<'s>, def_id: DefId) -> Self {
        let local_map = HashMap::default();
        let loop_stack = vec![];
        Self {
            compiler,
            def_id,
            local_map,
            loop_stack,
        }
    }
    fn compile_def(&'a mut self, cursor: &mut TreeCursor, block_id: BlockId) {
        let mut block_compiler = BlockCompiler::new(self, block_id);
        block_compiler.compile_expression(cursor);
        block_compiler.push(Op::Return);
        goto_parent!(cursor);
    }
}
struct BlockCompiler<'d, 'c, 's> {
    def_compiler: &'d mut DefCompiler<'c, 's>,
    block_id: BlockId,
}
impl<'d, 'c, 's> BlockCompiler<'d, 'c, 's> {
    fn new(def_compiler: &'d mut DefCompiler<'c, 's>, block_id: BlockId) -> Self {
        Self {
            def_compiler,
            block_id,
        }
    }
    fn push(&mut self, op: Op) {
        self.def_compiler
            .compiler
            .bytecode
            .get_block_mut(self.block_id)
            .push(op);
    }
    fn compile_expression(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, EXPRESSION, "expression");
        goto_first_child!(cursor);
        eprintln!("expression {:?}##", cursor.node().grammar_name());
        match cursor.node().grammar_id() {
            IDENTIFIER => self.compile_identifier(cursor),
            INT => self.compile_int(cursor),
            GROUPING => self.compile_grouping(cursor),
            OBJECT => self.compile_object(cursor),
            BUILTIN => self.compile_builtin(cursor),
            LOCAL_BIND => self.compile_local_bind(cursor),
            LOCAL_VAR => self.compile_local_var(cursor),
            PROP_BIND => self.compile_prop_bind(cursor),
            PROP_VAR => self.compile_prop_var(cursor),
            IF => self.compile_if(cursor),
            LOOP => self.compile_loop(cursor),
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        goto_parent!(cursor);
    }
    fn lookup_builtin(&self, name: &str) -> Option<Builtin> {
        match name {
            "dup" => Some(Builtin::Dup),
            "swap" => Some(Builtin::Swap),
            "pop" => Some(Builtin::Pop),
            "and" => Some(Builtin::And),
            "or" => Some(Builtin::Or),
            "not" => Some(Builtin::Not),
            "print" => Some(Builtin::Print),
            "break" => Some(Builtin::Break),
            _ => None,
        }
    }
    fn compile_identifier(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IDENTIFIER, "identifier");
        let string_repr = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("id {string_repr}");
        if let Some(builtin) = self.lookup_builtin(string_repr) {
            let op = match builtin {
                Builtin::Dup => Op::Dup,
                Builtin::Swap => Op::Swap,
                Builtin::Pop => Op::Pop,
                Builtin::And => Op::And,
                Builtin::Or => Op::Or,
                Builtin::Not => Op::Not,
                Builtin::Print => Op::Print,
                Builtin::Break => Op::GoTo(
                    *self
                        .def_compiler
                        .loop_stack
                        .last()
                        .expect("cannot break while outside of loop"),
                ),
            };
            self.push(op);
        } else if let Some(&def_id) = self.def_compiler.compiler.def_map.get(string_repr) {
            eprintln!("Looked up {def_id:?}");
            self.push(Op::Call(def_id));
        } else {
            panic!("{string_repr} is undefined");
        }
    }
    fn compile_int(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, INT, "int");
        goto_first_child!(cursor);
        eprintln!("{:?}##int", cursor.node().grammar_name());
        match cursor.node().grammar_id() {
            POSITIVE_INT => self.compile_positive_int(cursor),
            NEGATIVE_INT => self.compile_negative_int(cursor),
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        goto_parent!(cursor);
    }
    fn compile_negative_int(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, NEGATIVE_INT, "negative_int");
        let string_repr = &self.def_compiler.compiler.source[cursor.node().byte_range()][1..];
        eprintln!("-int {:?}", string_repr);
        let int = -string_repr
            .as_bytes()
            .iter()
            .filter(|&&b| b != b'_')
            .map(|b| (b - b'0') as i64)
            .fold(0_i64, |lhs, rhs| lhs * 10 + rhs);
        self.push(Op::Literal(int));
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
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        while cursor.node().grammar_id() == EXPRESSION {
            self.compile_expression(cursor);
            cursor.goto_next_sibling();
            skip_comments!(cursor);
        }
        eprintln!("{:?}", cursor.node());
        goto_parent!(cursor);
    }
    fn compile_object(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, OBJECT, "object");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let mut props = vec![];
        while cursor.node().grammar_id() == IDENTIFIER {
            let prop_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
            eprintln!("Propo {prop_name}");
            let prop_id = self.def_compiler.compiler.prop_id_for(prop_name);
            props.push(prop_id);
            goto_next_sibling!(cursor);
        }
        // TODO assume no collisions ¯\_(ツ)_/¯
        let layout_id = {
            let mut hasher = DefaultHasher::new();
            props[..].hash(&mut hasher);
            hasher.finish()
        };
        let layout_id = LayoutId::new(layout_id);
        self.def_compiler
            .compiler
            .bytecode
            .layouts
            .insert(layout_id, props);
        self.push(Op::Layout(layout_id));
        goto_parent!(cursor);
    }
    fn compile_builtin(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, BUILTIN, "builtin");
        goto_first_child!(cursor);
        match cursor.node().grammar_id() {
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
            MALLOC => self.compile_malloc(cursor),
            _ => unreachable!(
                "{} ({})",
                cursor.node().grammar_name(),
                cursor.node().grammar_id()
            ),
        }
        goto_parent!(cursor);
    }
    fn compile_local_bind(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOCAL_BIND, "local_bind");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let local_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  lb {:?}", local_name);
        let local_id = if let Some(id) = self.def_compiler.local_map.get(local_name) {
            *id
        } else {
            let id = self
                .def_compiler
                .compiler
                .bytecode
                .get_def_mut(self.def_compiler.def_id)
                .get_local_size();
            self.def_compiler
                .local_map
                .insert(local_name.to_string(), id);
            self.def_compiler
                .compiler
                .bytecode
                .get_def_mut(self.def_compiler.def_id)
                .incr_local_size();
            id
        };
        let local_id = LocalId::new(local_id as u64);
        self.push(Op::BindLocal(local_id));

        goto_parent!(cursor);
    }
    fn compile_local_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOCAL_VAR, "local_var");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let local_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  lv {:?}", local_name);
        let local_id = self
            .def_compiler
            .local_map
            .get(local_name)
            .unwrap_or_else(|| panic!("local {local_name} is unbound"));
        let local_id = LocalId::new(*local_id as u64);
        self.push(Op::PushLocal(local_id));
        goto_parent!(cursor);
    }
    fn compile_prop_bind(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, PROP_BIND, "prop_bind");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let prop_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  ob {:?}", prop_name);
        let prop_id = self.def_compiler.compiler.prop_id_for(prop_name);
        self.push(Op::BindProp(prop_id));

        goto_parent!(cursor);
    }
    fn compile_prop_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, PROP_VAR, "prop_var");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let prop_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  ov {:?}", prop_name);
        let prop_id = self.def_compiler.compiler.prop_id_for(prop_name);
        self.push(Op::PushProp(prop_id));
        goto_parent!(cursor);
    }
    fn compile_if(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IF, "if");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let then_block_id = self.def_compiler.compiler.bytecode.new_block();
        let finally_block_id = self.def_compiler.compiler.bytecode.new_block();
        {
            let mut then_block_compiler = BlockCompiler::new(self.def_compiler, then_block_id);
            then_block_compiler.compile_expression(cursor);
            then_block_compiler.push(Op::GoTo(finally_block_id));
        }
        self.push(Op::GoToIf(then_block_id));

        skip_comments!(cursor);
        if cursor.goto_next_sibling() {
            // else
            goto_next_sibling!(cursor);
            self.compile_expression(cursor);
        }

        self.push(Op::GoTo(finally_block_id));
        self.block_id = finally_block_id;
        goto_parent!(cursor);
    }
    fn compile_loop(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, LOOP, "loop");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        eprintln!("looperating {:?}", cursor.node());
        // [self.block_id] -> [loop_block_id] -> [finally_block_id]
        let finally_block_id = self.def_compiler.compiler.bytecode.new_block();
        self.def_compiler.loop_stack.push(finally_block_id);
        {
            let loop_block_id = self.def_compiler.compiler.bytecode.new_block();
            {
                let mut loop_block_compiler = BlockCompiler::new(self.def_compiler, loop_block_id);
                loop_block_compiler.compile_expression(cursor);
                loop_block_compiler.push(Op::GoTo(loop_block_id));
            }
            self.push(Op::GoTo(loop_block_id));
        }
        self.def_compiler
            .loop_stack
            .pop()
            .expect("while block to pop");
        self.block_id = finally_block_id;
        goto_parent!(cursor);
    }
    fn compile_malloc(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, MALLOC, "malloc");
        goto_first_child!(cursor);
        let layout_id = self.resolve_type_constraint(cursor);
        let layout_id = LayoutId::new(layout_id);
        self.push(Op::Malloc(layout_id));
        goto_parent!(cursor);
    }
}
impl BlockCompiler<'_, '_, '_> {
    fn resolve_type_constraint(&mut self, cursor: &mut TreeCursor) -> u64 {
        assert_node_id!(cursor, TYPE_CONSTRAINT, "type_constraint");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let block_id = self.def_compiler.compiler.bytecode.new_block();
        let mut block_compiler = BlockCompiler::new(self.def_compiler, block_id);
        block_compiler.compile_expression(cursor);
        eprintln!("bytecode {:?}", self.def_compiler.compiler.bytecode);
        eprintln!("block_id {block_id:?}");

        let layout_id = evaluate_at_compile_time(&self.def_compiler.compiler.bytecode, block_id);
        eprintln!("layout id??? {layout_id}\n\n\n");
        goto_parent!(cursor);
        layout_id
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
}
