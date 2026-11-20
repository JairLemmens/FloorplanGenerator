use super::types::EPS;
use opengeometry::geometry::poly2d::{Pt2, cross2, dot2};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

pub type Polyline = Vec<Pt2>;

// ---------------------------------------------------------------- points

pub fn point(x: f64, y: f64) -> Pt2 {
    Pt2::new(x, y)
}

pub fn add(a: Pt2, b: Pt2) -> Pt2 {
    point(a.x + b.x, a.z + b.z)
}

pub fn sub(a: Pt2, b: Pt2) -> Pt2 {
    point(a.x - b.x, a.z - b.z)
}

pub fn scale(a: Pt2, t: f64) -> Pt2 {
    point(a.x * t, a.z * t)
}

pub fn lerp(a: Pt2, b: Pt2, t: f64) -> Pt2 {
    add(a, scale(sub(b, a), t))
}

pub fn midpoint(a: Pt2, b: Pt2) -> Pt2 {
    point((a.x + b.x) * 0.5, (a.z + b.z) * 0.5)
}

pub fn length(v: Pt2) -> f64 {
    v.x.hypot(v.z)
}

pub fn distance(a: Pt2, b: Pt2) -> f64 {
    length(sub(a, b))
}

pub fn angle(v: Pt2) -> f64 {
    v.z.atan2(v.x)
}

pub fn point_segment_distance(p: Pt2, a: Pt2, b: Pt2) -> f64 {
    let ab = sub(b, a);
    let len2 = dot2(ab, ab);
    let t = if len2 == 0.0 {
        0.0
    } else {
        (dot2(sub(p, a), ab) / len2).clamp(0.0, 1.0)
    };
    distance(p, add(a, scale(ab, t)))
}

// ---------------------------------------------------------------- angles

/// Signed turning angle at `b` when walking `a -> b -> c`.
pub fn turn(a: Pt2, b: Pt2, c: Pt2) -> f64 {
    let (u, v) = (sub(b, a), sub(c, b));
    cross2(u, v).atan2(dot2(u, v))
}

/// Interior angle in `[0, 2π)` at `p` between its neighbours (counter-clockwise ring).
pub fn corner_angle(prev: Pt2, p: Pt2, next: Pt2) -> f64 {
    (angle(sub(prev, p)) - angle(sub(next, p))).rem_euclid(TAU)
}

/// Wrap an angle to `[-π/4, π/4)`, i.e. its distance to the nearest axis direction.
pub fn wrap_quarter(x: f64) -> f64 {
    (x + FRAC_PI_4).rem_euclid(FRAC_PI_2) - FRAC_PI_4
}

// ----------------------------------------------------------------- rings

/// Directed edges of a closed ring.
pub fn ring_edges(ring: &[Pt2]) -> impl Iterator<Item = (Pt2, Pt2)> + '_ {
    (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()]))
}

/// `(prev, here, next)` for every vertex of a closed ring.
pub fn ring_corners(ring: &[Pt2]) -> impl Iterator<Item = (Pt2, Pt2, Pt2)> + '_ {
    let n = ring.len();
    (0..n).map(move |i| (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]))
}

pub fn interior_angles(ring: &[Pt2]) -> Vec<f64> {
    ring_corners(ring)
        .map(|(prev, p, next)| corner_angle(prev, p, next))
        .collect()
}

/// Drop vertices closer than `micro` to their predecessor (and to the start).
pub fn clean_ring(ring: &[Pt2], micro: f64) -> Polyline {
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

// ---------------------------------------------------------------- bounds

pub fn bounds<'a>(points: impl IntoIterator<Item = &'a Pt2>) -> [f64; 4] {
    points.into_iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |b, p| [b[0].min(p.x), b[1].min(p.z), b[2].max(p.x), b[3].max(p.z)],
    )
}

pub fn diagonal<'a>(points: impl IntoIterator<Item = &'a Pt2>) -> f64 {
    let b = bounds(points);
    (b[2] - b[0]).hypot(b[3] - b[1])
}

// ------------------------------------------------------------- polylines

pub fn dedup(polyline: &[Pt2], tol: f64) -> Polyline {
    if polyline.is_empty() {
        return Vec::new();
    }
    let mut out = vec![polyline[0]];
    for &p in &polyline[1..] {
        if distance(p, *out.last().unwrap()) > tol {
            out.push(p);
        }
    }
    if out.len() == 1 {
        out.push(*polyline.last().unwrap());
    }
    out
}

pub fn poly_len(polyline: &[Pt2]) -> f64 {
    polyline.windows(2).map(|w| distance(w[0], w[1])).sum()
}

/// Arc length at every vertex, starting at 0.
pub fn cumulative(polyline: &[Pt2]) -> Vec<f64> {
    let mut out = Vec::with_capacity(polyline.len());
    out.push(0.0);
    for w in polyline.windows(2) {
        out.push(out.last().unwrap() + distance(w[0], w[1]));
    }
    out
}

fn interp(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    let hi = xs.partition_point(|&v| v < x);
    let lo = hi - 1;
    let t = (x - xs[lo]) / (xs[hi] - xs[lo]);
    ys[lo] + t * (ys[hi] - ys[lo])
}

