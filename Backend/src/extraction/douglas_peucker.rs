#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[inline]
fn perpendicular_distance_squared(point: Point, start: Point, end: Point) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;

    if dx == 0.0 && dy == 0.0 {
        let px = point.x - start.x;
        let py = point.y - start.y;
        return px * px + py * py;
    }

    let cross =
        dy * (point.x - start.x)
        - dx * (point.y - start.y);

    let denom = dx * dx + dy * dy;

    (cross * cross) / denom
}

pub fn douglas_peucker(points: &[Point], epsilon: f64) -> Vec<Point> {
    let len = points.len();

    if len <= 2 {
        return points.to_vec();
    }

    if epsilon <= 0.0 {
        return points.to_vec();
    }

    let epsilon_sq = epsilon * epsilon;

    // Keep flags instead of recursive allocations.
    let mut keep = vec![false; len];

    keep[0] = true;
    keep[len - 1] = true;

    // Stack of inclusive ranges.
    let mut stack = Vec::with_capacity(64);
    stack.push((0usize, len - 1));

    while let Some((start_idx, end_idx)) = stack.pop() {
        if end_idx <= start_idx + 1 {
            continue;
        }

        let start = points[start_idx];
        let end = points[end_idx];

        let mut max_dist_sq = 0.0;
        let mut max_idx = 0usize;

        for i in (start_idx + 1)..end_idx {
            let dist_sq =
                perpendicular_distance_squared(points[i], start, end);

            if dist_sq > max_dist_sq {
                max_dist_sq = dist_sq;
                max_idx = i;
            }
        }

        if max_dist_sq > epsilon_sq {
            keep[max_idx] = true;

            stack.push((start_idx, max_idx));
            stack.push((max_idx, end_idx));
        }
    }

    let mut result = Vec::with_capacity(len);

    for (i, &point) in points.iter().enumerate() {
        if keep[i] {
            result.push(point);
        }
    }

    result
}