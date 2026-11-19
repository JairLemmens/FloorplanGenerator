use crate::topology::ops::{QuadMeshOps, TopologyOps};
use std::collections::{HashMap, HashSet};

use crate::base_types::Id;
use opengeometry::geometry::poly2d::Pt2;

use super::polyline::{distance, point, resample};
use super::types::{Patch, QuadMesh};

pub fn coons(sides: &Patch, nu: usize, nv: usize) -> Vec<Vec<Pt2>> {
    let bottom = resample(&sides[0], nu);
    let right = resample(&sides[1], nv);
    let top: Vec<_> = resample(&sides[2], nu).into_iter().rev().collect();
    let left: Vec<_> = resample(&sides[3], nv).into_iter().rev().collect();
    let c0 = bottom[0];
    let c1 = bottom[nu];
    let c2 = top[nu];
    let c3 = top[0];
    let mut grid = vec![vec![point(0.0, 0.0); nv + 1]; nu + 1];
    for j in 0..=nv {
        let v = j as f64 / nv as f64;
        for i in 0..=nu {
            let u = i as f64 / nu as f64;
            let p0 = bottom[i];
            let p1 = top[i];
            let p2 = left[j];
            let p3 = right[j];
            let bilinear = point(
                (1.0 - u) * (1.0 - v) * c0.x
                    + u * (1.0 - v) * c1.x
                    + u * v * c2.x
                    + (1.0 - u) * v * c3.x,
                (1.0 - u) * (1.0 - v) * c0.z
                    + u * (1.0 - v) * c1.z
                    + u * v * c2.z
                    + (1.0 - u) * v * c3.z,
            );
            grid[i][j] = point(
                (1.0 - v) * p0.x + v * p1.x + (1.0 - u) * p2.x + u * p3.x - bilinear.x,
                (1.0 - v) * p0.z + v * p1.z + (1.0 - u) * p2.z + u * p3.z - bilinear.z,
            );
        }
    }
    grid
}

pub fn grid_to_mesh(grid: &[Vec<Pt2>]) -> QuadMesh {
    let nu = grid.len() - 1;
    let nv = grid[0].len() - 1;
    let mut mesh = QuadMesh::new();
    let ids: Vec<Vec<_>> = grid
        .iter()
        .map(|row| row.iter().map(|&p| mesh.add_mesh_vertex(p)).collect())
        .collect();
    for i in 0..nu {
        for j in 0..nv {
            mesh.add_quad([ids[i][j], ids[i + 1][j], ids[i + 1][j + 1], ids[i][j + 1]])
                .unwrap();
        }
    }
    mesh
}

pub fn corner_dets(mesh: &QuadMesh) -> Vec<f64> {
    let mut out = Vec::with_capacity(mesh.face_count() * 4);
    for (_, q) in mesh.faces() {
        for k in 0..4 {
            let p = mesh.point(q[k]).unwrap();
            let a = mesh.point(q[(k + 1) % 4]).unwrap();
            let b = mesh.point(q[(k + 3) % 4]).unwrap();
            out.push((a.x - p.x) * (b.z - p.z) - (a.z - p.z) * (b.x - p.x));
        }
    }
    out
}

pub fn min_det(mesh: &QuadMesh) -> f64 {
    corner_dets(mesh).into_iter().fold(f64::INFINITY, f64::min)
}

pub fn alpha_error(mesh: &QuadMesh) -> f64 {
    if mesh.face_count() == 0 {
        return f64::INFINITY;
    }
    let mut total = 0.0;
    for (_, q) in mesh.faces() {
        for k in 0..4 {
            let p = mesh.point(q[k]).unwrap();
            let a = mesh.point(q[(k + 1) % 4]).unwrap();
            let b = mesh.point(q[(k + 3) % 4]).unwrap();
            let e1 = [a.x - p.x, a.z - p.z];
            let e2 = [b.x - p.x, b.z - p.z];
            let det = e1[0] * e2[1] - e1[1] * e2[0];
            if det <= 0.0 {
                return f64::INFINITY;
            }
            let angle = det.atan2(e1[0] * e2[0] + e1[1] * e2[1]);
            total += (angle - std::f64::consts::FRAC_PI_2).abs();
        }
    }
    total / (4 * mesh.face_count()) as f64
}

#[derive(Clone, Debug)]
pub struct EdgeMap {
    pub edges: Vec<((Id, Id), Vec<Id>)>,
    index: HashMap<(Id, Id), usize>,
}

