use crate::error::{CompileError, ExternModelError, GraphError, NonExternModelError};
use crate::graphs::*;

use n3_parser::ast;

impl<'a> Compile<'a> for ast::File {
    type Args = &'a mut GraphRoot;
    type Output = Graph;

    fn compile(self, root: Self::Args) -> Result<Self::Output, CompileError> {
        let mut graph = Graph::new(self.model.is_extern);

        for model in self.uses {
            let (name, use_g) = model.compile(root)?;
            graph.add_graph(name, use_g.clone());
        }

        let (_, graph) = self.model.compile(&mut graph)?;
        Ok(graph)
    }
}

impl<'a> Compile<'a> for ast::Use {
    type Args = &'a mut GraphRoot;
    type Output = (String, &'a Graph);

    fn compile(self, root: Self::Args) -> Result<Self::Output, CompileError> {
        let model = root.find_graph(&self.model, self.origin)?;
        Ok((self.model, model))
    }
}

impl<'a> Compile<'a> for ast::Model {
    type Args = &'a mut Graph;
    type Output = (String, Graph);

    fn compile(self, parent: Self::Args) -> Result<Self::Output, CompileError> {
        let (mut child, is_override) = if self.is_extern {
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

            let prefab = Graph::new(true);

            (prefab, false)
        } else {
            match parent.find_graph(&self.name) {
                Some(prefab) => {
                    if !self.inner.children.is_empty() {
                        return Err(CompileError::NonExternModelError {
                            error: NonExternModelError::OverrideChild,
                            model: self.name,
                        });
                    }

                    if !self.inner.graph.is_empty() {
                        return Err(CompileError::NonExternModelError {
                            error: NonExternModelError::OverrideGraph,
                            model: self.name,
                        });
                    }

                    (prefab, true)
                }
                None => {
                    if self.inner.graph.is_empty() {
                        return Err(CompileError::NonExternModelError {
                            error: NonExternModelError::NoGraph,
                            model: self.name,
                        });
                    }

                    let mut prefab = parent.new_child();

                    let children = self
                        .inner
                        .children
                        .into_iter()
                        .map(|child| {
                            let (name, child) = child.compile(&mut prefab)?;
                            Ok((name, child))
                        })
                        .collect::<Result<Vec<_>, CompileError>>()?;

                    // once the children have been compiled all,
                    // add them respectively
                    for (name, child) in children {
                        prefab.add_graph(name, child);
                    }

                    (prefab, false)
                }
            }
        };

        if is_override {
            for variable in self.inner.variables {
                let (name, variable) = variable.compile(())?;
                let description = variable.description;
                let ty = variable.ty;
                if let Some(variable) = variable.value {
                    if let Err(error) =
                        child.update_variable(Some(description), Some(name), variable, ty)
                    {
                        return Err(CompileError::GraphError {
                            error,
                            model: self.name,
                        });
                    }
                } else {
                    return Err(CompileError::GraphError {
                        error: GraphError::NoVariableValue { name },
                        model: self.name,
                    });
                }
            }
        } else {
            for variable in self.inner.variables {
                let (name, variable) = variable.compile(())?;
                if let Err(error) = child.add_variable(Some(name), variable) {
                    return Err(CompileError::GraphError {
                        error,
                        model: self.name,
                    });
                }
            }
        }

        for node in self.inner.graph {
            node.compile(&mut child)?;
        }

        if !self.is_extern && !is_override {
            child.finalize()?;
        }
        Ok((self.name, child))
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
        let mut inline = if let Some(inline) = self.inline {
            let (_, inline) = inline.compile(graph)?;
            Some(inline)
        } else {
            None
        };

        for (pass_idx, pass) in self.passes.into_iter().enumerate() {
            for repeat in 0..pass.repeat {
                let id = GraphId {
                    node: self.id,
                    pass: pass_idx as u64,
                    repeat,
                };

                graph.attach(id, pass.name.clone(), inline.take(), pass.args.clone())?;
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
