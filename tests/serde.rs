#[test]
#[cfg(feature = "serde")]
fn serde_support() {
    static SOUECE: &str = "
use Linear
use ReLU

[Sample Model]
    #0 Input = 42
    #1 Linear + ReLU = 22
";

    let mut root = n3_core::GraphRoot::default();
    root.compile_from_source(SOUECE, n3_core::UseOrigin::Local)
        .unwrap();

    let root_bin = bincode::serialize(&root).unwrap();
    let mut root: n3_core::GraphRoot = bincode::deserialize(&root_bin[..]).unwrap();

    let graph = root
        .find_graph("Sample Model", n3_core::UseOrigin::Local)
        .unwrap();

    let shapes = graph.get_shapes();
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].len(), 1);
    assert_eq!(shapes[0][0], 22u64);
}
