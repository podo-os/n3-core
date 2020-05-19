use crate::error::GraphError;

pub use n3_parser::ast::Value;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Variable {
    pub description: String,
    pub ty: ValueType,
    pub value: Option<Value>,
}

impl Variable {
    pub fn update(&mut self, value: Value, ty: ValueType) -> Result<(), GraphError> {
        if self.ty == ty || self.ty == ValueType::Required {
            self.value = Some(value);
            self.ty = ty;
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub fn new(value: Option<&Value>, is_model: bool) -> Self {
        if is_model {
            return Self::Model;
        }
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
