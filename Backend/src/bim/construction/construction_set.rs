use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use super::construction_composite::*;
use super::construction_layer::*;
use super::construction_layergroup::*;
use super::construction_material::*;

use crate::bim::arena::Arena;
use crate::bim::base_types::{Id, MatOrCompId, Dictionary};

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstructionSetData {
    pub materials: HashMap<String, ConstructionMaterialData>,
    pub composites: HashMap<String, ConstructionCompositeData>,
    pub layers: HashMap<String, ConstructionLayerData>,
    pub layer_groups: HashMap<String, ConstructionLayerGroupData>,
}

pub struct ConstructionSet {
    materials: Arena<ConstructionMaterial>,
    composites: Arena<ConstructionComposite>,
    layers: Arena<ConstructionLayer>,
    layer_groups: Arena<ConstructionLayerGroup>,
}


impl ConstructionSet {
    pub fn new() -> Self {
        Self {
            materials: Arena::new(),
            composites: Arena::new(),
            layers: Arena::new(),
            layer_groups: Arena::new(),
        }
    }

    pub fn material(&self, id: Id) -> Option<&ConstructionMaterial> {self.materials.get(id)}

    pub fn composite(&self, id: Id) -> Option<&ConstructionComposite> {self.composites.get(id)}

    pub fn layer(&self, id: Id) -> Option<&ConstructionLayer> {self.layers.get(id)}

    pub fn layer_group(&self, id: Id) -> Option<&ConstructionLayerGroup> {self.layer_groups.get(id)}

    pub fn add_material(&mut self,uuid: String,name: String,density: f64,heat_transfer_coefficient: f64,specific_heat_capacity: f64,dictionary: Dictionary) -> Id {
        self.materials.insert(ConstructionMaterial {
            uuid,
            name,
            density,
            heat_transfer_coefficient,
            specific_heat_capacity,
            dictionary,
        })
    }

    pub fn add_composite(&mut self,uuid: String,name: String,part1: MatOrCompId,part2: MatOrCompId,part1_fraction: f64,part2_fraction: f64,dictionary: Dictionary) -> Id {
        self.composites.insert(ConstructionComposite {
            uuid,
            name,
            part1,
            part2,
            part1_fraction,
            part2_fraction,
            dictionary,
        })
    }

    pub fn add_layer(&mut self,uuid: String,name: String,material: MatOrCompId,thickness: f64,priority: f64,joint_type: String,dictionary: Dictionary) -> Id {
        self.layers.insert(ConstructionLayer {
            uuid,
            name,
            material,
            thickness,
            priority,
            joint_type,
            dictionary,
        })
    }

    pub fn add_layer_group(&mut self,uuid: String,name: String,layers: Vec<Id>,layer_offsets: Vec<f64>) -> Id {
        self.layer_groups.insert(ConstructionLayerGroup {
            uuid,
            name,
            layers,
            layer_offsets,
        })
    }

    fn resolve_mat_or_comp( &self, uuid: &str, materials: &HashMap<String, Id>, composites: &HashMap<String, Id>, ) -> Result<MatOrCompId, String> {
        if let Some(&id) = materials.get(uuid) { 
            return Ok(MatOrCompId::Material(id)); 
        }
        if let Some(&id) = composites.get(uuid) { 
            return Ok(MatOrCompId::Composite(id)); 
        } 
        Err(format!("unknown material or composite: {}", uuid)) 
    }

    pub fn from_data(data: ConstructionSetData) -> Result<Self, String> {
        let mut construction_set = ConstructionSet::new();

        let mut materials = HashMap::new();
        let mut composites = HashMap::new();
        let mut layers = HashMap::new();
        let mut layer_groups = HashMap::new();

        // Materials
        for (uuid, material) in data.materials {
            let id = construction_set.add_material(
                uuid.clone(),
                material.name,
                material.density,
                material.heat_transfer_coefficient,
                material.specific_heat_capacity,
                material.dictionary,
            );

            materials.insert(uuid, id);
        }

        for (uuid, composite) in data.composites {
            let part1 = construction_set.resolve_mat_or_comp(
                &composite.part1,
                &materials,
                &composites,
            )?;

            let part2 = construction_set.resolve_mat_or_comp(
                &composite.part2,
                &materials,
                &composites,
            )?;

            let id = construction_set.add_composite(
                uuid.clone(),
                composite.name,
                part1,
                part2,
                composite.part1_fraction,
                composite.part2_fraction,
                composite.dictionary,
            );

            composites.insert(uuid, id);
        }

        for (uuid, layer) in data.layers {
            let material = construction_set.resolve_mat_or_comp(
                &layer.material,
                &materials,
                &composites,
            )?;

            let id = construction_set.add_layer(
                uuid.clone(),
                layer.name,
                material,
                layer.thickness,
                layer.priority,
                layer.joint_type,
                layer.dictionary,
            );

            layers.insert(uuid, id);
        }

        // Layer groups reference layers.
        for (uuid, group) in data.layer_groups {
            if group.layers.len() != group.layer_offsets.len() {
                return Err(format!(
                    "layer group {} has {} layers but {} layer offsets",
                    uuid,
                    group.layers.len(),
                    group.layer_offsets.len(),
                ));
            }

            let mut layer_ids = Vec::with_capacity(group.layers.len());

            for layer_uuid in group.layers {
                let layer_id = *layers
                    .get(&layer_uuid)
                    .ok_or_else(|| {
                        format!("unknown layer: {}", layer_uuid)
                    })?;

                layer_ids.push(layer_id);
            }

            let id = construction_set.add_layer_group(
                uuid.clone(),
                group.name,
                layer_ids,
                group.layer_offsets,
            );

            layer_groups.insert(uuid, id);
        }

        Ok(construction_set)
    }
    
    pub fn from_json(json: &str) -> Result<Self, String> {
        let data: ConstructionSetData = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Self::from_data(data)
    }

    pub fn layer_group_by_uuid(&self, uuid: &str) -> Option<Id> {
        self.layer_groups
            .iter_with_ids()
            .find(|(_, group)| group.uuid == uuid)
            .map(|(id, _)| id)
    }

    pub fn material_ids(&self) -> Vec<Id> {
        self.materials.ids().collect()
    }

    pub fn composite_ids(&self) -> Vec<Id> {
        self.composites.ids().collect()
    }

    pub fn layer_ids(&self) -> Vec<Id> {
        self.layers.ids().collect()
    }

    pub fn layer_group_ids(&self) -> Vec<Id> {
        self.layer_groups.ids().collect()
    }
}
