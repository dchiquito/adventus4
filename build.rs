use std::path::{Path, PathBuf};

fn project_path(path: &str) -> PathBuf {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir).join(path)
}

mod treesitter {
    use super::*;

    use std::fmt::Write;

    use regex::Regex;

    fn read_parser_c() -> String {
        let parser_path = project_path("tree-sitter-adventus/src/parser.c");
        std::fs::read_to_string(parser_path).unwrap()
    }

    fn get_ts_symbol_identifiers(parser: &str) -> Vec<[&str; 2]> {
        let re = Regex::new(" sym_(.*) = ([0-9]+)").unwrap();
        re.captures_iter(parser).map(|c| c.extract().1).collect()
    }

    fn write_id(buf: &mut String, name: &str, id: &str) {
        writeln!(buf, "#[allow(unused)] const {name}: u16 = {id};").unwrap();
    }

    fn generate_ids() -> String {
        let parser_c = read_parser_c();
        let mut buf = String::new();
        for [name, id] in get_ts_symbol_identifiers(&parser_c) {
            let name = name.to_uppercase();
            write_id(&mut buf, &name, id);
        }
        buf
    }
    pub fn write_grammar_ids() {
        let out_dir = std::env::var_os("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir).join("grammar_ids.rs");
        std::fs::write(&dest_path, generate_ids()).unwrap();
        println!("cargo::rerun-if-changed=tree-sitter-adventus/src/parser.c");
    }
}

mod test_cases {
    use super::*;

    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fmt::Write;
    use std::fs::DirEntry;

    enum Output {
        Stdout(String),
        VmError(String),
    }
    struct TestCase {
        test_adv: String,
        id: String,
        input: Option<String>,
        output: Output,
    }

    fn build_test_case_map(dir_entry: DirEntry) -> Vec<TestCase> {
        let test_adv = dir_entry.path().join("test.adv");
        let test_adv = test_adv.to_str().unwrap();
        let mut ins = HashMap::new();
        let mut outs = HashMap::new();
        let mut errs = HashMap::new();
        for file in dir_entry.path().read_dir().unwrap() {
            let file = file.unwrap();
            let file_path = file.path();
            let stem = file_path.file_stem().unwrap().to_str().unwrap().to_string();
            let contents = std::fs::read_to_string(&file_path).unwrap();
            if file_path.extension() == Some(&OsString::from("in")) {
                ins.insert(stem, contents);
            } else if file_path.extension() == Some(&OsString::from("out")) {
                outs.insert(stem, contents);
            } else if file_path.extension() == Some(&OsString::from("err")) {
                errs.insert(stem, contents.replace("{{TEST_ADV}}", test_adv));
            }
        }
        outs.into_iter()
            .map(|(id, output)| {
                let input = ins.get(&id).cloned();
                let output = Output::Stdout(output);
                TestCase {
                    test_adv: test_adv.to_string(),
                    id,
                    input,
                    output,
                }
            })
            .chain(errs.into_iter().map(|(id, error)| {
                let input = ins.get(&id).cloned();
                let output = Output::VmError(error);
                TestCase {
                    test_adv: test_adv.to_string(),
                    id,
                    input,
                    output,
                }
            }))
            .collect()
    }

    fn write_internal_test_case(
        buf: &mut String,
        TestCase {
            test_adv,
            id,
            input,
            output,
        }: &TestCase,
    ) {
        let input = if let Some(input) = input { input } else { "" };
        match output {
            Output::Stdout(out) => {
                write!(
                    buf,
                    r#"
    #[test]
    fn test_{id}() {{
        let mut bytecode = ByteCode::default();
        compile_stdlib(&mut bytecode);
        compile_source(&mut bytecode, "{test_adv}");
        let mut types = Types::new(&bytecode);
        types.infer().unwrap();
        let mut vm = VM::new(&bytecode, TestIO::new("{input}"));
        vm.push_main();
        vm.run().map_err(|e| {{
            let mut buf = vec![];
            e.format(&mut buf, &bytecode).unwrap();
            panic!("{{}}", String::from_utf8(buf).unwrap())
        }});
        let actual_output = vm.get_io().get_output();
        assert_eq!(actual_output.to_string(),"{out}")
    }}"#
                )
                .unwrap();
            }
            Output::VmError(err) => {
                write!(
                    buf,
                    r#"
    #[test]
    fn test_{id}_internal() {{
        let mut bytecode = ByteCode::default();
        compile_stdlib(&mut bytecode);
        compile_source(&mut bytecode, "{test_adv}");
        let mut types = Types::new(&bytecode);
        types.infer().unwrap();
        let mut vm = VM::new(&bytecode, TestIO::new("{input}"));
        vm.push_main();
        if let Err(e) = vm.run() {{
            let mut buf = vec![];
            e.format(&mut buf, &bytecode).unwrap();
            assert_eq!(String::from_utf8(buf).unwrap(), "{err}");
        }} else {{
            panic!("test passed, but expected an error");
        }}
    }}"#
                )
                .unwrap();
            }
        };
    }

    fn write_cli_test_case(
        buf: &mut String,
        TestCase {
            test_adv,
            id,
            input,
            output,
        }: &TestCase,
    ) {
        let input = if let Some(input) = input { input } else { "" };
        let (out_or_err, output) = match output {
            Output::Stdout(out) => ("stdout", out),
            Output::VmError(err) => ("stderr", err),
        };
        write!(
            buf,
            r#"
    #[test]
    fn test_{id}_cli() {{
        let mut child = Command::new(&*BINARY_PATH)
            .arg("{test_adv}")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("error running binary");
        {{
            let mut stdin = child.stdin.take().expect("error grabbing input");
            stdin
                .write_all(b"{input}")
                .expect("error writing input");
            // Drop stdin here
        }}
        let status = child.wait().expect("error waiting for child");
        assert!(status.success(), "test case failed: {test_adv}");
        let output = {{
            let mut output = String::new();
            let mut {out_or_err} = child.{out_or_err}.take().expect("error grabbing {out_or_err}");
            {out_or_err}
                .read_to_string(&mut output)
                .expect("error reading {out_or_err}");
            output
        }};
        assert_eq!(output, "{output}");
    }}"#
        )
        .unwrap();
    }

    fn generate_test_case(buf: &mut String, dir_entry: DirEntry) {
        let name = dir_entry.file_name();
        let name = name.to_str().unwrap();
        let test_case_map = build_test_case_map(dir_entry);
        write!(
            buf,
            r#"
mod {name} {{
    use super::*;"#
        )
        .unwrap();

        for test_case in test_case_map {
            write_internal_test_case(buf, &test_case);
            write_cli_test_case(buf, &test_case);
        }
        write!(
            buf,
            r#"
}}"#
        )
        .unwrap()
    }

    fn generate_test_cases() -> String {
        let tests_root = project_path("test_cases");
        let mut buf = String::new();
        for test_case in tests_root.read_dir().unwrap() {
            generate_test_case(&mut buf, test_case.unwrap());
        }
        buf
    }
    pub fn write_test_cases() {
        let out_dir = std::env::var_os("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir).join("test_cases.rs");
        std::fs::write(&dest_path, generate_test_cases()).unwrap();
        println!("cargo::rerun-if-changed=test_cases");
    }
}

fn main() {
    treesitter::write_grammar_ids();
    test_cases::write_test_cases();
}
