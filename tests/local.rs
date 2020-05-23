#[test]
#[cfg(not(target_arch = "wasm32"))]
fn local_model() {
    let mut path = std::env::current_dir().unwrap();
    path.push("models");

    let mut root = n3_core::GraphRoot::with_path(path).unwrap();

    let graph = root
        .find_graph("LeNet Trainer", n3_core::UseOrigin::Local)
        .unwrap();

    let shapes = graph.get_shapes();
    assert_eq!(shapes.len(), 3);

    let last_shapes = shapes.values().rev().next().unwrap();
    assert_eq!(last_shapes.len(), 1);
    assert_eq!(last_shapes[0].len(), 1);
    assert_eq!(last_shapes[0][0], n3_core::Dim::Expr(2u64.into()));
}
