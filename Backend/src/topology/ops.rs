use std::collections::{HashMap, HashSet};

use opengeometry::geometry::poly2d::Pt2;
use openmaths::Vector3;

use crate::arena::Arena;
use crate::base_types::{ConstructionRef, Dictionary, Frame, Id};
use crate::quads::types::{QParams, Region};
use super::topology::{QuadMesh, Topology, TopologyData};
use super::topology_cell::TopologyCell;
use super::topology_edge::TopologyEdge;
use super::topology_face::TopologyFace;
use super::topology_vert::TopologyVert;

/// General topology operations. Implementors provide only access to their
/// topology storage; all common behavior is defined here.
pub trait TopologyOps: Sized {
    fn topology(&self) -> &Topology;
    fn topology_mut(&mut self) -> &mut Topology;

    fn new() -> Self {
        Self::from_topology(Topology { verts: Arena::new(), edges: Arena::new(), faces: Arena::new(), cells: Arena::new() })
    }

    fn from_topology(topology: Topology) -> Self;
    fn into_topology(self) -> Topology;

    fn vert(&self, id: Id) -> Option<&TopologyVert> { self.topology().verts.get(id) }
    fn edge(&self, id: Id) -> Option<&TopologyEdge> { self.topology().edges.get(id) }
    fn face(&self, id: Id) -> Option<&TopologyFace> { self.topology().faces.get(id) }
    fn cell(&self, id: Id) -> Option<&TopologyCell> { self.topology().cells.get(id) }

    fn vert_arena(&self) -> &Arena<TopologyVert> { &self.topology().verts }
    fn edge_arena(&self) -> &Arena<TopologyEdge> { &self.topology().edges }
    fn face_arena(&self) -> &Arena<TopologyFace> { &self.topology().faces }
    fn cell_arena(&self) -> &Arena<TopologyCell> { &self.topology().cells }

    fn vert_edge_ids(&self, id: Id) -> Option<&[Id]> { self.vert(id).map(|v| v.edges.as_slice()) }
    fn edge_vert_ids(&self, id: Id) -> Option<&[Id]> { self.edge(id).map(|e| e.verts.as_slice()) }
    fn edge_face_ids(&self, id: Id) -> Option<&[Id]> { self.edge(id).map(|e| e.faces.as_slice()) }
    fn face_vert_ids(&self, id: Id) -> Option<&[Id]> { self.face(id).map(|f| f.verts.as_slice()) }
    fn face_edge_ids(&self, id: Id) -> Option<&[Id]> { self.face(id).map(|f| f.edges.as_slice()) }

    fn add_vertex(&mut self, uuid: String, coords: Vector3, dictionary: Dictionary) -> Id {
        self.topology_mut().verts.insert(TopologyVert { uuid, coords, edges: Vec::new(), dictionary })
    }
    fn add_edge(&mut self, uuid: String, dictionary: Dictionary) -> Id {
        self.topology_mut().edges.insert(TopologyEdge { uuid, verts: Vec::new(), faces: Vec::new(), dictionary })
    }
    fn add_face(&mut self, uuid: String, construction: ConstructionRef, face_type: String, frame: Frame, dictionary: Dictionary) -> Id {
        self.topology_mut().faces.insert(TopologyFace {
            uuid, verts: Vec::new(), edges: Vec::new(), cells: Vec::new(), construction,
            layer_group_id: None, joints: Vec::new(), inner_edge_offsets: Vec::new(),
            outer_edge_offsets: Vec::new(), face_type, frame, dictionary,
        })
    }
    fn add_cell(&mut self, uuid: String, dictionary: Dictionary) -> Id {
        self.topology_mut().cells.insert(TopologyCell { uuid, faces: Vec::new(), dictionary })
    }

