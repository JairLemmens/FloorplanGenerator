use std::f64::consts::{FRAC_PI_2, PI};

use opengeometry::geometry::poly2d::{Pt2, point_in_ring2};

use super::mesh::{Quads, coons};
use super::polyline::{
    clean_ring, cumulative, distance, interior_angles, point, point_at, poly_len,
    segments, sub_pl,
};
use super::types::{EPS, Patch, QParams, Region};

/// A point strictly inside `ring`, found by scanning horizontal chords.
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
    let n = ring.len() as f64;
    ring.iter()
        .fold(point(0.0, 0.0), |p, q| point(p.x + q.x / n, p.z + q.z / n))
}

/// A closed ring unrolled twice, so arc intervals may wrap past the start.
struct Loop {
    doubled: Vec<Pt2>,
    /// Arc length at every ring vertex.
    arc: Vec<f64>,
    total: f64,
}

impl Loop {
    fn new(ring: &[Pt2]) -> Self {
        let mut closed = ring.to_vec();
        closed.push(ring[0]);
        let arc = cumulative(&closed);
        let total = *arc.last().unwrap();
        let mut doubled = closed.clone();
        doubled.extend_from_slice(&closed[1..]);
        Self {
            doubled,
            arc,
            total,
        }
    }

    fn side(&self, a: f64, b: f64) -> Vec<Pt2> {
        sub_pl(&self.doubled, a, b)
    }
}

/// Three patches meeting at an interior point, one per ring corner.
fn three_patch_template(ring: &[Pt2], corners: &[usize]) -> Vec<Patch> {
    let lp = Loop::new(ring);
    let s: Vec<_> = corners.iter().map(|&i| lp.arc[i]).collect();
    let spans = [(s[0], s[1]), (s[1], s[2]), (s[2], s[0] + lp.total)];
    let mut first_halves = Vec::new();
    let mut second_halves = Vec::new();
    let mut points = Vec::new();
    for (a, b) in spans {
        let m = 0.5 * (a + b);
        first_halves.push(lp.side(a, m));
        second_halves.push(lp.side(m, b));
        points.push(point_at(&lp.doubled, a));
    }
    let mut center = point(
        points.iter().map(|p| p.x).sum::<f64>() / 3.0,
        points.iter().map(|p| p.z).sum::<f64>() / 3.0,
    );
    if !point_in_ring2(center, ring) {
        center = representative_point(ring);
    }
    let mids: Vec<_> = second_halves.iter().map(|h| h[0]).collect();
    (0..3)
        .map(|i| {
            let j = (i + 1) % 3;
            [
                second_halves[i].clone(),
                first_halves[j].clone(),
                vec![mids[j], center],
                vec![center, mids[i]],
            ]
        })
        .collect()
}

/// Cut the ring at four arc positions into a single four-sided patch.
fn four_side_template(ring: &[Pt2], corners: &[usize], k: usize) -> Option<Patch> {
    let lp = Loop::new(ring);
    let total = lp.total;
    let s: Vec<_> = corners.iter().map(|&i| lp.arc[i]).collect();
    let arcs: Vec<f64> = match k {
        4 => s,
        2 => vec![
            s[0],
            0.5 * (s[0] + s[1]),
            s[1],
            0.5 * (s[1] + s[0] + total),
        ],
        1 => (0..4).map(|i| s[0] + i as f64 * total / 4.0).collect(),
        _ => (0..4).map(|i| i as f64 * total / 4.0).collect(),
    };
    let mut sides = Vec::with_capacity(4);
    for i in 0..4 {
        let (a, b) = (arcs[i], if i < 3 { arcs[i + 1] } else { arcs[0] + total });
        if b - a < EPS * total {
            return None;
        }
        sides.push(lp.side(a, b));
    }
    sides.try_into().ok()
}

/// Patches covering `ring` when cut at the given corner vertices (`k` = their count).
fn build_template(ring: &[Pt2], corners: &[usize], k: usize) -> Option<Vec<Patch>> {
    if k == 3 {
        return Some(three_patch_template(ring, corners));
    }
    four_side_template(ring, corners, k).map(|patch| vec![patch])
}

