pub mod arena;
pub mod base_types;
pub mod building;
pub mod bindings;
pub mod construction;
pub mod extraction;
pub mod quads;
mod serde_points;
pub mod topology;
pub mod utils;

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use crate::bindings::bim::pyo3::base_types::PyId;
#[cfg(feature = "python")]
use crate::bindings::bim::pyo3::building::PyBuilding;
#[cfg(feature = "python")]
use crate::bindings::extraction::pyo3::extract::extract_boundaries;
#[cfg(feature = "python")]
use crate::bindings::quads::{mesh_domain, segment_domain};

#[cfg(feature = "python")]
#[pymodule]
fn floorplan_backend(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_boundaries, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_domain, m)?)?;
    m.add_function(wrap_pyfunction!(segment_domain, m)?)?;
    m.add_class::<PyBuilding>()?;
    m.add_class::<PyId>()?;
    Ok(())
}
