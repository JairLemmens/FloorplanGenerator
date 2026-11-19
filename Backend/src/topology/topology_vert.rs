use serde::{Serialize, Deserialize};
use openmaths::Vector3;

use crate::base_types::{Id, Dictionary};

#[derive(Serialize, Deserialize)]
pub struct TopologyVertData {
    #[serde(with = "crate::serde_points::vector3_array")]
    pub coords: Vector3,
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
