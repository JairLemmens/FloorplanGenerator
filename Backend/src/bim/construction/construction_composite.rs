use serde::{Serialize, Deserialize};

use crate::bim::base_types::{MatOrCompId, Dictionary};

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionCompositeData{
    pub name: String,
    pub part1: String,
    pub part2: String,
    pub part1_fraction: f64,
    pub part2_fraction: f64,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionComposite {
    pub(crate) uuid: String,
    pub(crate) name: String,
    pub(crate) part1: MatOrCompId,
    pub(crate) part2: MatOrCompId,
    pub(crate) part1_fraction: f64,
    pub(crate) part2_fraction: f64,
    pub(crate) dictionary: Dictionary,
}

