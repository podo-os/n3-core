use crate::error::{CompileError, ExternModelError, GraphError, NonExternModelError};
use crate::graphs::*;

use n3_parser::ast;

impl<'a> Compile<'a> for ast::File {
    type Args = &'a mut GraphRoot;
    type Output = Graph;

    fn compile(self, root: Self::Args) -> Result<Self::Output, CompileError> {
        let mut graph = Graph::default();

        for model in self.uses {
            let (name, use_g) = model.compile(root)?;
            graph.add_graph(name, use_g)?;
        }

        self.model.compile(&mut graph)?;
        Ok(graph)
    }
}

impl<'a> Compile<'a> for ast::Use {
    type Args = &'a mut GraphRoot;
    type Output = (String, Graph);

    fn compile(self, root: Self::Args) -> Result<Self::Output, CompileError> {
        let model = root.find_graph(&self.model, self.origin)?;
        Ok((self.model, model))
    }
}

impl<'a> Compile<'a> for ast::Model {
    type Args = &'a mut Graph;
    type Output = ();

    fn compile(self, mut graph: Self::Args) -> Result<Self::Output, CompileError> {
        if self.is_extern {
            if let Some(model) = self.inner.children.into_iter().next() {
                return Err(CompileError::ExternModelError {
                    error: ExternModelError::UnexpectedChild { model: model.name },
                    model: self.name,
                });
            }

            if self.inner.graph.len() != 2 {
                return Err(CompileError::ExternModelError {
                    error: ExternModelError::UnknownGraph,
                    model: self.name,
                });
            }
        } else {
            for model in self.inner.children {
                match graph.update_graph(&model.name) {
                    Some(prefab) => {
                        if !model.inner.children.is_empty() {
                            return Err(CompileError::NonExternModelError {
                                error: NonExternModelError::OverrideChild,
                                model: model.name,
                            });
                        }

                        if !model.inner.graph.is_empty() {
                            return Err(CompileError::NonExternModelError {
                                error: NonExternModelError::OverrideGraph,
                                model: model.name,
                            });
                        }

                        for variable in model.inner.variables {
                            let (name, variable) = variable.compile(())?;
                            let description = variable.description;
                            let ty = variable.ty;
                            if let Some(variable) = variable.value {
                                if let Err(error) = prefab.update_variable(
                                    Some(description),
                                    Some(name),
                                    variable,
                                    ty,
                                ) {
                                    return Err(CompileError::GraphError {
                                        error,
                                        model: model.name,
                                    });
                                }
                            } else {
                                return Err(CompileError::GraphError {
                                    error: GraphError::NoVariableValue { name },
                                    model: model.name,
                                });
                            }
                        }
                    }
                    None => {
                        let mut prefab = graph.new_child(&model.name)?;
                        model.compile(&mut prefab)?;
                    }
                }
            }

            if self.inner.graph.is_empty() {
                return Err(CompileError::NonExternModelError {
                    error: NonExternModelError::NoGraph,
                    model: self.name,
                });
            }
        }

        for variable in self.inner.variables {
            let (name, variable) = variable.compile(())?;
            if let Err(error) = graph.add_variable(Some(name), variable) {
                return Err(CompileError::GraphError {
                    error,
                    model: self.name,
                });
            }
        }

        for node in self.inner.graph {
            node.compile(&mut graph)?;
        }

        if !self.is_extern {
            graph.finalize()
        } else {
            Ok(())
        }
    }
}

impl<'a> Compile<'a> for ast::Variable {
    type Args = ();
    type Output = (String, Variable);

    fn compile(self, (): Self::Args) -> Result<Self::Output, CompileError> {
        let name = if let Some(name) = self.name {
            name
        } else {
            self.description.clone()
        };

        let variable = Variable {
            description: self.description,
            ty: ValueType::new(self.default.as_ref(), self.is_model),
            value: self.default,
        };

        Ok((name, variable))
    }
}

impl<'a> Compile<'a> for ast::Graph {
    type Args = &'a mut Graph;
    type Output = ();

    fn compile(self, graph: Self::Args) -> Result<Self::Output, CompileError> {
        for (pass_idx, pass) in self.passes.into_iter().enumerate() {
            for repeat in 0..pass.repeat {
                let id = GraphId {
                    node: self.id,
                    pass: pass_idx as u64,
                    repeat,
                };

                graph.attach(id, pass.name.clone(), pass.args.clone())?;
            }
        }

        if let Some(shapes) = self.shapes {
            graph.adjust_shapes(shapes)
        } else {
            Ok(())
        }
    }
}

pub trait Compile<'a> {
    type Args;
    type Output;

    fn compile(self, args: Self::Args) -> Result<Self::Output, CompileError>;
}
