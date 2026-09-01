use adventus4::{compile::Compiler, type_check::Types, vm::VM};

fn main() {
    if let Some(in_file) = std::env::args().nth(1) {
        let source_code = std::fs::read_to_string(in_file).unwrap();
        let bytecode = Compiler::new(&source_code).compile();
        eprintln!("{bytecode:?}");
        bytecode.pretty_print();
        let mut types = Types::new(&bytecode);
        types.infer();
        eprintln!("{types:?}");
        let mut vm = VM::new_main(&bytecode);
        if let Err(e) = vm.run() {
            println!("eeee {e:?} {}", e.get_source_string(&source_code));
        }
    }
    // let root_node = tree.root_node();
    // eprintln!("{root_node:?}")
}
