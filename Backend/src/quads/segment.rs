use std::collections::HashMap;

use opengeometry::geometry::poly2d::Pt2;

use super::geom::{TWO_PI, split_region, valid_chord};
use super::polyline::{angle, distance, point, point_segment_distance, sub};
use super::types::{Region, SegmentParams};

#[derive(Clone, Copy, Debug)]
pub struct EcV {
    pub p: Pt2,
    pub prev: Pt2,
    pub next: Pt2,
}

#[derive(Clone)]
struct BoundaryField {
    edges: Vec<(Pt2, Pt2, f64)>,
}

impl BoundaryField {
    fn new(region: &Region) -> Self {
        let mut edges = Vec::new();
        for ring in region.rings() {
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                if distance(a, b) > 0.0 {
                    edges.push((a, b, angle(sub(b, a))));
                }
            }
        }
        Self { edges }
    }

    fn angle_at(&self, p: Pt2) -> f64 {
        self.edges
            .iter()
            .map(|&(a, b, theta)| (point_segment_distance(p, a, b), theta))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1)
            .unwrap_or(0.0)
    }
}

#[derive(Clone)]
struct SampledField {
    samples: Vec<(Pt2, f64, f64)>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    cell_size: f64,
    sigma2: f64,
}

impl SampledField {
    fn new(rows: &[[f64; 4]], area: f64) -> Result<Self, String> {
        if rows.is_empty() {
            return Err("field_samples must contain at least one row".to_string());
        }
        let mut samples = Vec::with_capacity(rows.len());
        for row in rows {
            if row.iter().any(|value| !value.is_finite()) {
                return Err("field_samples values must be finite".to_string());
            }
            let norm = row[2].hypot(row[3]);
            if norm <= 1.0e-12 {
                return Err(
                    "field_samples direction components must have non-zero length".to_string(),
                );
            }
            samples.push((Pt2::new(row[0], row[1]), row[2] / norm, row[3] / norm));
        }

        // Mean sample spacing sets a scale-aware Gaussian bandwidth.
        let sigma = (area / samples.len() as f64).sqrt().max(1.0e-12);
        let cell_size = 3.0 * sigma;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, (p, _, _)) in samples.iter().enumerate() {
            let key = (
                (p.x / cell_size).floor() as i64,
                (p.z / cell_size).floor() as i64,
            );
            cells.entry(key).or_default().push(i);
        }
        Ok(Self {
            samples,
            cells,
            cell_size,
            sigma2: sigma * sigma,
        })
    }

    fn angle_at(&self, p: Pt2) -> f64 {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut weight_sum = 0.0;
        let mut nearest = (f64::INFINITY, 1.0, 0.0);
        let cutoff2 = 9.0 * self.sigma2;
        let base = (
            (p.x / self.cell_size).floor() as i64,
            (p.z / self.cell_size).floor() as i64,
        );

        // A 3-sigma cutoff and 3-sigma cell width only require the 3x3 neighborhood.
        for dx_cell in -1..=1 {
            for dy_cell in -1..=1 {
                if let Some(indices) = self.cells.get(&(base.0 + dx_cell, base.1 + dy_cell)) {
                    for &i in indices {
                        let (q, x, y) = self.samples[i];
                        let dx = p.x - q.x;
                        let dy = p.z - q.z;
                        let d2 = dx * dx + dy * dy;
                        if d2 < nearest.0 {
                            nearest = (d2, x, y);
                        }
                        if d2 <= cutoff2 {
                            let weight = (-0.5 * d2 / self.sigma2).exp();
                            sum_x += weight * x;
                            sum_y += weight * y;
                            weight_sum += weight;
                        }
                    }
                }
            }
        }

        if weight_sum == 0.0 {
            nearest = self.samples.iter().fold(nearest, |best, &(q, x, y)| {
                let dx = p.x - q.x;
                let dy = p.z - q.z;
                let d2 = dx * dx + dy * dy;
                if d2 < best.0 { (d2, x, y) } else { best }
            });
        }
        let (x, y) = if weight_sum > 0.0 {
            (sum_x / weight_sum, sum_y / weight_sum)
        } else {
            (nearest.1, nearest.2)
        };
        if x.hypot(y) <= 1.0e-12 {
            return (nearest.2.atan2(nearest.1) * 0.25).rem_euclid(std::f64::consts::FRAC_PI_2);
        }
        (y.atan2(x) * 0.25).rem_euclid(std::f64::consts::FRAC_PI_2)
    }
}

