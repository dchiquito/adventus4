use adventus4::{
    bytecode::ByteCode,
    compile::{compile_source, compile_stdlib},
    type_check::Types,
    vm::{StdIO, VM},
};

fn main() {
    env_logger::init();

    if let Some(in_file) = std::env::args().nth(1) {
        let mut bytecode = ByteCode::default();
        compile_stdlib(&mut bytecode);
        compile_source(&mut bytecode, &in_file);
        {
            let mut buf = String::new();
            bytecode.pretty_print(&mut buf).unwrap();
            log::debug!("{buf}");
        }
        let mut types = Types::new(&bytecode);
        types.infer().expect("type error");

        let mut vm = VM::new(&bytecode, StdIO::default());
        vm.push_main();
        if let Err(e) = vm.run() {
            e.format(&mut std::io::stderr(), &bytecode).unwrap()
        }
    }
}
