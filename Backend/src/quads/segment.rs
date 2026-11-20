use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use opengeometry::geometry::poly2d::{Pt2, dot2};

use super::geom::{boundary_intersections, split_region, valid_chord};
use super::polyline::{
    Polyline, angle, clean_ring, corner_angle, distance, interior_angles, lerp, point,
    point_segment_distance, ring_corners, ring_edges, sub, wrap_quarter,
};
use super::types::{EPS, Region, SegmentParams};

/// Extreme concave vertex: a reflex corner that segmentation chords start from.
#[derive(Clone, Copy, Debug)]
pub struct EcV {
    pub p: Pt2,
    pub prev: Pt2,
    pub next: Pt2,
}

// ------------------------------------------------------- direction fields

/// Preferred (mod 90°) direction taken from the nearest boundary edge.
struct BoundaryField {
    edges: Vec<(Pt2, Pt2, f64)>,
}

impl BoundaryField {
    fn new(region: &Region) -> Self {
        Self {
            edges: region
                .edges()
                .filter(|&(a, b)| distance(a, b) > 0.0)
                .map(|(a, b)| (a, b, angle(sub(b, a))))
                .collect(),
        }
    }

    fn angle_at(&self, p: Pt2) -> f64 {
        self.edges
            .iter()
            .map(|&(a, b, theta)| (point_segment_distance(p, a, b), theta))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0.0, |x| x.1)
    }
}

type Sample = (Pt2, f64, f64);

/// Direction field interpolated from `[x, y, cos 4θ, sin 4θ]` samples with
/// Gaussian weights, truncated at 3σ.
struct SampledField {
    samples: Vec<Sample>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    cell_size: f64,
    sigma2: f64,
}

/// Field angle (mod 90°) from a 4θ vector.
fn quarter_angle(x: f64, y: f64) -> f64 {
    (y.atan2(x) * 0.25).rem_euclid(FRAC_PI_2)
}

impl SampledField {
    fn new(rows: &[[f64; 4]], area: f64, tol: f64) -> Result<Self, String> {
        if rows.is_empty() {
            return Err("field_samples must contain at least one row".to_string());
        }
        let mut samples = Vec::with_capacity(rows.len());
        for row in rows {
            if row.iter().any(|value| !value.is_finite()) {
                return Err("field_samples values must be finite".to_string());
            }
            let norm = row[2].hypot(row[3]);
            if norm <= EPS {
                return Err(
                    "field_samples direction components must have non-zero length".to_string(),
                );
            }
            samples.push((Pt2::new(row[0], row[1]), row[2] / norm, row[3] / norm));
        }

        // Mean sample spacing sets a scale-aware Gaussian bandwidth.
        let sigma = (area / samples.len() as f64).sqrt().max(tol);
        let cell_size = 3.0 * sigma;
        let mut field = Self {
            samples,
            cells: HashMap::new(),
            cell_size,
            sigma2: sigma * sigma,
        };
        for i in 0..field.samples.len() {
            let key = field.cell_of(field.samples[i].0);
            field.cells.entry(key).or_default().push(i);
        }
        Ok(field)
    }

    fn cell_of(&self, p: Pt2) -> (i64, i64) {
        (
            (p.x / self.cell_size).floor() as i64,
            (p.z / self.cell_size).floor() as i64,
        )
    }

    fn dist2(p: Pt2, s: &Sample) -> f64 {
        let (dx, dy) = (p.x - s.0.x, p.z - s.0.z);
        dx * dx + dy * dy
    }

