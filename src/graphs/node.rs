use super::graph::Graph;
use super::id::GraphIdArg;
use super::shape::Shapes;

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub graph: Option<Graph>,
    pub shapes: Shapes,
    pub inputs: Vec<GraphIdArg>,
}

impl Node {
    pub const INTRINSIC_DYNAMIC: &'static str = "dynamic";
    pub const INTRINSIC_FIXED: &'static str = "fixed";
    pub const INTRINSIC_IDENTITY: &'static str = "identity";

    pub const INTRINSIC_INPUT: &'static str = "Input";

    const INTRINSIC_GENERIC: &'static str = "";
}

impl Default for Node {
    fn default() -> Self {
        Self {
            name: Self::INTRINSIC_GENERIC.to_string(),
            graph: None,
            shapes: Shapes::Dynamic,
            inputs: vec![],
        }
    }
}
