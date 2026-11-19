use openmaths::Vector3;
use opengeometry::geometry::poly2d::{Pt2, signed_area2};
use crate::base_types::Frame;

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

pub fn normalize_2d(v: Pt2) -> Pt2 {
    let len = (v.x * v.x + v.z * v.z).sqrt();

    if len > f64::EPSILON {
        Pt2::new(v.x / len, v.z / len)
    } else {
        Pt2::new(0.0, 0.0)
    }
}

pub fn offset_from_intersection(intersection: Pt2, origin: Pt2, normal: Pt2) -> f64 {
    (intersection.x - origin.x) * normal.x + (intersection.z - origin.z) * normal.z
}

/// Projects a 3D world-space point into this work plane's 2D `(u, v)` coordinates.
/// The returned vector contains `(u, v)`, where `u` and `v` are the
/// coordinates along the work plane's local axes.

pub fn project_vector_to_frame(frame: &Frame,v: &Vector3) -> Pt2 {
    let u =v.x * frame.u_axis().x + v.y * frame.u_axis().y + v.z * frame.u_axis().z;
    let v_coord = v.x * frame.v_axis().x + v.y * frame.v_axis().y + v.z * frame.v_axis().z;
    Pt2::new(u, v_coord)
}

pub fn project_to_frame(frame: &Frame,point: &Vector3,) -> Pt2 {
    let (dx, dy, dz) = (
        point.x - frame.origin().x,
        point.y - frame.origin().y,
        point.z - frame.origin().z,
    );

    let u = dx * frame.u_axis().x+ dy * frame.u_axis().y+ dz * frame.u_axis().z;

    let v = dx * frame.v_axis().x+ dy * frame.v_axis().y+ dz * frame.v_axis().z;

    Pt2::new(u, v)
}

pub fn line_intersection_2d(a: Pt2, da: Pt2, b: Pt2, db: Pt2) -> Option<Pt2> {
    let denominator = cross_2d(da.x, da.z, db.x, db.z);
    if denominator.abs() < EPSILON {
        return None;
    }
    let q = Pt2::new(b.x - a.x, b.z - a.z);
    let t = cross_2d(q.x, q.z, db.x, db.z) / denominator;
    Some(Pt2::new(a.x + t * da.x, a.z + t * da.z))
}

pub fn offset_per_segments(corners: &[Pt2], offsets: &[f64]) -> Result<Vec<Pt2>, String> {
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

        let dx = corners[n1].x - corners[n].x;
        let dy = corners[n1].z - corners[n].z;

        let length = (dx * dx + dy * dy).sqrt();

        if length < EPSILON {
            return Err("corners must not contain consecutive duplicate points".to_string());
        }

        let tx = dx / length;
        let ty = dy / length;

        tangents.push(Pt2::new(tx, ty));
        offset_dirs.push(Pt2::new(ty, -tx));
    }

    let mut result = Vec::with_capacity(count);

    for n in 0..count {
        let n1 = (n + 1) % count;

        let offset_a = offsets[n];
        let offset_b = offsets[n1];

        let p1 = Pt2::new(corners[n].x + offset_dirs[n].x * offset_a,
            corners[n].z + offset_dirs[n].z * offset_a);

        let p2 = Pt2::new(corners[n1].x + offset_dirs[n1].x * offset_b,
            corners[n1].z + offset_dirs[n1].z * offset_b);

        let tangent_dot = tangents[n].x * tangents[n1].x + tangents[n].z * tangents[n1].z;
        if tangent_dot > 0.9 {
            if (offset_a - offset_b).abs() > 1e-6 {
                let p1 = Pt2::new(corners[n1].x + offset_dirs[n].x * offset_a,
                    corners[n1].z + offset_dirs[n].z * offset_a);
                result.push(p1);
            }
            result.push(p2);
            
        } else {
            // Intersect the two offset lines.
            if let Some(point) = line_intersection_2d(p1,tangents[n],p2,tangents[n1]) {
                result.push(point);
            } else {
                return Err("Failed intersection".to_string());
            }
        }
    }
    Ok(result)
}

pub fn area_2d(points: &[Pt2]) -> f64 {
    signed_area2(points).abs()
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

pub fn lift_points_with_offset(frame: &Frame, points: &[Pt2], offset: f64) -> Vec<Vector3> {
    let normal = frame.normal();
    points
        .iter()
        .map(|point| {
            let p = frame.lift_point(*point);

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
