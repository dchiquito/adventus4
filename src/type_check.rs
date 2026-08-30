use std::collections::{HashMap, HashSet};

use crate::bytecode::{BlockId, ByteCode, DefId, Definition, LayoutId, Op};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinType {
    Type,
    Int,
    Char,
    Bool,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Builtin(BuiltinType),
    Object(LayoutId),
    Unknown,
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
    ($b_field:ident($b_id:ident) => $a_field:ident($a_id:ident)) => {
        StackMutation {
            before: vec![Type::$b_field($b_id)],
            after: vec![Type::$a_field($a_id)],
        }
    };
    (=> $field:ident($id:ident)) => {
        StackMutation {
            before: vec![],
            after: vec![Type::$field($id)],
        }
    };
    ($($before:ident) * => $($after:ident) *) => {
        StackMutation {
            before: vec![$(Type::Builtin(BuiltinType::$before)),*],
            after: vec![$(Type::Builtin(BuiltinType::$after)),*],
        }
    };
}
impl StackMutation {
    // TODO make this mutate
    fn chain(&self, other: &StackMutation) -> StackMutation {
        for (lhs, rhs) in self.after.iter().rev().zip(other.before.iter().rev()) {
            if lhs != &Type::Unknown && rhs != &Type::Unknown {
                assert_eq!(lhs, rhs, "noooo {self:?}  :::  {other:?}");
            }
        }
        if self.after.len() >= other.before.len() {
            // (a->b c d) + (c d->) = (a-> b)
            let mut after = Vec::from(&self.after[..(self.after.len() - other.before.len())]);
            after.append(&mut other.after.clone());
            StackMutation {
                before: self.before.clone(),
                after,
            }
        } else {
            // (a->b) + (c d->e) = (c a->e)
            let mut before = Vec::from(&other.before[..other.before.len() - self.after.len()]);
            before.append(&mut self.before.clone());
            StackMutation {
                before,
                after: other.after.clone(),
            }
        }
    }
    fn reconcile(&self, other: &StackMutation) -> StackMutation {
        if self.after != other.after {
            panic!("two different types: {self:?} and {other:?}")
        } else if self.before != other.before {
            todo!(
                "This is technically allowed, previous needs to be extended after checking that the subset matches"
            )
        } else {
            self.clone()
        }
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
            let def = self.bytecode.get_def(def_id);
            let t = self.infer_def(def);
            eprintln!("{def_id:?} has type {t:?}!!!\n");
        }
    }
    fn generate_def_dependency_graph(&mut self) -> HashMap<DefId, Vec<DefId>> {
        let mut graph = HashMap::new();
        let mut blocks_to_check = vec![];
        for def_id in self.bytecode.iter_def_ids() {
            let def = self.bytecode.get_def(def_id);
            blocks_to_check.clear();
            blocks_to_check.push(def.block_id);
            let mut dependencies = vec![];
            // If there is a declared type, we do not need to rely on dependencies to
            // compute the type of the def.
            if def.declared_type.is_none() {
                while let Some(block_id) = blocks_to_check.pop() {
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
                            Op::GoTo(next_block_id) => blocks_to_check.push(next_block_id),
                            Op::GoToIf(next_block_id) => blocks_to_check.push(next_block_id),
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
                .unwrap_or_else(|| self.infer_def(self.bytecode.get_def(def_id)));
            // Save the declared or computed type for next time.
            self.def_types.insert(def_id, t);
            self.def_types.get(&def_id).unwrap()
        }
    }
    fn infer_def(&mut self, def: &Definition) -> StackMutation {
        DefInferer::new(self).infer_def(def)
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
    fn infer_def(mut self, def: &Definition) -> StackMutation {
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
            self.current_type = self.current_type.chain(&sm);
            match op {
                Op::Return => {
                    if let Some(existing) = &self.inferred_type {
                        self.inferred_type = Some(self.current_type.reconcile(existing));
                    } else {
                        self.inferred_type = Some(self.current_type.clone());
                    }
                    break;
                }
                Op::GoTo(block_id) => {
                    if let Some(previous_type) = self.blocks.get(&block_id) {
                        self.current_type = self.current_type.reconcile(previous_type);
                    }
                    self.walk_block(block_id);
                    break;
                }
                Op::GoToIf(block_id) => {
                    if let Some(previous_type) = self.blocks.get(&block_id) {
                        self.current_type = self.current_type.reconcile(previous_type);
                    }
                    let saved_current_type = self.current_type.clone();
                    self.walk_block(block_id);
                    self.current_type = saved_current_type;
                }
                _ => {}
            }
        }
    }
    fn infer_op(&mut self, op: Op) -> StackMutation {
        match op {
            Op::Literal(_) => stack_mutation!(=>Int),
            Op::Call(def_id) => self.types.type_of_def(def_id).clone(),
            Op::Return => stack_mutation!(=>),
            Op::GoTo(_) => stack_mutation!(=>),
            Op::GoToIf(_) => stack_mutation!(Bool=>),
            Op::Dup => stack_mutation!(Int=>Int Int),
            Op::Swap => stack_mutation!(Int Int=>Int Int),
            Op::Pop => stack_mutation!(Int=>),
            Op::BindLocal(_) => stack_mutation!(Int=>),
            Op::PushLocal(_) => stack_mutation!(=>Int),
            Op::BindProp(_) => stack_mutation!(Int=>),
            Op::PushProp(_) => stack_mutation!(=>Int),
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
            Op::Print => stack_mutation!(Int=>Int),
            Op::Layout(layout_id) => stack_mutation!(=>Object(layout_id)),
            Op::Malloc(layout_id) => {
                let mut sm = stack_mutation!(=>Object(layout_id));
                let props = self.types.bytecode.layouts.get(&layout_id).unwrap();
                for prop_id in props.iter() {
                    sm.before.push(Type::Unknown);
                }
                sm
            }
        }
    }
}

#[cfg(test)]
mod test {
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
}