    fn angle_at(&self, p: Pt2) -> f64 {
        // A 3σ cutoff and 3σ cell width only require the 3x3 neighbourhood.
        let (cx, cz) = self.cell_of(p);
        let near: Vec<&Sample> = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dz| (cx + dx, cz + dz)))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .map(|&i| &self.samples[i])
            .collect();

        let cutoff2 = 9.0 * self.sigma2;
        let (mut sum_x, mut sum_y, mut weight_sum) = (0.0, 0.0, 0.0);
        for s in &near {
            let d2 = Self::dist2(p, s);
            if d2 <= cutoff2 {
                let weight = (-0.5 * d2 / self.sigma2).exp();
                sum_x += weight * s.1;
                sum_y += weight * s.2;
                weight_sum += weight;
            }
        }

        let nearest = || {
            let by_dist = |a: &&Sample, b: &&Sample| {
                Self::dist2(p, a).total_cmp(&Self::dist2(p, b))
            };
            let s = if weight_sum > 0.0 {
                near.iter().copied().min_by(by_dist)
            } else {
                self.samples.iter().min_by(by_dist)
            };
            s.map_or((1.0, 0.0), |s| (s.1, s.2))
        };
        let (x, y) = if weight_sum > 0.0 {
            (sum_x / weight_sum, sum_y / weight_sum)
        } else {
            nearest()
        };
        if x.hypot(y) <= EPS {
            let (nx, ny) = nearest();
            return quarter_angle(nx, ny);
        }
        quarter_angle(x, y)
    }
}

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

// -------------------------------------------------------------- geometry

pub fn find_ecvs(rings: &[Polyline], p: &SegmentParams) -> Vec<EcV> {
    rings
        .iter()
        .flat_map(|ring| ring_corners(ring))
        .filter(|&(prev, here, next)| {
            let total = corner_angle(prev, here, next);
            PI + p.ecv_tol < total && total < TAU - EPS
        })
        .map(|(prev, p, next)| EcV { p, prev, next })
        .collect()
}

/// How a chord endpoint divides the boundary angle it lands on.
struct EndSplit {
    first: f64,
    second: f64,
    total: f64,
    at_vertex: bool,
}

impl EndSplit {
    fn new(prev: Pt2, next: Pt2, p: Pt2, dir: Pt2, at_vertex: bool) -> Self {
        let mut total = corner_angle(prev, p, next);
        if total < EPS {
            total = TAU;
        }
        let first = (angle(dir) - angle(sub(next, p))).rem_euclid(TAU);
        Self {
            first,
            second: total - first,
            total,
            at_vertex,
        }
    }

    fn degenerate(&self) -> bool {
        self.first <= EPS || self.second <= EPS
    }
}

/// Split of the boundary angle at `p` by a chord leaving in direction `dir`.
fn end_split(rings: &[Polyline], p: Pt2, dir: Pt2, tol: f64) -> Option<EndSplit> {
    for ring in rings {
        for (prev, here, next) in ring_corners(ring) {
            if distance(here, p) < tol {
                return Some(EndSplit::new(prev, next, p, dir, true));
            }
        }
    }
    for ring in rings {
        for (a, b) in ring_edges(ring) {
            let ab = sub(b, a);
            let len2 = dot2(ab, ab);
            if len2 == 0.0 {
                continue;
            }
            let t = dot2(sub(p, a), ab) / len2;
            if t > 0.0 && t < 1.0 && distance(lerp(a, b, t), p) < tol {
                return Some(EndSplit::new(a, b, p, dir, false));
            }
        }
    }
    None
}

/// Both ends of chord `a-b`, each with the split it produces.
fn chord_splits(rings: &[Polyline], a: Pt2, b: Pt2, tol: f64) -> Option<[EndSplit; 2]> {
    let end = |p, dir| end_split(rings, p, dir, tol).filter(|s| !s.degenerate());
    Some([end(a, sub(b, a))?, end(b, sub(a, b))?])
}

fn angles_ok(rings: &[Polyline], a: Pt2, b: Pt2, p: &SegmentParams, tol: f64) -> bool {
    let Some(ends) = chord_splits(rings, a, b, tol) else {
        return false;
    };
    ends.iter().all(|s| {
        let reflex = s.at_vertex && s.total > PI + p.ecv_tol;
        let high = if reflex && p.relax_reflex {
            PI + p.ecv_tol
        } else {
            p.theta_max
        };
        [s.first, s.second]
            .iter()
            .all(|&x| p.theta_min <= x && x <= high)
    })
}

/// `(max, mean)` deviation of the chord direction from the field along it.
fn misalignment(field: &DirectionField, a: Pt2, b: Pt2, n: usize) -> (f64, f64) {
    let d = angle(sub(b, a));
    let mut max: f64 = 0.0;
    let mut sum = 0.0;
    for i in 0..n {
        let t = 0.05 + 0.9 * i as f64 / (n.saturating_sub(1).max(1)) as f64;
        let dev = wrap_quarter(d - field.angle_at(lerp(a, b, t))).abs();
        max = max.max(dev);
        sum += dev;
    }
    (max, sum / n as f64)
}

