use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use tree_sitter::{Parser, Tree, TreeCursor};
use tree_sitter_adventus::LANGUAGE as ADVENTUS;

use crate::{
    bytecode::{
        BlockId, ByteCode, ClosureId, DefId, LayoutId, LocalId, Op, OpSize, PropId, SourceId,
    },
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
    vm.run().expect("no errors pls");
    assert_eq!(vm.stack_len(), 1);
    vm.pop_raw().unwrap()
}

enum Builtin {
    Dup,
    Swap,
    Drop,
    And,
    Or,
    Not,
    Print,
    Break,
    Return,
    EmptyArray,
    ArrayGet,
    ArraySet,
}
impl TryFrom<&str> for Builtin {
    type Error = ();

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Ok(match name {
            "dup" => Builtin::Dup,
            "swap" => Builtin::Swap,
            "drop" => Builtin::Drop,
            "and" => Builtin::And,
            "or" => Builtin::Or,
            "not" => Builtin::Not,
            "print" => Builtin::Print,
            "break" => Builtin::Break,
            "return" => Builtin::Return,
            "empty_array" => Builtin::EmptyArray,
            "array_get" => Builtin::ArrayGet,
            "array_set" => Builtin::ArraySet,
            _ => return Err(()),
        })
    }
}

pub struct Compiler<'s> {
    source: &'s str,
    source_id: SourceId,
    bytecode: &'s mut ByteCode,
}
impl<'s> Compiler<'s> {
    pub fn new(bytecode: &'s mut ByteCode, source_name: &str, source: &'s str) -> Self {
        let source_id = bytecode.source_map.new_source(source_name);
        Self {
            source,
            source_id,
            bytecode,
        }
    }
    fn prop_id_for(&mut self, prop_name: &str) -> PropId {
        if let Some(prop_id) = self.bytecode.prop_ids.get(prop_name) {
            *prop_id
        } else {
            let prop_id = PropId::new(self.bytecode.prop_ids.len() as u64);
            self.bytecode
                .prop_ids
                .insert(prop_name.to_string(), prop_id);
            self.bytecode
                .prop_names
                .insert(prop_id, prop_name.to_string());
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

    pub fn compile(mut self) {
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
    }
    fn compile_def(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, DEF, "def");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let name = &self.source[cursor.node().byte_range()];
        goto_next_sibling!(cursor);
        eprintln!("\nCompiling def {name}");

        let mut closure_vars = vec![];
        while cursor.node().grammar_id() == CLOSURE_VAR {
            assert_node_id!(cursor, CLOSURE_VAR, "closure_var");
            goto_first_child!(cursor);
            goto_next_sibling!(cursor); // @
            let var_name = &self.source[cursor.node().byte_range()];
            let var_id = self.prop_id_for(var_name);
            closure_vars.push(var_id);
            goto_parent!(cursor);
            goto_next_sibling!(cursor);
        }

        let block_id = self.bytecode.new_block();
        let def_id = self.bytecode.new_def(block_id, closure_vars.len());
        // Register the name of the definition now so that it can be referenced
        // recursively while compiling itself.
        self.bytecode.def_ids.insert(name.to_string(), def_id);
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
            let mut def_compiler = DefCompiler::new(self, def_id, closure_vars);
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
                let mut def_compiler = DefCompiler::new(self, dummy_def_id, vec![]);
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
    macro_vars: Vec<PropId>,
    local_map: HashMap<String, usize>,
    loop_stack: Vec<BlockId>,
}
impl<'a, 's> DefCompiler<'a, 's> {
    fn new(compiler: &'a mut Compiler<'s>, def_id: DefId, macro_vars: Vec<PropId>) -> Self {
        let local_map = HashMap::default();
        let loop_stack = vec![];
        Self {
            compiler,
            def_id,
            macro_vars,
            local_map,
            loop_stack,
        }
    }
    fn compile_def(&mut self, cursor: &mut TreeCursor, block_id: BlockId) {
        let mut block_compiler = BlockCompiler::new(self, block_id);
        block_compiler.compile_expression(cursor);
        goto_parent!(cursor);
        block_compiler.push(cursor, Op::Return);
    }
    fn compile_closure(&mut self, cursor: &mut TreeCursor, block_id: BlockId) {
        let mut block_compiler = BlockCompiler::new(self, block_id);
        block_compiler.compile_expression(cursor);
        block_compiler.push(cursor, Op::ReturnClosure);
    }
    fn closure_id(&self, macro_var: PropId) -> Result<ClosureId, ()> {
        self.macro_vars
            .iter()
            .position(|&mv| mv == macro_var)
            .map(ClosureId::from_index)
            .ok_or(())
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
    fn push(&mut self, cursor: &TreeCursor, op: Op) {
        let bytecode = &mut self.def_compiler.compiler.bytecode;
        match bytecode.get_block_mut(self.block_id).push(op) {
            OpSize::One => {}
            OpSize::Two => bytecode.source_map.push(
                self.def_compiler.compiler.source_id,
                self.block_id,
                cursor.node().byte_range(),
            ),
        }
        bytecode.source_map.push(
            self.def_compiler.compiler.source_id,
            self.block_id,
            cursor.node().byte_range(),
        );
    }
    fn compile_expression(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, EXPRESSION, "expression");
        goto_first_child!(cursor);
        eprintln!("expression {:?}##", cursor.node().grammar_name());
        match cursor.node().grammar_id() {
            IDENTIFIER => self.compile_identifier(cursor),
            INT => self.compile_int(cursor),
            CHARACTER => self.compile_character(cursor),
            STRING => self.compile_string(cursor),
            GROUPING => self.compile_grouping(cursor),
            OBJECT => self.compile_object(cursor),
            BUILTIN => self.compile_builtin(cursor),
            LOCAL_BIND => self.compile_local_bind(cursor),
            LOCAL_VAR => self.compile_local_var(cursor),
            PROP_BIND => self.compile_prop_bind(cursor),
            PROP_VAR => self.compile_prop_var(cursor),
            CLOSURE_VAR => self.compile_closure_var(cursor),
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
    fn compile_identifier(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, IDENTIFIER, "identifier");
        let string_repr = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("id {string_repr}");
        if let Ok(builtin) = Builtin::try_from(string_repr) {
            let op = match builtin {
                Builtin::Dup => Op::Dup,
                Builtin::Swap => Op::Swap,
                Builtin::Drop => Op::Drop,
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
                Builtin::Return => Op::Return,
                Builtin::EmptyArray => Op::EmptyArray,
                Builtin::ArrayGet => Op::ArrayGet,
                Builtin::ArraySet => Op::ArraySet,
            };
            self.push(cursor, op);
        } else if let Some(&def_id) = self.def_compiler.compiler.bytecode.def_ids.get(string_repr) {
            eprintln!("Looked up {def_id:?}");
            let macro_vars_size = self
                .def_compiler
                .compiler
                .bytecode
                .get_def(def_id)
                .closure_count;
            for _ in 0..macro_vars_size {
                // Return to parent expression
                goto_parent!(cursor);
                // Advance to the next token, hopefully an expression
                goto_next_sibling!(cursor);
                let closure_block_id = self.def_compiler.compiler.bytecode.new_block();
                let closure_def_id = self
                    .def_compiler
                    .compiler
                    .bytecode
                    .new_def(closure_block_id, 0);
                // Call compile_def in the existing def context
                self.def_compiler.compile_closure(cursor, closure_block_id);
                // Encode the def_ids of the closures as literals on the stack.
                // The VM will retrieve them when the def is called.
                self.push(cursor, Op::Integer(closure_def_id.to_value()));
                goto_first_child!(cursor);
            }
            self.push(cursor, Op::Call(def_id));
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
    fn compile_character(&mut self, cursor: &mut TreeCursor) {
        let bytes = self.def_compiler.compiler.source.as_bytes();
        let full_range = cursor.node().byte_range();
        let start = full_range.start;
        let c = if full_range.len() == 3 {
            bytes[start + 1]
        } else if full_range.len() == 4 {
            assert_eq!(bytes[start + 1], b'\\');

            match bytes[start + 2] {
                b'n' => b'\n',
                b't' => b'\t',
                b'\\' => b'\\',
                _ => unreachable!(),
            }
        } else {
            unreachable!()
        };
        self.push(cursor, Op::Character(c));
    }
    fn compile_string(&mut self, cursor: &mut TreeCursor) {
        let string = {
            let bytes = self.def_compiler.compiler.source.as_bytes();
            let full_range = cursor.node().byte_range();
            let range = full_range.start + 1..full_range.end - 1;
            let mut string = String::new();
            let mut slashed = false;
            for &b in &bytes[range] {
                if slashed {
                    slashed = false;
                    let c = match b {
                        b'\n' => '\n',
                        b'\t' => '\t',
                        b'\\' => '\\',
                        _ => unreachable!("invalid escape sequence"),
                    };
                    string.push(c);
                } else {
                    if b == b'\\' {
                        slashed = true;
                    } else {
                        string.push(b as char);
                    }
                }
            }
            string
        };
        let def_ids = &self.def_compiler.compiler.bytecode.def_ids;
        let empty_list_def_id = *def_ids.get("empty_list").expect("stdlib not loaded");
        let push_def_id = *def_ids.get("push").expect("stdlib not loaded");
        self.push(cursor, Op::Call(empty_list_def_id));
        for &c in string.as_bytes().iter() {
            self.push(cursor, Op::Dup);
            self.push(cursor, Op::Character(c));
            self.push(cursor, Op::Call(push_def_id));
        }
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
        self.push(cursor, Op::Integer(int as u64));
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
        self.push(cursor, Op::Integer(int as u64));
    }
    fn compile_grouping(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, GROUPING, "grouping");
        goto_first_child!(cursor);
        assert_node_id!(cursor, SYMBOL_LBRACKET, "symbol_lbracket");
        goto_next_sibling!(cursor);
        while cursor.node().grammar_id() == EXPRESSION {
            self.compile_expression(cursor);
            cursor.goto_next_sibling();
            skip_comments!(cursor);
        }
        eprintln!("{:?}", cursor.node());
        assert_node_id!(cursor, SYMBOL_RBRACKET, "symbol_rbracket");
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
        self.push(cursor, Op::Layout(layout_id));
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
                .get_local_count();
            self.def_compiler
                .local_map
                .insert(local_name.to_string(), id);
            self.def_compiler
                .compiler
                .bytecode
                .get_def_mut(self.def_compiler.def_id)
                .incr_local_count();
            id
        };
        let local_id = LocalId::new(local_id as u64);
        self.push(cursor, Op::BindLocal(local_id));

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
        self.push(cursor, Op::PushLocal(local_id));
        goto_parent!(cursor);
    }
    fn compile_prop_bind(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, PROP_BIND, "prop_bind");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let prop_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  ob {:?}", prop_name);
        let prop_id = self.def_compiler.compiler.prop_id_for(prop_name);
        self.push(cursor, Op::BindProp(prop_id));

        goto_parent!(cursor);
    }
    fn compile_prop_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, PROP_VAR, "prop_var");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let prop_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  ov {:?}", prop_name);
        let prop_id = self.def_compiler.compiler.prop_id_for(prop_name);
        self.push(cursor, Op::PushProp(prop_id));
        goto_parent!(cursor);
    }
    fn compile_closure_var(&mut self, cursor: &mut TreeCursor) {
        assert_node_id!(cursor, CLOSURE_VAR, "closure_var");
        goto_first_child!(cursor);
        goto_next_sibling!(cursor);
        let closure_name = &self.def_compiler.compiler.source[cursor.node().byte_range()];
        eprintln!("  mv {:?}", closure_name);
        let var_id = self.def_compiler.compiler.prop_id_for(closure_name);
        let closure_id = self
            .def_compiler
            .closure_id(var_id)
            .expect("closure var exists");
        self.push(cursor, Op::CallClosure(closure_id));
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
            then_block_compiler.push(cursor, Op::GoTo(finally_block_id));
        }
        self.push(cursor, Op::GoToIf(then_block_id));

