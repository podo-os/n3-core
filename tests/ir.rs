use n3_core::*;

#[test]
fn make_ir() {
    static SOUECE: &str = "
use Conv2d
use Linear

use ReLU
use Softmax
use Transform

[Sample Model]

    * N: number of channels = 10

    [Conv2d]
        * kernel size = 5
        * stride = 2

    #0 Input                = Ic, 28, 28
    #1 Conv2d (#0) + ReLU   = 32, 14, 14
    #2 Conv2d      + ReLU   = 64,  7,  7
    #3 Transform            = 64*  7*  7
    #4 Linear + Softmax     =  N
";

    let mut root = GraphRoot::default();

    let graph = root.compile_from_source(SOUECE).unwrap();

    let is_extern = graph.is_extern();
    assert_eq!(is_extern, false);

    // Variables

    let variables = graph.get_variables();
    assert_eq!(
        variables.get("number of channels"),
        Some(&Variable {
            description: "number of channels".to_string(),
            ty: ValueType::UInt,
            value: Some(Value::UInt(10)),
        })
    );
    assert_eq!(variables.get("N"), None);
    assert_eq!(variables.get("Ic"), None);

    // Nodes
    // note: if you want to get the shapes in each node,
    // node: use `graph.get_shapes()` instead.

    let mut nodes = graph.get_nodes().values();

    let input_node = nodes.next().unwrap();
    assert_eq!(&input_node.name, "Input");
    assert_eq!(input_node.graph.is_some(), false);

    let first_node_conv2d = nodes.next().unwrap();
    assert_eq!(&first_node_conv2d.name, "Conv2d");

    let first_graph_conv2d = first_node_conv2d.graph.as_ref().unwrap();
    let first_graph_conv2d_variables = first_graph_conv2d.get_variables();
    assert_eq!(first_graph_conv2d.is_extern(), true);
    assert_eq!(
        first_graph_conv2d_variables.get("kernel size"),
        Some(&Variable {
            description: "kernel size".to_string(),
            ty: ValueType::UInt,
            value: Some(Value::UInt(5)),
        })
    );
    assert_eq!(
        first_graph_conv2d_variables.get("stride"),
        Some(&Variable {
            description: "stride".to_string(),
            ty: ValueType::UInt,
            value: Some(Value::UInt(2)),
        })
    );
    assert_eq!(first_graph_conv2d_variables.get("S"), None);

    let first_node_conv2d_inputs = &first_node_conv2d.inputs;
    assert_eq!(first_node_conv2d_inputs.len(), 1);
    assert_eq!(
        *first_node_conv2d_inputs.iter().next().unwrap(),
        GraphIdArg {
            id: GraphId {
                node: 0,
                pass: 0,
                repeat: 0,
            },
            arg: Some(0),
        }
    );

    let first_node_relu = nodes.next().unwrap();
    assert_eq!(&first_node_relu.name, "ReLU");

    let first_graph_relu = first_node_relu.graph.as_ref().unwrap();
    assert_eq!(first_graph_relu.is_extern(), true);
    assert_eq!(first_graph_relu.get_variables().is_empty(), true);

    let first_node_relu_inputs = &first_node_relu.inputs;
    assert_eq!(first_node_relu_inputs.len(), 1);
    assert_eq!(
        *first_node_relu_inputs.iter().next().unwrap(),
        GraphIdArg {
            id: GraphId {
                node: 1,
                pass: 0,
                repeat: 0,
            },
            arg: None,
        }
    );

    // Shapes

    let shapes = graph.get_shapes();
    assert_eq!(shapes.len(), 8);

    let first_shapes = shapes.values().next().unwrap();
    assert_eq!(first_shapes.len(), 1);
    assert_eq!(first_shapes[0].len(), 3);
    assert_eq!(first_shapes[0][1], 28u64);
    assert_eq!(first_shapes[0][2], 28u64);
    assert_eq!(DimKey::try_from_expr(&first_shapes[0][1]), None);
    assert_eq!(DimKey::try_from_expr(&first_shapes[0][2]), None);
    assert_eq!(
        DimKey::try_from_expr(&first_shapes[0][0]),
        Some(DimKey::Placeholder("Ic".to_string(), true))
    );

    let last_shapes = shapes.values().rev().next().unwrap();
    assert_eq!(last_shapes.len(), 1);
    assert_eq!(last_shapes[0].len(), 1);
    assert_eq!(last_shapes[0][0], 10u64);
    assert_eq!(DimKey::try_from_expr(&last_shapes[0][0]), None);
}