/// `(mean deviation of the four sub-angles from the axes, smallest sub-angle)`.
fn angle_dev(rings: &[Polyline], a: Pt2, b: Pt2, tol: f64) -> Option<(f64, f64)> {
    let subs: Vec<f64> = chord_splits(rings, a, b, tol)?
        .iter()
        .flat_map(|s| [s.first, s.second])
        .collect();
    let mean = subs.iter().map(|&s| wrap_quarter(s).abs()).sum::<f64>() / subs.len() as f64;
    Some((mean, subs.into_iter().fold(f64::INFINITY, f64::min)))
}

// ------------------------------------------------------------ candidates

/// Successively looser acceptance rules for ranking chords.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    /// Aligned with the field and with sub-angles in the allowed range.
    Aligned,
    /// Sub-angles in range; alignment only affects the score.
    Angles,
    /// Any chord whose smallest sub-angle is at least 20°.
    MinAngle,
}

const STAGES: [Stage; 3] = [Stage::Aligned, Stage::Angles, Stage::MinAngle];

/// Where chord candidates come from.
#[derive(Clone, Copy, PartialEq)]
enum ChordSource {
    /// Chords joining pairs of reflex vertices.
    TwoEcv,
    /// Rays shot from a reflex vertex to the first boundary hit.
    Rays,
}

#[derive(Clone, Copy)]
struct Candidate {
    score: f64,
    a: Pt2,
    b: Pt2,
}

fn rank_candidates(
    rings: &[Polyline],
    candidates: &[(Pt2, Pt2)],
    field: &DirectionField,
    p: &SegmentParams,
    tol: f64,
    stage: Stage,
) -> Vec<Candidate> {
    let mut out: Vec<_> = candidates
        .iter()
        .filter_map(|&(a, b)| {
            let (max_mis, mean_mis) = misalignment(field, a, b, p.n_samples);
            let score = match stage {
                Stage::Aligned => {
                    if max_mis >= p.beta_f || !angles_ok(rings, a, b, p, tol) {
                        return None;
                    }
                    mean_mis
                }
                Stage::Angles | Stage::MinAngle => {
                    let (dev, min_sub) = angle_dev(rings, a, b, tol)?;
                    let admissible = match stage {
                        Stage::Angles => angles_ok(rings, a, b, p, tol),
                        _ => min_sub >= PI / 9.0,
                    };
                    if !admissible {
                        return None;
                    }
                    mean_mis + dev
                }
            };
            Some(Candidate { score, a, b })
        })
        .collect();
    out.sort_by(|a, b| a.score.total_cmp(&b.score));
    out
}

fn ray_candidates(region: &Region, ecvs: &[EcV], p: &SegmentParams, tol: f64) -> Vec<(Pt2, Pt2)> {
    let radius = 2.0 * region.scale();
    let n_rays = (TAU / p.ray_step).round() as usize;
    let mut candidates = Vec::new();
    for ecv in ecvs {
        let mut angles: Vec<f64> = (0..n_rays).map(|k| k as f64 * p.ray_step).collect();
        if p.edge_rays {
            angles.push(angle(sub(ecv.p, ecv.prev)));
            angles.push(angle(sub(ecv.p, ecv.next)));
        }
        for theta in angles {
            let dir = point(theta.cos(), theta.sin());
            let hit = boundary_intersections(region, ecv.p, dir, tol)
                .into_iter()
                .filter(|(t, _)| *t > tol && *t <= radius)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, end)) = hit {
                if valid_chord(region, ecv.p, end, tol) {
                    candidates.push((ecv.p, end));
                }
            }
        }
    }
    candidates
}

fn two_ecv_candidates(region: &Region, ecvs: &[EcV], tol: f64) -> Vec<(Pt2, Pt2)> {
    let mut out = Vec::new();
    for (i, a) in ecvs.iter().enumerate() {
        for b in &ecvs[i + 1..] {
            if valid_chord(region, a.p, b.p, tol) {
                out.push((a.p, b.p));
            }
        }
    }
    out
}