    fn connect_edge_vert(&mut self, edge_id: Id, vert_id: Id) -> Result<(), &'static str> {
        let topology = self.topology_mut();
        let edge = topology.edges.get_mut(edge_id).ok_or("invalid edge")?;
        if !edge.verts.contains(&vert_id) { edge.verts.push(vert_id); }
        let vert = topology.verts.get_mut(vert_id).ok_or("invalid vert")?;
        if !vert.edges.contains(&edge_id) { vert.edges.push(edge_id); }
        Ok(())
    }
    fn connect_face_edge(&mut self, face_id: Id, edge_id: Id) -> Result<(), &'static str> {
        let topology = self.topology_mut();
        let face = topology.faces.get_mut(face_id).ok_or("invalid face")?;
        if !face.edges.contains(&edge_id) { face.edges.push(edge_id); }
        let edge = topology.edges.get_mut(edge_id).ok_or("invalid edge")?;
        if !edge.faces.contains(&face_id) { edge.faces.push(face_id); }
        Ok(())
    }
    fn connect_face_vert(&mut self, face_id: Id, vert_id: Id) -> Result<(), &'static str> {
        let face = self.topology_mut().faces.get_mut(face_id).ok_or("invalid face")?;
        if !face.verts.contains(&vert_id) { face.verts.push(vert_id); }
        Ok(())
    }
    fn connect_cell_face(&mut self, cell_id: Id, face_id: Id) -> Result<(), &'static str> {
        let topology = self.topology_mut();
        let cell = topology.cells.get_mut(cell_id).ok_or("invalid cell")?;
        if !cell.faces.contains(&face_id) { cell.faces.push(face_id); }
        let face = topology.faces.get_mut(face_id).ok_or("invalid face")?;
        if !face.cells.contains(&cell_id) { face.cells.push(cell_id); }
        Ok(())
    }

    fn vert_ids(&self) -> Vec<Id> { self.topology().verts.ids().collect() }
    fn edge_ids(&self) -> Vec<Id> { self.topology().edges.ids().collect() }
    fn face_ids(&self) -> Vec<Id> { self.topology().faces.ids().collect() }
    fn cell_ids(&self) -> Vec<Id> { self.topology().cells.ids().collect() }

    fn vertex(&self, id: Id) -> Option<&TopologyVert> { self.vert(id) }
    fn vertex_ids(&self) -> Vec<Id> { self.vert_ids() }
    fn vertices(&self) -> impl Iterator<Item = (Id, &TopologyVert)> { self.vertex_ids().into_iter().map(|id| (id, self.vertex(id).unwrap())) }
    fn faces(&self) -> impl Iterator<Item = (Id, &[Id])> { self.face_ids().into_iter().map(|id| (id, self.face_vert_ids(id).unwrap())) }
    fn edges(&self) -> impl Iterator<Item = (Id, &[Id], &[Id])> {
        self.edge_ids().into_iter().map(|id| (id, self.edge_vert_ids(id).unwrap(), self.edge_face_ids(id).unwrap()))
    }
    fn face_vertices(&self, id: Id) -> Option<&[Id]> { self.face_vert_ids(id) }
    fn edge_vertices(&self, id: Id) -> Option<&[Id]> { self.edge_vert_ids(id) }
    fn edge_faces(&self, id: Id) -> &[Id] { self.edge_face_ids(id).unwrap_or(&[]) }

    /// Returns the unique faces that share an edge with `face_id`.
    /// Returns `None` when `face_id` does not exist.
    fn face_neighbors(&self, face_id: Id) -> Option<Vec<Id>> {
        let edges = self.face_edge_ids(face_id)?;
        let mut neighbors = Vec::new();
        let mut seen = HashSet::new();
        for &edge_id in edges {
            if let Some(faces) = self.edge_face_ids(edge_id) {
                for &neighbor_id in faces {
                    if neighbor_id != face_id && seen.insert(neighbor_id) {
                        neighbors.push(neighbor_id);
                    }
                }
            }
        }
        Some(neighbors)
    }

    fn vertex_count(&self) -> usize { self.topology().verts.ids().count() }
    fn edge_count(&self) -> usize { self.topology().edges.ids().count() }
    fn face_count(&self) -> usize { self.topology().faces.ids().count() }

    fn face_coords_2d(&self, face: &TopologyFace) -> Vec<Pt2> {
        face.verts.iter().map(|&id| crate::utils::project_to_frame(&face.frame, &self.vert(id).expect("face_coords_2d vertex not found").coords)).collect()
    }
    fn face_coords(&self, face: &TopologyFace) -> Vec<Vector3> {
        face.verts.iter().map(|&id| { let p = self.vert(id).expect("face_coords vertex not found").coords; Vector3::new(p.x, p.z, p.y) }).collect()
    }
    fn remove_face(&mut self, id: Id) -> Result<(), String> {
        let topology = self.topology_mut();
        let face = topology.faces.remove(id).ok_or_else(|| format!("invalid face {id:?}"))?;
        for edge in face.edges { if let Some(edge) = topology.edges.get_mut(edge) { edge.faces.retain(|&f| f != id); } }
        for cell in face.cells { if let Some(cell) = topology.cells.get_mut(cell) { cell.faces.retain(|&f| f != id); } }
        Ok(())
    }

    fn from_data(data: TopologyData) -> Result<Self, String> {
        let mut topology = Self::new();
        let mut verts = HashMap::new();
        let mut edges = HashMap::new();
        let mut faces = HashMap::new();
        let mut cells = HashMap::new();
        for (uuid, vert) in data.verts {
            let id = topology.add_vertex(uuid.clone(), vert.coords, vert.dictionary);
            verts.insert(uuid, id);
        }
        for (uuid, edge) in data.edges {
            let id = topology.add_edge(uuid.clone(), edge.dictionary);
            edges.insert(uuid, id);
            for vert_uuid in edge.verts {
                let vert_id = *verts.get(&vert_uuid).ok_or_else(|| format!("unknown vertex: {vert_uuid}"))?;
                topology.connect_edge_vert(id, vert_id)?;
            }
        }
        for (uuid, face) in data.faces {
            let uvn = [face.uvn[0], face.uvn[1], face.uvn[2]];
            let first = *verts.get(&face.verts[0]).ok_or_else(|| format!("unknown vertex: {}", face.verts[0]))?;
            let origin = topology.vert(first).unwrap().coords;
            let id = topology.add_face(uuid.clone(), ConstructionRef::Uuid(face.construction), face.face_type, Frame::new(origin, uvn[2], uvn[0]), face.dictionary);
            faces.insert(uuid, id);
            for edge_uuid in face.edges {
                let edge_id = *edges.get(&edge_uuid).ok_or_else(|| format!("unknown edge: {edge_uuid}"))?;
                topology.connect_face_edge(id, edge_id)?;
            }
            for vert_uuid in face.verts {
                let vert_id = *verts.get(&vert_uuid).ok_or_else(|| format!("unknown vertex: {vert_uuid}"))?;
                topology.connect_face_vert(id, vert_id)?;
            }
        }
        for (uuid, cell) in data.cells {
            let id = topology.add_cell(uuid.clone(), cell.dictionary);
            cells.insert(uuid, id);
            for face_uuid in cell.faces {
                let face_id = *faces.get(&face_uuid).ok_or_else(|| format!("unknown face: {face_uuid}"))?;
                topology.connect_cell_face(id, face_id)?;
            }
        }
        Ok(topology)
    }
    fn from_json(json: &str) -> Result<Self, String> {
        let data = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Self::from_data(data)
    }
}

