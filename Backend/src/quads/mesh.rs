use std::collections::HashMap;

use opengeometry::geometry::poly2d::{Pt2, cross2, dot2};

use super::polyline::{distance, point, resample, sub};
use super::types::{Patch, QuadMesh};
use super::union_find::UnionFind;
use crate::topology::ops::{QuadMeshOps, TopologyOps};

/// Transfinite (Coons) interpolation of a four-sided patch into an `(nu+1) x (nv+1)` grid.
pub fn coons(sides: &Patch, nu: usize, nv: usize) -> Vec<Vec<Pt2>> {
    let bottom = resample(&sides[0], nu);
    let right = resample(&sides[1], nv);
    let top: Vec<_> = resample(&sides[2], nu).into_iter().rev().collect();
    let left: Vec<_> = resample(&sides[3], nv).into_iter().rev().collect();
    let (c0, c1, c2, c3) = (bottom[0], bottom[nu], top[nu], top[0]);
    let mut grid = vec![vec![point(0.0, 0.0); nv + 1]; nu + 1];
    for j in 0..=nv {
        let v = j as f64 / nv as f64;
        for i in 0..=nu {
            let u = i as f64 / nu as f64;
            let mix = |f: fn(&Pt2) -> f64| {
                (1.0 - v) * f(&bottom[i]) + v * f(&top[i]) + (1.0 - u) * f(&left[j])
                    + u * f(&right[j])
                    - ((1.0 - u) * (1.0 - v) * f(&c0)
                        + u * (1.0 - v) * f(&c1)
                        + u * v * f(&c2)
                        + (1.0 - u) * v * f(&c3))
            };
            grid[i][j] = point(mix(|p| p.x), mix(|p| p.z));
        }
    }
    grid
}

/// Lightweight quad soup: shared vertex list plus quads indexing into it.
///
/// The whole meshing pipeline works on this type; the arena-backed
/// [`QuadMesh`] is only built once per result via [`Quads::to_mesh`].
#[derive(Clone, Debug, Default)]
pub struct Quads {
    pub pts: Vec<Pt2>,
    pub faces: Vec<[usize; 4]>,
}