/// Number of subdivisions needed along each patch direction.
pub fn local_counts(patch: &Patch, l_tar: f64) -> (usize, usize) {
    let runs: Vec<_> = patch.iter().map(|side| segments(side)).collect();
    let count = |a: usize, b: usize| {
        let target = (0.5 * (poly_len(&patch[a]) + poly_len(&patch[b])) / l_tar).round_ties_even();
        runs[a].max(runs[b]).max(target as usize).max(1)
    };
    (count(0, 2), count(1, 3))
}

/// Mean corner-angle error of the patches meshed at the local counts.
pub fn template_error(patches: &[Patch], l_tar: f64) -> f64 {
    let mut mesh = Quads::default();
    for patch in patches {
        let (nu, nv) = local_counts(patch, l_tar);
        mesh.append(&Quads::from_grid(&coons(patch, nu, nv)));
    }
    mesh.alpha_error()
}

fn combinations(values: &[usize], k: usize) -> Vec<Vec<usize>> {
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
        for i in at..=values.len() - (k - acc.len()) {
            acc.push(values[i]);
            visit(values, k, i + 1, acc, out);
            acc.pop();
        }
    }
    let mut out = Vec::new();
    if k <= values.len() {
        visit(values, k, 0, &mut Vec::new(), &mut out);
    }
    out
}

/// Best template for one ring: try 4, 3, 2, 1, 0 cut corners and keep the lowest error.
fn best_ring_layout(
    ring: &[Pt2],
    ang: &[f64],
    cand: &[usize],
    params: &QParams,
) -> Option<Vec<Patch>> {
    let deviation = |i: usize| (ang[i] - FRAC_PI_2).abs();
    let mut best: Option<(f64, Vec<Patch>)> = None;
    for k in [4, 3, 2, 1, 0] {
        let mut combos = combinations(cand, k);
        combos.sort_by(|a, b| {
            let score = |c: &[usize]| c.iter().map(|&i| deviation(i)).sum::<f64>();
            score(a).total_cmp(&score(b))
        });
        let kbest = combos
            .iter()
            .take(params.max_combos)
            .filter_map(|cmb| build_template(ring, cmb, k))
            .map(|patches| (template_error(&patches, params.l_tar), patches))
            .reduce(|best, next| if next.0 < best.0 { next } else { best });
        if let Some(candidate) = kbest {
            if best.as_ref().is_none_or(|(old, _)| candidate.0 < *old) {
                best = Some(candidate);
            }
        }
        if best.as_ref().is_some_and(|(err, _)| *err < params.alpha_th) {
            break;
        }
    }
    best.map(|(_, patches)| patches)
}

/// Turn segmented regions into a layout of four-sided patches.
pub fn build_layout(regions: &[Region], params: &QParams, tol: f64) -> Vec<Patch> {
    let rings: Vec<_> = regions
        .iter()
        .map(|r| clean_ring(&r.outer, tol))
        .collect();
    let angles: Vec<_> = rings.iter().map(|r| interior_angles(r)).collect();
    let is_corner = |a: f64| (a - PI).abs() > params.straight_tol;
    let corners: Vec<Pt2> = rings
        .iter()
        .zip(&angles)
        .flat_map(|(r, a)| r.iter().zip(a).filter(|&(_, &a)| is_corner(a)).map(|(&p, _)| p))
        .collect();

    let mut layout = Vec::new();
    for (ring, ang) in rings.iter().zip(&angles) {
        let real: Vec<usize> = (0..ring.len()).filter(|&i| is_corner(ang[i])).collect();
        if real.len() == 3 {
            layout.extend(three_patch_template(ring, &real));
            continue;
        }
        // Candidate cut vertices: real corners and ones shared with other regions,
        // keeping the `max_corner_cands` closest to 90°.
        let mut cand: Vec<usize> = (0..ring.len())
            .filter(|&i| is_corner(ang[i]) || corners.iter().any(|&p| distance(p, ring[i]) < tol))
            .collect();
        cand.sort_by(|&i, &j| {
            (ang[i] - FRAC_PI_2)
                .abs()
                .total_cmp(&(ang[j] - FRAC_PI_2).abs())
                .then(i.cmp(&j))
        });
        cand.truncate(params.max_corner_cands);
        cand.sort_unstable();
        layout.extend(best_ring_layout(ring, ang, &cand, params).unwrap_or_default());
    }
    layout
}
