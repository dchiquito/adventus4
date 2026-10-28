use std::collections::{HashMap, HashSet};

use log::{debug, trace};

use crate::bytecode::{
    BlockId, ByteCode, ClosureId, DefId, LayoutId, LocalId, Op, PropId, Terminator,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Foo,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinType {
    Type,
    Int,
    Char,
    Bool,
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct GenericId(u64);
impl GenericId {
    pub fn incr(&self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Builtin(BuiltinType),
    Object(LayoutId),
    Local(LocalId),
    Prop(PropId),
    Generic(GenericId),
    DefId(DefId),
    ClosureId(ClosureId),
    Unknown,
}
impl Type {
    fn union(&self, other: &Type) -> Result<Type> {
        if self == other {
            Ok(self.clone())
        } else if self == &Type::Unknown {
            Ok(other.clone())
        } else if other == &Type::Unknown {
            Ok(self.clone())
        } else {
            Err(Error::Foo)
        }
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
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
    (expand, $v:expr; DefId($def_id:expr) $($stack:tt)*) => {
        $v.push(Type::DefId($def_id));
        stack_mutation!(expand, $v; $($stack)*);
    };
    (expand, $v:expr; ClosureId($def_id:expr) $($stack:tt)*) => {
        $v.push(Type::ClosureId($def_id));
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
    (expand, $v:expr; T($a:expr) $($stack:tt)*) => {
        $v.push(Type::Generic($a));
        stack_mutation!(expand, $v; $($stack)*);
    };
}
impl StackMutation {
    /// Create a new StackMutation representing the union of an initial block state
    /// with a new one. Extend before appropriately, and return an error if after
    /// is different.
    fn reconcile(&self, other: &StackMutation) -> Result<StackMutation> {
        trace!("hmmmmm {:?} :: {:?}", self, other);
        let before = {
            let (shorter, longer) = if self.before.len() <= other.before.len() {
                (&self.before, &other.before)
            } else {
                (&other.before, &self.before)
            };
            shorter
                .iter()
                .rev()
                .chain(std::iter::repeat(&Type::Unknown))
                .zip(longer.iter().rev())
                .map(|(x, y)| x.union(y))
                .collect::<Result<Vec<Type>>>()?
        };
        let after = {
            if self.after.len() != other.after.len() {
                // panic!("{:?} != {:?}", self.after, other.after);
                Err(Error::Foo)?
            } else {
                self.after
                    .iter()
                    .zip(other.after.iter())
                    .map(|(x, y)| x.union(y))
                    .collect::<Result<Vec<Type>>>()?
            }
        };
        Ok(StackMutation { before, after })
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
    def_types: HashMap<(DefId, Vec<StackMutation>), StackMutation>,
    current_generic: GenericId,
    generics: HashMap<GenericId, Type>,
}
impl<'b> Types<'b> {
    pub fn new(bytecode: &'b ByteCode) -> Self {
        let def_types = Default::default();
        let current_generic = Default::default();
        let generics = Default::default();
        Self {
            bytecode,
            def_types,
            current_generic,
            generics,
        }
    }
    pub fn infer(&mut self) -> Result<()> {
        {
            let graph = self.generate_def_dependency_graph();
            self.check_for_def_dependency_loops(graph);
        }
        // At this point, we can be sure that any def dependency loops can be broken by using
        // the declared type of the def.
        for def_id in self.bytecode.iter_def_ids() {
            trace!("Beginning inference of {def_id:?}");
            let inferred_type = DefInferer::infer(self, def_id)?;
        }
        Ok(())
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
                        if let Op::Call(called_def_id) = op
                            && dependencies
                                .iter()
                                .find(|&&id| id == called_def_id)
                                .is_none()
                        {
                            dependencies.push(called_def_id);
                        }
                    }
                    match block.terminator {
                        Terminator::GoTo(block_id) => {
                            blocks_to_check.push(block_id);
                        }
                        Terminator::GoToIf { then, or_else } => {
                            blocks_to_check.push(then);
                            blocks_to_check.push(or_else);
                        }
                        _ => (),
                    }
                }
            }
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
    /// Get the cached type of a def, or infer it if it is not yet cached.
    fn type_of_def(
        &mut self,
        def_id: DefId,
        closures: Vec<StackMutation>,
    ) -> Result<&StackMutation> {
        // First, check the precomputed cache
        if self.def_types.contains_key(&(def_id, closures.clone())) {
            Ok(self.def_types.get(&(def_id, closures)).unwrap())
        } else {
            // Second, check if a type was declared.
            let t = self
                .bytecode
                .get_def(def_id)
                .declared_type
                .clone()
                .map(Ok)
                // Third, compute the type
                .unwrap_or_else(|| {
                    DefInferer::infer_with_closures(self, def_id, closures.clone())
                })?;
            // Save the declared or computed type for next time.
            self.def_types.insert((def_id, closures.clone()), t);
            Ok(self.def_types.get(&(def_id, closures)).unwrap())
        }
    }
    fn new_generic(&mut self) -> GenericId {
        let g = self.current_generic;
        self.current_generic = self.current_generic.incr();
        g
    }
    fn chain(&mut self, left: &StackMutation, right: &StackMutation) -> StackMutation {
        for (lhs, rhs) in left.after.iter().rev().zip(right.before.iter().rev()) {
            if let Type::Generic(t) = lhs {
                if !self.generics.contains_key(t) {
                    self.generics.insert(*t, lhs.clone());
                }
            } else if let Type::Generic(t) = rhs {
                if !self.generics.contains_key(t) {
                    self.generics.insert(*t, lhs.clone());
                }
            } else if lhs != &Type::Unknown && rhs != &Type::Unknown {
                assert_eq!(
                    lhs, rhs,
                    "noooo {left:?}  :::  {right:?}\n{lhs:?} != {rhs:?}"
                );
            }
        }
        let mut mutation = if left.after.len() >= right.before.len() {
            //   (a->b c d)
            // +      (c d->)
            // = (a->b)
            let mut after = Vec::from(&left.after[..(left.after.len() - right.before.len())]);
            after.append(&mut right.after.clone());
            StackMutation {
                before: left.before.clone(),
                after,
            }
        } else {
            //      (a->b)
            // + (c   b    ->d)
            // = (c   a    ->d)
            let mut before = Vec::from(&right.before[..right.before.len() - left.after.len()]);
            before.append(&mut left.before.clone());
            let after = right.after.clone();
            StackMutation { before, after }
        };
        {
            for b in mutation.before.iter_mut() {
                if let Type::Generic(t) = b
                    && let Some(v) = self.generics.get(t)
                {
                    *b = v.clone();
                }
            }
            for a in mutation.after.iter_mut() {
                if let Type::Generic(t) = a
                    && let Some(v) = self.generics.get(t)
                {
                    *a = v.clone();
                }
            }
        }
        mutation
    }
}

struct DefInferer<'a, 'b> {
    types: &'a mut Types<'b>,
    closures: Vec<StackMutation>,
    blocks: HashMap<BlockId, StackMutation>,
    current_type: StackMutation,
    inferred_type: Option<StackMutation>,
}
impl<'a, 'b> DefInferer<'a, 'b> {
    fn infer(types: &'a mut Types<'b>, def_id: DefId) -> Result<Option<StackMutation>> {
        trace!("BEGIN INFERENCE OF {def_id:?}");
        let def = types.bytecode.get_def(def_id);
        if def.closure_count == 0 {
            Self::new(types, vec![]).infer_def(def_id).map(Some)
        } else {
            Ok(None)
        }
    }
    fn infer_with_closures(
        types: &'a mut Types<'b>,
        def_id: DefId,
        closures: Vec<StackMutation>,
    ) -> Result<StackMutation> {
        Self::new(types, closures).infer_def(def_id)
    }
    fn new(types: &'a mut Types<'b>, closures: Vec<StackMutation>) -> Self {
        let blocks = Default::default();
        let inferred_type = None;
        let current_type = stack_mutation!(=>);
        Self {
            types,
            closures,
            blocks,
            current_type,
            inferred_type,
        }
    }
    fn infer_def(mut self, def_id: DefId) -> Result<StackMutation> {
        let name = self.types.bytecode.name_of_def(def_id);
        let def = self.types.bytecode.get_def(def_id);
        debug!(
            "Inferring def {name} ({def_id:?}) ({:?} closures: {:?})",
            def.closure_count, self.closures,
        );
        self.walk_block(def.block_id)?;
        debug!(
            "Inferred type of {name} ({def_id:?}) to be {:?}",
            self.inferred_type
        );
        Ok(self.inferred_type.expect("no return statements"))
    }
    fn walk_block(&mut self, block_id: BlockId) -> Result<()> {
        trace!("Inferring block {block_id:?}");
        self.blocks.insert(block_id, self.current_type.clone());
        let block = self.types.bytecode.get_block(block_id);
        for op in block.iter() {
            let sm = self.infer_op(op)?;
            trace!("Inferred {op:?} to be {sm:?}");
            self.current_type = self.types.chain(&self.current_type, &sm);
        }
        match block.terminator {
            Terminator::Return => {
                if let Some(existing) = &self.inferred_type {
                    self.inferred_type = Some(self.current_type.reconcile(existing)?);
                } else {
                    self.inferred_type = Some(self.current_type.clone());
                }
            }
            Terminator::ReturnClosure => {
                if let Some(existing) = &self.inferred_type {
                    self.inferred_type = Some(self.current_type.reconcile(existing)?);
                } else {
                    self.inferred_type = Some(self.current_type.clone());
                }
            }
            Terminator::GoTo(block_id) => {
                if let Some(previous_type) = self.blocks.get(&block_id) {
                    self.current_type = self.current_type.reconcile(previous_type)?;
                    self.blocks.insert(block_id, self.current_type.clone());
                } else {
                    self.walk_block(block_id)?;
                }
            }
            Terminator::GoToIf { then, or_else } => {
                // Consume the boolean
                self.current_type = self
                    .types
                    .chain(&self.current_type, &stack_mutation!(Bool=>));
                let mut then_type = self.current_type.clone();
                if let Some(previous_type) = self.blocks.get(&then) {
                    self.current_type = self.current_type.reconcile(previous_type)?;
                    self.blocks.insert(block_id, self.current_type.clone());
                } else {
                    self.walk_block(then)?;
                }
                // Restore self.current_type and save the type of the then branch
                std::mem::swap(&mut self.current_type, &mut then_type);
                if let Some(previous_type) = self.blocks.get(&or_else) {
                    self.current_type = self.current_type.reconcile(previous_type)?;
                    self.blocks.insert(block_id, self.current_type.clone());
                } else {
                    self.walk_block(or_else)?;
                }
                // Verify then and or_else branches have the same type
                self.current_type = self.current_type.reconcile(&then_type)?;
            }
        }
        Ok(())
    }
    fn infer_op(&mut self, op: Op) -> Result<StackMutation> {
        trace!("Inferring type of {op:?}");
        Ok(match op {
            Op::Integer(_) => stack_mutation!(=>Int),
            Op::Character(_) => stack_mutation!(=>Char),
            Op::Call(def_id) => {
                trace!("Call to {def_id:?}: {:?}!!!", self.current_type);
                let def = self.types.bytecode.get_def(def_id);
                trace!("It has {} closures", def.closure_count);
                let before: Vec<Type> = self
                    .current_type
                    .after
                    .iter()
                    .rev()
                    .take(def.closure_count)
                    .rev()
                    .cloned()
                    .collect();
                trace!("before: {before:?}");
                let closures: Vec<StackMutation> = before
                    .iter()
                    .map(|t| {
                        if let Type::ClosureId(closure_id) = t {
                            let closure = self.types.bytecode.get_closure(*closure_id);
                            let block_id = closure.block_id;
                            // Save the current_type to restore later
                            let mut closure_type = stack_mutation!(=>);
                            std::mem::swap(&mut self.current_type, &mut closure_type);
                            self.walk_block(block_id)?;
                            // Swap current_type to restore it and retrieve the
                            // closure_type
                            std::mem::swap(&mut self.current_type, &mut closure_type);
                            Ok(closure_type)
                        } else {
                            unreachable!("compilation error: not enough closures")
                        }
                    })
                    .collect::<Result<Vec<StackMutation>>>()?;
                trace!("closures: {closures:?}");
                let closure_consumer = StackMutation {
                    before,
                    after: vec![],
                };
                let def_type = self.types.type_of_def(def_id, closures)?.clone();
                self.types.chain(&closure_consumer, &def_type)
            }
            Op::CallClosure(closure_id) => {
                let idx = closure_id.to_index();
                trace!("Calling closure {idx} with closures {:?}", self.closures);
                self.closures[idx].clone()
            }
            Op::ClosureId(closure_id) => stack_mutation!(=>ClosureId(closure_id)),
            Op::Dup => {
                let a = self.types.new_generic();
                stack_mutation!(T(a)=>T(a) T(a))
            }
            Op::Swap => {
                let a = self.types.new_generic();
                let b = self.types.new_generic();
                stack_mutation!(T(a) T(b)=>T(b) T(a))
            }
            Op::Drop => {
                let a = self.types.new_generic();
                stack_mutation!(T(a)=>)
            }
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
            Op::Print => stack_mutation!(Unknown=>),
            Op::PrintStr => stack_mutation!(Unknown=>),
            Op::ReadLine => {
                let layout_id = self.types.bytecode.get_list_layout_id();
                stack_mutation!(=>Object(layout_id))
            }
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
        })
    }
}

#[cfg(test)]
mod test {
    use crate::compile::compile_test;

    use super::*;

    #[test]
    fn test_chain_stack_mutations() {
        macro_rules! test_stack_mutation{
            (($($b1:ident) * => $($a1:ident) *) + ($($b2:ident) * => $($a2:ident) *) == ($($b3:ident) * => $($a3:ident) *)) => {{
                let bc = ByteCode::default();
                let mut types = Types::new(&bc);
            assert_eq!(
                types.chain(
                    &stack_mutation!($($b1) * => $($a1) *),
                    &stack_mutation!($($b2) * => $($a2) *)
                ),
                stack_mutation!($($b3) * => $($a3) *)
            );
            }}
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

    macro_rules! infer_def_results {
        ($source:expr) => {{
            let bytecode = compile_test($source);
            let mut types = Types::new(&bytecode);
            let _ = types.infer();
            bytecode
                .iter_def_ids()
                .map(|def_id| DefInferer::infer(&mut types, def_id).map(Option::unwrap))
                .collect::<Vec<Result<StackMutation>>>()
        }};
    }
    macro_rules! infer_defs {
        ($source:expr) => {{
            let bytecode = compile_test($source);
            let mut types = Types::new(&bytecode);
            types.infer().expect("no errors");
            bytecode
                .iter_def_ids()
                .map(|def_id| DefInferer::infer(&mut types, def_id).unwrap().unwrap())
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
        assert_eq!(
            infer_def_results!(
                "def infinite [
                loop [
                    1
                    [1 2 ==] if [break]
                ]
            ]"
            ),
            [Err(Error::Foo)]
        );
    }
}
