use openmaths::{Vector3};
use serde::{Serialize, Deserialize};

use crate::bim::base_types::{Id, Dictionary};

#[derive(Serialize, Deserialize, Debug)]
pub struct TopologyVertData {
    pub coords: [f64; 3],
    pub edges: Vec<String>,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyVert {
    pub(crate) uuid: String, 
    pub coords: Vector3, 
    pub(crate) edges: Vec<Id>, 
    pub(crate) dictionary: Dictionary
}
