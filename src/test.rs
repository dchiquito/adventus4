use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::source::write_test_file;
use crate::{
    compile::compile_test_stdlib,
    type_check::Types,
    vm::{TestIO, VM},
};

fn run_internal_test(source: &str, input: &str) -> String {
    let bytecode = compile_test_stdlib(source);
    let mut types = Types::new(&bytecode);
    types.infer().unwrap();
    let mut vm = VM::new(&bytecode, TestIO::new(input));
    vm.push_main();
    if let Err(e) = vm.run() {
        let mut buf = vec![];
        e.format(&mut buf, &bytecode).unwrap();
        panic!("unexpected error:\n{}", str::from_utf8(&buf).unwrap());
    }
    let actual_output = vm.get_io().get_output();
    actual_output.to_string()
}

fn run_cli_test(source: &str, input: &str) -> (String, String) {
    let tmp_source = write_test_file(source).expect("error writing test file");
    let mut child = Command::new("cargo")
        .args(["run", "--"])
        .arg(&tmp_source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("error starting cargo");
    {
        let mut stdin = child.stdin.take().expect("error grabbing input");
        stdin
            .write_all(input.as_bytes())
            .expect("error writing input");
        // Drop stdin here
    }
    let status = child.wait().expect("error waiting for child");
    assert!(status.success(), "test case failed:\n{}", source);
    let out = {
        let mut out = String::new();
        let mut stdout = child.stdout.take().expect("error grabbing stdout");
        stdout
            .read_to_string(&mut out)
            .expect("error reading stdout");
        out
    };
    let err = {
        let mut err = String::new();
        let mut stderr = child.stderr.take().expect("error grabbing stdout");
        stderr
            .read_to_string(&mut err)
            .expect("error reading stderr");
        err
    };
    (out, err)
}

macro_rules! tests {
    () => {};
    (($name:ident : $source:expr => $out:expr), $($rest:tt)*) => {
        tests!(($name: "" => $source => $out), $($rest)*);
    };
    (($name:ident: $in:expr => $source:expr => $out:expr), $($rest:tt)*) => {
        mod $name {
            use super::*;

            #[test]
            fn test_internal() {
                let actual_output = run_internal_test($source, $in);
                assert_eq!($out, actual_output, "{}", $source);
            }

            #[test]
            fn test_cli() {
                let (actual_output, err) = run_cli_test($source, $in);
                if $out != actual_output {
                    panic!(
                        "{}\nexpected: {}\n  actual: {}\n  stderr:\n{}",
                        $source,
                        $out,
                        actual_output,
                        err
                    );
                }
            }
        }
        tests!($($rest)*);
    };
}

tests![
    (empty: "def main []" => ""),
    (print_zero: "def main [0 print]" => "0\n"),
    (print_one: "def main [1 print]" => "1\n"),
    (print_char: "def main ['a' print]" => "a\n"),
    (print_string_as_list: "def main [\"abc\" print]" => "[a, b, c]\n"),
    (print_string: "def main [\"abc\" print_str]" => "abc\n"),
    (print_twice: "def main [0 print 1 print]" => "0\n1\n"),
    (read_line: "Hello" => "def main [read_line print_str]" => "Hello\n"),
    (macro_var: "def foo @bar [@bar] def main [foo 1 print]" => "1\n"),
    (two_macro_vars: "def foo @bar @baz [@bar] def main [foo 1 2 print]" => "1\n"),
    (macro_var_ordering:
        "def foo @bar @baz [@bar @baz] def main [foo 1 print]" =>
        "1\n"
    ),
    (swap: "def main [1 2 swap print print]" => "1\n2\n"),
    (drop: "def main [1 2 drop print]" => "1\n"),
    (array_get: "def main [[10 empty_array] 0 array_get print]" => "0\n"),
    (array_out_of_bounds: "def main [[10 empty_array] -1 array_get print]" => "oh no"),
];
