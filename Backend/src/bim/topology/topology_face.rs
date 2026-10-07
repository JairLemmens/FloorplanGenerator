use serde::{Serialize, Deserialize};

use crate::bim::base_types::{Id, ConstructionRef, EdgeOffsets, LayerGeometry, Dictionary, EdgeMaterials, Frame};
use crate::bim::utils::{offset_per_segments, lift_points_with_offset, area_2d};
use crate::bim::construction::construction_set::{ConstructionSet};
use super::topology::{Topology};

#[derive(Serialize, Deserialize, Debug)]
pub struct TopologyFaceData {
    pub verts: Vec<String>,
    pub edges: Vec<String>,
    pub cells: Vec<String>,
    pub construction: String,
    #[serde(rename = "type")]
    pub face_type: String,
    pub uvn: Vec<[f64; 3]>,
    pub dictionary: Dictionary,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyFace {
    pub(crate) uuid: String,
    pub(crate) verts: Vec<Id>,
    pub(crate) edges: Vec<Id>,
    pub(crate) cells: Vec<Id>,

    pub(crate) construction: ConstructionRef,
    pub(crate) layer_group_id: Option<Id>,
    pub(crate) joint_materials: EdgeMaterials,
    pub(crate) inner_edge_offsets: EdgeOffsets,
    pub(crate) outer_edge_offsets: EdgeOffsets,

    pub(crate) face_type: String,
    pub(crate) frame: Frame,
    pub(crate) dictionary: Dictionary,
}

impl TopologyFace {
    pub fn geometry(&self,topology: &Topology,construction_set: &ConstructionSet,) -> Result<Vec<(String, String, EdgeMaterials, LayerGeometry, f64)>, String> {
        let layer_group_id = self
            .layer_group_id
            .ok_or_else(|| "Face has no layer group".to_string())?;

        let layer_group = construction_set
            .layer_group(layer_group_id)
            .ok_or_else(|| "Layer group not found".to_string())?;

        let layer_count = layer_group.layers.len();

        if self.inner_edge_offsets.len() != layer_count {
            return Err("inner_edge_offsets does not match layer count".to_string());
        }

        if self.outer_edge_offsets.len() != layer_count {
            return Err("outer_edge_offsets does not match layer count".to_string());
        }

        let corners = topology.face_coords_2d(self);

        let edge_count = corners.len();

        let mut geometry = Vec::with_capacity(layer_count);
        
        for (layer_index, layer_id) in layer_group.layers.iter().enumerate() {
            let layer = construction_set
                .layer(*layer_id)
                .ok_or_else(|| "Construction layer not found".to_string())?;

            let inner_offsets = &self.inner_edge_offsets[layer_index];
            let outer_offsets = &self.outer_edge_offsets[layer_index];

            if inner_offsets.len() != edge_count {
                return Err("inner edge offset count does not match face edge count".to_string());
            }

            if outer_offsets.len() != edge_count {
                return Err("outer edge offset count does not match face edge count".to_string());
            }

            let z_in = layer_group.layer_offsets[layer_index];
            let z_out = z_in + layer.thickness;

            let inner_2d = offset_per_segments(&corners, inner_offsets)?;
            let outer_2d = offset_per_segments(&corners, outer_offsets)?;
            
            let area = (area_2d(&inner_2d) + area_2d(&outer_2d))*0.5;

            let inner = lift_points_with_offset(&self.frame, &inner_2d, z_in);
            let outer = lift_points_with_offset(&self.frame, &outer_2d, z_out);

            geometry.push((layer.name.clone(), layer.uuid.clone(), self.joint_materials.clone(), LayerGeometry{inner, outer}, area.clone()));            
        }

        Ok(geometry)
    }
}