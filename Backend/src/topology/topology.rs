use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::arena::Arena;
use super::topology_cell::{TopologyCell, TopologyCellData};
use super::topology_edge::{TopologyEdge, TopologyEdgeData};
use super::topology_face::{TopologyFace, TopologyFaceData};
use super::topology_vert::{TopologyVert, TopologyVertData};

#[derive(Serialize, Deserialize)]
pub struct TopologyData {
    pub verts: HashMap<String, TopologyVertData>,
    pub edges: HashMap<String, TopologyEdgeData>,
    pub faces: HashMap<String, TopologyFaceData>,
    pub cells: HashMap<String, TopologyCellData>,
}

/// Arena-backed topology storage. General and specialized mesh behavior is
/// provided by extension traits, so a quad mesh is the same value type.
#[derive(Clone)]
pub struct Topology {
    pub(crate) verts: Arena<TopologyVert>,
    pub(crate) edges: Arena<TopologyEdge>,
    pub(crate) faces: Arena<TopologyFace>,
    pub(crate) cells: Arena<TopologyCell>,
}

/// A quad mesh is a topology viewed through the quad-specific operations.
pub type QuadMesh = Topology;
