use std::collections::{HashMap, HashSet};

use crate::bytecode::{BlockId, ByteCode, DefId, LayoutId, LocalId, Op, PropId, Terminator};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinType {
    Type,
    Int,
    Char,
    Bool,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GenericId(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Builtin(BuiltinType),
    Object(LayoutId),
    Local(LocalId),
    Prop(PropId),
    Generic(GenericId),
    Unknown,
}
impl Type {
    fn union(&self, other: &Type) -> Result<Type, ()> {
        if self == other {
            Ok(self.clone())
        } else if self == &Type::Unknown {
            Ok(other.clone())
        } else if other == &Type::Unknown {
            Ok(self.clone())
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct StackMutation {
    before: Vec<Type>,
    after: Vec<Type>,
}
impl StackMutation {
    pub fn new(before: Vec<Type>, after: Vec<Type>) -> Self {
        Self { before, after }
    }
}
macro_rules! stack_mutation {
    // (=>) => {
    //     StackMutation {
    //         before: vec![],
    //         after: vec![],
    //     }
    // };
    ($($l_field:ident $(($l_prop:tt))? )* => $($r_field:ident $(($r_prop:tt))?)*) => {
        StackMutation {
            before: stack_mutation!(stack_state $($l_field $(($l_prop))?)*),
            after: stack_mutation!(stack_state $($r_field $(($r_prop))?)*),
        }
    };
    // ([$($lhs:tt)*] => [$($rhs:tt)*]) => {
    //     StackMutation {
    //         before: stack_mutation!(stack_state $($lhs)*),
    //         after: stack_mutation!(stack_state $($rhs)*),
    //     }
    // };
    (stack_state) => {
        vec![]
    };
    (stack_state $($stack:tt)*) => {{
        #[allow(clippy::vec_init_then_push)]
        {
            let mut v = vec![];
            stack_mutation!(expand, v; $($stack)*);
            v
        }
    }};
    (expand, $v:expr; Type $($stack:tt)*) => {
        $v.push(Type::Builtin(BuiltinType::Type));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr;) => {};
    (expand, $v:expr; Int $($stack:tt)*) => {
        $v.push(Type::Builtin(BuiltinType::Int));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; Char $($stack:tt)*) => {
        $v.push(Type::Builtin(BuiltinType::Char));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; Bool $($stack:tt)*) => {
        $v.push(Type::Builtin(BuiltinType::Bool));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; None $($stack:tt)*) => {
        $v.push(Type::Builtin(BuiltinType::None));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; Unknown $($stack:tt)*) => {
        $v.push(Type::Unknown);
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; Object($layout_id:expr) $($stack:tt)*) => {
        $v.push(Type::Object($layout_id));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; T($id:tt) $($stack:tt)*) => {
        $v.push(Type::Generic(GenericId($id)));
        stack_mutation!(expand, $v; $($stack)*);
    };
}
impl StackMutation {
    // TODO make this mutate
    fn chain(&self, other: &StackMutation) -> StackMutation {
        let mut generics: HashMap<GenericId, Type> = HashMap::new();
        for (lhs, rhs) in self.after.iter().rev().zip(other.before.iter().rev()) {
            if let Type::Generic(_) = lhs {
                todo!("generics are not concrete types");
            } else if let Type::Generic(t) = rhs {
                if let Some(existing) = generics.get(t) {
                    todo!("reconcile");
                } else {
                    generics.insert(*t, lhs.clone());
                }
            } else if lhs != &Type::Unknown && rhs != &Type::Unknown {
                assert_eq!(
                    lhs, rhs,
                    "noooo {self:?}  :::  {other:?}\n{lhs:?} != {rhs:?}"
                );
            }
        }
        let mut mutation = if self.after.len() >= other.before.len() {
            //   (a->b c d)
            // +      (c d->)
            // = (a->b)
            let mut after = Vec::from(&self.after[..(self.after.len() - other.before.len())]);
            after.append(&mut other.after.clone());
            StackMutation {
                before: self.before.clone(),
                after,
            }
        } else {
            //      (a->b)
            // + (c   b    ->d)
            // = (c   a    ->d)
            let mut before = Vec::from(&other.before[..other.before.len() - self.after.len()]);
            before.append(&mut self.before.clone());
            let after = other.after.clone();
            StackMutation { before, after }
        };
        {
            for b in mutation.before.iter_mut() {
                if let Type::Generic(t) = b {
                    *b = generics.get(t).expect("undefined generic").clone()
                }
            }
            for a in mutation.after.iter_mut() {
                if let Type::Generic(t) = a {
                    *a = generics.get(t).expect("undefined generic").clone()
                }
            }
        }
        mutation
    }
    fn reconcile(&self, other: &StackMutation) -> StackMutation {
        let before = self
            .before
            .iter()
            .rev()
            .zip(other.before.iter().rev())
            .map(|(x, y)| x.union(y).expect("mismatched types"))
            .collect();
        let after = self
            .after
            .iter()
            .zip(other.after.iter())
            .map(|(x, y)| x.union(y).expect("mismatched types"))
            .collect();
        StackMutation { before, after }
    }
}
impl std::fmt::Debug for StackMutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.before.iter() {
            write!(f, "{b:?} ")?;
        }
        write!(f, "-> ")?;
        for a in self.after.iter() {
            write!(f, "{a:?} ")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Types<'b> {
    bytecode: &'b ByteCode,
    def_types: HashMap<DefId, StackMutation>,
}
impl<'b> Types<'b> {
    pub fn new(bytecode: &'b ByteCode) -> Self {
        let def_types = Default::default();
        Self {
            bytecode,
            def_types,
        }
    }
    pub fn infer(&mut self) {
        {
            let graph = self.generate_def_dependency_graph();
            self.check_for_def_dependency_loops(graph);
        }
        // At this point, we can be sure that any def dependency loops can be broken by using
        // the declared type of the def.
        for def_id in self.bytecode.iter_def_ids() {
            let t = self.infer_def(def_id);
            eprintln!("{def_id:?} has type {t:?}!!!\n");
        }
    }
    fn generate_def_dependency_graph(&mut self) -> HashMap<DefId, Vec<DefId>> {
        let mut graph = HashMap::new();
        let mut blocks_to_check = vec![];
        let mut checked_blocks = vec![];
        for def_id in self.bytecode.iter_def_ids() {
            let def = self.bytecode.get_def(def_id);
            blocks_to_check.clear();
            blocks_to_check.push(def.block_id);
            let mut dependencies = vec![];
            // If there is a declared type, we do not need to rely on dependencies to
            // compute the type of the def.
            if def.declared_type.is_none() {
                while let Some(block_id) = blocks_to_check.pop() {
                    if checked_blocks.contains(&block_id) {
                        continue;
                    }
                    checked_blocks.push(block_id);
                    let block = self.bytecode.get_block(block_id);
                    for op in block.iter() {
                        match op {
                            Op::Call(called_def_id) => {
                                if dependencies
                                    .iter()
                                    .find(|&&id| id == called_def_id)
                                    .is_none()
                                {
                                    dependencies.push(called_def_id);
                                }
                            }
                            // TODO
                            // Op::GoTo(next_block_id) | Op::GoToIf(next_block_id) => {
                            //     blocks_to_check.push(next_block_id)
                            // }
                            _ => {}
                        }
                    }
                }
            }
            println!("Def {def_id:?} has dependencies {dependencies:?}");
            graph.insert(def_id, dependencies);
        }
        graph
    }
    fn check_for_def_dependency_loops(&self, graph: HashMap<DefId, Vec<DefId>>) {
        fn check_def(
            graph: &HashMap<DefId, Vec<DefId>>,
            stack: &mut Vec<DefId>,
            checked: &mut HashSet<DefId>,
            def_id: DefId,
        ) {
            if checked.contains(&def_id) {
                return;
            }
            if stack.contains(&def_id) {
                panic!("cycle detected: {stack:?}")
            }
            stack.push(def_id);
            let deps = graph.get(&def_id).unwrap();
            for dep in deps {
                check_def(graph, stack, checked, *dep);
            }
            stack.pop();
            checked.insert(def_id);
        }
        let mut stack = vec![];
        let mut checked = HashSet::new();
        for (root_def_id, _) in graph.iter() {
            check_def(&graph, &mut stack, &mut checked, *root_def_id);
        }
    }
    fn type_of_def(&mut self, def_id: DefId) -> &StackMutation {
        // First, check the precomputed cache
        if self.def_types.contains_key(&def_id) {
            self.def_types.get(&def_id).unwrap()
        } else {
            // Second, check if a type was declared.
            let t = self
                .bytecode
                .get_def(def_id)
                .declared_type
                .clone()
                // Third, compute the type
                .unwrap_or_else(|| self.infer_def(def_id));
            // Save the declared or computed type for next time.
            self.def_types.insert(def_id, t);
            self.def_types.get(&def_id).unwrap()
        }
    }
    fn infer_def(&mut self, def_id: DefId) -> StackMutation {
        DefInferer::new(self).infer_def(def_id)
    }
}

struct DefInferer<'a, 'b> {
    types: &'a mut Types<'b>,
    blocks: HashMap<BlockId, StackMutation>,
    current_type: StackMutation,
    inferred_type: Option<StackMutation>,
}
impl<'a, 'b> DefInferer<'a, 'b> {
    fn new(types: &'a mut Types<'b>) -> Self {
        let blocks = Default::default();
        let inferred_type = None;
        let current_type = stack_mutation!(=>);
        Self {
            types,
            blocks,
            current_type,
            inferred_type,
        }
    }
    fn infer_def(mut self, def_id: DefId) -> StackMutation {
        let def = self.types.bytecode.get_def(def_id);
        eprintln!("Inferring def {def:?}");
        self.walk_block(def.block_id);
        self.inferred_type.expect("no return statements")
    }
    fn walk_block(&mut self, block_id: BlockId) {
        eprintln!("Walking {block_id:?}: {:?}", self.current_type);
        self.blocks.insert(block_id, self.current_type.clone());
        let block = self.types.bytecode.get_block(block_id);
        eprintln!("{:?}", block.pretty_print());
        for op in block.iter() {
            let sm = self.infer_op(op);
            eprintln!("ct {:?} sm {:?} op {:?}", self.current_type, sm, op);
            self.current_type = self.current_type.chain(&sm);
        }
        match block.terminator {
            Terminator::Return => {
                if let Some(existing) = &self.inferred_type {
                    self.inferred_type = Some(self.current_type.reconcile(existing));
                } else {
                    self.inferred_type = Some(self.current_type.clone());
                }
            }
            Terminator::ReturnClosure => {
                if let Some(existing) = &self.inferred_type {
                    self.inferred_type = Some(self.current_type.reconcile(existing));
                } else {
                    self.inferred_type = Some(self.current_type.clone());
                }
            }
            Terminator::GoTo(block_id) => {
                if let Some(previous_type) = self.blocks.get(&block_id) {
                    self.current_type = self.current_type.reconcile(previous_type);
                    self.blocks.insert(block_id, self.current_type.clone());
                } else {
                    self.walk_block(block_id);
                }
            }
            Terminator::GoToIf { then, or_else } => {
                todo!();
                if let Some(previous_type) = self.blocks.get(&block_id) {
                    self.current_type = self.current_type.reconcile(previous_type);
                    self.blocks.insert(block_id, self.current_type.clone());
                } else {
                    let saved_current_type = self.current_type.clone();
                    self.walk_block(block_id);
                    self.current_type = saved_current_type;
                }
            }
        }
    }
    fn infer_op(&mut self, op: Op) -> StackMutation {
        match op {
            Op::Integer(_) => stack_mutation!(=>Int),
            Op::Character(_) => stack_mutation!(=>Char),
            Op::Call(def_id) => self.types.type_of_def(def_id).clone(),
            Op::CallClosure(_closure_id) => stack_mutation!(=>), // TODO
            Op::Dup => stack_mutation!(T(0)=>T(0) T(0)),
            Op::Swap => stack_mutation!(T(0) T(1)=>T(1) T(0)),
            Op::Drop => stack_mutation!(T(0)=>),
            Op::BindLocal(_local_id) => stack_mutation!(Unknown=>),
            Op::PushLocal(_local_id) => stack_mutation!(=>Unknown),
            Op::BindProp(_prop_id) => stack_mutation!(Unknown Unknown=>),
            Op::PushProp(_prop_id) => stack_mutation!(Unknown=>Unknown),
            Op::Add => stack_mutation!(Int Int=>Int),
            Op::Sub => stack_mutation!(Int Int=>Int),
            Op::Mul => stack_mutation!(Int Int=>Int),
            Op::Div => stack_mutation!(Int Int=>Int),
            Op::Eq => stack_mutation!(Int Int=>Bool),
            Op::Ne => stack_mutation!(Int Int=>Bool),
            Op::Gt => stack_mutation!(Int Int=>Bool),
            Op::Lt => stack_mutation!(Int Int=>Bool),
            Op::Gte => stack_mutation!(Int Int=>Bool),
            Op::Lte => stack_mutation!(Int Int=>Bool),
            Op::And => stack_mutation!(Bool Bool=>Bool),
            Op::Or => stack_mutation!(Bool Bool=>Bool),
            Op::Not => stack_mutation!(Bool=>Bool),
            Op::Print => stack_mutation!(Unknown=>Unknown),
            Op::Layout(layout_id) => stack_mutation!(=>Object(layout_id)),
            Op::Malloc(layout_id) => {
                let mut sm = stack_mutation!(=>Object(layout_id));
                let props = self.types.bytecode.layouts.get(&layout_id).unwrap();
                for _prop_id in props.iter() {
                    sm.before.push(Type::Unknown);
                }
                sm
            }
            Op::EmptyArray => stack_mutation!(Int => Unknown),
            Op::ArrayGet => stack_mutation!(Unknown Int => Unknown),
            Op::ArraySet => stack_mutation!(Unknown Int Unknown =>),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::compile::compile_test;

    use super::*;

    #[test]
    fn test_chain_stack_mutations() {
        macro_rules! test_stack_mutation{
            (($($b1:ident) * => $($a1:ident) *) + ($($b2:ident) * => $($a2:ident) *) == ($($b3:ident) * => $($a3:ident) *)) => {
            assert_eq!(
                stack_mutation!($($b1) * => $($a1) *)
                    .chain(&stack_mutation!($($b2) * => $($a2) *)),
                stack_mutation!($($b3) * => $($a3) *)
            );
            }
        }
        test_stack_mutation!((=>) + (=>) == (=>));
        test_stack_mutation!((Int=>) + (=>) == (Int=>));
        test_stack_mutation!((Int Int=>) + (=>) == (Int Int=>));
        test_stack_mutation!((=>) + (=>Int) == (=>Int));
        test_stack_mutation!((=>) + (=>Int Int) == (=>Int Int));
        test_stack_mutation!((=>Int) + (Int=>) == (=>));
        test_stack_mutation!((Int=>Int) + (Int=>) == (Int=>));
        test_stack_mutation!((=>Int) + (Int=>Int) == (=>Int));
        test_stack_mutation!((Int=>Int) + (Int=>Int) == (Int=>Int));
        test_stack_mutation!((=>Int) + (=>) == (=>Int));
        test_stack_mutation!((=>) + (Int=>) == (Int=>));
        test_stack_mutation!((=>Char) + (Int Char=>) == (Int=>));
    }

    macro_rules! infer_defs {
        ($source:expr) => {{
            let bytecode = compile_test($source);
            let mut types = Types::new(&bytecode);
            types.infer();
            bytecode
                .iter_def_ids()
                .map(|def_id| types.type_of_def(def_id).clone())
                .collect::<Vec<StackMutation>>()
        }};
    }

    #[test]
    fn test_simple() {
        assert_eq!(infer_defs!("def one [1]"), [stack_mutation!(=> Int)]);
        assert_eq!(infer_defs!("def two [1 2]"), [stack_mutation!(=> Int Int)]);
        assert_eq!(infer_defs!("def three [1 2 +]"), [stack_mutation!(=> Int)]);
        assert_eq!(
            infer_defs!("def three [1 2 +] def four [5 1 -]"),
            [stack_mutation!(=> Int), stack_mutation!(=> Int)]
        );
        assert_eq!(
            infer_defs!("def one [1] def two [one one +]"),
            [stack_mutation!(=> Int), stack_mutation!(=> Int)]
        );
    }
    #[test]
    fn test_types() {
        assert_eq!(infer_defs!("def zero [0]"), [stack_mutation!(=> Int)]);
        assert_eq!(infer_defs!("def a ['a']"), [stack_mutation!(=> Char)]);
        // TODO use builtins
        assert_eq!(infer_defs!("def t [1 1 ==]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def f [1 2 ==]"), [stack_mutation!(=> Bool)]);
    }
    #[test]
    fn test_arithmetic() {
        assert_eq!(infer_defs!("def add [1 2 +]"), [stack_mutation!(=> Int)]);
        assert_eq!(infer_defs!("def sub [1 2 -]"), [stack_mutation!(=> Int)]);
        assert_eq!(infer_defs!("def mul [1 2 *]"), [stack_mutation!(=> Int)]);
        assert_eq!(infer_defs!("def div [1 2 /]"), [stack_mutation!(=> Int)]);
    }
    #[test]
    fn test_comparison() {
        assert_eq!(infer_defs!("def eq [1 2 ==]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def neq [1 2 !=]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def gt [1 2 >]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def lt [1 2 <]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def gte [1 2 >=]"), [stack_mutation!(=> Bool)]);
        assert_eq!(infer_defs!("def lte [1 2 <=]"), [stack_mutation!(=> Bool)]);
    }
    #[test]
    fn test_bool_ops() {
        // TODO use builtins
        assert_eq!(
            infer_defs!("def and [[1 1 ==] [1 1 ==] and]"),
            [stack_mutation!(=> Bool)]
        );
        assert_eq!(
            infer_defs!("def or [[1 1 ==] [1 1 ==] or]"),
            [stack_mutation!(=> Bool)]
        );
        assert_eq!(
            infer_defs!("def not [[1 1 ==] not]"),
            [stack_mutation!(=> Bool)]
        );
    }
    #[test]
    fn test_dup() {
        assert_eq!(
            infer_defs!("def clone [1 dup]"),
            [stack_mutation!(=> Int Int)]
        );
    }
    #[test]
    fn test_drop() {
        assert_eq!(infer_defs!("def nothin [1 drop]"), [stack_mutation!(=> )]);
    }
    #[test]
    fn test_swap() {
        assert_eq!(
            infer_defs!("def exchange ['1' 2 swap]"),
            [stack_mutation!(=> Int Char)]
        );
    }
    #[test]
    fn test_return() {
        assert_eq!(infer_defs!("def one [1 return]"), [stack_mutation!(=> Int)]);
    }
    #[test]
    fn test_if() {
        assert_eq!(
            infer_defs!("def one [[1 1 ==] if [1] else [2]]"),
            [stack_mutation!(=> Int)]
        );
    }
    #[test]
    fn test_if_return() {
        assert_eq!(
            infer_defs!("def one [[1 1 ==] if [1 return] else [2 return]]"),
            [stack_mutation!(=> Int)]
        );
    }
    #[test]
    fn test_infinite_loop() {
        // TODO this should error because the stack could grow by a variable amount
        assert_ne!(
            infer_defs!(
                "def infinite [
                loop [
                    1
                    [1 2 ==] if [break]
                ]
            ]"
            ),
            [stack_mutation!(=>Int)]
        );
    }
}
