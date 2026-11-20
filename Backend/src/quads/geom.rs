use opengeometry::geometry::boolean2d::regions_from_edges_by;
use opengeometry::geometry::poly2d::{Edge2, Pt2, cross2, segment_cross_t2};

use super::polyline::{add, distance, lerp, midpoint, scale, sub};
use super::types::{EPS, Region};

/// Split a region by an internal chord, classifying each side with OpenGeometry's
/// arrangement engine so all noded boundary intersections remain in Rust.
pub fn split_region(region: &Region, a: Pt2, b: Pt2, tol: f64, min_area: f64) -> Vec<Region> {
    let chord = sub(b, a);
    let edges: Vec<Edge2> = region.edges().chain([(a, b)]).collect();
    let mut result = Vec::new();
    for (sign, start) in [(-1.0, b), (1.0, a)] {
        let Ok(pieces) = regions_from_edges_by(&edges, tol, |p| {
            region.contains(p) && sign * cross2(chord, sub(p, a)) > 0.0
        }) else {
            continue;
        };
        for piece in pieces {
            let mut piece = Region {
                outer: piece.outer,
                holes: piece.holes,
            };
            if piece.area() <= min_area {
                continue;
            }
            piece.orient();
            // Start the ring at the chord endpoint so patch sides line up.
            let nearest = piece
                .outer
                .iter()
                .enumerate()
                .map(|(i, p)| (i, distance(*p, start)))
                .min_by(|x, y| x.1.total_cmp(&y.1));
            if let Some((at, d)) = nearest {
                if d <= tol {
                    piece.outer.rotate_left(at);
                }
            }
            result.push(piece);
        }
    }
    result
}

/// A chord is valid if it lies strictly inside the region and touches the
/// boundary only at its endpoints.
pub fn valid_chord(region: &Region, a: Pt2, b: Pt2, tol: f64) -> bool {
    let len = distance(a, b);
    if len <= 2.0 * tol {
        return false;
    }
    let margin = tol / len;
    if !region.contains(lerp(a, b, margin)) || !region.contains(lerp(b, a, margin)) {
        return false;
    }
    let crosses_boundary = region.edges().any(|(c, d)| {
        segment_cross_t2(a, b, c, d, tol).is_some_and(|(_, ta, tb)| {
            ta > margin && ta < 1.0 - margin && tb >= -EPS && tb <= 1.0 + EPS
        })
    });
    !crosses_boundary && region.contains(midpoint(a, b))
}

/// Intersections of the ray `origin + t * direction` with the region boundary,
/// as `(t, point)` with coincident points merged.
pub fn boundary_intersections(
    region: &Region,
    origin: Pt2,
    direction: Pt2,
    tol: f64,
) -> Vec<(f64, Pt2)> {
    let mut points: Vec<(f64, Pt2)> = Vec::new();
    for (a, b) in region.edges() {
        let edge = sub(b, a);
        let denom = cross2(direction, edge);
        if denom.abs() <= tol {
            continue;
        }
        let diff = sub(a, origin);
        let t = cross2(diff, edge) / denom;
        let u = cross2(diff, direction) / denom;
        if u >= -EPS && u <= 1.0 + EPS {
            let p = add(origin, scale(direction, t));
            if !points.iter().any(|&(_, q)| distance(q, p) < tol) {
                points.push((t, p));
            }
        }
    }
    points
}
