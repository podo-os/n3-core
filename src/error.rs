use crate::graphs::{Dim, GraphId, Value, ValueType};

use n3_parser::ast;

#[derive(Debug)]
pub enum CompileError {
    ExternModelError {
        error: ExternModelError,
        model: String,
    },
    NonExternModelError {
        error: NonExternModelError,
        model: String,
    },
    ModelError {
        error: ModelError,
        model: String,
        origin: ast::UseOrigin,
    },
    GraphError {
        error: GraphError,
        model: String,
    },
    OsError {
        error: std::io::Error,
    },
    ParseError {
        error: n3_parser::error::ParseError,
        path: std::path::PathBuf,
    },
}

#[derive(Debug)]
pub enum ExternModelError {
    UnknownGraph,
    MalformedShape,
    UnexpectedChild { model: String },
}

#[derive(Debug)]
pub enum NonExternModelError {
    NoGraph,
    ModelNotFound,
    OverrideChild,
    OverrideGraph,
}

#[derive(Debug)]
pub enum ModelError {
    ModelNotFound,
    RecursiveUsage,
}

#[derive(Debug)]
pub enum GraphError {
    InputNodeNotFound,
    FirstNodeNotFound,
    UnvalidNodeId {
        last: GraphId,
        id: GraphId,
    },
    UnvalidNodeArg {
        id: GraphId,
        arg: u64,
        given: u64,
    },
    ShapeNotDefined {
        id: GraphId,
    },
    FullShapeRequired {
        id: GraphId,
    },
    NoSuchVariable {
        name: String,
    },
    NoVariableValue {
        name: String,
    },
    NoSuchNode {
        query_id: GraphId,
        node: u64,
    },
    CannotEstimateShape {
        id: GraphId,
        arg: u64,
        axis: usize,
    },
    DifferentDimension {
        id: GraphId,
        arg: u64,
        axis: usize,
        expected: Dim,
        given: Dim,
    },
    DifferentArgs {
        id: GraphId,
        last_args: Vec<u64>,
        args: Vec<u64>,
    },
    DifferentRank {
        id: GraphId,
        arg: u64,
        last_rank: usize,
        rank: usize,
    },
    DifferentVariableType {
        variable: String,
        expected: ValueType,
        given: Option<Value>,
    },
    DivideByZero {
        id: GraphId,
        arg: u64,
    },
}

impl From<std::io::Error> for CompileError {
    fn from(error: std::io::Error) -> Self {
        Self::OsError { error }
    }
}
