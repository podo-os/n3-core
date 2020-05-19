#[test]
fn inline_model() {
    static SOUECE: &str = "
use Linear
use ReLU

[Recursive Model]
    #0 Input = 42
    #1 Linear = 12
    #2 [Inner Model]
        #1 Linear = 22
    #3 ReLU = 22
";

    let mut root = n3_core::GraphRoot::default();

    let graph = root
        .compile_from_source(SOUECE, n3_core::UseOrigin::Local)
        .unwrap();

    let shapes = graph.get_shapes();
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].len(), 1);
    assert_eq!(shapes[0][0], 22u64);
}
