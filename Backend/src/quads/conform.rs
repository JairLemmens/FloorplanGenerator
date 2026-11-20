use std::collections::HashMap;

use opengeometry::geometry::poly2d::Pt2;

use super::mesh::{Quads, coons};
use super::polyline::{distance, point_at, poly_len, project, segments, split_pl};
use super::types::{EPS, Patch, QParams, rotate_patch};
use super::union_find::UnionFind;

/// First patch corner lying strictly inside the side of another patch:
/// `(patch index, side index, corner, arc length of the corner along that side)`.
pub fn find_tjunction(patches: &[Patch], tol: f64) -> Option<(usize, usize, Pt2, f64)> {
    for (pi, patch) in patches.iter().enumerate() {
        for side in patch {
            let c = side[0];
            for (qi, target) in patches.iter().enumerate().filter(|&(qi, _)| qi != pi) {
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

/// Split `patch` across side `si` at `(ct, st)`, cutting the opposite side at a
/// matching position (snapped to nearby corners of `others`).
fn split_patch(
    patch: &Patch,
    si: usize,
    ct: Pt2,
    st: f64,
    others: &[Patch],
    tol: f64,
    l_mer: f64,
) -> (Patch, Patch) {
    let [s0, s1, s2, s3] = rotate_patch(patch, si);
    let l0 = poly_len(&s0);
    let l2 = poly_len(&s2);
    let margin = (0.5 * l_mer).min(0.25 * l2);
    let mut so = ((1.0 - st / l0) * l2).clamp(margin, l2 - margin);
    let mut snap: Option<Pt2> = None;
    let mut best = l_mer;
    for side in others.iter().flatten() {
        let c = side[0];
        let (d, s, _) = project(&s2, c);
        if d < tol && s > tol && s < l2 - tol && (s - so).abs() < best {
            best = (s - so).abs();
            so = s;
            snap = Some(c);
        }
    }
    let (a1, a2) = split_pl(&s0, st, Some(ct));
    let (b1, b2) = split_pl(&s2, so, snap);
    let cut = vec![ct, *b1.last().unwrap()];
    let q1 = [a1, cut.clone(), b2, s3];
    let q2 = [a2, s1, b1, cut.into_iter().rev().collect()];
    (q1, q2)
}

/// Split patches until no corner lies in the interior of another patch's side.
pub fn remove_tjunctions(mut patches: Vec<Patch>, p: &QParams, tol: f64) -> Vec<Patch> {
    let l_mer = p.l_mer_factor * p.l_tar;
    for _ in 0..p.max_t_iter {
        let Some((qi, si, ct, st)) = find_tjunction(&patches, tol) else {
            break;
        };
        let removed = patches.remove(qi);
        let (q1, q2) = split_patch(&removed, si, ct, st, &patches, tol, l_mer);
        patches.push(q1);
        patches.push(q2);
    }
    patches
}

/// Index of `p` in `pool`, adding it if no pooled point lies within `tol`.
fn point_id(pool: &mut Vec<Pt2>, p: Pt2, tol: f64) -> usize {
    let nearest = pool
        .iter()
        .enumerate()
        .map(|(i, &q)| (distance(q, p), i))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match nearest {
        Some((d, i)) if d < tol => i,
        _ => {
            pool.push(p);
            pool.len() - 1
        }
    }
}

/// Shared patch sides, grouped into classes that must get the same subdivision
/// count (opposite sides of a patch).
struct EdgeClasses {
    /// Edge ids of each patch's four sides.
    patch_edges: Vec<[usize; 4]>,
    lengths: Vec<f64>,
    /// Minimum subdivisions per edge (one per straight run).
    min_counts: Vec<usize>,
    uf: UnionFind,
}

impl EdgeClasses {
    fn new(patches: &[Patch], tol: f64) -> Self {
        let mut pool = Vec::new();
        let mut lookup = HashMap::<(usize, usize, usize), usize>::new();
        let mut lengths = Vec::new();
        let mut min_counts = Vec::new();
        let mut patch_edges = Vec::with_capacity(patches.len());
        for patch in patches {
            patch_edges.push(std::array::from_fn(|k| {
                let side = &patch[k];
                let a = point_id(&mut pool, side[0], tol);
                let b = point_id(&mut pool, *side.last().unwrap(), tol);
                let m = point_id(&mut pool, point_at(side, 0.5 * poly_len(side)), tol);
                *lookup.entry((a.min(b), a.max(b), m)).or_insert_with(|| {
                    lengths.push(poly_len(side));
                    min_counts.push(segments(side));
                    lengths.len() - 1
                })
            }));
        }
        let mut uf = UnionFind::new(lengths.len());
        for ids in &patch_edges {
            uf.join(ids[0], ids[2]);
            uf.join(ids[1], ids[3]);
        }
        Self {
            patch_edges,
            lengths,
            min_counts,
            uf,
        }
    }

    /// Edge ids of every non-empty class, keyed by the class representative.
    fn members(&mut self) -> Vec<(usize, Vec<usize>)> {
        let mut classes = vec![Vec::new(); self.lengths.len()];
        for e in 0..self.lengths.len() {
            classes[self.uf.find(e)].push(e);
        }
        classes
            .into_iter()
            .enumerate()
            .filter(|(_, edges)| !edges.is_empty())
            .collect()
    }
}

/// Subdivision counts `(nu, nv)` per patch, equal across opposite shared sides.
pub fn choose_counts(patches: &[Patch], p: &QParams, tol: f64) -> Vec<(usize, usize)> {
    let mut classes = EdgeClasses::new(patches, tol);
    let mut count_of = vec![1; classes.lengths.len()];
    for (root, edges) in classes.members() {
        let lens: Vec<f64> = edges.iter().map(|&e| classes.lengths[e]).collect();
        let min = edges.iter().map(|&e| classes.min_counts[e]).fold(1, usize::max);
        let max_len = lens.iter().copied().fold(0.0_f64, f64::max);
        let stop = min.max((max_len / p.l_tar) as usize) + 4;
        let cost = |n: usize| {
            lens.iter()
                .map(|&l| (l / n as f64 / p.l_tar - 1.0).powi(2))
                .sum::<f64>()
        };
        // First minimum wins on ties.
        count_of[root] = (min..stop)
            .map(|n| (cost(n), n))
            .fold((f64::INFINITY, min), |best, next| {
                if next.0 < best.0 { next } else { best }
            })
            .1;
    }
    let EdgeClasses {
        patch_edges, mut uf, ..
    } = classes;
    patch_edges
        .iter()
        .map(|ids| (count_of[uf.find(ids[0])], count_of[uf.find(ids[1])]))
        .collect()
}

/// Bisect patches whose meshed grid folds over, then re-conform.
pub fn refine_bad_patches(mut patches: Vec<Patch>, p: &QParams, tol: f64) -> Vec<Patch> {
    let min_det = EPS * p.l_tar * p.l_tar;
    for _ in 0..4 {
        let counts = choose_counts(&patches, p, tol);
        let mut refined = Vec::with_capacity(patches.len());
        let mut changed = false;
        for (patch, (nu, nv)) in patches.iter().zip(counts) {
            let lens: Vec<_> = patch.iter().map(|s| poly_len(s)).collect();
            let shortest = lens.iter().copied().fold(f64::INFINITY, f64::min);
            let folded = Quads::from_grid(&coons(patch, nu, nv)).min_det() <= min_det;
            if !folded || shortest < p.l_tar {
                refined.push(patch.clone());
                continue;
            }
            // Cut across the longer pair of sides, at the middle of side `si`.
            let si = usize::from(lens[1] + lens[3] > lens[0] + lens[2]);
            let l = lens[si];
            let (q1, q2) = split_patch(
                patch,
                si,
                point_at(&patch[si], 0.5 * l),
                0.5 * l,
                &[],
                tol,
                p.l_mer_factor * p.l_tar,
            );
            refined.extend([q1, q2]);
            changed = true;
        }
        if !changed {
            break;
        }
        patches = remove_tjunctions(refined, p, tol);
    }
    patches
}

/// Mesh every patch, weld shared boundary vertices and drop duplicate quads.
pub fn mesh_layout(patches: &[Patch], counts: &[(usize, usize)], tol: f64) -> Quads {
    let mut mesh = Quads::default();
    for (patch, &(nu, nv)) in patches.iter().zip(counts) {
        mesh.append(&Quads::from_grid(&coons(patch, nu, nv)));
    }
    mesh.welded(tol)
}