impl TopologyOps for Topology {
    fn topology(&self) -> &Topology { self }
    fn topology_mut(&mut self) -> &mut Topology { self }
    fn from_topology(topology: Topology) -> Self { topology }
    fn into_topology(self) -> Topology { self }
}

/// Quad-specific operations layered directly on the topology value.
pub trait QuadMeshOps: TopologyOps {
    fn point(&self, id: Id) -> Option<Pt2> { self.vertex(id).map(|v| Pt2::new(v.coords.x, v.coords.z)) }

    fn add_mesh_vertex(&mut self, point: Pt2) -> Id {
        let mut n = self.vertex_count();
        let uuid = loop { let s = format!("mesh-vertex-{n}"); if !self.topology().verts.iter().any(|v| v.uuid == s) { break s; } n += 1; };
        self.add_vertex(uuid, Vector3::new(point.x, 0.0, point.z), Dictionary::new())
    }

    fn add_quad(&mut self, verts: [Id; 4]) -> Result<Id, String> {
        if verts.into_iter().collect::<HashSet<_>>().len() != 4 { return Err("quad must contain four distinct vertices".into()); }
        if verts.iter().any(|&id| self.vert(id).is_none()) { return Err("quad references an invalid vertex".into()); }
        let origin = self.vert(verts[0]).unwrap().coords;
        let frame = Frame::from_origin_normal(origin, Vector3::new(0.0, 1.0, 0.0));
        let mut n = self.face_count();
        let uuid = loop { let s = format!("mesh-face-{n}"); if !self.topology().faces.iter().any(|f| f.uuid == s) { break s; } n += 1; };
        let face = self.add_face(uuid, ConstructionRef::Uuid("quad-mesh".into()), "quad".into(), frame, Dictionary::new());
        for i in 0..4 {
            self.connect_face_vert(face, verts[i]).map_err(str::to_owned)?;
            let (a, b) = (verts[i], verts[(i + 1) % 4]);
            let edge = self.edge_ids().into_iter().find(|&id| self.edge_vert_ids(id).is_some_and(|vs| vs.len() == 2 && ((vs[0] == a && vs[1] == b) || (vs[0] == b && vs[1] == a)))).unwrap_or_else(|| {
                let mut n = self.edge_count();
                let uuid = loop { let s = format!("mesh-edge-{n}"); if !self.topology().edges.iter().any(|e| e.uuid == s) { break s; } n += 1; };
                let id = self.add_edge(uuid, Dictionary::new());
                self.connect_edge_vert(id, a).unwrap(); self.connect_edge_vert(id, b).unwrap(); id
            });
            self.connect_face_edge(face, edge).map_err(str::to_owned)?;
        }
        Ok(face)
    }

