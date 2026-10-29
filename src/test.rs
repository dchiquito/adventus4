use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::{
    compile::compile_test_stdlib,
    type_check::Types,
    vm::{TestIO, VM},
};

pub struct TestCase {
    source: &'static str,
    input: &'static str,
    output: &'static str,
}

macro_rules! with_input {
    ($source:expr, $input:expr, $output:expr) => {
        TestCase {
            source: $source,
            input: $input,
            output: $output,
        }
    };
}

macro_rules! no_input {
    ($source:expr, $output:expr) => {
        with_input!($source, "", $output)
    };
}

pub static TEST_CASES: [TestCase; 15] = [
    no_input!("def main []", ""),
    no_input!("def main [0 print]", "0\n"),
    no_input!("def main [1 print]", "1\n"),
    no_input!("def main ['a' print]", "a\n"),
    no_input!("def main [\"abc\" print]", "[a, b, c]\n"),
    no_input!("def main [\"abc\" print_str]", "abc\n"),
    no_input!("def main [0 print 1 print]", "0\n1\n"),
    with_input!("def main [read_line print_str]", "Hello", "Hello\n"),
    no_input!("def foo @bar [@bar] def main [foo 1 print]", "1\n"),
    no_input!("def foo @bar @baz [@bar] def main [foo 1 2 print]", "1\n"),
    no_input!(
        "def foo @bar @baz [@bar @baz] def main [foo 1 print]",
        "1\n"
    ),
    no_input!("def main [1 2 swap print print]", "1\n2\n"),
    no_input!("def main [1 2 drop print]", "1\n"),
    no_input!("def main [[10 empty_array] 0 array_get print]", "0\n"),
    no_input!("def main [[10 empty_array] -1 array_get print]", "0\n"),
];

#[test]
fn run_test_cases() {
    for (index, test_case) in TEST_CASES.iter().enumerate() {
        let bytecode = compile_test_stdlib(test_case.source);
        let mut types = Types::new(&bytecode);
        types.infer().unwrap();
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
#[test]
fn run_test_cases_cli() {
    for (index, test_case) in TEST_CASES.iter().enumerate() {
        let tmp_source = {
            let mut tmp_source = std::env::temp_dir();
            tmp_source.push(format!("test_{index}.adv"));
            tmp_source
        };
        std::fs::write(&tmp_source, test_case.source).expect("error writing file");
        let mut child = Command::new("cargo")
            .args(["run", "--"])
            .arg(&tmp_source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("error starting cargo");
        {
            let mut stdin = child.stdin.take().expect("error grabbing input");
            stdin
                .write_all(test_case.input.as_bytes())
                .expect("error writing input");
            // Drop stdin here
        }
        let status = child.wait().expect("error waiting for child");
        assert!(
            status.success(),
            "test case #{index} failed:\n{}",
            test_case.source
        );
        let actual_output = {
            let mut actual_output = String::new();
            let mut stdout = child.stdout.take().expect("error grabbing stdout");
            stdout
                .read_to_string(&mut actual_output)
                .expect("error reading output");
            actual_output
        };
        assert_eq!(
            test_case.output, actual_output,
            "test case #{index}: {}",
            test_case.source
        );
    }
}
