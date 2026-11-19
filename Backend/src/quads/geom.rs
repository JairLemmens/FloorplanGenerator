use crate::topology::ops::{QuadMeshOps, TopologyOps};
use opengeometry::geometry::boolean2d::{RingRegion, regions_from_edges_by};
use opengeometry::geometry::poly2d::{Edge2, Pt2, cross2, segment_cross_t2};

use super::polyline::{distance, point, point_segment_distance, sub};
use super::types::Region;

pub const TWO_PI: f64 = std::f64::consts::TAU;

fn arrangement_edges(region: &Region, extra: &[(Pt2, Pt2)]) -> Vec<Edge2> {
    let mut edges = Vec::new();
    for ring in region.rings() {
        for i in 0..ring.len() {
            edges.push((ring[i], ring[(i + 1) % ring.len()]));
        }
    }
    edges.extend_from_slice(extra);
    edges
}

fn from_og_regions(regions: Vec<RingRegion>) -> Vec<Region> {
    regions
        .into_iter()
        .map(|r| Region {
            outer: r.outer,
            holes: r.holes,
        })
        .collect()
}

/// Split a region by an internal chord, classifying each side with OpenGeometry's
/// arrangement engine so all noded boundary intersections remain in Rust.
pub fn split_region(region: &Region, a: Pt2, b: Pt2, eps: f64, min_area: f64) -> Vec<Region> {
    let chord = sub(b, a);
    let edges = arrangement_edges(region, &[(a, b)]);
    let mut result = Vec::new();
    for (sign, start) in [(-1.0, b), (1.0, a)] {
        let pieces = regions_from_edges_by(&edges, eps, |p| {
            region.contains(p) && sign * cross2(chord, sub(p, a)) > 0.0
        });
        if let Ok(pieces) = pieces {
            for mut piece in from_og_regions(pieces) {
                if piece.area() > min_area {
                    piece.orient();
                    if let Some((at, d)) = piece
                        .outer
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (i, distance(*p, start)))
                        .min_by(|x, y| x.1.total_cmp(&y.1))
                    {
                        if d <= 100.0 * eps {
                            piece.outer.rotate_left(at);
                        }
                    }
                    result.push(piece);
                }
            }
        }
    }
    result
}

pub fn valid_chord(region: &Region, a: Pt2, b: Pt2, eps: f64) -> bool {
    let len = distance(a, b);
    if len < 40.0 * eps {
        return false;
    }
    let inner_a = point(
        a.x + (b.x - a.x) * (10.0 * eps / len),
        a.z + (b.z - a.z) * (10.0 * eps / len),
    );
    let inner_b = point(
        b.x - (b.x - a.x) * (10.0 * eps / len),
        b.z - (b.z - a.z) * (10.0 * eps / len),
    );
    if !region.contains(inner_a) || !region.contains(inner_b) {
        return false;
    }
    for ring in region.rings() {
        for i in 0..ring.len() {
            if let Some((q, ta, tb)) =
                segment_cross_t2(a, b, ring[i], ring[(i + 1) % ring.len()], eps)
            {
                if ta > 10.0 * eps / len
                    && ta < 1.0 - 10.0 * eps / len
                    && tb >= -eps
                    && tb <= 1.0 + eps
                {
                    let _ = q;
                    return false;
                }
            }
        }
    }
    let midpoint = point((a.x + b.x) * 0.5, (a.z + b.z) * 0.5);
    region.contains(midpoint)
}

pub fn boundary_intersections(
    region: &Region,
    origin: Pt2,
    direction: Pt2,
    eps: f64,
) -> Vec<(f64, Pt2)> {
    let mut points = Vec::new();
    for ring in region.rings() {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let edge = sub(b, a);
            let denom = cross2(direction, edge);
            if denom.abs() <= eps {
                continue;
            }
            let diff = sub(a, origin);
            let t = cross2(diff, edge) / denom;
            let u = cross2(diff, direction) / denom;
            if u >= -eps && u <= 1.0 + eps {
                let p = point(origin.x + t * direction.x, origin.z + t * direction.z);
                if !points
                    .iter()
                    .any(|(_, q): &(f64, Pt2)| distance(*q, p) < eps)
                {
                    points.push((t, p));
                }
            }
        }
    }
    points
}

pub fn hausdorff_mesh_boundary(mesh: &super::types::QuadMesh, region: &Region) -> f64 {
    let mesh_edges: Vec<_> = mesh
        .edges()
        .filter(|(_, _, faces)| faces.len() == 1)
        .map(|(_, verts, _)| (mesh.point(verts[0]).unwrap(), mesh.point(verts[1]).unwrap()))
        .collect();
    let distance_to = |p: Pt2, edges: &[(Pt2, Pt2)]| {
        edges
            .iter()
            .map(|&(a, b)| point_segment_distance(p, a, b))
            .fold(f64::INFINITY, f64::min)
    };
    let sampled = |edges: &[(Pt2, Pt2)], targets: &[(Pt2, Pt2)]| {
        edges
            .iter()
            .map(|&(a, b)| {
                (0..=50)
                    .map(|i| {
                        distance_to(
                            point(
                                a.x + (b.x - a.x) * i as f64 / 50.0,
                                a.z + (b.z - a.z) * i as f64 / 50.0,
                            ),
                            targets,
                        )
                    })
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    };
    let domain_edges: Vec<_> = region
        .rings()
        .flat_map(|ring| (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()])))
        .collect();
    sampled(&domain_edges, &mesh_edges).max(sampled(&mesh_edges, &domain_edges))
}
