use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::{
    compile::compile_test_stdlib,
    type_check::Types,
    vm::{TestIO, VM},
};

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
                let bytecode = compile_test_stdlib($source);
                let mut types = Types::new(&bytecode);
                types.infer().unwrap();
                let mut vm = VM::new(&bytecode, TestIO::new($in));
                vm.push_main();
                if let Err(e) = vm.run() {
                    e.format(&mut std::io::stderr(), &bytecode).unwrap();
                    panic!("unexpected error");
                }
                let actual_output = vm.get_io().get_output();
                assert_eq!($out, actual_output, "{}", $source);
            }

            #[test]
            fn test_cli() {
                let tmp_source = {
                    let mut tmp_source = std::env::temp_dir();
                    tmp_source.push(format!("test_{}.adv", stringify!($name)));
                    tmp_source
                };
                std::fs::write(&tmp_source, $source).expect("error writing file");
                let mut child = Command::new("cargo")
                    .args(["run", "--"])
                    .arg(&tmp_source)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    // TODO log stderr
                    .spawn()
                    .expect("error starting cargo");
                {
                    let mut stdin = child.stdin.take().expect("error grabbing input");
                    stdin
                        .write_all($in.as_bytes())
                        .expect("error writing input");
                    // Drop stdin here
                }
                let status = child.wait().expect("error waiting for child");
                assert!(
                    status.success(),
                    "test case {} failed:\n{}",
                    stringify!($name),
                    $source
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
                    $out, actual_output,
                    "{}",
                    $source
                );
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
    (array_out_of_bounds: "def main [[10 empty_array] -1 array_get print]" => "0\n"),
];