#[derive(Clone)]
enum DirectionField {
    Boundary(BoundaryField),
    Samples(SampledField),
}

impl DirectionField {
    fn angle_at(&self, p: Pt2) -> f64 {
        match self {
            Self::Boundary(field) => field.angle_at(p),
            Self::Samples(field) => field.angle_at(p),
        }
    }
}

fn rings(region: &Region, micro: f64) -> Vec<Vec<Pt2>> {
    region
        .rings()
        .map(|r| super::layout::clean_ring_for_segmentation(r, micro))
        .collect()
}

pub fn find_ecvs(rings: &[Vec<Pt2>], p: &SegmentParams) -> Vec<(usize, EcV)> {
    let mut out = Vec::new();
    for (ri, ring) in rings.iter().enumerate() {
        for i in 0..ring.len() {
            let prev = ring[(i + ring.len() - 1) % ring.len()];
            let here = ring[i];
            let next = ring[(i + 1) % ring.len()];
            let total = (angle(sub(prev, here)) - angle(sub(next, here))).rem_euclid(TWO_PI);
            if std::f64::consts::PI + p.ecv_tol < total && total < TWO_PI - 1e-9 {
                out.push((
                    ri,
                    EcV {
                        p: here,
                        prev,
                        next,
                    },
                ));
            }
        }
    }
    out
}

fn split_angles(prev: Pt2, next: Pt2, p: Pt2, dir: Pt2, is_vertex: bool) -> (f64, f64, f64, bool) {
    let mut total = (angle(sub(prev, p)) - angle(sub(next, p))).rem_euclid(TWO_PI);
    if total < 1e-9 {
        total = TWO_PI;
    }
    let first = (angle(dir) - angle(sub(next, p))).rem_euclid(TWO_PI);
    (first, total - first, total, is_vertex)
}

fn endpoint_angles(
    rings: &[Vec<Pt2>],
    p: Pt2,
    dir: Pt2,
    tol: f64,
) -> Option<(f64, f64, f64, bool)> {
    for ring in rings {
        for i in 0..ring.len() {
            if distance(ring[i], p) < tol {
                return Some(split_angles(
                    ring[(i + ring.len() - 1) % ring.len()],
                    ring[(i + 1) % ring.len()],
                    p,
                    dir,
                    true,
                ));
            }
        }
    }
    for ring in rings {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let ab = sub(b, a);
            let len2 = ab.x * ab.x + ab.z * ab.z;
            if len2 == 0.0 {
                continue;
            }
            let t = ((p.x - a.x) * ab.x + (p.z - a.z) * ab.z) / len2;
            if t > 0.0 && t < 1.0 && distance(point(a.x + t * ab.x, a.z + t * ab.z), p) < tol {
                return Some(split_angles(a, b, p, dir, false));
            }
        }
    }
    None
}

fn angles_ok(rings: &[Vec<Pt2>], a: Pt2, b: Pt2, p: &SegmentParams, tol: f64) -> bool {
    let u = sub(b, a);
    for (point, dir) in [(a, u), (b, sub(a, b))] {
        let Some((s1, s2, total, is_vertex)) = endpoint_angles(rings, point, dir, tol) else {
            return false;
        };
        if s1 <= 1e-9 || s2 <= 1e-9 {
            return false;
        }
        let reflex = is_vertex && total > std::f64::consts::PI + p.ecv_tol;
        let high = if reflex && p.relax_reflex {
            std::f64::consts::PI + p.ecv_tol
        } else {
            p.theta_max
        };
        if !(p.theta_min <= s1 && s1 <= high && p.theta_min <= s2 && s2 <= high) {
            return false;
        }
    }
    true
}

