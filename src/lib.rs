#![feature(map_first_last)]

#[macro_use]
extern crate generator;

mod compile;
mod error;
mod graphs;

pub use self::error::CompileError;
pub use self::graphs::{
    DimKey, Graph, GraphId, GraphIdArg, GraphRoot, Node, Value, ValueType, Variable,
};

pub use n3_parser::ast::UseOrigin;
pub use symengine::Expression;
