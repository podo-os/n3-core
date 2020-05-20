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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct GraphRoot {
    graphs: HashMap<String, Graph>,
    compiling: HashSet<String>,

    prefabs: HashMap<String, ast::File>,
}

impl Default for GraphRoot {
    fn default() -> Self {
        Self {
            graphs: HashMap::default(),
            compiling: HashSet::default(),

            prefabs: Self::load_graph_prefabs_no_local().unwrap(),
        }
    }
}

impl GraphRoot {
    pub fn with_path<P: AsRef<Path>>(pwd: P) -> Result<Self, CompileError> {
        Ok(Self {
            graphs: HashMap::default(),
            compiling: HashSet::default(),

            prefabs: Self::load_graph_prefabs(Some(pwd))?,
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

    pub fn compile_from_source(&mut self, source: &str) -> Result<Graph, CompileError> {
        let (name, ast) = Self::load_graph_prefab(PathBuf::new(), source)?;
        let graph = ast.compile(self)?;
        self.graphs.insert(name, graph.clone());
        Ok(graph)
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
            self.graphs.insert(name.to_string(), model.clone());
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

    fn load_graph_prefabs<P: AsRef<Path>>(
        pwd: Option<P>,
    ) -> Result<HashMap<String, ast::File>, CompileError> {
        match pwd {
            Some(pwd) => Self::load_graph_prefabs_local(pwd),
            None => Self::load_graph_prefabs_no_local(),
        }
    }

    fn load_graph_prefabs_no_local() -> Result<HashMap<String, ast::File>, CompileError> {
        Self::load_graph_prefabs_embed()
            .into_iter()
            .map(|(path, source)| Self::load_graph_prefab(path, &source))
            .collect()
    }

    fn load_graph_prefabs_embed() -> Vec<(PathBuf, String)> {
        STD_DIR
            .find("**/*.n3")
            .unwrap()
            .filter_map(|r| STD_DIR.get_file(r.path()))
            .filter_map(|p| p.contents_utf8().map(|s| (p.path().into(), s.to_string())))
            .collect()
    }

    fn load_graph_prefab(path: PathBuf, source: &str) -> Result<(String, ast::File), CompileError> {
        let ast = parser::parse_file(source)
            .or_else(|e| Err(CompileError::ParseError { error: e, path }))?;

        let name = ast.model.name.clone();

        Ok((name, ast))
    }
}

static STD_DIR: Dir<'static> = include_dir!("std");

#[cfg(not(target_arch = "wasm32"))]
impl GraphRoot {
    fn load_graph_prefabs_local<P: AsRef<Path>>(
        pwd: P,
    ) -> Result<HashMap<String, ast::File>, CompileError> {
        walkdir::WalkDir::new(pwd)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|r| !r.metadata().map(|m| m.is_dir()).unwrap_or(true))
            .map(|r| r.path().into())
            .filter_map(|p: PathBuf| {
                let source = fs::read_to_string(&p).ok()?;
                if p.to_str().unwrap().ends_with(".n3") {
                    Some((p, source))
                } else {
                    None
                }
            })
            .chain(Self::load_graph_prefabs_embed())
            .map(|(p, s)| Self::load_graph_prefab(p, &s))
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
impl GraphRoot {
    fn load_graph_prefabs_local<P: AsRef<Path>>(
        pwd: P,
    ) -> Result<HashMap<String, ast::File>, CompileError> {
        println!(
            "Initializing GraphRoot with path on wasm is not supported yet: {}",
            pwd.as_ref().display()
        );
        Self::load_graph_prefabs_no_local()
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