/// Interpolate the polyline at each arc-length in `ts` (`s` = cumulative lengths).
fn sample(polyline: &[Pt2], s: &[f64], ts: &[f64]) -> Polyline {
    let xs: Vec<_> = polyline.iter().map(|p| p.x).collect();
    let zs: Vec<_> = polyline.iter().map(|p| p.z).collect();
    ts.iter()
        .map(|&d| point(interp(d, s, &xs), interp(d, s, &zs)))
        .collect()
}

pub fn point_at(polyline: &[Pt2], d: f64) -> Pt2 {
    sample(polyline, &cumulative(polyline), &[d])[0]
}

pub fn sub_pl(polyline: &[Pt2], a: f64, b: f64) -> Polyline {
    let s = cumulative(polyline);
    let tol = EPS * s.last().copied().unwrap_or(0.0);
    let mut out = vec![point_at(polyline, a)];
    out.extend(
        polyline
            .iter()
            .zip(&s)
            .filter(|&(_, &d)| d > a + tol && d < b - tol)
            .map(|(&p, _)| p),
    );
    out.push(point_at(polyline, b));
    dedup(&out, tol)
}

/// Arc lengths of interior vertices turning by more than `ang_tol`.
pub fn bends(polyline: &[Pt2], ang_tol: f64) -> Vec<f64> {
    let s = cumulative(polyline);
    (1..polyline.len().saturating_sub(1))
        .filter(|&i| turn(polyline[i - 1], polyline[i], polyline[i + 1]).abs() > ang_tol)
        .map(|i| s[i])
        .collect()
}

/// Number of straight runs in a polyline.
pub fn segments(polyline: &[Pt2]) -> usize {
    bends(polyline, EPS).len() + 1
}

/// Resample into `n` equal steps, snapping bend vertices to sample points.
fn resample_c(polyline: &[Pt2], n: usize) -> Polyline {
    let s = cumulative(polyline);
    let total = *s.last().unwrap_or(&0.0);
    let bs = bends(polyline, EPS);
    let mut t: Vec<f64> = (0..=n).map(|i| total * i as f64 / n as f64).collect();
    if !bs.is_empty() && n > bs.len() {
        let mut idx = Vec::with_capacity(bs.len());
        let mut prev = 0usize;
        for &sb in &bs {
            prev = (((sb / total) * n as f64).round_ties_even() as usize).max(prev + 1);
            idx.push(prev);
        }
        let m = bs.len();
        for j in (0..m).rev() {
            idx[j] = idx[j].min(n - (m - j));
        }
        let mut xs = vec![0.0];
        xs.extend(idx.iter().map(|&i| i as f64));
        xs.push(n as f64);
        let mut ys = vec![0.0];
        ys.extend_from_slice(&bs);
        ys.push(total);
        t = (0..=n).map(|i| interp(i as f64, &xs, &ys)).collect();
    }
    sample(polyline, &s, &t)
}

/// Resample in a canonical direction so shared edges of neighbouring patches
/// receive identical points regardless of traversal direction.
pub fn resample(polyline: &[Pt2], n: usize) -> Polyline {
    let tol = EPS * poly_len(polyline);
    let key = |p: Pt2| {
        (
            (p.x / tol).round_ties_even() as i64,
            (p.z / tol).round_ties_even() as i64,
        )
    };
    if key(polyline[0]) > key(polyline[polyline.len() - 1]) {
        let reversed: Vec<_> = polyline.iter().rev().copied().collect();
        let mut out = resample_c(&reversed, n);
        out.reverse();
        out
    } else {
        resample_c(polyline, n)
    }
}

/// Closest point on the polyline: `(distance, arc length, point)`.
pub fn project(polyline: &[Pt2], p: Pt2) -> (f64, f64, Pt2) {
    let mut best = (f64::INFINITY, 0.0, polyline[0]);
    let mut arc = 0.0;
    for w in polyline.windows(2) {
        let ab = sub(w[1], w[0]);
        let len = length(ab);
        let t = if len == 0.0 {
            0.0
        } else {
            (dot2(sub(p, w[0]), ab) / (len * len)).clamp(0.0, 1.0)
        };
        let q = add(w[0], scale(ab, t));
        let d = distance(q, p);
        if d < best.0 {
            best = (d, arc + t * len, q);
        }
        arc += len;
    }
    best
}

pub fn split_pl(polyline: &[Pt2], d: f64, exact: Option<Pt2>) -> (Polyline, Polyline) {
    let total = poly_len(polyline);
    let p = exact.unwrap_or_else(|| point_at(polyline, d.clamp(0.0, total)));
    let s = cumulative(polyline);
    let k = s
        .partition_point(|&v| v <= d)
        .saturating_sub(1)
        .min(polyline.len() - 2);
    let mut a = polyline[..=k].to_vec();
    a.push(p);
    let mut b = vec![p];
    b.extend_from_slice(&polyline[k + 1..]);
    let tol = EPS * total;
    let mut a = dedup(&a, tol);
    let mut b = dedup(&b, tol);
    *a.last_mut().unwrap() = p;
    b[0] = p;
    (a, b)
}