    fn validate(&self) -> Result<(), String> {
        for id in self.face_ids() {
            let v = self.face_vertices(id).ok_or_else(|| format!("missing face {id:?}"))?;
            let e = self.face_edge_ids(id).ok_or_else(|| format!("missing face edges {id:?}"))?;
            if v.len() != 4 || e.len() != 4 { return Err(format!("quad face {id:?} must have four vertices and edges")); }
            for i in 0..4 {
                let (a, b) = (v[i], v[(i + 1) % 4]);
                let edge = self.edge_vert_ids(e[i]).ok_or_else(|| format!("face {id:?} references stale edge"))?;
                if edge.len() != 2 || !((edge[0] == a && edge[1] == b) || (edge[0] == b && edge[1] == a)) { return Err(format!("face {id:?} edge order is inconsistent")); }
                if !self.edge_face_ids(e[i]).is_some_and(|fs| fs.contains(&id)) { return Err(format!("face {id:?} is not linked from edge")); }
            }
        }
        for id in self.edge_ids() {
            let verts = self.edge_vert_ids(id).ok_or_else(|| format!("missing edge {id:?}"))?;
            let faces = self.edge_faces(id);
            if verts.len() != 2 || faces.len() > 2 { return Err(format!("invalid edge connectivity {id:?}")); }
            for &vert in verts { if !self.vert_edge_ids(vert).is_some_and(|es| es.contains(&id)) { return Err(format!("edge {id:?} is not linked from vertex")); } }
            for &face in faces { if !self.face_edge_ids(face).is_some_and(|es| es.contains(&id)) { return Err(format!("edge {id:?} is not linked from face")); } }
        }
        for id in self.vertex_ids() { for edge in self.vert_edge_ids(id).unwrap_or(&[]) { if !self.edge_vert_ids(*edge).is_some_and(|vs| vs.contains(&id)) { return Err(format!("vertex {id:?} has stale edge link")); } } }
        Ok(())
    }

    fn append(&mut self, other: &QuadMesh) -> HashMap<Id, Id> {
        let remap: HashMap<_, _> = other.vertices().map(|(id, v)| (id, self.add_mesh_vertex(Pt2::new(v.coords.x, v.coords.z)))).collect();
        for (_, face) in other.faces() { let q: [Id; 4] = face.try_into().unwrap(); self.add_quad(q.map(|id| remap[&id])).unwrap(); }
        remap
    }
    fn edge_map(&self) -> crate::quads::mesh::EdgeMap { crate::quads::mesh::EdgeMap::new(self.topology()) }
    fn boundary_info(&self) -> (crate::quads::mesh::EdgeMap, HashMap<Id, bool>) { crate::quads::mesh::boundary_info(self.topology()) }
    fn feature_vertices(&self, edges: &crate::quads::mesh::EdgeMap, tolerance: f64) -> HashMap<Id, bool> { crate::quads::mesh::feature_vertices(self.topology(), edges, tolerance) }
    fn compress(&self) -> QuadMesh { crate::quads::mesh::compress(self.topology()) }
    fn remove_duplicate_faces(self) -> QuadMesh { crate::quads::mesh::remove_duplicate_quads(self.into_topology()) }
    fn min_jacobian_determinant(&self) -> f64 { crate::quads::mesh::min_det(self.topology()) }
    fn alpha_error(&self) -> f64 { crate::quads::mesh::alpha_error(self.topology()) }
    fn is_valid_connectivity(&self) -> bool { self.validate().is_ok() }
    fn simplify(self, region: &Region, params: &QParams) -> QuadMesh { crate::quads::simplify::simplify(self.into_topology(), region, params) }
}

impl QuadMeshOps for Topology {}

impl std::fmt::Debug for Topology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuadMesh").field("vertices", &self.vertex_count()).field("edges", &self.edge_count()).field("faces", &self.face_count()).finish()
    }
}
