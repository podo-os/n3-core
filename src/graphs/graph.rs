use std::collections::{BTreeMap, HashMap};

use super::id::{GraphId, GraphIdArg};
use super::node::Node;
use super::shape::{Dim, DimKey, FitState, Shape, ShapeState, Shapes};
use super::variable::{Value, ValueType, Variable};
use crate::error::{CompileError, GraphError, NonExternModelError};

use n3_parser::ast;
use symengine::ExpressionMap;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Graph {
    variables: HashMap<String, Variable>,
    variable_aliases: HashMap<String, String>,
    keys: ExpressionMap<DimKey>,

    graphs: HashMap<String, Graph>,

    nodes: BTreeMap<GraphId, Node>,
    shape_state: ShapeState,

    is_extern: bool,
}

impl Graph {
    pub(crate) fn new(is_extern: bool) -> Self {
        Self {
            variables: HashMap::new(),
            variable_aliases: HashMap::new(),
            keys: ExpressionMap::new(),
            graphs: HashMap::new(),
            nodes: BTreeMap::new(),
            shape_state: ShapeState::default(),
            is_extern,
        }
    }

    pub(crate) fn new_child(&mut self) -> Self {
        Self {
            variables: HashMap::new(),
            variable_aliases: HashMap::new(),
            keys: ExpressionMap::new(),
            graphs: self.graphs.clone(),
            nodes: BTreeMap::new(),
            shape_state: ShapeState::default(),
            is_extern: false,
        }
    }
}

impl Graph {
    pub fn is_extern(&self) -> bool {
        self.is_extern
    }

    pub fn get_variables(&self) -> &HashMap<String, Variable> {
        &self.variables
    }

    pub fn get_nodes(&self) -> &BTreeMap<GraphId, Node> {
        &self.nodes
    }

    pub fn get_shapes(&self) -> BTreeMap<GraphId, Vec<Vec<Dim>>> {
        self.nodes
            .iter()
            .map(|(id, node)| {
                let shapes = match &node.shapes {
                    Shapes::Dynamic => unreachable!(),
                    Shapes::Fixed(shapes) => shapes
                        .values()
                        .map(|s| match s {
                            Shape::Dynamic => unreachable!(),
                            Shape::Fixed(dims) => {
                                dims.iter().map(|d| self.eval_dim_for_output(d)).collect()
                            }
                        })
                        .collect(),
                };
                (*id, shapes)
            })
            .collect()
    }
}

impl Graph {
    pub(crate) fn add_variable(
        &mut self,
        alias: Option<String>,
        variable: Variable,
    ) -> Result<(), GraphError> {
        let name = &variable.description;
        if self.variables.contains_key(name) {
            if let Some(value) = variable.unwrap_uint() {
                self.keys.insert(DimKey::Variable(name.clone()), value);
            }
            if let Some(value) = variable.value {
                self.update_variable(Some(variable.description), alias, value, variable.ty)?;
            }
        } else {
            if let Some(alias) = alias {
                self.variable_aliases.insert(alias, name.clone());
            }
            if let Some(value) = variable.unwrap_uint() {
                self.keys.insert(DimKey::Variable(name.clone()), value);
            }
            self.variables.insert(name.clone(), variable);
        }
        Ok(())
    }

    pub(crate) fn update_variable(
        &mut self,
        name: Option<String>,
        alias: Option<String>,
        value: ast::Value,
        ty: ValueType,
    ) -> Result<(), GraphError> {
        if let Some(name) = name {
            match self.variables.get_mut(&name) {
                Some(var) => {
                    var.update(value, ty)?;
                    if let Some(alias) = alias {
                        self.variable_aliases.insert(alias, name.clone());
                    }
                    if let Some(value) = var.unwrap_uint() {
                        self.keys.insert(DimKey::Variable(name), value);
                    }
                    Ok(())
                }
                None => Err(GraphError::NoSuchVariable { name }),
            }
        } else if let Some(alias) = alias {
            if let Some(name) = self.variable_aliases.get(&alias) {
                let name = name.clone();
                self.update_variable(Some(name), None, value, ty)
            } else {
                self.update_variable(Some(alias), None, value, ty)
            }
        } else {
            unreachable!("either name or alias is needed")
        }
    }

    pub(crate) fn add_graph(&mut self, name: String, graph: Self) {
        self.graphs.insert(name, graph);
    }

    pub(crate) fn find_graph(&mut self, name: &str) -> Option<Self> {
        self.graphs.get(name).cloned()
    }