impl Quads {
    pub fn from_grid(grid: &[Vec<Pt2>]) -> Self {
        let (nu, nv) = (grid.len() - 1, grid[0].len() - 1);
        let id = |i: usize, j: usize| i * (nv + 1) + j;
        let faces = (0..nu)
            .flat_map(|i| (0..nv).map(move |j| (i, j)))
            .map(|(i, j)| [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)])
            .collect();
        Self {
            pts: grid.iter().flatten().copied().collect(),
            faces,
        }
    }

    pub fn append(&mut self, other: &Quads) {
        let offset = self.pts.len();
        self.pts.extend_from_slice(&other.pts);
        self.faces
            .extend(other.faces.iter().map(|q| q.map(|v| v + offset)));
    }

    /// Edge vectors `(e1, e2)` leaving every quad corner towards its two neighbours.
    fn corner_frames(&self) -> impl Iterator<Item = (Pt2, Pt2)> + '_ {
        self.faces.iter().flat_map(move |q| {
            (0..4).map(move |k| {
                let p = self.pts[q[k]];
                (
                    sub(self.pts[q[(k + 1) % 4]], p),
                    sub(self.pts[q[(k + 3) % 4]], p),
                )
            })
        })
    }

    /// Smallest corner Jacobian determinant (negative or zero means inverted).
    pub fn min_det(&self) -> f64 {
        self.corner_frames()
            .map(|(e1, e2)| cross2(e1, e2))
            .fold(f64::INFINITY, f64::min)
    }

    /// Mean deviation of corner angles from 90°; infinite if any corner is inverted.
    pub fn alpha_error(&self) -> f64 {
        if self.faces.is_empty() {
            return f64::INFINITY;
        }
        let mut total = 0.0;
        for (e1, e2) in self.corner_frames() {
            let det = cross2(e1, e2);
            if det <= 0.0 {
                return f64::INFINITY;
            }
            total += (det.atan2(dot2(e1, e2)) - std::f64::consts::FRAC_PI_2).abs();
        }
        total / (4 * self.faces.len()) as f64
    }

    pub fn edge_table(&self) -> EdgeTable {
        let mut table = EdgeTable::default();
        for (fi, q) in self.faces.iter().enumerate() {
            for k in 0..4 {
                let (a, b) = (q[k], q[(k + 1) % 4]);
                let key = (a.min(b), a.max(b));
                let next = table.edges.len();
                let slot = *table.index.entry(key).or_insert(next);
                if slot == next {
                    table.edges.push((key, Vec::new()));
                }
                table.edges[slot].1.push(fi);
            }
        }
        table
    }

    /// Every edge borders at most two quads.
    pub fn is_manifold(&self) -> bool {
        self.edge_table().edges.iter().all(|(_, f)| f.len() <= 2)
    }

    /// Rebuild with each vertex replaced by `map(v)` and vertex positions `pts`;
    /// quads that lose a corner are dropped and unused vertices removed.
    pub fn merged(&self, pts: Vec<Pt2>, map: impl Fn(usize) -> usize) -> Self {
        let faces = self
            .faces
            .iter()
            .map(|q| q.map(&map))
            .filter(|q| (1..4).all(|i| !q[..i].contains(&q[i])))
            .collect();
        Self { pts, faces }.compressed()
    }

    /// Drop vertices not used by any quad, keeping order.
    pub fn compressed(&self) -> Self {
        let mut remap = vec![usize::MAX; self.pts.len()];
        for &v in self.faces.iter().flatten() {
            remap[v] = 0;
        }
        let mut pts = Vec::new();
        for (v, slot) in remap.iter_mut().enumerate() {
            if *slot == 0 {
                *slot = pts.len();
                pts.push(self.pts[v]);
            }
        }
        Self {
            pts,
            faces: self.faces.iter().map(|q| q.map(|v| remap[v])).collect(),
        }
    }

    /// Merge vertices closer than `tol` (the lowest index wins) and drop
    /// degenerate or duplicated quads. Unused vertices are kept.
    pub fn welded(&self, tol: f64) -> Self {
        let mut uf = UnionFind::new(self.pts.len());
        let mut cells = HashMap::<(i64, i64), Vec<usize>>::new();
        for (i, &p) in self.pts.iter().enumerate() {
            let cell = ((p.x / tol).floor() as i64, (p.z / tol).floor() as i64);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    for &j in cells.get(&(cell.0 + dx, cell.1 + dz)).into_iter().flatten() {
                        if distance(p, self.pts[j]) <= tol {
                            uf.join_min(j, i);
                        }
                    }
                }
            }
            cells.entry(cell).or_default().push(i);
        }
        let mut new_id = vec![usize::MAX; self.pts.len()];
        let mut pts = Vec::new();
        for i in 0..self.pts.len() {
            let root = uf.find(i);
            if new_id[root] == usize::MAX {
                new_id[root] = pts.len();
                pts.push(self.pts[root]);
            }
            new_id[i] = new_id[root];
        }
        let mut seen = std::collections::HashSet::new();
        let faces = self
            .faces
            .iter()
            .map(|q| q.map(|v| new_id[v]))
            .filter(|q| {
                let mut key = *q;
                key.sort_unstable();
                key.windows(2).all(|w| w[0] != w[1]) && seen.insert(key)
            })
            .collect();
        Self { pts, faces }
    }

    /// Build the arena-backed mesh.
    pub fn to_mesh(&self) -> QuadMesh {
        let mut mesh = QuadMesh::new();
        let ids: Vec<_> = self.pts.iter().map(|&p| mesh.add_mesh_vertex(p)).collect();
        for q in &self.faces {
            mesh.add_quad(q.map(|v| ids[v])).unwrap();
        }
        mesh
    }
}

/// Undirected edges (vertex pair, adjacent quads) in order of first appearance.
#[derive(Clone, Debug, Default)]
pub struct EdgeTable {
    pub edges: Vec<((usize, usize), Vec<usize>)>,
    index: HashMap<(usize, usize), usize>,
}

impl EdgeTable {
    pub fn adjacent(&self, edge: (usize, usize)) -> &[usize] {
        let key = (edge.0.min(edge.1), edge.0.max(edge.1));
        self.index
            .get(&key)
            .map_or(&[], |&i| self.edges[i].1.as_slice())
    }

    pub fn boundary_edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.edges
            .iter()
            .filter(|(_, f)| f.len() == 1)
            .map(|&(e, _)| e)
    }

    pub fn boundary_vertices(&self, n: usize) -> Vec<bool> {
        let mut on = vec![false; n];
        for (a, b) in self.boundary_edges() {
            on[a] = true;
            on[b] = true;
        }
        on
    }

    /// Boundary vertices that are not on a straight run (within `tol` of π).
    pub fn feature_vertices(&self, mesh: &Quads, tol: f64) -> Vec<bool> {
        let mut neighbors = vec![Vec::new(); mesh.pts.len()];
        for (a, b) in self.boundary_edges() {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
        neighbors
            .iter()
            .enumerate()
            .map(|(v, n)| match n.as_slice() {
                [] => false,
                &[a, b] => {
                    let p = mesh.pts[v];
                    let (u1, u2) = (sub(mesh.pts[a], p), sub(mesh.pts[b], p));
                    let cos = dot2(u1, u2)
                        / (u1.x.hypot(u1.z) * u2.x.hypot(u2.z) + 1e-300);
                    cos.clamp(-1.0, 1.0).acos() < std::f64::consts::PI - tol
                }
                _ => true,
            })
            .collect()
    }
}
