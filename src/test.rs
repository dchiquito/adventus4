use crate::{
    compile::{compile_stdlib, compile_test, compile_test_stdlib},
    type_check::Types,
    vm::{TestIO, VM},
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

pub static TEST_CASES: [(usize, TestCase); 5] = [
    (0, no_input!("def main []", "")),
    (1, no_input!("def main [0 print]", "0")),
    (2, no_input!("def main [1 print]", "1")),
    (3, no_input!("def main ['a' print]", "a")),
    (4, no_input!("def main [\"abc\" print]", "abc")),
];

#[test]
fn check_test_cases() {
    for (actual_index, (index, _)) in TEST_CASES.iter().enumerate() {
        assert_eq!(actual_index, *index);
    }
}

#[test]
fn run_test_cases() {
    for (index, test_case) in &TEST_CASES {
        let bytecode = compile_test_stdlib(test_case.source);
        let mut types = Types::new(&bytecode);
        types.infer();
        let mut vm = VM::new(&bytecode, TestIO::new(test_case.input));
        vm.push_main();
        if let Err(e) = vm.run() {
            e.format(&mut std::io::stderr(), &bytecode).unwrap();
            panic!("unexpected error in test case #{index}");
        }
        let actual_output = vm.get_io().get_output();
        assert_eq!(
            test_case.output, actual_output,
            "test case #{index}: {}",
            test_case.source
        );
    }
}
