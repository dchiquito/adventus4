use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use crate::{
    bytecode::ByteCode,
    compile::{compile_source, compile_stdlib},
    type_check::Types,
    vm::{TestIO, VM},
};

static BINARY_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    Command::new("cargo")
        .args(["build"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("error building project");
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir).join("target/debug/adventus4")
});

include!(concat!(env!("OUT_DIR"), "/test_cases.rs"));
