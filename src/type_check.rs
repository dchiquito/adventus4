use crate::bytecode::{ByteCode, Definition, OpCode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
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
    def_types: Vec<StackMutation>,
}
impl Types {
    pub fn infer(bytecode: &ByteCode) -> Self {
        let mut types = Self::default();
        for def in bytecode.defs.iter() {
            let def_type = types.infer_def(def);
            types.def_types.push(def_type);
        }
        types
    }
    fn infer_def(&self, def: &Definition) -> StackMutation {
        let mut def_type = stack_mutation!(=>);
        for op in def.iter() {
            let op = OpCode::from(op);
            let sm = match op {
                OpCode::Literal => stack_mutation!(=>Int),
                OpCode::Call => todo!(),
                OpCode::Return => stack_mutation!(=>),
                OpCode::Dup => stack_mutation!(Int=>Int Int),
                OpCode::Swap => stack_mutation!(Int Int=>Int Int),
                OpCode::BindLocal => stack_mutation!(Int=>),
                OpCode::PushLocal => stack_mutation!(=>Int),
                OpCode::Add => stack_mutation!(Int Int=>Int),
                OpCode::Sub => stack_mutation!(Int Int=>Int),
                OpCode::Mul => stack_mutation!(Int Int=>Int),
                OpCode::Div => stack_mutation!(Int Int=>Int),
                OpCode::Print => stack_mutation!(Int=>Int),
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
