use super::geom::hausdorff_mesh_boundary;
use super::mesh::{EdgeMap, compress, min_det};
use super::polyline::{distance, point};
use super::types::{QParams, QuadMesh, Region};
use super::union_find::IdUnionFind;
use crate::base_types::Id;
use crate::topology::ops::{QuadMeshOps, TopologyOps};
use std::collections::{HashMap, HashSet};

fn edge_key(a: Id, b: Id) -> (Id, Id) {
    (a.min(b), a.max(b))
}

fn trace_chord(
    mesh: &QuadMesh,
    edges: &EdgeMap,
    start: (Id, Id),
) -> Option<(Vec<(Id, Id)>, Vec<Id>)> {
    let mut rungs = vec![start];
    let mut faces = Vec::new();
    let mut current = start;
    let mut face = *edges.adjacent(start).first()?;
    loop {
        if faces.contains(&face) {
            return None;
        }
        faces.push(face);
        let vs = mesh.face_vertices(face)?;
        let k = (0..4).find(|&k| edge_key(vs[k], vs[(k + 1) % 4]) == current)?;
        let opposite = edge_key(vs[(k + 2) % 4], vs[(k + 3) % 4]);
        rungs.push(opposite);
        let next = edges
            .adjacent(opposite)
            .iter()
            .copied()
            .find(|&f| f != face);
        let Some(next) = next else {
            return Some((rungs, faces));
        };
        face = next;
        current = opposite;
    }
}

#[derive(Clone)]
struct Chord {
    width: f64,
    rungs: Vec<(Id, Id)>,
    faces: Vec<Id>,
}

fn open_chords(mesh: &QuadMesh) -> Vec<Chord> {
    let edges = mesh.edge_map();
    let mut seen = HashSet::<Vec<Id>>::new();
    let mut out = Vec::new();
    for (_edge_id, verts, faces) in mesh.edges() {
        if faces.len() != 1 {
            continue;
        }
        let start = edge_key(verts[0], verts[1]);
        let Some((rungs, faces)) = trace_chord(mesh, &edges, start) else {
            continue;
        };
        let mut key = faces.clone();
        key.sort_unstable();
        if !seen.insert(key) {
            continue;
        }
        let width = rungs
            .iter()
            .map(|&(a, b)| distance(mesh.point(a).unwrap(), mesh.point(b).unwrap()))
            .sum::<f64>()
            / rungs.len() as f64;
        out.push(Chord {
            width,
            rungs,
            faces,
        });
    }
    out
}

fn collapse_chord(
    mesh: &QuadMesh,
    rungs: &[(Id, Id)],
    boundary: &HashMap<Id, bool>,
    feature: &HashMap<Id, bool>,
) -> Option<QuadMesh> {
    let mut uf = IdUnionFind::new(mesh.vertex_ids());
    let mut touched = HashSet::new();
    for &(a, b) in rungs {
        if feature[&a] && feature[&b] {
            return None;
        }
        uf.join(a, b);
        touched.insert(a);
        touched.insert(b);
    }
    let mut groups = HashMap::<Id, Vec<Id>>::new();
    for v in touched {
        groups.entry(uf.find(v)).or_default().push(v);
    }
    let mut positions = HashMap::<Id, opengeometry::geometry::poly2d::Pt2>::new();
    for (root, group) in groups {
        let fixed: Vec<_> = group.iter().copied().filter(|v| feature[v]).collect();
        if fixed.len() > 1 {
            return None;
        }
        let pos = if let Some(&v) = fixed.first() {
            mesh.point(v)?
        } else {
            let bvs: Vec<_> = group.iter().copied().filter(|v| boundary[v]).collect();
            let avg = if bvs.is_empty() { &group } else { &bvs };
            point(
                avg.iter().map(|&v| mesh.point(v).unwrap().x).sum::<f64>() / avg.len() as f64,
                avg.iter().map(|&v| mesh.point(v).unwrap().z).sum::<f64>() / avg.len() as f64,
            )
        };
        positions.insert(root, pos);
    }
    let mut out = QuadMesh::new();
    let mut remap = HashMap::new();
    for v in mesh.vertex_ids() {
        let root = uf.find(v);
        let p = positions.get(&root).copied().unwrap_or(mesh.point(v)?);
        remap.insert(v, out.add_mesh_vertex(p));
    }
    for (_, face) in mesh.faces() {
        let q: [Id; 4] = face.try_into().ok()?;
        let mapped = q.map(|v| remap[&uf.find(v)]);
        if mapped.into_iter().collect::<HashSet<_>>().len() == 4 {
            out.add_quad(mapped).ok()?;
        }
    }
    if out.face_count() == 0 {
        None
    } else {
        Some(compress(&out))
    }
}

