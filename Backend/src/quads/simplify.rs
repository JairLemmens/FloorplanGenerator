use std::collections::{HashMap, HashSet};

use opengeometry::geometry::poly2d::Pt2;

use super::mesh::{EdgeTable, Quads};
use super::polyline::{distance, lerp, midpoint, point_segment_distance};
use super::types::{EPS, QParams, Region};
use super::union_find::UnionFind;

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

/// A strip of quads crossing the mesh: its rungs (the edges shared between
/// consecutive quads, plus both ends) and the quads themselves.
struct Chord {
    width: f64,
    rungs: Vec<(usize, usize)>,
    faces: Vec<usize>,
}

/// Walk across quads starting from a boundary edge until the other boundary.
fn trace_chord(mesh: &Quads, edges: &EdgeTable, start: (usize, usize)) -> Option<Chord> {
    let mut rungs = vec![start];
    let mut faces = Vec::new();
    let mut current = start;
    let mut face = *edges.adjacent(start).first()?;
    loop {
        if faces.contains(&face) {
            return None;
        }
        faces.push(face);
        let vs = mesh.faces[face];
        let k = (0..4).find(|&k| edge_key(vs[k], vs[(k + 1) % 4]) == current)?;
        let opposite = edge_key(vs[(k + 2) % 4], vs[(k + 3) % 4]);
        rungs.push(opposite);
        match edges.adjacent(opposite).iter().find(|&&f| f != face) {
            Some(&next) => {
                face = next;
                current = opposite;
            }
            None => break,
        }
    }
    let width = rungs
        .iter()
        .map(|&(a, b)| distance(mesh.pts[a], mesh.pts[b]))
        .sum::<f64>()
        / rungs.len() as f64;
    Some(Chord {
        width,
        rungs,
        faces,
    })
}

/// One chord per boundary-to-boundary strip.
fn open_chords(mesh: &Quads, edges: &EdgeTable) -> Vec<Chord> {
    let mut seen = HashSet::<Vec<usize>>::new();
    edges
        .boundary_edges()
        .filter_map(|start| trace_chord(mesh, edges, start))
        .filter(|chord| {
            let mut key = chord.faces.clone();
            key.sort_unstable();
            seen.insert(key)
        })
        .collect()
}

/// Collapse every rung of the chord to a point. Rungs between two feature
/// vertices, or groups containing two, cannot collapse.
fn collapse_chord(
    mesh: &Quads,
    rungs: &[(usize, usize)],
    boundary: &[bool],
    feature: &[bool],
) -> Option<Quads> {
    let mut uf = UnionFind::new(mesh.pts.len());
    let mut touched = Vec::new();
    for &(a, b) in rungs {
        if feature[a] && feature[b] {
            return None;
        }
        uf.join(a, b);
        touched.extend([a, b]);
    }
    touched.sort_unstable();
    touched.dedup();
    let mut groups = HashMap::<usize, Vec<usize>>::new();
    for v in touched {
        groups.entry(uf.find(v)).or_default().push(v);
    }
    let mut pts = mesh.pts.clone();
    for (root, group) in groups {
        let fixed: Vec<_> = group.iter().copied().filter(|&v| feature[v]).collect();
        pts[root] = match fixed.as_slice() {
            [] => {
                // Average the boundary vertices if any, otherwise all of them.
                let on_boundary: Vec<_> = group.iter().copied().filter(|&v| boundary[v]).collect();
                let avg = if on_boundary.is_empty() { &group } else { &on_boundary };
                let n = avg.len() as f64;
                Pt2::new(
                    avg.iter().map(|&v| mesh.pts[v].x).sum::<f64>() / n,
                    avg.iter().map(|&v| mesh.pts[v].z).sum::<f64>() / n,
                )
            }
            &[v] => mesh.pts[v],
            _ => return None,
        };
    }
    let roots: Vec<_> = (0..mesh.pts.len()).map(|v| uf.find(v)).collect();
    let merged = mesh.merged(pts, |v| roots[v]);
    (!merged.faces.is_empty()).then_some(merged)
}

