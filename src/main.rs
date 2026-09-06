use adventus4::{
    bytecode::ByteCode,
    compile::{compile_source, compile_stdlib},
    type_check::Types,
    vm::VM,
};

fn main() {
    if let Some(in_file) = std::env::args().nth(1) {
        let mut bytecode = ByteCode::default();
        compile_stdlib(&mut bytecode);
        compile_source(&mut bytecode, &in_file);
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
