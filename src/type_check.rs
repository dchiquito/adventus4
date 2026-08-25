use crate::bytecode::{Block, ByteCode, OpCode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Bool,
    Int,
    Char,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackMutation {
    before: Vec<Type>,
    after: Vec<Type>,
}
macro_rules! stack_mutation {
    ($($before:ident) * => $($after:ident) *) => {
        StackMutation {
            before: vec![$(Type::$before),*],
            after: vec![$(Type::$after),*],
        }
    };
}
impl StackMutation {
    fn chain(&self, rhs: &StackMutation) -> StackMutation {
        for (lhs, rhs) in self.after.iter().rev().zip(rhs.before.iter().rev()) {
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

#[derive(Debug, Default)]
pub struct Types {
    block_types: Vec<StackMutation>,
}
impl Types {
    pub fn infer(bytecode: &ByteCode) -> Self {
        let mut types = Self::default();
        for block in bytecode.blocks.iter() {
            let block_type = types.infer_block(block);
            types.block_types.push(block_type);
        }
        types
    }
    fn infer_block(&self, block: &Block) -> StackMutation {
        let mut def_type = stack_mutation!(=>);
        for op in block.iter() {
            let op = OpCode::from(op);
            let sm = match op {
                OpCode::Literal => stack_mutation!(=>Int),
                OpCode::Call => todo!(),
                OpCode::Return => stack_mutation!(=>),
                OpCode::GoTo => todo!(),
                OpCode::GoToIf => todo!(),
                OpCode::Dup => stack_mutation!(Int=>Int Int),
                OpCode::Swap => stack_mutation!(Int Int=>Int Int),
                OpCode::Pop => stack_mutation!(Int=>),
                OpCode::BindLocal => stack_mutation!(Int=>),
                OpCode::PushLocal => stack_mutation!(=>Int),
                OpCode::BindProp => stack_mutation!(Int=>),
                OpCode::PushProp => stack_mutation!(=>Int),
                OpCode::Add => stack_mutation!(Int Int=>Int),
                OpCode::Sub => stack_mutation!(Int Int=>Int),
                OpCode::Mul => stack_mutation!(Int Int=>Int),
                OpCode::Div => stack_mutation!(Int Int=>Int),
                OpCode::Eq => stack_mutation!(Int Int=>Bool),
                OpCode::Ne => stack_mutation!(Int Int=>Bool),
                OpCode::Gt => stack_mutation!(Int Int=>Bool),
                OpCode::Lt => stack_mutation!(Int Int=>Bool),
                OpCode::Gte => stack_mutation!(Int Int=>Bool),
                OpCode::Lte => stack_mutation!(Int Int=>Bool),
                OpCode::And => stack_mutation!(Bool Bool=>Bool),
                OpCode::Or => stack_mutation!(Bool Bool=>Bool),
                OpCode::Not => stack_mutation!(Bool=>Bool),
                OpCode::Print => stack_mutation!(Int=>Int),
                OpCode::ObjectId => todo!(),
                OpCode::Malloc => todo!(),
            };
            def_type = def_type.chain(&sm);
        }
        def_type
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
