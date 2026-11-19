use opengeometry::geometry::poly2d::{Pt2, point_in_ring2};

use super::mesh::{alpha_error, coons, grid_to_mesh};
use super::polyline::{
    Polyline, angle, cumulative, diagonal, distance, point, point_at, poly_len, sub, sub_pl,
};
use super::types::{Patch, QParams, Region};

fn clean_ring(ring: &[Pt2], micro: f64) -> Polyline {
    let mut out = vec![ring[0]];
    for &p in &ring[1..] {
        if distance(p, *out.last().unwrap()) >= micro {
            out.push(p);
        }
    }
    while out.len() > 3 && distance(out[0], *out.last().unwrap()) < micro {
        out.pop();
    }
    out
}

pub fn clean_ring_for_segmentation(ring: &[Pt2], micro: f64) -> Polyline {
    clean_ring(ring, micro)
}

fn interior_angles(ring: &[Pt2]) -> Vec<f64> {
    (0..ring.len())
        .map(|i| {
            let p = ring[i];
            (angle(sub(ring[(i + ring.len() - 1) % ring.len()], p))
                - angle(sub(ring[(i + 1) % ring.len()], p)))
            .rem_euclid(std::f64::consts::TAU)
        })
        .collect()
}

fn representative_point(ring: &[Pt2]) -> Pt2 {
    let min_y = ring.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    let max_y = ring.iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max);
    for frac in [0.5, 0.3819660112501051, 0.6180339887498949, 0.25, 0.75] {
        let y = min_y + frac * (max_y - min_y);
        let mut xs = Vec::new();
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            if (a.z > y) != (b.z > y) {
                xs.push(a.x + (y - a.z) * (b.x - a.x) / (b.z - a.z));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            if pair[1] > pair[0] {
                let p = point(0.5 * (pair[0] + pair[1]), y);
                if point_in_ring2(p, ring) {
                    return p;
                }
            }
        }
    }
    ring.iter().fold(point(0.0, 0.0), |p, q| {
        point(p.x + q.x / ring.len() as f64, p.z + q.z / ring.len() as f64)
    })
}

fn build_template(ring: &[Pt2], corner_ids: &[usize], k: usize) -> Option<Vec<Patch>> {
    let mut closed = ring.to_vec();
    closed.push(ring[0]);
    let s = cumulative(&closed);
    let total = *s.last().unwrap();
    let mut doubled = closed.clone();
    doubled.extend_from_slice(&closed[1..]);
    let sc: Vec<_> = corner_ids.iter().map(|&i| s[i]).collect();
    let arcs = match k {
        4 => sc.clone(),
        2 => vec![
            sc[0],
            0.5 * (sc[0] + sc[1]),
            sc[1],
            0.5 * (sc[1] + sc[0] + total),
        ],
        1 => (0..4).map(|i| sc[0] + i as f64 * total / 4.0).collect(),
        _ => (0..4).map(|i| i as f64 * total / 4.0).collect(),
    };

    if k == 3 {
        let bounds = [(sc[0], sc[1]), (sc[1], sc[2]), (sc[2], sc[0] + total)];
        let mut first_halves = Vec::new();
        let mut second_halves = Vec::new();
        let mut points = Vec::new();
        for (a, b) in bounds {
            let m = 0.5 * (a + b);
            first_halves.push(sub_pl(&doubled, a, m));
            second_halves.push(sub_pl(&doubled, m, b));
            points.push(point_at(&doubled, a));
        }
        let mut center = point(
            points.iter().map(|p| p.x).sum::<f64>() / 3.0,
            points.iter().map(|p| p.z).sum::<f64>() / 3.0,
        );
        if !point_in_ring2(center, ring) {
            center = representative_point(ring);
        }
        let mids: Vec<_> = second_halves.iter().map(|h| h[0]).collect();
        let mut patches = Vec::with_capacity(3);
        for i in 0..3 {
            let j = (i + 1) % 3;
            patches.push([
                second_halves[i].clone(),
                first_halves[j].clone(),
                vec![mids[j], center],
                vec![center, mids[i]],
            ]);
        }
        return Some(patches);
    }

    let mut sides = Vec::with_capacity(4);
    for i in 0..4 {
        let a = arcs[i];
        let b = if i < 3 { arcs[i + 1] } else { arcs[0] + total };
        if b - a < 1e-12 {
            return None;
        }
        sides.push(sub_pl(&doubled, a, b));
    }
    Some(vec![sides.try_into().ok()?])
}

