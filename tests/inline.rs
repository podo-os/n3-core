#[test]
fn inline_model() {
    static SOUECE: &str = "
use Linear
use ReLU

[Recursive Model]
    #0 Input = 42
    #1 Linear = 12
    #2 [Inner Model]
        * weight = 2

        #0 Input = N
        #1 ReLU + Linear = N * weight + 1
    #3 ReLU = 25
";

    let mut root = n3_core::GraphRoot::default();

    let graph = root.compile_from_source(SOUECE).unwrap();

    let shapes = graph.get_shapes();
    assert_eq!(shapes.len(), 4);

    let last_shapes = shapes.values().rev().next().unwrap();
    assert_eq!(last_shapes[0].len(), 1);
    assert_eq!(last_shapes[0][0], n3_core::Dim::Expr(25u64.into()));
    assert_eq!(last_shapes[0][0], n3_core::Dim::Expr(25u64.into()));
}
