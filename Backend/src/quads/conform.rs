use crate::topology::ops::{QuadMeshOps, TopologyOps};
use std::collections::HashMap;

use opengeometry::geometry::poly2d::Pt2;

use super::mesh::{grid_to_mesh, min_det, remove_duplicate_quads};
use super::polyline::{distance, point_at, poly_len, project, split_pl};
use super::types::{Patch, QParams, QuadMesh};
use super::union_find::IdUnionFind;
use super::union_find::UnionFind;
use crate::base_types::Id;

pub fn find_tjunction(patches: &[Patch], tol: f64) -> Option<(usize, usize, Pt2, f64)> {
    for (pi, patch) in patches.iter().enumerate() {
        for side in patch {
            let c = side[0];
            for (qi, target) in patches.iter().enumerate() {
                if qi == pi {
                    continue;
                }
                for (si, target_side) in target.iter().enumerate() {
                    let (d, s, _) = project(target_side, c);
                    if d < tol && s > tol && s < poly_len(target_side) - tol {
                        return Some((qi, si, c, s));
                    }
                }
            }
        }
    }
    None
}

fn split_patch(
    patch: &Patch,
    si: usize,
    ct: Pt2,
    st: f64,
    others: &[Patch],
    tol: f64,
    l_mer: f64,
) -> (Patch, Patch) {
    let sides: Vec<_> = (0..4).map(|i| patch[(si + i) % 4].clone()).collect();
    let (s0, s1, s2, s3) = (&sides[0], &sides[1], &sides[2], &sides[3]);
    let l0 = poly_len(s0);
    let l2 = poly_len(s2);
    let mut so = (1.0 - st / l0) * l2;
    let margin = (0.5 * l_mer).min(0.25 * l2);
    so = so.clamp(margin, l2 - margin);
    let mut snap: Option<Pt2> = None;
    let mut best = l_mer;
    for other in others {
        for side in other {
            let c = side[0];
            let (d, s, _) = project(s2, c);
            if d < tol && s > tol && s < l2 - tol && (s - so).abs() < best {
                best = (s - so).abs();
                so = s;
                snap = Some(c);
            }
        }
    }
    let (a1, a2) = split_pl(s0, st, Some(ct));
    let (b1, b2) = split_pl(s2, so, snap);
    let tp = *b1.last().unwrap();
    let cut = vec![ct, tp];
    let q1 = [a1, cut.clone(), b2, s3.clone()];
    let q2 = [a2, s1.clone(), b1, cut.into_iter().rev().collect()];
    (q1, q2)
}

pub fn remove_tjunctions(patches: Vec<Patch>, p: &QParams, scale: f64) -> (Vec<Patch>, bool) {
    let tol = 1e-5 * scale;
    let l_mer = p.l_mer_factor * p.l_tar;
    let mut patches = patches;
    for _ in 0..p.max_t_iter {
        let Some((qi, si, ct, st)) = find_tjunction(&patches, tol) else {
            return (patches, false);
        };
        let removed = patches.remove(qi);
        let (q1, q2) = split_patch(&removed, si, ct, st, &patches, tol, l_mer);
        patches.push(q1);
        patches.push(q2);
    }
    (patches.clone(), find_tjunction(&patches, tol).is_some())
}

fn point_id(pool: &mut Vec<Pt2>, p: Pt2, tol: f64) -> usize {
    if !pool.is_empty() {
        let mut best = (f64::INFINITY, 0);
        for (i, &q) in pool.iter().enumerate() {
            let d = distance(q, p);
            if d < best.0 {
                best = (d, i);
            }
        }
        if best.0 < tol {
            return best.1;
        }
    }
    pool.push(p);
    pool.len() - 1
}

pub fn choose_counts(patches: &[Patch], p: &QParams, scale: f64) -> Vec<(usize, usize)> {
    let tol = 1e-5 * scale;
    let mut pool = Vec::new();
    let mut edge_lookup = HashMap::<(usize, usize, usize), usize>::new();
    let mut edge_lengths = Vec::new();
    let mut min_counts = Vec::new();
    let mut patch_edges = Vec::new();
    for patch in patches {
        let mut ids = Vec::new();
        for side in patch {
            let a = point_id(&mut pool, side[0], tol);
            let b = point_id(&mut pool, *side.last().unwrap(), tol);
            let m = point_id(&mut pool, point_at(side, 0.5 * poly_len(side)), tol);
            let key = (a.min(b), a.max(b), m);
            let id = *edge_lookup.entry(key).or_insert_with(|| {
                let id = edge_lengths.len();
                edge_lengths.push(poly_len(side));
                min_counts.push(super::polyline::bends(side, 1e-4).len() + 1);
                id
            });
            ids.push(id);
        }
        patch_edges.push(ids);
    }

    let mut uf = UnionFind::new(edge_lengths.len());
    for ids in &patch_edges {
        uf.join(ids[0], ids[2]);
        uf.join(ids[1], ids[3]);
    }
    let mut classes: Vec<Vec<f64>> = vec![Vec::new(); edge_lengths.len()];
    let mut class_min = vec![1; edge_lengths.len()];
    for e in 0..edge_lengths.len() {
        let root = uf.find(e);
        classes[root].push(edge_lengths[e]);
        class_min[root] = class_min[root].max(min_counts[e]);
    }
    let mut n_of = vec![1; edge_lengths.len()];
    for root in 0..classes.len() {
        if classes[root].is_empty() {
            continue;
        }
        let max_len = classes[root].iter().copied().fold(0.0_f64, f64::max);
        let end = (max_len / p.l_tar) as usize;
        let stop = class_min[root].max(end) + 4;
        let mut best = (f64::INFINITY, class_min[root]);
        for n in class_min[root]..stop {
            let cost = classes[root]
                .iter()
                .map(|&l| (l / n as f64 / p.l_tar - 1.0).powi(2))
                .sum::<f64>();
            if cost < best.0 {
                best = (cost, n);
            }
        }
        n_of[root] = best.1;
    }
    patch_edges
        .iter()
        .map(|ids| (n_of[uf.find(ids[0])], n_of[uf.find(ids[1])]))
        .collect()
}

