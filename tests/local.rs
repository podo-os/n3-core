use n3_parser::ast;

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn test_compile_local() {
    let mut path = std::env::current_dir().unwrap();
    path.push("models");

    let mut root = n3_core::GraphRoot::new(path).unwrap();

    dbg!(root.find_graph("LeNet Trainer", ast::UseOrigin::Local).unwrap());
}
