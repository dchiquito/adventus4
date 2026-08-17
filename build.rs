use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use regex::Regex;

fn read_parser_c() -> String {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let parser_path = Path::new(&manifest_dir).join("tree-sitter-adventus/src/parser.c");
    fs::read_to_string(parser_path).unwrap()
}

fn get_ts_symbol_identifiers(parser: &str) -> Vec<[&str; 2]> {
    let re = Regex::new(" sym_(.*) = ([0-9]+)").unwrap();
    re.captures_iter(parser).map(|c| c.extract().1).collect()
}

fn write_id(buf: &mut String, name: &str, id: &str) {
    writeln!(buf, "const {name}: u16 = {id};").unwrap();
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

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("grammar_ids.rs");
    fs::write(&dest_path, generate_ids()).unwrap();
    println!("cargo::rerun-if-changed=tree-sitter-adventus/src/parser.c");
}
