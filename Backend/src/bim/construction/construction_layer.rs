use serde::{Serialize, Deserialize};

use crate::bim::base_types::{MatOrCompId, Dictionary};

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionLayerData {
    pub name: String,
    pub material: String,
    pub thickness: f64,
    pub priority: f64,
    pub joint_type: String,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionLayer {
    pub(crate) uuid: String,
    pub(crate) name: String,
    pub(crate) material: MatOrCompId,
    pub(crate) thickness: f64,
    pub(crate) priority: f64,
    pub(crate) joint_type: String,
    pub(crate) dictionary: Dictionary,
}