    pub(crate) fn attach(
        &mut self,
        id: GraphId,
        name: String,
        graph: Option<Self>,
        args: Vec<ast::GraphPassArg>,
    ) -> Result<(), CompileError> {
        let last_id = if self.nodes.is_empty() {
            if id.is_input() {
                None
            } else {
                return Err(CompileError::GraphError {
                    error: GraphError::FirstNodeNotFound,
                    model: name,
                });
            }
        } else {
            self.nodes.last_key_value().map(|kv| *kv.0)
        };

        if let Some(last_id) = last_id {
            if !id.validate(&last_id) {
                return Err(CompileError::GraphError {
                    error: GraphError::UnvalidNodeId { last: last_id, id },
                    model: name,
                });
            }
        }

        let mut node = match &*name {
            // intrinsics
            Node::INTRINSIC_DYNAMIC => match get_flag(&args) {
                Ok(true) => {
                    self.shape_state = ShapeState::Transform;
                    Node::default()
                }
                Ok(false) => {
                    if self.nodes.is_empty() {
                        self.shape_state = ShapeState::Required(FitState::Full);
                        Node::default()
                    } else {
                        return Err(CompileError::GraphError {
                            error: GraphError::FullShapeRequired { id },
                            model: name,
                        });
                    }
                }
                Err(error) => return Err(CompileError::GraphError { error, model: name }),
            },
            Node::INTRINSIC_FIXED => {
                self.shape_state = match &self.shape_state {
                    ShapeState::Fixed(_) | ShapeState::Required(_) => {
                        ShapeState::Required(FitState::Weak)
                    }
                    ShapeState::Transform => {
                        return Err(CompileError::GraphError {
                            error: GraphError::ShapeNotDefined { id },
                            model: name,
                        })
                    }
                };
                Node {
                    name,
                    graph: None,
                    shapes: Shapes::Dynamic,
                    ..Default::default()
                }
            }
            Node::INTRINSIC_IDENTITY => {
                if last_id.is_some() {
                    // assume that the input has full fixed shapes
                    self.shape_state = ShapeState::Fixed(FitState::Full);

                    Node {
                        name,
                        graph: None,
                        shapes: self.get_last_shapes(None),
                        ..Default::default()
                    }
                } else {
                    return Err(CompileError::GraphError {
                        error: GraphError::InputNodeNotFound,
                        model: name,
                    });
                }
            }

            // user-defined or extern graphs
            _ => {
                if id.is_input() {
                    self.shape_state = ShapeState::Required(FitState::Weak);
                    Node {
                        name,
                        graph: None,
                        shapes: Shapes::Dynamic,
                        ..Default::default()
                    }
                } else if let Some(graph) = graph {
                    self.attach_model(id, name, graph, args)?
                } else if let Some(graph) = self.graphs.get(&name).cloned() {
                    self.attach_model(id, name, graph, args)?
                } else {
                    return Err(CompileError::NonExternModelError {
                        error: NonExternModelError::ModelNotFound,
                        model: name,
                    });
                }
            }
        };
        if node.inputs.is_empty() {
            let inputs = last_id
                .map(|id| vec![GraphIdArg::with_id(id)])
                .unwrap_or_default();
            node.inputs = inputs.into_iter().collect();
        }

        self.nodes.insert(id, node);
        Ok(())
    }

