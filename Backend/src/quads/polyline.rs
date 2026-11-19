use opengeometry::geometry::poly2d::{Pt2, cross2, dot2};

pub type Polyline = Vec<Pt2>;

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

pub fn length(v: Pt2) -> f64 {
    v.x.hypot(v.z)
}

pub fn distance(a: Pt2, b: Pt2) -> f64 {
    length(sub(a, b))
}

pub fn point_segment_distance(p: Pt2, a: Pt2, b: Pt2) -> f64 {
    let ab = sub(b, a);
    let len2 = ab.x * ab.x + ab.z * ab.z;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * ab.x + (p.z - a.z) * ab.z) / len2).clamp(0.0, 1.0)
    };
    distance(p, point(a.x + t * ab.x, a.z + t * ab.z))
}

pub fn bounds(points: &[Pt2]) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in points {
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.z);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.z);
    }
    b
}

pub fn diagonal(points: &[Pt2]) -> f64 {
    let b = bounds(points);
    (b[2] - b[0]).hypot(b[3] - b[1])
}

pub fn lerp(a: Pt2, b: Pt2, t: f64) -> Pt2 {
    add(a, scale(sub(b, a), t))
}

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

pub fn lengths(polyline: &[Pt2]) -> Vec<f64> {
    polyline.windows(2).map(|w| distance(w[0], w[1])).collect()
}

pub fn poly_len(polyline: &[Pt2]) -> f64 {
    lengths(polyline).iter().sum()
}

pub fn cumulative(polyline: &[Pt2]) -> Vec<f64> {
    let mut out = Vec::with_capacity(polyline.len());
    out.push(0.0);
    for l in lengths(polyline) {
        out.push(out.last().copied().unwrap() + l);
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

pub fn point_at(polyline: &[Pt2], d: f64) -> Pt2 {
    let s = cumulative(polyline);
    point(
        interp(d, &s, &polyline.iter().map(|p| p.x).collect::<Vec<_>>()),
        interp(d, &s, &polyline.iter().map(|p| p.z).collect::<Vec<_>>()),
    )
}

pub fn sub_pl(polyline: &[Pt2], a: f64, b: f64) -> Polyline {
    let s = cumulative(polyline);
    let mut out = vec![point_at(polyline, a)];
    out.extend(
        polyline
            .iter()
            .zip(s.iter())
            .filter(|entry| *entry.1 > a + 1e-12 && *entry.1 < b - 1e-12)
            .map(|(&p, _)| p),
    );
    out.push(point_at(polyline, b));
    dedup(&out, 1e-12)
}

pub fn bends(polyline: &[Pt2], ang_tol: f64) -> Vec<f64> {
    let s = cumulative(polyline);
    let mut out = Vec::new();
    for i in 1..polyline.len().saturating_sub(1) {
        let a = sub(polyline[i], polyline[i - 1]);
        let b = sub(polyline[i + 1], polyline[i]);
        if cross2(a, b).atan2(dot2(a, b)).abs() > ang_tol {
            out.push(s[i]);
        }
    }
    out
}

fn interp_samples(values: &[f64], s: &[f64], t: &[f64]) -> Vec<f64> {
    t.iter().map(|&d| interp(d, s, values)).collect()
}

fn resample_c(polyline: &[Pt2], n: usize) -> Polyline {
    let s = cumulative(polyline);
    let total = *s.last().unwrap_or(&0.0);
    let mut t: Vec<f64> = (0..=n).map(|i| total * i as f64 / n as f64).collect();
    let bs = bends(polyline, 1e-4);
    if !bs.is_empty() && n >= bs.len() + 1 {
        let mut idx = Vec::new();
        let mut prev = 0usize;
        for &sb in &bs {
            prev = (((sb / total) * n as f64).round_ties_even() as usize).max(prev + 1);
            idx.push(prev);
        }
        let m = bs.len();
        for j in (0..m).rev() {
            idx[j] = idx[j].min(n - (m - j));
        }
        let mut xs = Vec::with_capacity(m + 2);
        let mut ys = Vec::with_capacity(m + 2);
        xs.push(0.0);
        xs.extend(idx.iter().map(|&i| i as f64));
        xs.push(n as f64);
        ys.push(0.0);
        ys.extend(bs.iter().copied());
        ys.push(total);
        t = (0..=n).map(|i| interp(i as f64, &xs, &ys)).collect();
    }
    let xs: Vec<_> = polyline.iter().map(|p| p.x).collect();
    let ys: Vec<_> = polyline.iter().map(|p| p.z).collect();
    interp_samples(&xs, &s, &t)
        .into_iter()
        .zip(interp_samples(&ys, &s, &t))
        .map(|(x, z)| point(x, z))
        .collect()
}

pub fn resample(polyline: &[Pt2], n: usize) -> Polyline {
    let first = (polyline[0].x * 1e9).round_ties_even() as i64;
    let first_y = (polyline[0].z * 1e9).round_ties_even() as i64;
    let last = (polyline[polyline.len() - 1].x * 1e9).round_ties_even() as i64;
    let last_y = (polyline[polyline.len() - 1].z * 1e9).round_ties_even() as i64;
    if (first, first_y) > (last, last_y) {
        resample_c(&polyline.iter().copied().rev().collect::<Vec<_>>(), n)
            .into_iter()
            .rev()
            .collect()
    } else {
        resample_c(polyline, n)
    }
}

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
    let mut a = dedup(&a, 1e-12);
    let mut b = dedup(&b, 1e-12);
    *a.last_mut().unwrap() = p;
    b[0] = p;
    (a, b)
}

pub fn angle(v: Pt2) -> f64 {
    v.z.atan2(v.x)
}
