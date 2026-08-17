use adventus4::{compile::ByteCode, vm::VM};

fn main() {
    let source_code = "def mean [+ 2 /]
        def main [10 20 mean print]";
    let bytecode = ByteCode::compile(source_code);
    eprintln!("{bytecode:?}");
    let mut vm = VM::new(bytecode);
    vm.run();
    // let root_node = tree.root_node();
    // eprintln!("{root_node:?}")
}
