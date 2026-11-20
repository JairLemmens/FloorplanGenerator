use super::polyline::{self, Polyline, ring_edges, turn};
pub use crate::topology::topology::QuadMesh;
use opengeometry::geometry::poly2d::{Pt2, point_in_ring2, signed_area2};

#[derive(Clone, Debug)]
pub struct Region {
    pub outer: Polyline,
    pub holes: Vec<Polyline>,
}

fn orient_ring(ring: &mut [Pt2], ccw: bool) {
    if (signed_area2(ring) > 0.0) != ccw {
        ring.reverse();
    }
}

impl Region {
    pub fn simple(outer: Polyline) -> Self {
        Self {
            outer,
            holes: Vec::new(),
        }
    }

    /// Counter-clockwise outer ring, clockwise holes.
    pub fn orient(&mut self) {
        orient_ring(&mut self.outer, true);
        for hole in &mut self.holes {
            orient_ring(hole, false);
        }
    }

    pub fn area(&self) -> f64 {
        signed_area2(&self.outer).abs()
            - self
                .holes
                .iter()
                .map(|r| signed_area2(r).abs())
                .sum::<f64>()
    }

    pub fn contains(&self, p: Pt2) -> bool {
        point_in_ring2(p, &self.outer) && !self.holes.iter().any(|h| point_in_ring2(p, h))
    }

    pub fn rings(&self) -> impl Iterator<Item = &[Pt2]> {
        std::iter::once(self.outer.as_slice()).chain(self.holes.iter().map(Vec::as_slice))
    }

    /// Directed boundary edges of every ring.
    pub fn edges(&self) -> impl Iterator<Item = (Pt2, Pt2)> + '_ {
        self.rings().flat_map(ring_edges)
    }

    pub fn bounds(&self) -> [f64; 4] {
        polyline::bounds(self.rings().flatten())
    }

    /// Bounding-box diagonal; all tolerances are expressed relative to it.
    pub fn scale(&self) -> f64 {
        polyline::diagonal(self.rings().flatten())
    }

    /// Ring vertices turning by more than `ang_tol`.
    pub fn corners(&self, ang_tol: f64) -> Vec<Pt2> {
        self.rings()
            .flat_map(polyline::ring_corners)
            .filter(|&(prev, here, next)| turn(prev, here, next).abs() > ang_tol)
            .map(|(_, here, _)| here)
            .collect()
    }
}

/// The single numerical tolerance of the quad pipeline.
///
/// Lengths are compared against `EPS * scale` (`scale` = bounding-box diagonal
/// of the domain), areas against `(EPS * scale)^2` or `EPS * l_tar^2`, and
/// angles (radians) and unit-interval parameters against `EPS` itself.
pub const EPS: f64 = 1e-6;

pub type Patch = [Polyline; 4];

/// Rotate a patch so side `k` comes first.
pub fn rotate_patch(patch: &Patch, k: usize) -> Patch {
    std::array::from_fn(|i| patch[(k + i) % 4].clone())
}

#[derive(Clone, Debug)]
pub struct QParams {
    pub l_tar: f64,
    pub alpha_th: f64,
    pub straight_tol: f64,
    pub max_corner_cands: usize,
    pub max_combos: usize,
    pub l_mer_factor: f64,
    pub max_t_iter: usize,
    pub hausdorff_tol: Option<f64>,
    pub max_outer_iter: usize,
    pub collapse_ratio: f64,
    pub split_ratio: f64,
    pub max_ops_per_iter: usize,
}

impl Default for QParams {
    fn default() -> Self {
        Self {
            l_tar: 30.0,
            alpha_th: 0.5,
            straight_tol: 20.0_f64.to_radians(),
            max_corner_cands: 10,
            max_combos: 40,
            l_mer_factor: 1.0,
            max_t_iter: 300,
            hausdorff_tol: None,
            max_outer_iter: 10,
            collapse_ratio: 2.0 / 3.0,
            split_ratio: 4.0 / 3.0,
            max_ops_per_iter: 200,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SegmentParams {
    pub ray_step: f64,
    pub theta_min: f64,
    pub theta_max: f64,
    pub beta_f: f64,
    pub ecv_tol: f64,
    pub n_samples: usize,
    pub relax_reflex: bool,
    pub edge_rays: bool,
    pub relax_if_stuck: bool,
    pub relax_top_k: usize,
}

impl Default for SegmentParams {
    fn default() -> Self {
        Self {
            ray_step: std::f64::consts::PI / 36.0,
            theta_min: std::f64::consts::PI / 3.0,
            theta_max: 2.0 * std::f64::consts::PI / 3.0,
            beta_f: std::f64::consts::PI / 12.0,
            ecv_tol: 15.0_f64.to_radians(),
            n_samples: 25,
            relax_reflex: true,
            edge_rays: true,
            relax_if_stuck: true,
            relax_top_k: 40,
        }
    }
}

#[derive(Debug)]
pub struct PipelineTrace {
    pub regions: Vec<Region>,
    pub patches: Vec<Patch>,
    pub initial: QuadMesh,
    pub mesh: QuadMesh,
}
