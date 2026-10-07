use serde::{Serialize, Deserialize};

use crate::bim::base_types::{Id};

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionLayerGroupData {
    pub name: String,
    pub layers: Vec<String>,
    pub layer_offsets: Vec<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionLayerGroup {
    pub(crate) uuid: String,
    pub(crate) name: String,
    pub(crate) layers: Vec<Id>,
    pub(crate) layer_offsets: Vec<f64>,
}

