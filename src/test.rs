use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::{
    bytecode::ByteCode,
    compile::{compile_source, compile_stdlib},
    type_check::Types,
    vm::{TestIO, VM},
};

include!(concat!(env!("OUT_DIR"), "/test_cases.rs"));
