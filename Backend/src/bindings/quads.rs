use crate::topology::ops::{QuadMeshOps, TopologyOps};
use opengeometry::geometry::poly2d::Pt2;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::HashMap;

/// Return segmented regions as rings of `[x, y]` points (outer ring first, then holes).
#[pyfunction]
#[pyo3(signature = (outer, field_samples=None))]
pub fn segment_domain(
    outer: Vec<[f64; 2]>,
    field_samples: Option<Vec<[f64; 4]>>,
) -> PyResult<Vec<Vec<Vec<[f64; 2]>>>> {
    let points: Vec<_> = outer.into_iter().map(|[x, y]| Pt2::new(x, y)).collect();
    let regions = crate::quads::segment_domain(&points, field_samples.as_deref())
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    Ok(regions
        .into_iter()
        .map(|region| {
            std::iter::once(region.outer)
                .chain(region.holes)
                .map(|ring| ring.into_iter().map(|p| [p.x, p.z]).collect())
                .collect()
        })
        .collect())
}

/// Build a quad mesh with optional `[x, y, cos(4*theta), sin(4*theta)]` field samples.
/// Samples are interpolated with Gaussian distance weights; the bandwidth is
/// `sqrt(domain_area / sample_count)` and weights are truncated beyond 3 sigma.
#[pyfunction]
#[pyo3(signature = (outer, holes=None, l_tar=30.0, field_samples=None))]
pub fn mesh_domain(
    outer: Vec<[f64; 2]>,
    holes: Option<Vec<Vec<[f64; 2]>>>,
    l_tar: f64,
    field_samples: Option<Vec<[f64; 4]>>,
) -> PyResult<(Vec<[f64; 2]>, Vec<[usize; 4]>)> {
    if holes.as_ref().is_some_and(|h| !h.is_empty()) {
        return Err(PyValueError::new_err(
            "the parity pipeline currently accepts an exterior ring only",
        ));
    }
    let outer: Vec<_> = outer.into_iter().map(|[x, y]| Pt2::new(x, y)).collect();
    let mesh = crate::quads::mesh_domain_with_field(&outer, &[], l_tar, field_samples.as_deref())
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    let vertex_ids = mesh.vertex_ids();
    let index: HashMap<_, _> = vertex_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();
    let vertices = vertex_ids
        .iter()
        .map(|&id| {
            let p = mesh.point(id).unwrap();
            [p.x, p.z]
        })
        .collect();
    let faces = mesh
        .faces()
        .map(|(_, q)| {
            q.iter()
                .map(|id| index[id])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
        .collect();
    Ok((vertices, faces))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quads::quad_mesh_from_coords;

    #[test]
    fn mesh_domain_binding_preserves_indexed_geometry_output() {
        let outer = vec![[0.0, 0.0], [120.0, 0.0], [120.0, 80.0], [0.0, 80.0]];
        let actual = mesh_domain(outer.clone(), None, 30.0, None).unwrap();

        // Rebuild the former binding conversion directly from the backing
        // topology, independently of the TopologyOps iterator methods.
        let points: Vec<_> = outer.into_iter().map(|[x, y]| Pt2::new(x, y)).collect();
        let mesh = quad_mesh_from_coords(&points, 30.0).unwrap();
        let ids: Vec<_> = mesh.topology().vert_ids();
        let index: HashMap<_, _> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        let expected_vertices = ids
            .iter()
            .map(|&id| {
                let p = mesh.topology().vert(id).unwrap().coords;
                [p.x, p.z]
            })
            .collect::<Vec<_>>();
        let expected_faces = mesh
            .topology()
            .face_ids()
            .iter()
            .map(|&id| {
                mesh.topology()
                    .face_vert_ids(id)
                    .unwrap()
                    .iter()
                    .map(|vertex| index[vertex])
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<[usize; 4]>>();

        assert_eq!(actual, (expected_vertices, expected_faces));
    }
}
