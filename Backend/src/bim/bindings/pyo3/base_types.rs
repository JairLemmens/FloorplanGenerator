use pyo3::prelude::*;

use crate::bim::base_types::Id;

#[pyclass(name = "Id")]
#[derive(Clone)]
pub struct PyId {
    #[pyo3(get)]
    pub index: u32,

    #[pyo3(get)]
    pub generation: u32,
}

#[pymethods]
impl PyId {
    #[new]
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}

impl From<Id> for PyId {
    fn from(id: Id) -> Self {
        Self {
            index: id.index,
            generation: id.generation,
        }
    }
}

impl From<&PyId> for Id {
    fn from(id: &PyId) -> Self {
        Id {
            index: id.index,
            generation: id.generation,
        }
    }
}