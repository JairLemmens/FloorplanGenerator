pub mod conform;
pub mod geom;
pub mod layout;
pub mod mesh;
pub mod polyline;
pub mod segment;
pub mod simplify;
pub mod types;
mod union_find;

use crate::base_types::Id;
use opengeometry::geometry::poly2d::{Pt2, dedupe_ring, self_intersects2, signed_area2};

use self::types::{PipelineTrace, QParams, QuadMesh, Region, SegmentParams};

#[derive(Debug)]
pub enum MeshError {
    TooFewVertices,
    NonFiniteCoordinate,
    DegenerateRing,
    SelfIntersectingRing,
    InvalidTargetLength,
    InvalidFieldSamples(String),
    UnsupportedHoles,
    Pipeline(String),
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooFewVertices => "coords must define at least three distinct vertices",
            Self::NonFiniteCoordinate => "coordinates must be finite",
            Self::DegenerateRing => "polygon ring has zero area",
            Self::SelfIntersectingRing => "polygon ring self-intersects",
            Self::InvalidTargetLength => "l_tar must be finite and greater than zero",
            Self::InvalidFieldSamples(message) => return f.write_str(message),
            Self::UnsupportedHoles => "the parity pipeline currently accepts an exterior ring only",
            Self::Pipeline(message) => message,
        })
    }
}

impl std::error::Error for MeshError {}

fn input_region(coords: &[Pt2]) -> Result<Region, MeshError> {
    if coords.iter().any(|p| !p.x.is_finite() || !p.z.is_finite()) {
        return Err(MeshError::NonFiniteCoordinate);
    }
    let mut ring = dedupe_ring(coords, 1e-12);
    if ring.len() < 3 {
        return Err(MeshError::TooFewVertices);
    }
    if signed_area2(&ring).abs() < 1e-12 {
        return Err(MeshError::DegenerateRing);
    }
    let scale = super::quads::polyline::diagonal(&ring);
    if self_intersects2(&ring, 1e-12 * scale.max(1.0)) {
        return Err(MeshError::SelfIntersectingRing);
    }
    if signed_area2(&ring) < 0.0 {
        ring[1..].reverse();
    }
    Ok(Region::simple(ring))
}

pub fn quad_mesh_from_coords(coords: &[Pt2], l_tar: f64) -> Result<QuadMesh, MeshError> {
    quad_mesh_from_coords_with_field(coords, l_tar, None)
}

pub fn quad_mesh_from_coords_with_field(
    coords: &[Pt2],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<QuadMesh, MeshError> {
    if !l_tar.is_finite() || l_tar <= 0.0 {
        return Err(MeshError::InvalidTargetLength);
    }
    Ok(quad_pipeline_with_field(coords, l_tar, field_samples)?.mesh)
}

pub fn quad_pipeline(coords: &[Pt2], l_tar: f64) -> Result<PipelineTrace, MeshError> {
    quad_pipeline_with_field(coords, l_tar, None)
}

/// Segment a polygon into regions without building quadrilateral patches.
pub fn segment_domain(
    coords: &[Pt2],
    field_samples: Option<&[[f64; 4]]>,
) -> Result<Vec<Region>, MeshError> {
    let domain = input_region(coords)?;
    segment::segment_domain(domain, &SegmentParams::default(), field_samples)
        .map_err(MeshError::InvalidFieldSamples)
}

pub fn quad_pipeline_with_field(
    coords: &[Pt2],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<PipelineTrace, MeshError> {
    if !l_tar.is_finite() || l_tar <= 0.0 {
        return Err(MeshError::InvalidTargetLength);
    }
    let domain = input_region(coords)?;
    let scale = domain.scale();
    let qparams = QParams {
        l_tar,
        ..QParams::default()
    };
    let sparams = SegmentParams::default();
    let regions = segment::segment_domain(domain.clone(), &sparams, field_samples)
        .map_err(MeshError::InvalidFieldSamples)?;
    let patches = layout::build_layout(&regions, &qparams);
    let (patches, _) = conform::remove_tjunctions(patches, &qparams, scale);
    let patches = conform::refine_bad_patches(patches, &qparams, scale);
    let counts = conform::choose_counts(&patches, &qparams, scale);
    let initial = conform::mesh_layout(&patches, &counts, &qparams, scale);
    let mesh = initial.clone().simplify(&domain, &qparams);
    Ok(PipelineTrace {
        regions,
        patches,
        initial,
        mesh,
    })
}

pub fn mesh_domain(outer: &[Pt2], holes: &[Vec<Pt2>]) -> Result<QuadMesh, MeshError> {
    if !holes.is_empty() {
        return Err(MeshError::UnsupportedHoles);
    }
    quad_mesh_from_coords(outer, 30.0)
}

pub fn mesh_domain_with_field(
    outer: &[Pt2],
    holes: &[Vec<Pt2>],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<QuadMesh, MeshError> {
    if !holes.is_empty() {
        return Err(MeshError::UnsupportedHoles);
    }
    quad_mesh_from_coords_with_field(outer, l_tar, field_samples)
}

pub fn signed_quad_area(mesh: &QuadMesh, q: [Id; 4]) -> f64 {
    (0..4)
        .map(|i| {
            let a = mesh.point(q[i]).unwrap();
            let b = mesh.point(q[(i + 1) % 4]).unwrap();
            a.x * b.z - b.x * a.z
        })
        .sum::<f64>()
        * 0.5
}
use crate::topology::ops::QuadMeshOps;