pub fn local_counts(patch: &Patch, l_tar: f64) -> (usize, usize) {
    let bends: Vec<_> = patch
        .iter()
        .map(|side| super::polyline::bends(side, 1e-4).len() + 1)
        .collect();
    let n0 = bends[0].max(bends[2]).max(
        (0.5 * (poly_len(&patch[0]) + poly_len(&patch[2])) / l_tar).round_ties_even() as usize,
    );
    let n1 = bends[1].max(bends[3]).max(
        (0.5 * (poly_len(&patch[1]) + poly_len(&patch[3])) / l_tar).round_ties_even() as usize,
    );
    (n0.max(1), n1.max(1))
}

pub fn template_error(patches: &[Patch], l_tar: f64) -> f64 {
    let mut mesh = super::types::QuadMesh::new();
    for patch in patches {
        let (nu, nv) = local_counts(patch, l_tar);
        let local = grid_to_mesh(&coons(patch, nu, nv));
        mesh.append(&local);
    }
    alpha_error(&mesh)
}

pub fn build_layout(regions: &[Region], params: &QParams) -> Vec<Patch> {
    let scale = regions
        .iter()
        .map(|r| diagonal(&r.outer))
        .fold(0.0, f64::max);
    let micro = 1e-4 * scale;
    let rings: Vec<_> = regions
        .iter()
        .map(|r| clean_ring(&r.outer, micro))
        .collect();
    let angles: Vec<_> = rings.iter().map(|r| interior_angles(r)).collect();
    let corners: Vec<_> = rings
        .iter()
        .zip(&angles)
        .flat_map(|(r, a)| {
            r.iter()
                .zip(a)
                .filter(|(_, a)| (**a - std::f64::consts::PI).abs() > params.straight_tol)
                .map(|(&p, _)| p)
        })
        .collect();
    let mut layout = Vec::new();
    for (ring, ang) in rings.iter().zip(&angles) {
        let mut cand: Vec<usize> = (0..ring.len())
            .filter(|&i| {
                (ang[i] - std::f64::consts::PI).abs() > params.straight_tol
                    || corners.iter().any(|&p| distance(p, ring[i]) < 10.0 * micro)
            })
            .collect();
        cand.sort_by(|&i, &j| {
            (ang[i] - std::f64::consts::FRAC_PI_2)
                .abs()
                .total_cmp(&(ang[j] - std::f64::consts::FRAC_PI_2).abs())
                .then(i.cmp(&j))
        });
        cand.truncate(params.max_corner_cands);
        cand.sort_unstable();
        let real: Vec<_> = (0..ring.len())
            .filter(|&i| (ang[i] - std::f64::consts::PI).abs() > params.straight_tol)
            .collect();
        if real.len() == 3 {
            if let Some(patches) = build_template(ring, &real, 3) {
                layout.extend(patches);
                continue;
            }
        }

        let mut best: Option<(f64, Vec<Patch>)> = None;
        for k in [4, 3, 2, 1, 0] {
            if cand.len() < k {
                continue;
            }
            let combinations = combinations(&cand, k);
            let mut combos = combinations;
            combos.sort_by(|a, b| {
                let score = |c: &[usize]| {
                    c.iter()
                        .map(|&i| (ang[i] - std::f64::consts::FRAC_PI_2).abs())
                        .sum::<f64>()
                };
                score(a).total_cmp(&score(b))
            });
            let mut kbest: Option<(f64, Vec<Patch>)> = None;
            for cmb in combos.iter().take(params.max_combos) {
                if let Some(patches) = build_template(ring, cmb, k) {
                    let err = template_error(&patches, params.l_tar);
                    if kbest.as_ref().is_none_or(|(old, _)| err < *old) {
                        kbest = Some((err, patches));
                    }
                }
            }
            if let Some(candidate) = kbest {
                if best.as_ref().is_none_or(|(old, _)| candidate.0 < *old) {
                    best = Some(candidate);
                }
            }
            if best.as_ref().is_some_and(|(err, _)| *err < params.alpha_th) {
                break;
            }
        }
        if let Some((_, patches)) = best {
            layout.extend(patches);
        }
    }
    layout
}

fn combinations(values: &[usize], k: usize) -> Vec<Vec<usize>> {
    if k == 0 {
        return vec![Vec::new()];
    }
    if k > values.len() {
        return Vec::new();
    }
    fn visit(
        values: &[usize],
        k: usize,
        at: usize,
        acc: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if acc.len() == k {
            out.push(acc.clone());
            return;
        }
        let remaining = k - acc.len();
        for i in at..=values.len() - remaining {
            acc.push(values[i]);
            visit(values, k, i + 1, acc, out);
            acc.pop();
        }
    }
    let mut out = Vec::new();
    visit(values, k, 0, &mut Vec::new(), &mut out);
    out
}
use crate::topology::ops::{QuadMeshOps, TopologyOps};