        skip_comments!(cursor);
        if cursor.goto_next_sibling() {
            // else
            goto_next_sibling!(cursor);
            self.compile_expression(cursor);
        }

        self.push(cursor, Op::GoTo(finally_block_id));
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
                loop_block_compiler.push(cursor, Op::GoTo(loop_block_id));
            }
            self.push(cursor, Op::GoTo(loop_block_id));
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
        goto_next_sibling!(cursor); // "malloc"
        let layout_id = self.resolve_type_constraint(cursor);
        let layout_id = LayoutId::new(layout_id);
        self.push(cursor, Op::Malloc(layout_id));
        goto_parent!(cursor);
    }
}
impl BlockCompiler<'_, '_, '_> {
    fn resolve_type_constraint(&mut self, cursor: &mut TreeCursor) -> u64 {
        let block_id = self.def_compiler.compiler.bytecode.new_block();
        let mut block_compiler = BlockCompiler::new(self.def_compiler, block_id);
        block_compiler.compile_expression(cursor);
        eprintln!("bytecode {:?}", self.def_compiler.compiler.bytecode);
        eprintln!("block_id {block_id:?}");

        let layout_id = evaluate_at_compile_time(self.def_compiler.compiler.bytecode, block_id);
        eprintln!("layout id??? {layout_id}\n\n\n");
        layout_id
    }
}

macro_rules! compile_builtin_method {
    ($method:ident, $lower:ident, $pascal:ident, $upper:ident) => {
        fn $method(&mut self, cursor: &mut TreeCursor) {
            assert_node_id!(cursor, $upper, stringify!($lower));
            self.push(cursor, Op::$pascal);
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

pub fn compile_source(bytecode: &mut ByteCode, source_name: &str) {
    let source = crate::source::read_source_file(source_name);
    Compiler::new(bytecode, source_name, &source).compile();
}

pub fn compile_stdlib(bytecode: &mut ByteCode) {
    for lib in crate::source::LIBS {
        compile_source(bytecode, lib);
    }
}
