use openmaths::{Vector3};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use super::topology_cell::*;
use super::topology_edge::*;
use super::topology_face::*;
use super::topology_vert::*;
use crate::bim::arena::Arena;
use crate::bim::base_types::{Id, ConstructionRef, Dictionary, Frame};
use crate::bim::utils::{project_to_frame};

#[derive(Serialize, Deserialize, Debug)]
pub struct TopologyData {
    pub verts: HashMap<String, TopologyVertData>,
    pub edges: HashMap<String, TopologyEdgeData>,
    pub faces: HashMap<String, TopologyFaceData>,
    pub cells: HashMap<String, TopologyCellData>,
}

pub struct Topology { 
    pub(crate) verts: Arena<TopologyVert>, 
    pub(crate) edges: Arena<TopologyEdge>, 
    pub(crate) faces: Arena<TopologyFace>, 
    pub(crate) cells: Arena<TopologyCell>, 
}

impl Topology {
    pub fn new() -> Self {Self {verts: Arena::new(),edges: Arena::new(),faces: Arena::new(),cells: Arena::new()}}

    pub fn vert(&self, id: Id) -> Option<&TopologyVert> {self.verts.get(id)}
    
    pub fn edge(&self, id: Id) -> Option<&TopologyEdge> {self.edges.get(id)}
    
    pub fn face(&self, id: Id) -> Option<&TopologyFace> {self.faces.get(id)}

    pub fn cell(&self, id: Id) -> Option<&TopologyCell> {self.cells.get(id)}
    
    pub fn add_vert(&mut self,uuid: String,coords: Vector3,dictionary: Dictionary,   ) -> Id {
        self.verts.insert(TopologyVert {
            uuid,
            coords,
            edges: Vec::new(),
            dictionary,
        })
    }

    pub fn add_edge(&mut self,uuid: String,dictionary: Dictionary,) -> Id {
        self.edges.insert(TopologyEdge {
            uuid,
            verts: Vec::new(),
            faces: Vec::new(),
            dictionary,
        })
    }

    pub fn add_face(&mut self,uuid: String,construction: ConstructionRef,face_type: String,frame: Frame,dictionary: Dictionary,) -> Id {
        self.faces.insert(TopologyFace {
            uuid,
            verts: Vec::new(),
            edges: Vec::new(),
            cells: Vec::new(),
            construction,
            layer_group_id: None,
            joint_materials: Vec::new(),
            inner_edge_offsets: Vec::new(),
            outer_edge_offsets: Vec::new(),
            face_type,
            frame,
            dictionary,
        })
    }

    pub fn add_cell(&mut self,uuid: String,dictionary: Dictionary,) -> Id {
        self.cells.insert(TopologyCell {
            uuid,
            faces: Vec::new(),
            dictionary,
        })
    }

    pub fn connect_edge_vert(&mut self,edge_id: Id,vert_id: Id,) -> Result<(), &'static str> {
        let edge = self.edges.get_mut(edge_id).ok_or("invalid edge")?;

        if !edge.verts.contains(&vert_id) {
            edge.verts.push(vert_id);
        }

        let vert = self.verts.get_mut(vert_id)
            .ok_or("invalid vert")?;

        if !vert.edges.contains(&edge_id) {
            vert.edges.push(edge_id);
        }

