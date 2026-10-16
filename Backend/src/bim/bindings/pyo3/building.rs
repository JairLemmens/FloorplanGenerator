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
        
        let face_id = Id {index: face_id.index, generation: face_id.generation};

        let geometry = self
            .binding
            .layer_geometry(face_id)
            .map_err(PyValueError::new_err)?;
        
        let face_uuid = self
            .binding
            .topology
            .face(face_id)
            .ok_or_else(|| PyValueError::new_err("Invalid face Id"))?
            .uuid
            .clone();

        let result = PyList::empty(py);

        for (layer_idx, (name, uuid, joints, layer, area)) in geometry.into_iter().enumerate() {
            let layer_dict = PyDict::new(py);

            layer_dict.set_item("name", name)?;
            layer_dict.set_item("uuid", uuid)?;
            layer_dict.set_item("area", area)?;

            let py_joints = PyList::empty(py);

            let layer_joints = joints
                .get(layer_idx)
                .ok_or_else(|| PyValueError::new_err("Joints do not match layer count"))?;

            for joint in layer_joints {
                match joint {
                    Some(joint) => {
                        let face = self
                            .binding
                            .topology
                            .face(joint.face_id)
                            .ok_or_else(|| PyValueError::new_err("Invalid joint face Id"))?;

                        let py_joint = PyDict::new(py);
                        py_joint.set_item("face_Id", &face.uuid)?;
                        py_joint.set_item("layer_idx", joint.layer_idx)?;
                        py_joints.append(py_joint)?;
                    }

                    None => {
                        py_joints.append(py.None())?;
                    }
                }
            }

            layer_dict.set_item("joints", py_joints)?;

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
        let output = PyDict::new(py);
        output.set_item(face_uuid.to_string(), result)?;

        Ok(output.into())
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