/// Lower is better: corner deviation from 90°, corner-count deviation from 4,
/// and a penalty for sharp corners.
fn piece_cost(region: &Region, tol: f64) -> f64 {
    let ring = clean_ring(&region.outer, tol);
    let corners: Vec<f64> = interior_angles(&ring)
        .into_iter()
        .filter(|a| (a - PI).abs() >= 10.0_f64.to_radians())
        .collect();
    let (avg, sharp) = if corners.is_empty() {
        (0.0, 0.0)
    } else {
        let min_corner = corners.iter().copied().fold(PI, f64::min);
        (
            corners.iter().map(|a| (a - FRAC_PI_2).abs()).sum::<f64>() / corners.len() as f64,
            (FRAC_PI_4 - min_corner).max(0.0),
        )
    };
    avg + 0.3 * (corners.len() as f64 - 4.0).abs() + 0.5 * sharp
}

// ------------------------------------------------------------- splitting

/// Pick the split for the ranked candidates: the first that works when
/// `Aligned`, otherwise the cheapest among the top `relax_top_k`.
fn pick_split(
    region: &Region,
    ranked: &[Candidate],
    p: &SegmentParams,
    stage: Stage,
    tol: f64,
    min_area: f64,
) -> Option<Vec<Region>> {
    let split = |c: &Candidate| {
        Some(split_region(region, c.a, c.b, tol, min_area)).filter(|parts| parts.len() >= 2)
    };
    if stage == Stage::Aligned {
        return ranked.iter().find_map(split);
    }
    ranked
        .iter()
        .take(p.relax_top_k)
        .filter_map(|c| {
            let parts = split(c)?;
            let mean_cost = parts.iter().map(|r| piece_cost(r, tol)).sum::<f64>()
                / parts.len() as f64;
            Some((mean_cost + c.score, parts))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, parts)| parts)
}

fn try_split(
    region: &Region,
    field: &DirectionField,
    p: &SegmentParams,
    source: ChordSource,
    tol: f64,
    min_area: f64,
) -> Option<Vec<Region>> {
    let rings: Vec<Polyline> = region.rings().map(|r| clean_ring(r, tol)).collect();
    let ecvs = find_ecvs(&rings, p);
    if ecvs.is_empty() || (source == ChordSource::TwoEcv && ecvs.len() < 2) {
        return None;
    }
    let candidates = match source {
        ChordSource::TwoEcv => two_ecv_candidates(region, &ecvs, tol),
        ChordSource::Rays => ray_candidates(region, &ecvs, p, tol),
    };
    let stages = if source == ChordSource::Rays && p.relax_if_stuck {
        &STAGES[..]
    } else {
        &STAGES[..1]
    };
    stages.iter().find_map(|&stage| {
        let ranked = rank_candidates(&rings, &candidates, field, p, tol, stage);
        pick_split(region, &ranked, p, stage, tol, min_area)
    })
}

fn recurse(
    regions: Vec<Region>,
    field: &DirectionField,
    p: &SegmentParams,
    source: ChordSource,
    tol: f64,
    min_area: f64,
) -> Vec<Region> {
    let mut stack = regions;
    let mut done = Vec::new();
    while let Some(region) = stack.pop() {
        match try_split(&region, field, p, source, tol, min_area) {
            Some(parts) => stack.extend(parts),
            None => done.push(region),
        }
    }
    done
}

/// Recursively cut `domain` into near-rectangular regions.
pub fn segment_domain(
    domain: Region,
    p: &SegmentParams,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<Vec<Region>, String> {
    if !domain.holes.is_empty() {
        return Err("the parity prototype currently segments exterior-only domains".to_string());
    }
    let tol = EPS * domain.scale();
    let min_area = tol * tol;
    let field = match field_samples {
        Some(rows) => DirectionField::Samples(SampledField::new(rows, domain.area(), tol)?),
        None => DirectionField::Boundary(BoundaryField::new(&domain)),
    };
    let regions = recurse(vec![domain], &field, p, ChordSource::TwoEcv, tol, min_area);
    Ok(recurse(regions, &field, p, ChordSource::Rays, tol, min_area))
}
