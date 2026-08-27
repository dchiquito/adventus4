use std::collections::HashMap;

use crate::bytecode::{Block, ByteCode, DefId, ObjectId, Op};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinType {
    Bool,
    Int,
    Char,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Builtin(BuiltinType),
    ObjectId(ObjectId),
    DefBefore(DefId),
    DefAfter(DefId),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackMutation {
    before: Vec<Type>,
    after: Vec<Type>,
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
    fn chain(&self, rhs: &StackMutation) -> StackMutation {
        for (lhs, rhs) in self.after.iter().rev().zip(rhs.before.iter().rev()) {
            eprintln!("Chaining {self:?} -> {rhs:?}");
            assert_eq!(lhs, rhs);
        }
        if self.after.len() >= rhs.before.len() {
            // (a->b c d) + (c d->) = (a-> b)
            let mut after = Vec::from(&self.after[..(self.after.len() - rhs.before.len())]);
            after.append(&mut rhs.after.clone());
            StackMutation {
                before: self.before.clone(),
                after,
            }
        } else {
            // (a->b) + (c d->e) = (c a->e)
            let mut before = Vec::from(&rhs.before[..rhs.before.len() - self.after.len()]);
            before.append(&mut self.before.clone());
            StackMutation {
                before,
                after: rhs.after.clone(),
            }
        }
    }
}

#[derive(Debug)]
pub struct Types<'b> {
    bytecode: &'b ByteCode,
    block_types: Vec<StackMutation>,
}
impl<'b> Types<'b> {
    pub fn new(bytecode: &'b ByteCode) -> Self {
        Self {
            bytecode,
            block_types: vec![],
        }
    }
    pub fn infer(&mut self) {
        self.generate_def_dependency_graph();
        for block in self.bytecode.blocks.iter() {
            let block_type = self.infer_block(block);
            self.block_types.push(block_type);
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
            println!("Def {def_id:?} has dependencies {dependencies:?}");
            graph.insert(def_id, dependencies);
        }
        graph
    }
    fn infer_block(&self, block: &Block) -> StackMutation {
        let mut def_type = stack_mutation!(=>);
        for op in block.iter() {
            let sm = self.infer_op(op);
            def_type = def_type.chain(&sm);
        }
        def_type
    }
    fn infer_op(&self, op: Op) -> StackMutation {
        match op {
            Op::Literal(_) => stack_mutation!(=>Int),
            Op::Call(def_id) => stack_mutation!(DefBefore(def_id)=>DefAfter(def_id)),
            Op::Return => stack_mutation!(=>),
            Op::GoTo(block_id) => self.infer_block(self.bytecode.get_block(block_id)),
            Op::GoToIf(_block_id) => todo!(),
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
            Op::ObjectId(obj_id) => stack_mutation!(=>ObjectId(obj_id)),
            Op::Malloc(obj_id) => {
                let mut sm = stack_mutation!(=>ObjectId(obj_id));
                let props = self.bytecode.object_ids.get(&obj_id).unwrap();
                for prop_id in props.iter() {
                    // TODO get type of props
                    sm.before.push(Type::Builtin(BuiltinType::Int));
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

/*
 *
 * def plus +
 * def main [
 *   1 2 plus
 *   4 *
 * ]
 * Block 0: Int Int -> Int
 * Def 0: Int Int -> Int
 * Block 1:
 *   -> Int Int
 *   ~~~Call def 0~~~???
 *   Int -> Int
 * Def 1:
 *   -> Int
 *
 * def main [
 *   1 2 == if [
 *     3
 *   ] else [
 *     4
 *   ]
 *   5 +
 * ]
 * Block 0: GoToIf Block1; 4; GoTo Block 2
 * Block 1: 3; GoTo Block 2
 * Block 2: 5; +; Return
 *
 * Block 0:
 * Block 1:
 * Block 2: Int -> Int
 *
 *
 * fibo: D0 (spoilers: Int -> Int)
 * Block 0:
 *   Int -> Int Int
 *   ?? D0 ??
 *   Int -> Int
 *   ?? D0 ??
 *   Int Int -> Int
 * Block 1:
 *   Int -> Int
 * Block 2:
 *   ->
 *
 * Initial pass for D0: [Int -> Int, Unknowable]
 * Depends on D0
 * Fold them together to get Int -> Int
 * Derived constraints:
 *   Int Int -> D0
 *   D0 -> Int
 *   Int -> D0
 *   D0 -> Int Int
 * All constraints satisfied!
 */
