use adventus4::{compile::ByteCode, vm::VM};

fn main() {
    let source_code = "def mean:(int int -> int)  [+ 2 /]
        def new ?(-> int int) [1 2]
        def swip [
            >$a
            >$b
            $a $b
        ]
        def main [10 20 swip / print]";
    let bytecode = ByteCode::compile(source_code);
    eprintln!("{bytecode:?}");
    let mut vm = VM::new(bytecode);
    vm.run();
    // let root_node = tree.root_node();
    // eprintln!("{root_node:?}")
}
