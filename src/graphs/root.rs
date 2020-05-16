use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::graph::Graph;
use crate::compile::Compile;
use crate::error::{CompileError, ModelError};

use include_dir::{include_dir, Dir};
use n3_parser::ast;
use n3_parser::parser;

pub struct GraphRoot {
    graphs: HashMap<String, Graph>,
    compiling: HashSet<String>,

    prefabs: HashMap<String, ast::File>,
}

impl GraphRoot {
    pub fn new(pwd: PathBuf) -> Result<Self, CompileError> {
        Ok(Self {
            graphs: HashMap::default(),
            compiling: HashSet::default(),

            prefabs: Self::load_graph_prefabs(pwd)?,
        })
    }

    pub fn find_graph(
        &mut self,
        name: &str,
        origin: ast::UseOrigin,
    ) -> Result<Graph, CompileError> {
        if let Some(graph) = self.graphs.get(name) {
            Ok(graph.clone())
        } else if self.compiling.contains(name) {
            recursive_model(name, origin)
        } else {
            self.load_graph(name, origin)
        }
    }
}

impl GraphRoot {
    fn load_graph(&mut self, name: &str, origin: ast::UseOrigin) -> Result<Graph, CompileError> {
        if self.compiling.insert(name.to_string()) {
            let model = match origin {
                ast::UseOrigin::Site(site) => self.load_graph_site(name, site),
                ast::UseOrigin::User(user) => self.load_graph_user(name, user),
                ast::UseOrigin::Local => self.load_graph_local(name),
            }?;
            self.compiling.remove(name);
            Ok(model)
        } else {
            recursive_model(name, origin)
        }
    }

    fn load_graph_site(&mut self, name: &str, site: String) -> Result<Graph, CompileError> {
        unimplemented!()
    }

    fn load_graph_user(&mut self, name: &str, site: String) -> Result<Graph, CompileError> {
        unimplemented!()
    }

    fn load_graph_local(&mut self, name: &str) -> Result<Graph, CompileError> {
        if let Some(ast) = self.prefabs.remove(name) {
            ast.compile(self)
        } else {
            model_not_found(name, ast::UseOrigin::Local)
        }
    }
}

static STD_DIR: Dir<'static> = include_dir!("std");

#[cfg(not(target_arch = "wasm32"))]
impl GraphRoot {
    fn load_graph_prefabs(pwd: PathBuf) -> Result<HashMap<String, ast::File>, CompileError> {
        walkdir::WalkDir::new(pwd)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|r| !r.metadata().map(|m| m.is_dir()).unwrap_or(true))
            .map(|r| r.path().into())
            .filter_map(|p| {
                let source = fs::read_to_string(&p).ok()?;
                Some((p, source))
            })
            .chain(Self::load_graph_prefabs_embed())
            .map(|(p, s)| Self::load_graph_prefab(p, s))
            .collect()
    }

    fn load_graph_prefab(
        path: PathBuf,
        source: String,
    ) -> Result<(String, ast::File), CompileError> {
        let ast = parser::parse_file(&source)
            .or_else(|e| Err(CompileError::ParseError { error: e, path }))?;

        let name = ast.model.name.clone();

        Ok((name, ast))
    }

    fn load_graph_prefabs_embed() -> Vec<(PathBuf, String)> {
        STD_DIR
            .find("**/*.n3")
            .unwrap()
            .filter_map(|r| STD_DIR.get_file(r.path()))
            .filter_map(|p| p.contents_utf8().map(|s| (p.path().into(), s.to_string())))
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
impl GraphRoot {
    fn load_graph_prefabs(_pwd: PathBuf) -> Result<HashMap<String, ast::File>, CompileError> {
        Self::load_graph_prefabs_embed()
            .into_iter()
            .map(Self::load_graph_prefab)
            .collect()
    }
}

fn model_not_found<T>(name: &str, origin: ast::UseOrigin) -> Result<T, CompileError> {
    Err(CompileError::ModelError {
        error: ModelError::ModelNotFound,
        model: name.to_string(),
        origin,
    })
}

fn recursive_model<T>(name: &str, origin: ast::UseOrigin) -> Result<T, CompileError> {
    Err(CompileError::ModelError {
        error: ModelError::RecursiveUsage,
        model: name.to_string(),
        origin,
    })
}