/// Split every quad of the chord in two by joining the midpoints of its rungs.
fn split_chord(mesh: &Quads, chord: &Chord) -> Quads {
    let mut out = mesh.clone();
    let mut mids = HashMap::new();
    for &(a, b) in &chord.rungs {
        mids.insert(edge_key(a, b), out.pts.len());
        out.pts.push(midpoint(mesh.pts[a], mesh.pts[b]));
    }
    let removed: HashSet<_> = chord.faces.iter().copied().collect();
    out.faces = (0..mesh.faces.len())
        .filter(|f| !removed.contains(f))
        .map(|f| mesh.faces[f])
        .collect();
    for &f in &chord.faces {
        let vs = mesh.faces[f];
        let rung = (0..4).find_map(|k| {
            let a = mids.get(&edge_key(vs[k], vs[(k + 1) % 4]))?;
            let b = mids.get(&edge_key(vs[(k + 2) % 4], vs[(k + 3) % 4]))?;
            Some((k, *a, *b))
        });
        if let Some((k, mi, mo)) = rung {
            let [v0, v1, v2, v3] = [vs[k], vs[(k + 1) % 4], vs[(k + 2) % 4], vs[(k + 3) % 4]];
            out.faces.push([v0, mi, mo, v3]);
            out.faces.push([mi, v1, v2, mo]);
        }
    }
    out
}

/// Whether some point of `from` (sampled along each edge) is farther than `tol` from `to`.
fn drifts_beyond(from: &[(Pt2, Pt2)], to: &[(Pt2, Pt2)], tol: f64) -> bool {
    const SAMPLES: usize = 50;
    let far = |p: Pt2| {
        to.iter()
            .map(|&(a, b)| point_segment_distance(p, a, b))
            .fold(f64::INFINITY, f64::min)
            > tol
    };
    from.iter().any(|&(a, b)| {
        (0..=SAMPLES).any(|i| far(lerp(a, b, i as f64 / SAMPLES as f64)))
    })
}

/// Everything fixed during one simplification run.
struct Context<'a> {
    params: &'a QParams,
    domain_edges: Vec<(Pt2, Pt2)>,
    /// Sharp corners of the domain, which mesh vertices must keep.
    corners: Vec<Pt2>,
    hausdorff_tol: f64,
    corner_tol: f64,
}

impl Context<'_> {
    /// A candidate is acceptable if it is manifold, not inverted, and its
    /// boundary stays within the Hausdorff tolerance of the domain.
    fn accepts(&self, mesh: &Quads) -> bool {
        let p = self.params;
        if !mesh.is_manifold() || mesh.min_det() <= EPS * p.l_tar * p.l_tar {
            return false;
        }
        let edges = mesh.edge_table();
        let boundary: Vec<_> = edges
            .boundary_edges()
            .map(|(a, b)| (mesh.pts[a], mesh.pts[b]))
            .collect();
        !drifts_beyond(&self.domain_edges, &boundary, self.hausdorff_tol)
            && !drifts_beyond(&boundary, &self.domain_edges, self.hausdorff_tol)
    }

    /// Apply the first chord operation that yields an acceptable mesh.
    fn step(&self, mesh: &Quads, collapse: bool) -> Option<Quads> {
        let p = self.params;
        let edges = mesh.edge_table();
        let boundary = edges.boundary_vertices(mesh.pts.len());
        let mut feature = edges.feature_vertices(mesh, p.straight_tol);
        for (v, f) in feature.iter_mut().enumerate() {
            let at_corner = self
                .corners
                .iter()
                .any(|&c| distance(mesh.pts[v], c) < self.corner_tol);
            *f |= boundary[v] && at_corner;
        }
        let mut chords = open_chords(mesh, &edges);
        if collapse {
            chords.retain(|c| c.width < p.collapse_ratio * p.l_tar);
            chords.sort_by(|a, b| a.width.total_cmp(&b.width));
        } else {
            chords.retain(|c| c.width > p.split_ratio * p.l_tar);
            chords.sort_by(|a, b| b.width.total_cmp(&a.width));
        }
        chords.iter().find_map(|chord| {
            let candidate = if collapse {
                collapse_chord(mesh, &chord.rungs, &boundary, &feature)?
            } else {
                split_chord(mesh, chord)
            };
            self.accepts(&candidate).then_some(candidate)
        })
    }
}

/// Collapse narrow chords and split wide ones towards the target edge length.
pub fn simplify(mut mesh: Quads, region: &Region, p: &QParams, tol: f64) -> Quads {
    let ctx = Context {
        params: p,
        domain_edges: region.edges().collect(),
        corners: region.corners(EPS),
        hausdorff_tol: p.hausdorff_tol.unwrap_or(tol),
        corner_tol: tol,
    };
    for _ in 0..p.max_outer_iter {
        let mut ops = 0;
        for collapse in [true, false] {
            while ops < p.max_ops_per_iter {
                let Some(next) = ctx.step(&mesh, collapse) else {
                    break;
                };
                mesh = next;
                ops += 1;
            }
        }
        if ops == 0 {
            break;
        }
    }
    mesh
}
