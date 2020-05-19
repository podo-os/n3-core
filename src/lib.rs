#![feature(map_first_last)]

#[macro_use]
extern crate generator;

mod compile;
mod error;
mod graphs;

pub use self::graphs::GraphRoot;