fn misalignment(field: &DirectionField, a: Pt2, b: Pt2, n: usize) -> (f64, f64) {
    let d = angle(sub(b, a));
    let mut max: f64 = 0.0;
    let mut sum = 0.0;
    for i in 0..n {
        let t = 0.05 + 0.9 * i as f64 / (n.saturating_sub(1).max(1)) as f64;
        let p = point(a.x + t * (b.x - a.x), a.z + t * (b.z - a.z));
        let theta = field.angle_at(p);
        let diff = (d - theta + std::f64::consts::PI / 4.0).rem_euclid(std::f64::consts::FRAC_PI_2)
            - std::f64::consts::PI / 4.0;
        let dev = diff.abs();
        max = max.max(dev);
        sum += dev;
    }
    (max, sum / n as f64)
}

fn angle_dev(rings: &[Vec<Pt2>], a: Pt2, b: Pt2, tol: f64) -> Option<(f64, f64)> {
    let mut devs = Vec::with_capacity(4);
    let mut subs = Vec::with_capacity(4);
    for (p, dir) in [(a, sub(b, a)), (b, sub(a, b))] {
        let (s1, s2, _, _) = endpoint_angles(rings, p, dir, tol)?;
        if s1 <= 1e-9 || s2 <= 1e-9 {
            return None;
        }
        for s in [s1, s2] {
            subs.push(s);
            devs.push(
                ((s + std::f64::consts::PI / 4.0).rem_euclid(std::f64::consts::FRAC_PI_2)
                    - std::f64::consts::PI / 4.0)
                    .abs(),
            );
        }
    }
    Some((
        devs.iter().sum::<f64>() / devs.len() as f64,
        subs.into_iter().fold(f64::INFINITY, f64::min),
    ))
}

#[derive(Clone, Copy)]
struct Candidate {
    score: f64,
    a: Pt2,
    b: Pt2,
}

fn rank_candidates(
    rings: &[Vec<Pt2>],
    candidates: &[(Pt2, Pt2)],
    field: &DirectionField,
    p: &SegmentParams,
    eps: f64,
    stage: u8,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for &(a, b) in candidates {
        let (max_mis, mean_mis) = misalignment(field, a, b, p.n_samples);
        let score = if stage == 0 {
            if max_mis >= p.beta_f || !angles_ok(rings, a, b, p, 10.0 * eps) {
                continue;
            }
            mean_mis
        } else {
            let Some((dev, min_sub)) = angle_dev(rings, a, b, 10.0 * eps) else {
                continue;
            };
            if stage == 1 && !angles_ok(rings, a, b, p, 10.0 * eps) {
                continue;
            }
            if stage == 2 && min_sub < std::f64::consts::PI / 9.0 {
                continue;
            }
            mean_mis + dev
        };
        out.push(Candidate { score, a, b });
    }
    out.sort_by(|a, b| a.score.total_cmp(&b.score));
    out
}

fn ray_candidates(region: &Region, ecvs: &[EcV], p: &SegmentParams, eps: f64) -> Vec<(Pt2, Pt2)> {
    let b = region.bounds();
    let radius = 2.0 * (b[2] - b[0]).hypot(b[3] - b[1]);
    let n_rays = (TWO_PI / p.ray_step).round() as usize;
    let field = region;
    let mut candidates = Vec::new();
    for ecv in ecvs {
        let mut angles: Vec<f64> = (0..n_rays).map(|k| k as f64 * p.ray_step).collect();
        if p.edge_rays {
            angles.push(angle(sub(ecv.p, ecv.prev)));
            angles.push(angle(sub(ecv.p, ecv.next)));
        }
        for theta in angles {
            let d = point(theta.cos(), theta.sin());
            let points = super::geom::boundary_intersections(field, ecv.p, d, eps);
            let mut forward: Vec<_> = points
                .into_iter()
                .filter(|(t, _)| *t > 10.0 * eps && *t <= radius)
                .collect();
            forward.sort_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, endpoint)) = forward.first() {
                if valid_chord(region, ecv.p, *endpoint, eps) {
                    candidates.push((ecv.p, *endpoint));
                }
            }
        }
    }
    candidates
}

fn two_ecv_candidates(region: &Region, ecvs: &[EcV], eps: f64) -> Vec<(Pt2, Pt2)> {
    let mut out = Vec::new();
    for i in 0..ecvs.len() {
        for j in i + 1..ecvs.len() {
            if valid_chord(region, ecvs[i].p, ecvs[j].p, eps) {
                out.push((ecvs[i].p, ecvs[j].p));
            }
        }
    }
    out
}