impl EdgeMap {
    pub(crate) fn from_edges(edges: Vec<((Id, Id), Vec<Id>)>) -> Self {
        let index = edges
            .iter()
            .enumerate()
            .map(|(i, (edge, _))| (*edge, i))
            .collect();
        Self { edges, index }
    }

    pub fn new(mesh: &QuadMesh) -> Self {
        Self::from_edges(
            mesh.edges()
                .map(|(_, verts, faces)| {
                    (
                        (verts[0].min(verts[1]), verts[0].max(verts[1])),
                        faces.to_vec(),
                    )
                })
                .collect(),
        )
    }

    pub fn adjacent(&self, edge: (Id, Id)) -> &[Id] {
        let key = (edge.0.min(edge.1), edge.0.max(edge.1));
        self.index
            .get(&key)
            .map_or(&[], |&i| self.edges[i].1.as_slice())
    }
}

pub fn boundary_info(mesh: &QuadMesh) -> (EdgeMap, HashMap<Id, bool>) {
    let edges = mesh.edge_map();
    let mut boundary = HashMap::new();
    for &((a, b), ref faces) in &edges.edges {
        if faces.len() == 1 {
            boundary.insert(a, true);
            boundary.insert(b, true);
        }
    }
    for id in mesh.vertex_ids() {
        boundary.entry(id).or_insert(false);
    }
    (edges, boundary)
}

pub fn feature_vertices(mesh: &QuadMesh, edge_map: &EdgeMap, tol: f64) -> HashMap<Id, bool> {
    let mut neighbors = HashMap::<Id, Vec<Id>>::new();
    for &((a, b), ref faces) in &edge_map.edges {
        if faces.len() == 1 {
            neighbors.entry(a).or_default().push(b);
            neighbors.entry(b).or_default().push(a);
        }
    }
    let mut feat = HashMap::new();
    for v in mesh.vertex_ids() {
        let n = neighbors.get(&v).map(Vec::as_slice).unwrap_or(&[]);
        if n.is_empty() {
            feat.insert(v, false);
            continue;
        }
        if n.len() != 2 {
            feat.insert(v, true);
            continue;
        }
        let a = mesh.point(n[0]).unwrap();
        let b = mesh.point(n[1]).unwrap();
        let p = mesh.point(v).unwrap();
        let u1 = [a.x - p.x, a.z - p.z];
        let u2 = [b.x - p.x, b.z - p.z];
        let dot =
            (u1[0] * u2[0] + u1[1] * u2[1]) / ((u1[0].hypot(u1[1]) * u2[0].hypot(u2[1])) + 1e-300);
        if dot.clamp(-1.0, 1.0).acos() < std::f64::consts::PI - tol {
            feat.insert(v, true);
        } else {
            feat.insert(v, false);
        }
    }
    feat
}

pub fn compress(mesh: &QuadMesh) -> QuadMesh {
    let used: HashSet<_> = mesh.faces().flat_map(|(_, f)| f.iter().copied()).collect();
    let mut out = QuadMesh::new();
    let mut remap = HashMap::new();
    for old in mesh.vertex_ids().into_iter().filter(|id| used.contains(id)) {
        remap.insert(old, out.add_mesh_vertex(mesh.point(old).unwrap()));
    }
    for (_, f) in mesh.faces() {
        let q: [Id; 4] = f.try_into().unwrap();
        out.add_quad(q.map(|id| remap[&id])).unwrap();
    }
    out
}

pub fn remove_duplicate_quads(mesh: QuadMesh) -> QuadMesh {
    let mut seen = HashSet::new();
    let mut out = QuadMesh::new();
    let mut remap = HashMap::new();
    for id in mesh.vertex_ids() {
        let new = out.add_mesh_vertex(mesh.point(id).unwrap());
        remap.insert(id, new);
    }
    for (_, q) in mesh.faces() {
        let mut key = q.to_vec();
        key.sort_unstable();
        if key.windows(2).all(|w| w[0] != w[1]) && seen.insert(key) {
            let face: [Id; 4] = q.try_into().unwrap();
            out.add_quad(face.map(|id| remap[&id])).unwrap();
        }
    }
    out
}

pub fn edge_len(mesh: &QuadMesh, e: (Id, Id)) -> f64 {
    distance(mesh.point(e.0).unwrap(), mesh.point(e.1).unwrap())
}
