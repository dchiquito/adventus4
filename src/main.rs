use adventus4::{compile::ByteCode, type_check::Types, vm::VM};

fn main() {
    if let Some(in_file) = std::env::args().nth(1) {
        let source_code = std::fs::read_to_string(in_file).unwrap();
        let bytecode = ByteCode::compile(&source_code);
        eprintln!("{bytecode:?}");
        let types = Types::infer(&bytecode);
        eprintln!("{types:?}");
        let mut vm = VM::new(bytecode);
        vm.run();
    }
    // let root_node = tree.root_node();
    // eprintln!("{root_node:?}")
}
