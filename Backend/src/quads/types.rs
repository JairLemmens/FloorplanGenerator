use super::polyline::{Polyline, sub};
pub use crate::topology::topology::QuadMesh;
use opengeometry::geometry::poly2d::{Pt2, point_in_ring2, signed_area2};

#[derive(Clone, Debug)]
pub struct Region {
    pub outer: Polyline,
    pub holes: Vec<Polyline>,
}

impl Region {
    pub fn simple(outer: Polyline) -> Self {
        Self {
            outer,
            holes: Vec::new(),
        }
    }

    pub fn orient_ring(ring: &mut [Pt2], ccw: bool) {
        if (signed_area2(ring) > 0.0) != ccw {
            ring.reverse();
        }
    }

    pub fn orient(&mut self) {
        Self::orient_ring(&mut self.outer, true);
        for hole in &mut self.holes {
            Self::orient_ring(hole, false);
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

    pub fn bounds(&self) -> [f64; 4] {
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for ring in self.rings() {
            let r = super::polyline::bounds(ring);
            b[0] = b[0].min(r[0]);
            b[1] = b[1].min(r[1]);
            b[2] = b[2].max(r[2]);
            b[3] = b[3].max(r[3]);
        }
        b
    }

    pub fn scale(&self) -> f64 {
        let b = self.bounds();
        (b[2] - b[0]).hypot(b[3] - b[1])
    }

    pub fn corners(&self, ang_tol: f64) -> Vec<Pt2> {
        let mut out = Vec::new();
        for ring in self.rings() {
            for i in 0..ring.len() {
                let a = sub(ring[i], ring[(i + ring.len() - 1) % ring.len()]);
                let b = sub(ring[(i + 1) % ring.len()], ring[i]);
                if (a.x * b.z - a.z * b.x).atan2(a.x * b.x + a.z * b.z).abs() > ang_tol {
                    out.push(ring[i]);
                }
            }
        }
        out
    }
}

pub type Patch = [Polyline; 4];

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
    pub eps_rel: f64,
    pub relax_reflex: bool,
    pub max_hole_samples: usize,
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
            eps_rel: 1.0e-7,
            relax_reflex: true,
            max_hole_samples: 16,
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