/// Shortest/longest side-length ratio for each opposite-edge subdivision class.
/// These are the same classes whose edges must receive the same subdivision count.
pub fn subdivision_ratios(patches: &[Patch], scale: f64) -> Vec<f64> {
    let tol = 1e-5 * scale;
    let mut pool = Vec::new();
    let mut edge_lookup = HashMap::<(usize, usize, usize), usize>::new();
    let mut lengths = Vec::new();
    let mut patch_edges = Vec::new();
    for patch in patches {
        let mut ids = Vec::new();
        for side in patch {
            let a = point_id(&mut pool, side[0], tol);
            let b = point_id(&mut pool, *side.last().unwrap(), tol);
            let m = point_id(&mut pool, point_at(side, 0.5 * poly_len(side)), tol);
            let key = (a.min(b), a.max(b), m);
            let id = *edge_lookup.entry(key).or_insert_with(|| {
                let id = lengths.len();
                lengths.push(poly_len(side));
                id
            });
            ids.push(id);
        }
        patch_edges.push(ids);
    }
    let mut uf = UnionFind::new(lengths.len());
    for ids in &patch_edges {
        uf.join(ids[0], ids[2]);
        uf.join(ids[1], ids[3]);
    }
    let mut classes: Vec<Vec<f64>> = vec![Vec::new(); lengths.len()];
    for (edge, &length) in lengths.iter().enumerate() {
        classes[uf.find(edge)].push(length);
    }
    classes.into_iter().filter(|c| !c.is_empty()).map(|c| {
        let min = c.iter().copied().fold(f64::INFINITY, f64::min);
        let max = c.iter().copied().fold(0.0_f64, f64::max);
        min / max
    }).collect()
}

pub fn refine_bad_patches(mut patches: Vec<Patch>, p: &QParams, scale: f64) -> Vec<Patch> {
    let tol = 1e-5 * scale;
    for _ in 0..4 {
        let counts = choose_counts(&patches, p, scale);
        let mut new_patches = Vec::new();
        let mut changed = false;
        for (patch, (nu, nv)) in patches.iter().zip(counts) {
            let mesh = grid_to_mesh(&super::mesh::coons(patch, nu, nv));
            let lens: Vec<_> = patch.iter().map(|s| poly_len(s)).collect();
            if min_det(&mesh) > 1e-9 * p.l_tar * p.l_tar
                || lens.iter().copied().fold(f64::INFINITY, f64::min) < p.l_tar
            {
                new_patches.push(patch.clone());
                continue;
            }
            let si = if lens[1] + lens[3] > lens[0] + lens[2] {
                1
            } else {
                0
            };
            let sides: Vec<_> = (0..4).map(|i| patch[(si + i) % 4].clone()).collect();
            let l0 = poly_len(&sides[0]);
            let (q1, q2) = split_patch(
                &[
                    sides[0].clone(),
                    sides[1].clone(),
                    sides[2].clone(),
                    sides[3].clone(),
                ],
                0,
                point_at(&sides[0], 0.5 * l0),
                0.5 * l0,
                &[],
                tol,
                p.l_mer_factor * p.l_tar,
            );
            new_patches.extend([q1, q2]);
            changed = true;
        }
        if !changed {
            break;
        }
        patches = remove_tjunctions(new_patches, p, scale).0;
    }
    patches
}

fn pairs_within(mesh: &QuadMesh, tol: f64) -> Vec<(Id, Id)> {
    let mut cells = HashMap::<(i64, i64), Vec<usize>>::new();
    let mut out = Vec::new();
    let mut point_ids = Vec::new();
    for id in mesh.vertex_ids() {
        let p = mesh.point(id).unwrap();
        let i = point_ids.len();
        point_ids.push(id);
        let cell = ((p.x / tol).floor() as i64, (p.z / tol).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = cells.get(&(cell.0 + dx, cell.1 + dy)) {
                    for &j in ids {
                        if distance(p, mesh.point(point_ids[j]).unwrap()) <= tol {
                            out.push((point_ids[j], id));
                        }
                    }
                }
            }
        }
        cells.entry(cell).or_default().push(i);
    }
    out
}

pub fn mesh_layout(
    patches: &[Patch],
    counts: &[(usize, usize)],
    p: &QParams,
    scale: f64,
) -> QuadMesh {
    let mut mesh = QuadMesh::new();
    for (patch, &(nu, nv)) in patches.iter().zip(counts) {
        let local = grid_to_mesh(&super::mesh::coons(patch, nu, nv));
        mesh.append(&local);
    }
    let tol = (1e-5 * scale).min(0.01 * p.l_tar);
    let mut uf = IdUnionFind::new(mesh.vertex_ids());
    for (a, b) in pairs_within(&mesh, tol) {
        uf.join_min(a, b);
    }
    let mut out = QuadMesh::new();
    let mut remap = HashMap::new();
    for id in mesh.vertex_ids() {
        let root = uf.find(id);
        let new = *remap
            .entry(root)
            .or_insert_with(|| out.add_mesh_vertex(mesh.point(root).unwrap()));
        remap.insert(id, new);
    }
    for (_, face) in mesh.faces() {
        let q: [Id; 4] = face.try_into().unwrap();
        out.add_quad(q.map(|id| remap[&id])).unwrap();
    }
    remove_duplicate_quads(out)
}