        Ok(())
    }

    pub fn connect_face_edge(&mut self,face_id: Id,edge_id: Id,) -> Result<(), &'static str> {

        let face = self.faces.get_mut(face_id)
            .ok_or("invalid face")?;

        if !face.edges.contains(&edge_id) {
            face.edges.push(edge_id);
        }

        let edge = self.edges.get_mut(edge_id)
            .ok_or("invalid edge")?;

        if !edge.faces.contains(&face_id) {
            edge.faces.push(face_id);
        }

        Ok(())
    }

    pub fn connect_face_vert(&mut self,face_id: Id,vert_id: Id,) -> Result<(), &'static str> {
        let face = self.faces.get_mut(face_id)
            .ok_or("invalid face")?;

        if !face.verts.contains(&vert_id) {
            face.verts.push(vert_id);
        }
        Ok(())
    }
    
    pub fn connect_cell_face(&mut self,cell_id: Id,face_id: Id,) -> Result<(), &'static str> {
        let cell = self.cells.get_mut(cell_id).ok_or("invalid cell")?;

        if !cell.faces.contains(&face_id) {cell.faces.push(face_id);}

        let face = self.faces.get_mut(face_id).ok_or("invalid face")?;

        if !face.cells.contains(&cell_id) {face.cells.push(cell_id);}

        Ok(())
    }

    pub fn from_data(data: TopologyData) -> Result<Self, String> {
        let mut topology = Topology::new();

        let mut verts = std::collections::HashMap::new();
        let mut edges = std::collections::HashMap::new();
        let mut faces = std::collections::HashMap::new();
        let mut cells = std::collections::HashMap::new();

        for (uuid, vert) in data.verts {
            let id = topology.add_vert(
                uuid.clone(),
                Vector3::new(vert.coords[0], vert.coords[1], vert.coords[2]),
                vert.dictionary,
            );
            
            verts.insert(uuid, id);
        }

        for (uuid, edge) in data.edges {
            let id = topology.add_edge(
                uuid.clone(),
                edge.dictionary,
            );

            edges.insert(uuid.clone(), id);

            for vert_uuid in edge.verts {
                let vert_id = *verts
                    .get(&vert_uuid)
                    .ok_or_else(|| format!("unknown vertex: {}", vert_uuid))?;

                topology.connect_edge_vert(id, vert_id)?;
            }
        }

        for (uuid, face) in data.faces {
            let uvn = [
                Vector3::new(face.uvn[0][0], face.uvn[0][1], face.uvn[0][2]),
                Vector3::new(face.uvn[1][0], face.uvn[1][1], face.uvn[1][2]),
                Vector3::new(face.uvn[2][0], face.uvn[2][1], face.uvn[2][2]),
            ];

            let vert0 = topology.vert(*verts.get(&face.verts[0]).ok_or_else(|| format!("unknown vertex: {}", face.verts[0]))?).unwrap();
            let origin = vert0.coords;
            
            let id = topology.add_face(
                uuid.clone(),
                ConstructionRef::Uuid(face.construction),
                face.face_type,
                Frame::new(origin,uvn[2],uvn[0]),
                face.dictionary,
            );

            faces.insert(uuid.clone(), id);

            for edge_uuid in face.edges {
                let edge_id = *edges
                    .get(&edge_uuid)
                    .ok_or_else(|| format!("unknown edge: {}", edge_uuid))?;

                topology.connect_face_edge(id, edge_id)?;
            }

            for vert_uuid in face.verts {
                let vert_id = *verts
                    .get(&vert_uuid)
                    .ok_or_else(|| format!("unknown vertex: {}", vert_uuid))?;

                topology.connect_face_vert(id, vert_id)?;
            }
        }

        for (uuid, cell) in data.cells {
            let id = topology.add_cell(uuid.clone(),cell.dictionary,);
            cells.insert(uuid.clone(), id);
            for face_uuid in cell.faces {
                let face_id = *faces
                    .get(&face_uuid)
                    .ok_or_else(|| format!("unknown face: {}", face_uuid))?;

                topology.connect_cell_face(id, face_id)?;
            }
        }

        Ok(topology)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let data: TopologyData =
            serde_json::from_str(json)
                .map_err(|e| e.to_string())?;

        Self::from_data(data)
    }
        
    pub fn face_coords_2d(&self, face: &TopologyFace) -> Vec<(f64, f64)> {
        face.verts
            .iter()
            .map(|vert_id| {
                let point = &self
                    .vert(*vert_id)
                    .expect("face_coords_2d vertex not found")
                    .coords;

                project_to_frame(&face.frame, point)
            })
            .collect()
    }

    pub fn face_coords(&self, face: &TopologyFace) -> Vec<Vector3> {
        face.verts
            .iter()
            .map(|vert_id| {
                let coords = self
                    .vert(*vert_id)
                    .expect("face_coords vertex not found")
                    .coords;

                Vector3::new(coords.x, coords.z, coords.y)
            })
            .collect()
    }

    pub fn vert_ids(&self) -> Vec<Id>{
        self.verts.ids().collect()
    }

    pub fn edge_ids(&self) -> Vec<Id>{
        self.edges.ids().collect()
    }

    pub fn face_ids(&self) -> Vec<Id>{
        self.faces.ids().collect()
    }

    pub fn cell_ids(&self) -> Vec<Id>{
        self.cells.ids().collect()
    }

}
