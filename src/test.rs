use crate::{
    compile::{compile_stdlib, compile_test},
    type_check::Types,
    vm::VM,
};

pub struct TestCase {
    source: &'static str,
    input: &'static str,
    output: &'static str,
}

macro_rules! no_input {
    ($source:expr, $output:expr) => {
        TestCase {
            source: $source,
            input: "",
            output: $output,
        }
    };
}

pub static TEST_CASES: [TestCase; 1] = [no_input!("def main []", "")];

#[test]
fn run_test_cases() {
    for test_case in &TEST_CASES {
        let bytecode = compile_test(test_case.source);
        let mut types = Types::new(&bytecode);
        types.infer();
        let mut vm = VM::new_main(&bytecode);
        // vm.push_string(&stdin).expect("failed to load stdin");
        if let Err(e) = vm.run() {
            e.format(&mut std::io::stderr(), &bytecode).unwrap()
        }
        // TODO check output
    }
}
