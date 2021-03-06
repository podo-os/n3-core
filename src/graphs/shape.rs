use std::collections::BTreeMap;
use std::ops;

use super::id::GraphId;
use crate::error::GraphError;

use generator::{Generator, Gn};
use heck::CamelCase;
use symengine::{Expression, ExpressionMapKey};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeState {
    Fixed(FitState),
    Required(FitState),
    Transform,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FitState {
    /// Any dim can get new variables.
    Weak,
    /// Any dim should be deduced immediately.
    Full,
}

impl Default for ShapeState {
    fn default() -> Self {
        Self::Fixed(FitState::Weak)
    }
}

impl ShapeState {
    pub fn is_new_var_available(&self) -> bool {
        match self {
            Self::Fixed(FitState::Weak) => false,
            Self::Fixed(FitState::Full) => true,
            Self::Required(FitState::Weak) => true,
            Self::Required(FitState::Full) => false,
            Self::Transform => true,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum Shapes {
    Dynamic,
    Fixed(BTreeMap<u64, Shape>),
}

impl Shapes {
    pub fn product(self) -> Self {
        match self {
            Self::Dynamic => self,
            Self::Fixed(shapes) => {
                let shapes = shapes
                    .into_iter()
                    .map(|(arg, shape)| {
                        let shape = shape.product();
                        (arg, shape)
                    })
                    .collect();
                Self::Fixed(shapes)
            }
        }
    }

    pub fn validate_args_rank(&self, last: &Self, id: &GraphId) -> Result<bool, GraphError> {
        match (self, last) {
            (Self::Fixed(shapes), Self::Fixed(last_shapes)) => {
                let args = shapes.keys().cloned().collect();
                let last_args = last_shapes.keys().cloned().collect();
                if args == last_args {
                    shapes
                        .iter()
                        .zip(last_shapes.values())
                        .map(|((arg, a), b)| a.validate_rank(b, id, arg))
                        .fold(Ok(true), |a, b| Ok(a? == b?))
                } else {
                    Err(GraphError::DifferentArgs {
                        id: *id,
                        last_args,
                        args,
                    })
                }
            }
            _ => Ok(false),
        }
    }

    pub fn unwrap_shapes(&self) -> &BTreeMap<u64, Shape> {
        match self {
            Self::Fixed(shapes) => shapes,
            _ => unreachable!(),
        }
    }

    pub fn index_args(&self, args: &[u64]) -> Self {
        match self {
            Self::Dynamic => Self::Dynamic,
            Self::Fixed(shapes) => Self::Fixed(
                shapes
                    .iter()
                    .filter(|(a, _)| args.contains(a))
                    .map(|(a, s)| (*a, s.clone()))
                    .collect(),
            ),
        }
    }

    pub fn append(self, other: Self) -> Self {
        match (self, other) {
            (Self::Fixed(mut shapes), Self::Fixed(others)) => {
                let bias = shapes.len();
                for (arg, (_, other)) in others.into_iter().enumerate() {
                    shapes.insert((arg + bias) as u64, other);
                }
                Self::Fixed(shapes)
            }
            _ => Self::Dynamic,
        }
    }

    pub fn try_archive_placeholders(&mut self, id: GraphId) -> Generator<'_, (), String> {
        match self {
            Self::Fixed(shapes) => Gn::new_scoped(move |mut s| {
                for shape in shapes.values_mut() {
                    if let Shape::Fixed(dims) = shape {
                        for dim in dims {
                            if let Dim::Key(DimKey::Placeholder(ph, _)) = dim {
                                *ph = format!("_node_{:?}_{}", id.node, ph);
                                s.yield_(ph.clone());
                            }
                        }
                    }
                }
                done!();
            }),
            Self::Dynamic => Gn::new_scoped(|_| done!()),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum Shape {
    Dynamic,
    Fixed(Vec<Dim>),
}

impl Shape {
    pub fn product(self) -> Self {
        match self {
            Self::Dynamic => self,
            Self::Fixed(dims) => {
                let product = dims
                    .iter()
                    .map(|dim| match dim {
                        Dim::Key(key) => Expression::new(key.to_string()),
                        Dim::Expr(expr) => expr.clone(),
                    })
                    .fold(1.0.into(), ops::Mul::mul);
                Self::Fixed(vec![Dim::Expr(product)])
            }
        }
    }

    pub fn validate_rank(&self, last: &Self, id: &GraphId, arg: &u64) -> Result<bool, GraphError> {
        match (self, last) {
            (Self::Fixed(dims), Self::Fixed(last_dims)) => {
                let rank = dims.len();
                let last_rank = last_dims.len();
                if rank == last_rank {
                    Ok(true)
                } else {
                    Err(GraphError::DifferentRank {
                        id: *id,
                        arg: *arg,
                        last_rank,
                        rank,
                    })
                }
            }
            _ => Ok(false),
        }
    }

    pub fn unwrap_dims(&self) -> &[Dim] {
        match self {
            Self::Fixed(dims) => dims,
            _ => unreachable!(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Dim {
    Key(DimKey),
    Expr(Expression),
}

impl Dim {
    pub fn to_expr(&self) -> Expression {
        match self {
            Self::Key(key) => key.to_expr(),
            Self::Expr(expr) => expr.clone(),
        }
    }
}

impl ops::Add for Dim {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Dim::Expr(self.to_expr() + rhs.to_expr())
    }
}

impl ops::Sub for Dim {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Dim::Expr(self.to_expr() - rhs.to_expr())
    }
}

impl ops::Mul for Dim {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Dim::Expr(self.to_expr() * rhs.to_expr())
    }
}

impl ops::Div for Dim {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Dim::Expr(self.to_expr() / rhs.to_expr())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DimKey {
    Variable(String),
    Placeholder(String, bool),
}

impl ExpressionMapKey for DimKey {
    fn to_string(&self) -> String {
        match self {
            Self::Variable(var) => format!("var_{}", var.to_camel_case()),
            Self::Placeholder(ph, _) => format!("ph_{}", ph.to_camel_case()),
        }
    }
}

impl DimKey {
    pub fn into_name(self) -> String {
        match self {
            Self::Variable(var) => var,
            Self::Placeholder(ph, _) => ph,
        }
    }

    pub fn to_expr(&self) -> Expression {
        Expression::new(self.to_string())
    }
}
