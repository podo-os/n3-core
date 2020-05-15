use crate::error::GraphError;

pub use n3_parser::ast::Value;

#[derive(Clone, Debug)]
pub struct Variable {
    pub description: String,
    pub ty: ValueType,
    pub value: Option<Value>,
}

impl Variable {
    pub fn update(&mut self, value: Value) -> Result<(), GraphError> {
        let ty = ValueType::new(Some(&value));
        if self.ty == ty || self.ty == ValueType::Required {
            self.value = Some(value);
            Ok(())
        } else {
            Err(GraphError::DifferentVariableType {
                variable: self.description.clone(),
                expected: self.ty.clone(),
                given: Some(value),
            })
        }
    }

    pub fn unwrap_uint(&self) -> Option<u64> {
        match self.value {
            Some(Value::UInt(value)) => Some(value),
            _ => None,
        }
    }

    pub fn expect_or_default(&mut self, ty: ValueType) -> Result<(), GraphError> {
        if self.ty == ty {
            Ok(())
        } else if self.ty == ValueType::Required {
            self.ty = ty;
            Ok(())
        } else {
            Err(GraphError::DifferentVariableType {
                variable: self.description.clone(),
                expected: ty,
                given: self.value.clone(),
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueType {
    Required,
    Bool,
    Int,
    UInt,
    Real,
    Model,
}

impl ValueType {
    pub fn new(value: Option<&Value>) -> Self {
        match value {
            Some(Value::Bool(_)) => Self::Bool,
            Some(Value::Int(_)) => Self::Int,
            Some(Value::UInt(_)) => Self::UInt,
            Some(Value::Real(_)) => Self::Real,
            Some(Value::Model(_)) => Self::Model,
            None => Self::Required,
        }
    }
}
