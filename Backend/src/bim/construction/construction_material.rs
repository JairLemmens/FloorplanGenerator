use serde::{Serialize, Deserialize};
use crate::bim::base_types::{Dictionary};


#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionMaterialData {
    pub name: String,
    pub density: f64,
    pub heat_transfer_coefficient: f64,
    pub specific_heat_capacity: f64,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionMaterial {
    pub(crate) uuid: String,
    pub(crate) name: String,
    pub(crate) density: f64,
    pub(crate) heat_transfer_coefficient: f64,
    pub(crate) specific_heat_capacity: f64,
    pub(crate) dictionary: Dictionary,
}