fn piece_cost(region: &Region, micro: f64) -> f64 {
    let ring = super::layout::clean_ring_for_segmentation(&region.outer, micro);
    let mut devs = Vec::new();
    let mut min_corner = std::f64::consts::PI;
    for i in 0..ring.len() {
        let prev = ring[(i + ring.len() - 1) % ring.len()];
        let here = ring[i];
        let next = ring[(i + 1) % ring.len()];
        let a = (angle(sub(prev, here)) - angle(sub(next, here))).rem_euclid(TWO_PI);
        if (a - std::f64::consts::PI).abs() < 10.0_f64.to_radians() {
            continue;
        }
        devs.push((a - std::f64::consts::FRAC_PI_2).abs());
        min_corner = min_corner.min(a);
    }
    let sharp = if devs.is_empty() {
        0.0
    } else {
        (std::f64::consts::FRAC_PI_4 - min_corner).max(0.0)
    };
    let avg = if devs.is_empty() {
        0.0
    } else {
        devs.iter().sum::<f64>() / devs.len() as f64
    };
    avg + 0.3 * (devs.len() as f64 - 4.0).abs() + 0.5 * sharp
}

fn try_split(
    region: &Region,
    field: &DirectionField,
    p: &SegmentParams,
    method_two: bool,
    eps: f64,
    min_area: f64,
) -> Option<Vec<Region>> {
    let ring_set = rings(region, 1e3 * eps);
    let ecvs: Vec<_> = find_ecvs(&ring_set, p)
        .into_iter()
        .map(|(_, e)| e)
        .collect();
    if ecvs.is_empty() || (method_two && ecvs.len() < 2) {
        return None;
    }
    let candidates = if method_two {
        two_ecv_candidates(region, &ecvs, eps)
    } else {
        ray_candidates(region, &ecvs, p, eps)
    };
    let stages: &[u8] = if !method_two && p.relax_if_stuck {
        &[0, 1, 2]
    } else {
        &[0]
    };
    for &stage in stages {
        let ranked = rank_candidates(&ring_set, &candidates, field, p, eps, stage);
        if stage == 0 {
            for c in ranked {
                let parts = split_region(region, c.a, c.b, eps, min_area);
                if parts.len() >= 2 {
                    return Some(parts);
                }
            }
        } else {
            let mut best: Option<(f64, Vec<Region>)> = None;
            for c in ranked.iter().take(p.relax_top_k) {
                let parts = split_region(region, c.a, c.b, eps, min_area);
                if parts.len() < 2 {
                    continue;
                }
                let cost = parts.iter().map(|r| piece_cost(r, 1e3 * eps)).sum::<f64>()
                    / parts.len() as f64
                    + c.score;
                if best.as_ref().is_none_or(|(old, _)| cost < *old) {
                    best = Some((cost, parts));
                }
            }
            if let Some((_, parts)) = best {
                return Some(parts);
            }
        }
    }
    None
}

fn recurse(
    regions: Vec<Region>,
    field: &DirectionField,
    p: &SegmentParams,
    method_two: bool,
    eps: f64,
    min_area: f64,
) -> Vec<Region> {
    let mut stack = regions;
    let mut final_regions = Vec::new();
    while let Some(region) = stack.pop() {
        if let Some(parts) = try_split(&region, field, p, method_two, eps, min_area) {
            stack.extend(parts);
        } else {
            final_regions.push(region);
        }
    }
    final_regions
}

pub fn segment_domain(
    domain: Region,
    p: &SegmentParams,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<Vec<Region>, String> {
    let scale = domain.scale();
    let eps = p.eps_rel * scale;
    let min_area = eps * eps;
    let field = match field_samples {
        Some(rows) => DirectionField::Samples(SampledField::new(rows, domain.area())?),
        None => DirectionField::Boundary(BoundaryField::new(&domain)),
    };
    if !domain.holes.is_empty() {
        return Err("the parity prototype currently segments exterior-only domains".to_string());
    }
    let mut regions = recurse(vec![domain], &field, p, true, eps, min_area);
    regions = recurse(regions, &field, p, false, eps, min_area);
    Ok(regions)
}
