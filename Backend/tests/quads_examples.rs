use floorplan_backend::quads::{mesh_domain, signed_quad_area};
use opengeometry::geometry::poly2d::Pt2;
use serde_json::Value;
use std::fs;

fn points(value: &Value) -> Vec<[f64; 2]> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
        .collect()
}

fn faces(value: &Value) -> Vec<[usize; 4]> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            [
                q[0].as_u64().unwrap() as usize,
                q[1].as_u64().unwrap() as usize,
                q[2].as_u64().unwrap() as usize,
                q[3].as_u64().unwrap() as usize,
            ]
        })
        .collect()
}

fn area(points: &[[f64; 2]], quads: &[[usize; 4]]) -> f64 {
    quads
        .iter()
        .map(|q| {
            (0..4)
                .map(|i| {
                    let a = points[q[i]];
                    let b = points[q[(i + 1) % 4]];
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>()
                * 0.5
        })
        .sum::<f64>()
}

fn polygon_area(coords: &[[f64; 2]]) -> f64 {
    (0..coords.len() - 1)
        .map(|i| {
            let a = coords[i];
            let b = coords[i + 1];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
        * 0.5
}

fn boundary_edges(quads: &[[usize; 4]]) -> Vec<(usize, usize)> {
    let mut counts = std::collections::HashMap::<(usize, usize), usize>::new();
    for q in quads {
        for i in 0..4 {
            let a = q[i];
            let b = q[(i + 1) % 4];
            *counts.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    counts.into_iter().filter_map(|(edge, count)| (count == 1).then_some(edge)).collect()
}

fn point_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 == 0.0 { 0.0 } else {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    };
    (p[0] - (a[0] + t * d[0])).hypot(p[1] - (a[1] + t * d[1]))
}

fn directed_boundary_distance(
    source_vertices: &[[f64; 2]], source_quads: &[[usize; 4]],
    target_vertices: &[[f64; 2]], target_quads: &[[usize; 4]],
) -> f64 {
    let source = boundary_edges(source_quads);
    let target = boundary_edges(target_quads);
    source.iter().map(|&(a, b)| {
        (0..=32).map(|step| {
            let t = step as f64 / 32.0;
            let p = [source_vertices[a][0] * (1.0 - t) + source_vertices[b][0] * t,
                     source_vertices[a][1] * (1.0 - t) + source_vertices[b][1] * t];
            target.iter().map(|&(c, d)| point_segment_distance(p, target_vertices[c], target_vertices[d]))
                .fold(f64::INFINITY, f64::min)
        }).fold(0.0, f64::max)
    }).fold(0.0, f64::max)
}

fn boundary_distance(
    a_vertices: &[[f64; 2]], a_quads: &[[usize; 4]],
    b_vertices: &[[f64; 2]], b_quads: &[[usize; 4]],
) -> f64 {
    directed_boundary_distance(a_vertices, a_quads, b_vertices, b_quads)
        .max(directed_boundary_distance(b_vertices, b_quads, a_vertices, a_quads))
}

#[test]
fn rust_kernel_covers_all_saved_examples() {
    let paths = fs::read_dir("data")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 49, "unexpected number of data fixtures");

    for path in paths {
        let data: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let coords = points(&data["coords"]);
        let expected_vertices = points(&data["V"]);
        let expected_quads = faces(&data["Q"]);
        let expected_area = area(&expected_vertices, &expected_quads);
        let domain_area = polygon_area(&coords);

        assert!(
            (expected_area - domain_area).abs() <= domain_area.max(1.0) * 1.0e-8,
            "saved V/Q do not cover input domain in {}",
            path.display()
        );

        let coords_pt2: Vec<_> = coords.iter().map(|p| Pt2::new(p[0], p[1])).collect();
        let mesh = mesh_domain(&coords_pt2, &[])
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        let vertex_ids: Vec<_> = mesh.vertex_ids().collect();
        let vertex_index: std::collections::HashMap<_, _> = vertex_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        let actual_vertices: Vec<_> = vertex_ids.iter().map(|&id| { let p = mesh.point(id).unwrap(); [p.x, p.z] }).collect();
        let actual_quads: Vec<[usize; 4]> = mesh.faces().map(|(_, face)| face.iter().map(|id| vertex_index[id]).collect::<Vec<_>>().try_into().unwrap()).collect();
        let actual_area = area(&actual_vertices, &actual_quads);
        assert!(
            (actual_area - domain_area).abs() <= domain_area.max(1.0) * 1.0e-8,
            "Rust mesh area differs from input in {}",
            path.display()
        );
        let scale = coords.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
            [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
        });
        let diagonal = (scale[2] - scale[0]).hypot(scale[3] - scale[1]).max(1.0);
        let boundary_delta = boundary_distance(&actual_vertices, &actual_quads, &expected_vertices, &expected_quads);
        assert!(
            boundary_delta <= diagonal * 5.0e-2,
            "new mesh boundary differs from saved original by {boundary_delta} in {}",
            path.display()
        );
        assert!(
            mesh.faces().all(|(_, q)| {
                q.len() == 4
                    && signed_quad_area(&mesh, q.try_into().unwrap()) > domain_area.max(1.0) * 1.0e-14
            }),
            "invalid quad in {}",
            path.display()
        );
    }
}

