use adventus4::{
    bytecode::ByteCode,
    compile::Compiler,
    type_check::Types,
    vm::{VM, read_source_file},
};

fn main() {
    if let Some(in_file) = std::env::args().nth(1) {
        let source = read_source_file(&in_file);
        let mut bytecode = ByteCode::default();
        Compiler::new(&mut bytecode, &in_file, &source).compile();
        eprintln!("{bytecode:?}");
        bytecode.pretty_print();
        let mut types = Types::new(&bytecode);
        types.infer();
        eprintln!("{types:?}");
        let mut vm = VM::new_main(&bytecode);
        if let Err(e) = vm.run() {
            e.format(&mut std::io::stderr(), &bytecode).unwrap()
        }
    }
    // let root_node = tree.root_node();
    // eprintln!("{root_node:?}")
}
