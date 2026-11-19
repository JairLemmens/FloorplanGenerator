use openmaths::Vector3;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(crate) mod vector3_array {
    use super::*;

    pub fn serialize<S: Serializer>(point: &Vector3, serializer: S) -> Result<S::Ok, S::Error> {
        [point.x, point.y, point.z].serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vector3, D::Error> {
        let [x, y, z] = <[f64; 3]>::deserialize(deserializer)?;
        Ok(Vector3::new(x, y, z))
    }
}

pub(crate) mod vector3_array_vec {
    use super::*;

    pub fn serialize<S: Serializer>(points: &[Vector3], serializer: S) -> Result<S::Ok, S::Error> {
        points.iter().map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Vector3>, D::Error> {
        let points = Vec::<[f64; 3]>::deserialize(deserializer)?;
        Ok(points.into_iter().map(|[x, y, z]| Vector3::new(x, y, z)).collect())
    }
}
