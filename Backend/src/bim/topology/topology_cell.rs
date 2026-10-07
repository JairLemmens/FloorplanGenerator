use serde::{Serialize, Deserialize};

use crate::bim::base_types::{Id, Dictionary};

#[derive(Serialize, Deserialize, Debug)]
pub struct TopologyCellData {
    pub faces: Vec<String>,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyCell {
    pub(crate) uuid: String, 
    pub(crate) faces: Vec<Id>, 
    pub(crate) dictionary: Dictionary
}
