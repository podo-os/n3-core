use std::collections::{BTreeMap, HashMap};

use super::id::GraphId;
use super::node::Node;
use super::shape::{Dim, DimKey, FitState, Shape, ShapeState};
use super::variable::{Value, ValueType, Variable};
use crate::error::{CompileError, GraphError, NonExternModelError};

use n3_parser::ast;
use symengine::{Expression, ExpressionMap, ExpressionMapKey};

#[derive(Clone, Debug, Default)]
pub struct Graph {
    variables: HashMap<String, Variable>,
    variable_aliases: HashMap<String, String>,
    keys: ExpressionMap<DimKey>,

    graphs: HashMap<String, Graph>,

    nodes: BTreeMap<GraphId, Node>,
    shape_state: ShapeState,
}

impl Graph {
    pub(crate) fn new_child(&mut self, name: &str) -> Result<&mut Self, CompileError> {
        let child = Self {
            variables: HashMap::new(),
            variable_aliases: HashMap::new(),
            keys: ExpressionMap::new(),
            graphs: self.graphs.clone(),
            nodes: BTreeMap::new(),
            shape_state: ShapeState::default(),
        };
        self.add_graph(name.to_string(), child)?;
        Ok(self.graphs.get_mut(name).unwrap())
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

    pub(crate) fn add_graph(&mut self, name: String, graph: Self) -> Result<(), CompileError> {
        self.graphs.entry(name).or_insert_with(|| graph);
        Ok(())
    }

    pub(crate) fn update_graph(&mut self, name: &str) -> Option<&mut Self> {
        self.graphs.get_mut(name)
    }

    pub(crate) fn attach(
        &mut self,
        id: GraphId,
        name: String,
        args: Vec<ast::GraphPassArg>,
    ) -> Result<(), CompileError> {
        let last_id = if self.nodes.is_empty() {
            if id.is_first() {
                let id = GraphId::new_input();
                let node = Node::default();
                self.nodes.insert(id, node);
                Some(id)
            } else if id.is_input() {
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

        let node = match &*name {
            // intrinsics
            Node::INTRINSIC_DYNAMIC => match get_flag(&args) {
                Ok(true) => {
                    self.shape_state = ShapeState::Transform;
                    Node::default()
                }
                Ok(false) => {
                    self.shape_state = ShapeState::Required(FitState::Full);
                    Node::default()
                }
                Err(error) => return Err(CompileError::GraphError { error, model: name }),
            },
            Node::INTRINSIC_FIXED => {
                self.shape_state = match &self.shape_state {
                    ShapeState::Fixed(_) => ShapeState::Required(FitState::Weak),
                    ShapeState::Required(_) | ShapeState::Transform => {
                        return Err(CompileError::GraphError {
                            error: GraphError::ShapeNotDefined { id },
                            model: name,
                        })
                    }
                };
                Node {
                    name,
                    graph: None,
                    shape: Shape::Dynamic,
                }
            }
            Node::INTRINSIC_IDENTITY => {
                if last_id.is_some() {
                    Node {
                        name,
                        graph: None,
                        shape: self.get_last_shape().clone(),
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
                        shape: Shape::Dynamic,
                    }
                } else if let Some(mut graph) = self.graphs.get(&name).cloned() {
                    let model_name = name;
                    for arg in args {
                        if let ast::GraphPassArg::Node(nodes) = arg {
                            unimplemented!();
                        } else if let ast::GraphPassArg::Keyword { name, value } = arg {
                            let ty = ValueType::new(Some(&value), false);
                            if let Err(error) = graph.update_variable(None, Some(name), value, ty) {
                                return Err(CompileError::GraphError {
                                    error,
                                    model: model_name,
                                });
                            }
                        }
                    }

                    let shape = match self.apply_shape_as_input(&mut graph, id) {
                        Ok(shape) => shape,
                        Err(error) => {
                            return Err(CompileError::GraphError {
                                error,
                                model: model_name,
                            })
                        }
                    };
                    self.shape_state = graph.shape_state.clone();

                    Node {
                        name: model_name,
                        graph: Some(graph),
                        shape,
                    }
                } else {
                    return Err(CompileError::NonExternModelError {
                        error: NonExternModelError::ModelNotFound,
                        model: name,
                    });
                }
            }
        };

        self.nodes.insert(id, node);
        Ok(())
    }

    pub(crate) fn adjust_shape(&mut self, shape: ast::Shape) -> Result<(), CompileError> {
        let mut is_new_var_created = false;
        let shape: Vec<_> = shape
            .0
            .into_iter()
            .map(|d| self.convert_dim(d, &mut is_new_var_created))
            .collect::<Result<_, _>>()?;
        let mut shape = Shape::Fixed(shape);
        let shape_to = shape.clone();

        let (&id, last_node) = self.nodes.last_key_value().unwrap();
        let model = last_node.name.clone();
        let mut last_shape = last_node.shape.clone();

        if self.shape_state == ShapeState::Transform {
            shape = shape.product();
            last_shape = last_shape.product();
        }

        match shape.validate_rank(&last_shape, &id) {
            Ok(true) => {
                let last_dims = last_shape.unwrap_dims();
                let dims = shape.unwrap_dims();
                for (axis, (last_dim, dim)) in last_dims.iter().zip(dims).enumerate() {
                    if let Err(error) = self.update_dim(id, last_dim, dim, axis) {
                        return Err(CompileError::GraphError { error, model });
                    }
                }
            }
            Ok(false) => {}
            Err(error) => return Err(CompileError::GraphError { error, model }),
        }

        self.nodes.last_entry().unwrap().get_mut().shape = shape_to;
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
    fn apply_shape_as_input(&self, target: &mut Self, id: GraphId) -> Result<Shape, GraphError> {
        let node = &self.nodes.last_key_value().unwrap().1;
        let shape = &node.shape;
        let target_shape = &target.nodes.first_key_value().unwrap().1.shape;

        if target_shape.validate_rank(shape, &id)? {
            let dims = shape.unwrap_dims();
            let target_dims = target_shape.unwrap_dims().to_vec();
            let shape = dims
                .iter()
                .zip(target_dims)
                .enumerate()
                .map(|(axis, (dim, target_dim))| target.update_dim(id, &target_dim, dim, axis))
                .collect::<Result<_, _>>()?;

            let shape = match &target.nodes.last_key_value().unwrap().1.shape {
                Shape::Dynamic => shape,
                Shape::Fixed(dims) => dims.iter().map(|d| target.eval_dim(d.clone())).collect(),
            };

            Ok(Shape::Fixed(shape))
        } else if let Shape::Dynamic = target_shape {
            Ok(shape.clone())
        } else {
            unimplemented!()
        }
    }

    fn convert_dim(
        &mut self,
        dim: ast::Dim,
        is_new_var_created: &mut bool,
    ) -> Result<Dim, CompileError> {
        match dim {
            ast::Dim::Fixed(dim) => Ok(Dim::Expr(dim.into())),
            ast::Dim::Semantic(var) => self.find_var(var, is_new_var_created),
            ast::Dim::Expr { lhs, rhs, op } => {
                let lhs = self.convert_dim(*lhs, is_new_var_created)?;
                let rhs = self.convert_dim(*rhs, is_new_var_created)?;
                match op {
                    ast::DimOp::Add => Ok(lhs + rhs),
                    ast::DimOp::Sub => Ok(lhs - rhs),
                    ast::DimOp::Mul => Ok(lhs * rhs),
                    ast::DimOp::Div => {
                        if let Dim::Expr(rhs) = self.eval_dim(rhs.clone()) {
                            if rhs == 0u64 {
                                return Err(CompileError::GraphError {
                                    error: GraphError::DivideByZero {
                                        id: *self.get_last_node_id(),
                                    },
                                    model: self.get_last_node_name().to_string(),
                                });
                            }
                        }
                        Ok(lhs / rhs)
                    }
                }
            }
        }
    }

    fn find_var(
        &mut self,
        var: String,
        is_new_var_created: &mut bool,
    ) -> Result<Dim, CompileError> {
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
            match graph_var.expect_or_default(ValueType::UInt) {
                Ok(()) => {
                    let key = DimKey::Variable(var);
                    Ok(Dim::Key(key))
                }
                Err(error) => Err(CompileError::GraphError {
                    error,
                    model: self.get_last_node_name().to_string(),
                }),
            }
        } else if self.shape_state.is_new_var_available() {
            *is_new_var_created = true;
            let key = DimKey::Placeholder(var, self.get_last_node_id().node == 0);
            let value = Expression::new(key.to_string());
            self.keys.insert(key.clone(), value);
            Ok(Dim::Key(key))
        } else {
            Err(CompileError::GraphError {
                error: GraphError::FullShapeRequired {
                    id: *self.get_last_node_id(),
                },
                model: self.get_last_node_name().to_string(),
            })
        }
    }

    fn eval_dim(&self, dim: Dim) -> Dim {
        Self::eval_dim_with_keys(&self.keys, dim)
    }

    fn eval_dim_with_keys(keys: &ExpressionMap<DimKey>, dim: Dim) -> Dim {
        match dim {
            Dim::Key(DimKey::Placeholder(_, _)) => dim,
            _ => Dim::Expr(keys.eval_once(&dim.into_expr())),
        }
    }

    fn update_dim(
        &mut self,
        id: GraphId,
        dim: &Dim,
        ground: &Dim,
        axis: usize,
    ) -> Result<Dim, GraphError> {
        match dim {
            Dim::Key(DimKey::Placeholder(ph, ph_is_input)) => match ground {
                Dim::Key(DimKey::Placeholder(ground, ground_is_input)) => {
                    if ph == ground {
                        Ok(Dim::Key(DimKey::Placeholder(
                            ground.clone(),
                            *ground_is_input,
                        )))
                    } else if *ph_is_input && *ground_is_input {
                        let key = DimKey::Placeholder(ph.clone(), *ph_is_input);
                        let ground = DimKey::Placeholder(ground.clone(), *ph_is_input);
                        let ground = Expression::new(ground.to_string());
                        self.keys.insert(key, ground.clone());
                        Ok(Dim::Expr(ground))
                    } else {
                        Err(GraphError::CannotEstimateShape { id, axis })
                    }
                }
                _ => {
                    let ground = ground.clone().into_expr();

                    let key = DimKey::Placeholder(ph.clone(), *ph_is_input);
                    self.keys.insert(key, ground.clone());
                    Ok(Dim::Expr(ground))
                }
            },
            _ => {
                let dim = self.eval_dim(dim.clone());
                let ground = ground.clone();
                let ground_eval = self.eval_dim(ground.clone());

                if dim == ground_eval {
                    Ok(ground)
                } else {
                    Err(GraphError::DifferentDimension {
                        id,
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

    fn get_last_node_name(&self) -> &str {
        &self.nodes.last_key_value().unwrap().1.name
    }

    fn get_last_shape(&self) -> &Shape {
        &self.nodes.last_key_value().unwrap().1.shape
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
