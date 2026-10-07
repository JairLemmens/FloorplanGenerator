use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyDict};
use pythonize::{depythonize};
use numpy::PyArray2;
use openmaths::{Vector3};

use crate::bim::topology::topology::{Topology, TopologyData};
use crate::bim::construction::construction_set::{ConstructionSet, ConstructionSetData};
use crate::bim::building::{Building};
use crate::bim::base_types::{Id};

use super::base_types::PyId;


#[pyclass(name = "Building")]
pub struct PyBuilding {
    binding: Building,
}


#[pymethods]
impl PyBuilding {
    #[staticmethod]
    pub fn from_json(topology_json: &str, construction_json: &str) -> PyResult<Self> {
        let binding = Building::from_json(topology_json, construction_json).map_err(PyValueError::new_err)?;
        Ok(Self { binding })
    }

    #[staticmethod]
    pub fn from_data(topology: &Bound<'_, PyAny>, construction: &Bound<'_, PyAny>) -> PyResult<Self> {
        let topology_data: TopologyData = depythonize(topology).map_err(|e| PyValueError::new_err(e.to_string()))?;

        let construction_data: ConstructionSetData = depythonize(construction).map_err(|e| PyValueError::new_err(e.to_string()))?;

        let topology = Topology::from_data(topology_data).map_err(PyValueError::new_err)?;

        let construction_set = ConstructionSet::from_data(construction_data).map_err(PyValueError::new_err)?;

        Building::from_data(topology, construction_set).map(|binding| Self { binding }).map_err(PyValueError::new_err)
    }

    pub fn face_ids(&self) -> Vec<PyId> {
        self.binding
            .topology
            .face_ids()
            .into_iter()
            .map(PyId::from)
            .collect()
    }

    pub fn layer_geometry(&self, py: Python<'_>, face_id: &PyId) -> PyResult<Py<PyAny>> {
        let geometry = self
            .binding
            .layer_geometry(Id {
                index: face_id.index,
                generation: face_id.generation,
            })
            .map_err(PyValueError::new_err)?;

        let result = PyList::empty(py);

        for (name, uuid, edge_materials, layer, area) in geometry {
            let layer_dict = PyDict::new(py);

            layer_dict.set_item("name", name)?;
            layer_dict.set_item("uuid", uuid)?;
            layer_dict.set_item("area", area)?;

            let py_edge_materials = PyList::empty(py);

            for edge in edge_materials {
                let py_edge = PyList::empty(py);

                for id in edge {
                    match id {
                        Some(id) => {
                            let construction_layer = self
                                .binding
                                .construction_set
                                .layer(id)
                                .ok_or_else(|| {PyValueError::new_err("Invalid construction layer Id")
                                })?;

                            py_edge.append(&construction_layer.uuid)?;
                        }

                        None => {
                            py_edge.append(py.None())?;
                        }
                    }
                }

                py_edge_materials.append(py_edge)?;
            }

            layer_dict.set_item("edge_materials", py_edge_materials)?;

            layer_dict.set_item(
                "inner",
                points_to_numpy(py, &layer.inner)?,
            )?;

            layer_dict.set_item(
                "outer",
                points_to_numpy(py, &layer.outer)?,
            )?;

            result.append(layer_dict)?;
        }

        Ok(result.into())
    }


    pub fn solve_joints(&mut self) {
        self.binding.solve_joints();
    }
}

fn points_to_numpy<'py>(py: Python<'py>, points: &[Vector3]) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let data: Vec<Vec<f64>> = points
        .iter()
        .map(|p| vec![p.x, p.y, p.z])
        .collect();

    PyArray2::from_vec2(py, &data)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}