    pub(crate) fn adjust_shapes(&mut self, shapes: ast::Shapes) -> Result<(), CompileError> {
        let mut is_new_var_created = false;
        let shapes = shapes
            .0
            .into_iter()
            .enumerate()
            .map(|(arg_ground, (arg, shape))| {
                let arg_ground = arg_ground as u64;
                if arg_ground != arg {
                    return Err(CompileError::GraphError {
                        error: GraphError::UnvalidNodeArg {
                            id: *self.get_last_node_id(),
                            arg: arg_ground,
                            given: arg,
                        },
                        model: self.get_last_node_name().to_string(),
                    });
                }

                let shape = shape
                    .0
                    .into_iter()
                    .map(|d| self.convert_dim(d, arg, &mut is_new_var_created))
                    .collect::<Result<_, _>>();
                match shape {
                    Ok(shape) => Ok((arg, Shape::Fixed(shape))),
                    Err(error) => Err(CompileError::GraphError {
                        error,
                        model: self.get_last_node_name().to_string(),
                    }),
                }
            })
            .collect::<Result<_, CompileError>>()?;
        let mut shapes = Shapes::Fixed(shapes);
        let shapes_to = shapes.clone();

        let (&id, last_node) = self.nodes.last_key_value().unwrap();
        let model = last_node.name.clone();
        let mut last_shapes = last_node.shapes.clone();

        if self.shape_state == ShapeState::Transform {
            shapes = shapes.product();
            last_shapes = last_shapes.product();
        }

        match shapes.validate_args_rank(&last_shapes, &id) {
            Ok(true) => {
                for ((&arg, last_shape), shape) in last_shapes
                    .unwrap_shapes()
                    .iter()
                    .zip(shapes.unwrap_shapes().values())
                {
                    let last_dims = last_shape.unwrap_dims();
                    let dims = shape.unwrap_dims();
                    for (axis, (last_dim, dim)) in last_dims.iter().zip(dims).enumerate() {
                        if let Err(error) = self.update_dim(id, arg, last_dim, dim, axis) {
                            return Err(CompileError::GraphError { error, model });
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(error) => return Err(CompileError::GraphError { error, model }),
        }

        self.set_last_shapes(shapes_to);
        self.shape_state = ShapeState::Fixed(if is_new_var_created {
            FitState::Weak
        } else {
            FitState::Full
        });
        Ok(())
    }
}

impl Graph {
    pub(crate) fn finalize(&mut self) -> Result<(), CompileError> {
        self.graphs.clear();
        match self.shape_state {
            ShapeState::Fixed(FitState::Full) => Ok(()),
            _ => Err(CompileError::GraphError {
                error: GraphError::FullShapeRequired {
                    id: *self.get_last_node_id(),
                },
                model: self.get_last_node_name().to_string(),
            }),
        }
    }
}

impl Graph {
    fn attach_model(
        &mut self,
        id: GraphId,
        model_name: String,
        mut graph: Self,
        args: Vec<ast::GraphPassArg>,
    ) -> Result<Node, CompileError> {
        let mut inputs = vec![];
        for arg in args {
            match arg {
                ast::GraphPassArg::NodeArg(args) => {
                    if id.repeat == 0 {
                        for arg in args {
                            let id_arg = match self.get_last_specific_node_id(arg.node, &id) {
                                Ok(arg_id) => GraphIdArg {
                                    id: *arg_id,
                                    arg: Some(arg.arg),
                                },
                                Err(error) => {
                                    return Err(CompileError::GraphError {
                                        error,
                                        model: model_name,
                                    })
                                }
                            };
                            inputs.push(id_arg);
                        }
                    }
                }
                ast::GraphPassArg::Keyword { name, value } => {
                    let ty = ValueType::new(Some(&value), false);
                    if let Err(error) = graph.update_variable(None, Some(name), value, ty) {
                        return Err(CompileError::GraphError {
                            error,
                            model: model_name,
                        });
                    }
                }
            }
        }

        let shapes = match self.apply_shapes_as_input(&mut graph, &inputs, id) {
            Ok(shapes) => shapes,
            Err(error) => {
                return Err(CompileError::GraphError {
                    error,
                    model: model_name,
                })
            }
        };
        self.shape_state = graph.shape_state.clone();

        Ok(Node {
            name: model_name,
            graph: Some(graph),
            inputs,
            shapes,
        })
    }

    fn apply_shapes_as_input(
        &mut self,
        target: &mut Self,
        inputs: &[GraphIdArg],
        id: GraphId,
    ) -> Result<Shapes, GraphError> {
        let shapes = self.get_last_shapes(Some(inputs));
        let target_shapes = target.get_first_shapes().clone();

        if target_shapes.validate_args_rank(&shapes, &id)? {
            let shapes = shapes
                .unwrap_shapes()
                .iter()
                .zip(target_shapes.unwrap_shapes().values())
                .map(|((&arg, shape), target_shape)| {
                    let dims = shape.unwrap_dims();
                    let target_dims = target_shape.unwrap_dims().to_vec();
                    let shape = dims
                        .iter()
                        .zip(target_dims)
                        .enumerate()
                        .map(|(axis, (dim, target_dim))| {
                            target.update_dim(id, arg, &target_dim, dim, axis)
                        })
                        .collect::<Result<_, _>>()?;

                    Ok((arg, Shape::Fixed(shape)))
                })
                .collect::<Result<_, _>>()?;

            let shapes = match &target.get_last_shapes(None) {
                Shapes::Dynamic => shapes,
                Shapes::Fixed(shapes) => shapes
                    .iter()
                    .map(|(arg, shape)| {
                        let shape = shape
                            .unwrap_dims()
                            .iter()
                            .map(|d| target.eval_dim(d))
                            .collect();
                        Ok((*arg, Shape::Fixed(shape)))
                    })
                    .collect::<Result<_, _>>()?,
            };
            let mut shapes = Shapes::Fixed(shapes);

            // update remained placeholders
            for _ in shapes.try_archive_placeholders(id) {}

            Ok(shapes)
        } else if let Shapes::Dynamic = target_shapes {
            Ok(shapes)
        // dynamic inputs
        } else if inputs.is_empty() && id.is_first() {
            self.set_last_shapes_from_child(target_shapes, id)
        } else {
            unimplemented!()
        }
    }

    fn convert_dim(
        &mut self,
        dim: ast::Dim,
        arg: u64,
        is_new_var_created: &mut bool,
    ) -> Result<Dim, GraphError> {
        match dim {
            ast::Dim::Fixed(dim) => Ok(Dim::Expr(dim.into())),
            ast::Dim::Semantic(var) => self.find_var(var, is_new_var_created),
            ast::Dim::Expr { lhs, rhs, op } => {
                let lhs = self.convert_dim(*lhs, arg, is_new_var_created)?;
                let rhs = self.convert_dim(*rhs, arg, is_new_var_created)?;
                match op {
                    ast::DimOp::Add => Ok(lhs + rhs),
                    ast::DimOp::Sub => Ok(lhs - rhs),
                    ast::DimOp::Mul => Ok(lhs * rhs),
                    ast::DimOp::Div => {
                        if let Dim::Expr(rhs) = self.eval_dim(&rhs) {
                            if rhs == 0u64 {
                                return Err(GraphError::DivideByZero {
                                    id: *self.get_last_node_id(),
                                    arg,
                                });
                            }
                        }
                        Ok(lhs / rhs)
                    }
                }
            }
        }
    }

    fn find_var(&mut self, var: String, is_new_var_created: &mut bool) -> Result<Dim, GraphError> {
        let key = DimKey::Placeholder(var, true);
        if self.keys.contains_key(&key) {
            return Ok(Dim::Key(key));
        }
        let key = DimKey::Placeholder(key.into_name(), false);
        if self.keys.contains_key(&key) {
            return Ok(Dim::Key(key));
        }
        let mut var = key.into_name();
        if let Some(alias) = self.variable_aliases.get(&var) {
            var = alias.clone();
        }
        if let Some(graph_var) = self.variables.get_mut(&var) {
            graph_var.expect_or_default(ValueType::UInt)?;
            let key = DimKey::Variable(var);
            Ok(Dim::Key(key))
        } else if self.shape_state.is_new_var_available() {
            *is_new_var_created = true;
            let key = DimKey::Placeholder(var, self.get_last_node_id().node == 0);
            let value = key.to_expr();
            self.keys.insert(key.clone(), value);
            Ok(Dim::Key(key))
        } else {
            Err(GraphError::FullShapeRequired {
                id: *self.get_last_node_id(),
            })
        }
    }

    fn eval_dim(&self, dim: &Dim) -> Dim {
        Self::eval_dim_with_keys(&self.keys, dim)
    }

    fn eval_dim_for_output(&self, dim: &Dim) -> Dim {
        match dim {
            Dim::Key(key) => match key {
                DimKey::Placeholder(_, _) => Dim::Expr(self.keys.eval_once(&key.to_expr())),
                _ => dim.clone(),
            },
            _ => dim.clone(),
        }
    }

    fn eval_dim_with_keys(keys: &ExpressionMap<DimKey>, dim: &Dim) -> Dim {
        match dim {
            Dim::Key(DimKey::Placeholder(_, false)) => dim.clone(),
            _ => Dim::Expr(keys.eval_once(&dim.to_expr())),
        }
    }

    fn update_dim(
        &mut self,
        id: GraphId,
        arg: u64,
        dim: &Dim,
        ground: &Dim,
        axis: usize,
    ) -> Result<Dim, GraphError> {
        match dim {
            Dim::Key(DimKey::Placeholder(ph, ph_is_input)) => {
                // test placeholders
                let key = DimKey::Placeholder(ph.to_string(), *ph_is_input);
                if let Some(dim) = self.keys.get(&key) {
                    if dim != key.to_expr() && dim != ground.to_expr() {
                        return Err(GraphError::DifferentDimension {
                            id,
                            arg,
                            axis,
                            expected: Dim::Expr(dim),
                            given: ground.clone(),
                        });
                    }
                }

                match ground {
                    Dim::Key(DimKey::Placeholder(ground, ground_is_input)) => {
                        if ph == ground {
                            Ok(Dim::Key(DimKey::Placeholder(
                                ground.clone(),
                                *ground_is_input,
                            )))
                        } else if *ph_is_input && *ground_is_input {
                            let key = DimKey::Placeholder(ph.clone(), *ph_is_input);
                            let ground = DimKey::Placeholder(ground.clone(), *ph_is_input);
                            let ground = ground.to_expr();
                            self.keys.insert(key, ground.clone());
                            Ok(Dim::Expr(ground))
                        } else {
                            Err(GraphError::CannotEstimateShape { id, arg, axis })
                        }
                    }
                    _ => {
                        let ground = ground.to_expr();
                        let key = DimKey::Placeholder(ph.clone(), *ph_is_input);
                        self.keys.insert(key, ground.clone());
                        Ok(Dim::Expr(ground))
                    }
                }
            }
            _ => {
                let dim = self.eval_dim(dim);
                let ground = ground.clone();
                let ground_eval = self.eval_dim(&ground);

                if dim == ground_eval {
                    Ok(ground)
                } else {
                    Err(GraphError::DifferentDimension {
                        id,
                        arg,
                        axis,
                        expected: dim,
                        given: ground_eval,
                    })
                }
            }
        }
    }

    fn get_last_node_id(&self) -> &GraphId {
        self.nodes.last_key_value().unwrap().0
    }

    fn get_last_specific_node_id(
        &self,
        node: u64,
        query_id: &GraphId,
    ) -> Result<&GraphId, GraphError> {
        match self.nodes.keys().rev().find(|n| n.node == node) {
            Some(id) => Ok(id),
            None => Err(GraphError::NoSuchNode {
                query_id: *query_id,
                node,
            }),
        }
    }

    fn get_last_node_name(&self) -> &str {
        &self.nodes.last_key_value().unwrap().1.name
    }

    fn get_first_shapes(&self) -> &Shapes {
        &self.nodes.first_key_value().unwrap().1.shapes
    }

    fn get_last_shapes(&self, inputs: Option<&[GraphIdArg]>) -> Shapes {
        let (last_id, last_node) = &self.nodes.last_key_value().unwrap();
        let inputs = inputs
            .map(|a| a.to_vec())
            .or_else(|| Some(vec![GraphIdArg::with_id(**last_id)]))
            .unwrap();

        match inputs.len() {
            0 => self.get_last_shapes(None),
            1 => {
                let id_arg = inputs.last().unwrap();
                match &id_arg.arg {
                    Some(arg) => {
                        let shapes = &self.nodes[&id_arg.id].shapes;
                        shapes.index_args(&[*arg])
                    }
                    None => {
                        if id_arg.id == **last_id {
                            last_node.shapes.clone()
                        } else {
                            self.nodes[&id_arg.id].shapes.clone()
                        }
                    }
                }
            }
            _ => inputs
                .iter()
                .map(|id_arg| {
                    let shapes = &self.nodes[&id_arg.id].shapes;
                    shapes.index_args(&[id_arg.arg.unwrap()])
                })
                .fold(Shapes::Fixed(Default::default()), |a, b| a.append(b)),
        }
    }

    fn set_last_shapes(&mut self, shapes: Shapes) {
        self.nodes.last_entry().unwrap().get_mut().shapes = shapes;
    }

    fn set_last_shapes_from_child(
        &mut self,
        mut shapes: Shapes,
        id: GraphId,
    ) -> Result<Shapes, GraphError> {
        match shapes {
            Shapes::Dynamic => Err(GraphError::FullShapeRequired { id }),
            Shapes::Fixed(_) => {
                for ph in shapes.try_archive_placeholders(id) {
                    self.find_var(ph, &mut false)?;
                }

                self.set_last_shapes(shapes.clone());
                Ok(shapes)
            }
        }
    }
}

fn get_flag(args: &[ast::GraphPassArg]) -> Result<bool, GraphError> {
    args.iter()
        .find(|a| a.is_named("transform"))
        .map(|a| match a.unwrap_value().clone() {
            Value::Bool(v) => Ok(v),
            other => Err(GraphError::DifferentVariableType {
                variable: a.unwrap_name().to_string(),
                expected: ValueType::Bool,
                given: Some(other),
            }),
        })
        .unwrap_or(Ok(false))
}
