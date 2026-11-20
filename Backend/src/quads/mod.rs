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
use crate::topology::ops::QuadMeshOps;
use opengeometry::geometry::poly2d::{Pt2, dedupe_ring, self_intersects2, signed_area2};

use self::mesh::Quads;
use self::types::{EPS, PipelineTrace, QParams, QuadMesh, Region, SegmentParams};

#[derive(Debug)]
pub enum MeshError {
    TooFewVertices,
    NonFiniteCoordinate,
    DegenerateRing,
    SelfIntersectingRing,
    InvalidTargetLength,
    InvalidFieldSamples(String),
    UnsupportedHoles,
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooFewVertices => "coords must define at least three distinct vertices",
            Self::NonFiniteCoordinate => "coordinates must be finite",
            Self::DegenerateRing => "polygon ring has zero area",
            Self::SelfIntersectingRing => "polygon ring self-intersects",
            Self::InvalidTargetLength => "l_tar must be finite and greater than zero",
            Self::InvalidFieldSamples(message) => message,
            Self::UnsupportedHoles => "the parity pipeline currently accepts an exterior ring only",
        })
    }
}

impl std::error::Error for MeshError {}

/// Validate the input ring and return it as a counter-clockwise region.
fn input_region(coords: &[Pt2]) -> Result<Region, MeshError> {
    if coords.iter().any(|p| !p.x.is_finite() || !p.z.is_finite()) {
        return Err(MeshError::NonFiniteCoordinate);
    }
    let tol = EPS * polyline::diagonal(coords);
    let mut ring = dedupe_ring(coords, tol);
    if ring.len() < 3 {
        return Err(MeshError::TooFewVertices);
    }
    if signed_area2(&ring).abs() < tol * tol {
        return Err(MeshError::DegenerateRing);
    }
    if self_intersects2(&ring, tol) {
        return Err(MeshError::SelfIntersectingRing);
    }
    if signed_area2(&ring) < 0.0 {
        ring[1..].reverse();
    }
    Ok(Region::simple(ring))
}

/// Segment a polygon into regions without building quadrilateral patches.
pub fn segment_domain(
    coords: &[Pt2],
    field_samples: Option<&[[f64; 4]]>,
) -> Result<Vec<Region>, MeshError> {
    segment::segment_domain(input_region(coords)?, &SegmentParams::default(), field_samples)
        .map_err(MeshError::InvalidFieldSamples)
}

struct Stages {
    regions: Vec<Region>,
    patches: Vec<types::Patch>,
    initial: Quads,
    mesh: Quads,
}

fn run_pipeline(
    coords: &[Pt2],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<Stages, MeshError> {
    if !l_tar.is_finite() || l_tar <= 0.0 {
        return Err(MeshError::InvalidTargetLength);
    }
    let domain = input_region(coords)?;
    let tol = EPS * domain.scale();
    let params = QParams {
        l_tar,
        ..QParams::default()
    };
    let regions = segment::segment_domain(domain.clone(), &SegmentParams::default(), field_samples)
        .map_err(MeshError::InvalidFieldSamples)?;
    let patches = layout::build_layout(&regions, &params, tol);
    let patches = conform::remove_tjunctions(patches, &params, tol);
    let patches = conform::refine_bad_patches(patches, &params, tol);
    let counts = conform::choose_counts(&patches, &params, tol);
    let initial = conform::mesh_layout(&patches, &counts, tol);
    let mesh = simplify::simplify(initial.clone(), &domain, &params, tol);
    Ok(Stages {
        regions,
        patches,
        initial,
        mesh,
    })
}

/// Run the full pipeline, keeping every intermediate stage.
pub fn quad_pipeline(
    coords: &[Pt2],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<PipelineTrace, MeshError> {
    let stages = run_pipeline(coords, l_tar, field_samples)?;
    Ok(PipelineTrace {
        regions: stages.regions,
        patches: stages.patches,
        initial: stages.initial.to_mesh(),
        mesh: stages.mesh.to_mesh(),
    })
}

/// Mesh a polygon with quads of roughly `l_tar` edge length.
pub fn quad_mesh_from_coords(
    coords: &[Pt2],
    l_tar: f64,
    field_samples: Option<&[[f64; 4]]>,
) -> Result<QuadMesh, MeshError> {
    Ok(run_pipeline(coords, l_tar, field_samples)?.mesh.to_mesh())
}

/// Mesh an exterior ring with the default target length.
pub fn mesh_domain(outer: &[Pt2], holes: &[Vec<Pt2>]) -> Result<QuadMesh, MeshError> {
    if !holes.is_empty() {
        return Err(MeshError::UnsupportedHoles);
    }
    quad_mesh_from_coords(outer, QParams::default().l_tar, None)
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
