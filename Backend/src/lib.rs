pub mod bim;
pub mod extraction;


#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use crate::bim::bindings::pyo3::base_types::{PyId};
#[cfg(feature = "python")]
use crate::bim::bindings::pyo3::building::{PyBuilding};
#[cfg(feature = "python")]
use crate::extraction::bindings::pyo3::extract::{extract_boundaries};

#[cfg(feature = "python")]
#[pymodule]
fn floorplan_backend(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_boundaries, m)?)?;
    m.add_class::<PyBuilding>()?;
    m.add_class::<PyId>()?;
    Ok(())
}