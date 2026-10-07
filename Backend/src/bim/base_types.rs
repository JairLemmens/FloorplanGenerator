use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use openmaths::Vector3;
use crate::bim::utils::*;

pub type Dictionary = HashMap<String, Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash,Deserialize,Serialize)]
pub struct Id {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}


#[derive(Clone, Serialize, Deserialize)] 
pub enum MatOrCompId { Material(Id), Composite(Id), }

pub type EdgeOffsets = Vec<Vec<f64>>;

pub type EdgeMaterials = Vec<Vec<Option<Id>>>;

#[derive(Serialize)]
pub struct LayerGeometry {
    pub(crate) inner: Vec<Vector3>,
    pub(crate) outer: Vec<Vector3>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConstructionRef {
    Uuid(String),
    Id(Id),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointType {Miter, Butt, DoubleButt}


#[derive(Clone, Serialize, Deserialize)]
pub struct Frame {
    origin: Vector3,
    normal: Vector3,
    u_axis: Vector3,
    v_axis: Vector3,
}

impl Frame {
    pub fn new(origin: Vector3, normal: Vector3, u_hint: Vector3) -> Frame {
        let n = normalize(normal);
        // Remove the normal component from the u hint so u lies in the plane.
        let dot = u_hint.x * n.x + u_hint.y * n.y + u_hint.z * n.z;
        let projected = Vector3::new(
            u_hint.x - n.x * dot,
            u_hint.y - n.y * dot,
            u_hint.z - n.z * dot,
        );
        
        let u = normalize(projected);
        let v = normalize(cross(n, u));
        Frame {
            origin,
            normal: n,
            u_axis: u,
            v_axis: v,
        }
    }

    /// Builds a work plane from an origin and normal, choosing a stable in-plane
    /// `u` axis automatically (`v = normal × u`).
    pub fn from_origin_normal(origin: Vector3, normal: Vector3) -> Frame {
        let n = normalize(normal);
        let u = normalize(any_perpendicular(n));
        let v = normalize(cross(n, u));
        Frame {
            origin,
            normal: n,
            u_axis: u,
            v_axis: v,
        }
    }

    /// Lifts a single 2D in-plane coordinate to 3D world space.
    pub fn lift_point(&self, u: f64, v: f64) -> Vector3 {
        Vector3::new(
            self.origin.x + self.u_axis.x * u + self.v_axis.x * v,
            self.origin.y + self.u_axis.y * u + self.v_axis.y * v,
            self.origin.z + self.u_axis.z * u + self.v_axis.z * v,
        )
    }

    /// Lifts a flat `[u0,v0,u1,v1,…]` buffer to a flat `[x0,y0,z0,…]` world buffer.
    pub fn lift_points_flat(&self, uv: Vec<f64>) -> Vec<f64> {
        let mut out = Vec::with_capacity(uv.len() / 2 * 3);
        for pair in uv.chunks_exact(2) {
            let p = self.lift_point(pair[0], pair[1]);
            out.push(p.x);
            out.push(p.y);
            out.push(p.z);
        }
        out
    }

    pub fn u_axis(&self) -> Vector3 {
        self.u_axis
    }

    pub fn v_axis(&self) -> Vector3 {
        self.v_axis
    }

    /// Lifts owned 2D points to world `Vector3`s.
    pub fn lift(&self, points: &[(f64, f64)]) -> Vec<Vector3> {
        points
            .iter()
            .map(|(u, v)| self.lift_point(*u, *v))
            .collect()
    }

    pub fn origin(&self) -> Vector3 {
        self.origin
    }

    pub fn normal(&self) -> Vector3 {
        self.normal
    }
}
