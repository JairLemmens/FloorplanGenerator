use openmaths::Vector3;
use crate::bim::base_types::Frame;

const EPSILON: f64 = 1.0e-12;

pub fn any_perpendicular(n: Vector3) -> Vector3 {
    if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
        Vector3::new(0.0, -n.z, n.y)
    } else if n.y.abs() <= n.z.abs() {
        Vector3::new(-n.z, 0.0, n.x)
    } else {
        Vector3::new(-n.y, n.x, 0.0)
    }
}

pub fn dot(a: &Vector3, b: &Vector3) -> f64 {a.x * b.x + a.y * b.y + a.z * b.z}

pub fn cross(a: Vector3, b: Vector3) -> Vector3{
    Vector3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

pub fn normalize(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();

    if len > f64::EPSILON {
        Vector3::new(v.x / len, v.y / len, v.z / len)
    } else {
        Vector3::new(0.0, 0.0, 0.0)
    }
}

pub fn cross_2d(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {ax * by - ay * bx}

pub fn normalize_2d(v: (f64, f64)) -> (f64, f64) {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();

    if len > f64::EPSILON {
        (v.0 / len, v.1 / len)
    } else {
        (0.0, 0.0)
    }
}

pub fn offset_from_intersection(intersection: (f64, f64),origin: (f64, f64),normal: (f64, f64),) -> f64 {
    (intersection.0 - origin.0) * normal.0
    + (intersection.1 - origin.1) * normal.1
}

/// Projects a 3D world-space point into this work plane's 2D `(u, v)` coordinates.
/// The returned vector contains `(u, v)`, where `u` and `v` are the
/// coordinates along the work plane's local axes.

pub fn project_vector_to_frame(frame: &Frame,v: &Vector3) -> (f64, f64) {
    let u =v.x * frame.u_axis().x + v.y * frame.u_axis().y + v.z * frame.u_axis().z;
    let v_coord = v.x * frame.v_axis().x + v.y * frame.v_axis().y + v.z * frame.v_axis().z;
    (u, v_coord)
}

pub fn project_to_frame(frame: &Frame,point: &Vector3,) -> (f64, f64) {
    let (dx, dy, dz) = (
        point.x - frame.origin().x,
        point.y - frame.origin().y,
        point.z - frame.origin().z,
    );

    let u = dx * frame.u_axis().x+ dy * frame.u_axis().y+ dz * frame.u_axis().z;

    let v = dx * frame.v_axis().x+ dy * frame.v_axis().y+ dz * frame.v_axis().z;

    (u, v)
}

pub fn line_intersection_2d(a: (f64, f64),da: (f64, f64),b: (f64, f64),db: (f64, f64),) -> Option<(f64, f64)> {
    let denominator = cross_2d(da.0, da.1, db.0, db.1);
    if denominator.abs() < EPSILON {
        return None;
    }
    let q = (b.0 - a.0, b.1 - a.1);
    let t = cross_2d(q.0, q.1, db.0, db.1) / denominator;
    Some((a.0 + t * da.0,a.1 + t * da.1,))
}

pub fn offset_per_segments(corners: &Vec<(f64, f64)>,offsets: &[f64]) -> Result<Vec<(f64, f64)>, String> {
    if corners.len() != offsets.len() {
        return Err("corners and offsets must have the same length".to_string());
    }

    if corners.len() < 2 {
        return Err("at least two corners are required".to_string());
    }   

    let count = corners.len();
    
    // Calculate normalized edge tangents and their right-hand
    // perpendicular offset directions.
    let mut tangents = Vec::with_capacity(count);
    let mut offset_dirs = Vec::with_capacity(count);

    for n in 0..count {
        let n1 = (n + 1) % count;

        let dx = corners[n1].0 - corners[n].0;
        let dy = corners[n1].1 - corners[n].1;

        let length = (dx * dx + dy * dy).sqrt();

        if length < EPSILON {
            return Err("corners must not contain consecutive duplicate points".to_string());
        }

        let tx = dx / length;
        let ty = dy / length;

        tangents.push((tx, ty));
        offset_dirs.push((ty, -tx));
    }

    let mut result = Vec::with_capacity(count);

    for n in 0..count {
        let n1 = (n + 1) % count;

        let offset_a = offsets[n];
        let offset_b = offsets[n1];

        let p1 = (
            corners[n].0 + offset_dirs[n].0 * offset_a,
            corners[n].1 + offset_dirs[n].1 * offset_a,
        );

        let p2 = (
            corners[n1].0 + offset_dirs[n1].0 * offset_b,
            corners[n1].1 + offset_dirs[n1].1 * offset_b,
        );

        let tangent_dot = tangents[n].0 * tangents[n1].0 + tangents[n].1 * tangents[n1].1;
        if tangent_dot > 0.9 {
            if (offset_a - offset_b).abs() > 1e-6 {
                let p1 = (   
                    corners[n1].0 + offset_dirs[n].0 * offset_a,
                    corners[n1].1 + offset_dirs[n].1 * offset_a
                );
                result.push(p1);
            }
            result.push(p2);
            
        } else {
            // Intersect the two offset lines.
            if let Some((u, v)) = line_intersection_2d(p1,tangents[n],p2,tangents[n1]) {
                result.push((u,v));
            } else {
                return Err("Failed intersection".to_string());
            }
        }
    }
    Ok(result)
}

pub fn area_2d(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let sum: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(&(x1, y1), &(x2, y2))| x1 * y2 - x2 * y1)
        .sum();

    0.5 * sum.abs()
}

pub fn ccw_angle(vector: &Vector3, frame: &Frame) -> f64 {
    let normal = frame.normal();
    let ref_axis = frame.u_axis();

    let dot = vector.x * normal.x
        + vector.y * normal.y
        + vector.z * normal.z;

    let vp = Vector3::new(
        vector.x - dot * normal.x,
        vector.y - dot * normal.y,
        vector.z - dot * normal.z,
    );

    let y = normal.x * (ref_axis.y * vp.z - ref_axis.z * vp.y)
        + normal.y * (ref_axis.z * vp.x - ref_axis.x * vp.z)
        + normal.z * (ref_axis.x * vp.y - ref_axis.y * vp.x);

    let x = ref_axis.x * vp.x
        + ref_axis.y * vp.y
        + ref_axis.z * vp.z;

    let mut angle = y.atan2(x);

    if angle < 0.0 {angle += std::f64::consts::TAU;}

    angle
}

pub fn lift_points_with_offset(frame: &Frame, points: &[(f64, f64)], offset: f64) -> Vec<Vector3> {
    let normal = frame.normal();
    points
        .iter()
        .map(|(u, v)| {
            let p = frame.lift_point(*u, *v);

            Vector3::new(
                p.x + normal.x * offset,
                p.y + normal.y * offset,
                p.z + normal.z * offset,
            )
        })
        .collect()
}





// fn line_intersection(origin_a: &Vector3, direction_a: &Vector3, origin_b: &Vector3, direction_b: &Vector3, work_plane: &WorkPlane) -> Option<Vector3> {   
//     let a = project_to_frame(work_plane, origin_a);
//     let b = project_to_frame(work_plane, origin_b);

//     let u_axis = work_plane.u_axis();
//     let v_axis = work_plane.v_axis();

//     let da = (
//         direction_a.x * u_axis.x + direction_a.y * u_axis.y + direction_a.z * u_axis.z,
//         direction_a.x * v_axis.x + direction_a.y * v_axis.y + direction_a.z * v_axis.z,
//     );

//     let db = (
//         direction_b.x * u_axis.x + direction_b.y * u_axis.y + direction_b.z * u_axis.z,
//         direction_b.x * v_axis.x + direction_b.y * v_axis.y + direction_b.z * v_axis.z,
//     );
    
//     if da.0.abs() < EPSILON && da.1.abs() < EPSILON {return None;}
//     if db.0.abs() < EPSILON && db.1.abs() < EPSILON {return None;}

//     line_intersection_2d(a, da, b, db).map(|(u, v)| work_plane.lift_point(u, v))
// }