fn split_chord(mesh: &QuadMesh, rungs: &[(Id, Id)], chord_faces: &[Id]) -> QuadMesh {
    let mut out = QuadMesh::new();
    let mut remap = HashMap::new();
    for v in mesh.vertex_ids() {
        remap.insert(v, out.add_mesh_vertex(mesh.point(v).unwrap()));
    }
    let mut mids = HashMap::new();
    for &(a, b) in rungs {
        let p = mesh.point(a).unwrap();
        let q = mesh.point(b).unwrap();
        mids.insert(
            edge_key(a, b),
            out.add_mesh_vertex(point(0.5 * (p.x + q.x), 0.5 * (p.z + q.z))),
        );
    }
    let rung_set: HashSet<_> = rungs.iter().map(|&(a, b)| edge_key(a, b)).collect();
    let removed: HashSet<_> = chord_faces.iter().copied().collect();
    for (fid, face) in mesh.faces() {
        if removed.contains(&fid) {
            continue;
        }
        let q: [Id; 4] = face.try_into().unwrap();
        out.add_quad(q.map(|v| remap[&v])).unwrap();
    }
    for &fid in chord_faces {
        let vs: [Id; 4] = mesh.face_vertices(fid).unwrap().try_into().unwrap();
        let selected = (0..4).find_map(|k| {
            let a = edge_key(vs[k], vs[(k + 1) % 4]);
            let b = edge_key(vs[(k + 2) % 4], vs[(k + 3) % 4]);
            if rung_set.contains(&a) && rung_set.contains(&b) {
                Some((k, a, b))
            } else {
                None
            }
        });
        if let Some((k, a, b)) = selected {
            let mi = mids[&a];
            let mo = mids[&b];
            let [v0, v1, v2, v3] = [vs[k], vs[(k + 1) % 4], vs[(k + 2) % 4], vs[(k + 3) % 4]];
            out.add_quad([remap[&v0], mi, mo, remap[&v3]]).unwrap();
            out.add_quad([mi, remap[&v1], remap[&v2], mo]).unwrap();
        }
    }
    out
}

fn nearest_distances(
    mesh: &QuadMesh,
    targets: &[opengeometry::geometry::poly2d::Pt2],
) -> HashMap<Id, f64> {
    mesh.vertex_ids()
        .into_iter()
        .map(|id| {
            (
                id,
                targets
                    .iter()
                    .map(|q| distance(mesh.point(id).unwrap(), *q))
                    .fold(f64::INFINITY, f64::min),
            )
        })
        .collect()
}

fn try_accept(mesh: QuadMesh, region: &Region, p: &QParams, htol: f64) -> Option<QuadMesh> {
    if !mesh.is_valid_connectivity() || min_det(&mesh) <= 1e-9 * p.l_tar * p.l_tar {
        return None;
    }
    if hausdorff_mesh_boundary(&mesh, region) > htol {
        return None;
    }
    Some(mesh)
}

pub fn simplify(mut mesh: QuadMesh, region: &Region, p: &QParams) -> QuadMesh {
    let scale = region.scale();
    let htol = p.hausdorff_tol.unwrap_or(1e-6 * scale);
    let corners = region.corners(1e-4);
    for _ in 0..p.max_outer_iter {
        let mut ops = 0;
        for collapse in [true, false] {
            while ops < p.max_ops_per_iter {
                let (edges, boundary) = mesh.boundary_info();
                let feature = mesh.feature_vertices(&edges, p.straight_tol);
                let near = nearest_distances(&mesh, &corners);
                let mut feature = feature;
                for id in mesh.vertex_ids() {
                    if boundary[&id] && near[&id] < 1e-6 * scale {
                        feature.insert(id, true);
                    }
                }
                let mut chords = open_chords(&mesh);
                if collapse {
                    chords.retain(|c| c.width < p.collapse_ratio * p.l_tar);
                    chords.sort_by(|a, b| a.width.total_cmp(&b.width));
                } else {
                    chords.retain(|c| c.width > p.split_ratio * p.l_tar);
                    chords.sort_by(|a, b| b.width.total_cmp(&a.width));
                }
                let mut changed = false;
                for chord in chords {
                    let candidate = if collapse {
                        collapse_chord(&mesh, &chord.rungs, &boundary, &feature)
                    } else {
                        Some(split_chord(&mesh, &chord.rungs, &chord.faces))
                    };
                    let Some(candidate) = candidate.and_then(|m| try_accept(m, region, p, htol))
                    else {
                        continue;
                    };
                    mesh = candidate;
                    ops += 1;
                    changed = true;
                    break;
                }
                if !changed {
                    break;
                }
            }
        }
        if ops == 0 {
            break;
        }
    }
    mesh
}
