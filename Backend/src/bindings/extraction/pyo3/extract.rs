use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::extraction::extract::{extract_boundaries_raw};

#[pyfunction]
#[pyo3(signature = (edge_ptr, b, n, h, w, smoothing=2.0))]
pub fn extract_boundaries(edge_ptr: usize, b: usize, n: usize, h: usize, w: usize, smoothing: f64) -> PyResult<(Vec<Vec<[i32; 2]>>, Vec<[i64; 2]>)> {
    if b == 0 {return Err(PyValueError::new_err("B must be > 0"))}

    if n == 0 {return Err(PyValueError::new_err("N must be > 0"))}

    if n > 256 {return Err(PyValueError::new_err("N must be <= 256"))}

    if h == 0 || w == 0 {return Ok((Vec::new(), Vec::new()));}

    if edge_ptr == 0 {return Err(PyValueError::new_err("edge_ptr is null"));}

    if !smoothing.is_finite() || smoothing < 0.0 {
        return Err(PyValueError::new_err(
            "smoothing must be finite and >= 0",
        ));
    }

    let result = unsafe {
        extract_boundaries_raw(edge_ptr as *const u8, b, n, h, w, smoothing)
    };

    Ok(result)
}
