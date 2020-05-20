use super::graph::Graph;
use super::id::GraphIdArg;
use super::shape::Shapes;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub graph: Option<Graph>,
    pub inputs: Vec<GraphIdArg>,
    pub(crate) shapes: Shapes,
}

impl Node {
    pub const INTRINSIC_DYNAMIC: &'static str = "dynamic";
    pub const INTRINSIC_FIXED: &'static str = "fixed";
    pub const INTRINSIC_IDENTITY: &'static str = "identity";

    const INTRINSIC_GENERIC: &'static str = "";
}

impl Default for Node {
    fn default() -> Self {
        Self {
            name: Self::INTRINSIC_GENERIC.to_string(),
            graph: None,
            inputs: vec![],
            shapes: Shapes::Dynamic,
        }
    }
}
