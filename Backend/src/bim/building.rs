use openmaths::Vector3;
use std::collections::HashMap;

use crate::bim::topology::topology::Topology;
use crate::bim::construction::construction_set::ConstructionSet;
use crate::bim::construction::construction_layer::ConstructionLayer;
use crate::bim::base_types::{Id, JointType, ConstructionRef, EdgeOffsets, LayerGeometry, Frame, EdgeMaterials};
use crate::bim::utils::*;


pub struct Building {
    pub topology: Topology,
    pub construction_set: ConstructionSet,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerRef {
    face_id: Id,
    layer_idx: usize,
    assembly_idx: usize,
}

#[derive(Clone, Copy)]
struct JointAssembly {
    face_id: Id,
    is_ccw: bool,
    direction: Vector3,
}

#[inline]
fn priority_class(priority: f64) -> f64 {
    priority.trunc()
}

impl Building {
    fn layer_from_ref(&self, r: LayerRef) -> &ConstructionLayer {
        let face = self.topology.face(r.face_id).unwrap();
        let group = self.construction_set.layer_group(face.layer_group_id.unwrap()).unwrap();

        self.construction_set.layer(group.layers[r.layer_idx]).unwrap()
    }

    pub fn from_data(mut topology: Topology, construction_set: ConstructionSet) -> Result<Self, String> {
        let face_ids: Vec<_> = topology.faces.ids().collect();
        for face_id in face_ids {
            let (num_edges, construction) = {
                let face = topology
                    .face(face_id)
                    .ok_or("Face not found")?;

                (face.edges.len(), face.construction.clone())
            };

            let layer_group_id = match construction {
                ConstructionRef::Uuid(uuid) => construction_set
                    .layer_group_by_uuid(&uuid)
                    .ok_or("Layer group not found")?,

                ConstructionRef::Id(id) => id,
            };

            let num_layers = construction_set
                .layer_group(layer_group_id)
                .ok_or("Layer group not found")?
                .layers
                .len();

            let offsets: EdgeOffsets =
                vec![vec![0.0; num_edges]; num_layers];

            let face = topology
                .faces
                .get_mut(face_id)
                .ok_or("Face not found")?;

            face.construction = ConstructionRef::Id(layer_group_id);
            face.layer_group_id = Some(layer_group_id);
            face.inner_edge_offsets = offsets.clone();
            face.outer_edge_offsets = offsets.clone();
            face.joint_materials = offsets
                .iter()
                .map(|edge_offsets| vec![None; edge_offsets.len()])
                .collect();
        }

        Ok(Self {topology, construction_set,})
    }

    pub fn from_json(topology_json: &str, construction_json: &str) -> Result<Self, String> {
        let topology = Topology::from_json(topology_json)?;
        let construction_set = ConstructionSet::from_json(construction_json)?;
        Self::from_data(topology, construction_set)
    }
        
    pub fn layer_geometry(&self,face_id: Id) -> Result<Vec<(String, String, EdgeMaterials, LayerGeometry, f64)>, String> {
        let face = self.topology.face(face_id).ok_or_else(|| "Face not found".to_string())?;
        face.geometry(&self.topology,&self.construction_set)
    }

    pub fn apply_joint(&mut self,face0_idx: usize,face1_idx: usize,joint_edge_id: Id,layer0_idx: usize,layer1_idx: usize,mut joint_type: JointType,) {

        let (joint_edge_vert0, joint_edge_vert1, face0_id, face1_id) = {
            let joint_edge = self.topology.edge(joint_edge_id).unwrap();
            (
                joint_edge.verts[0],
                joint_edge.verts[1],
                joint_edge.faces[face0_idx],
                joint_edge.faces[face1_idx],
            )
        };

        let joint_edge_dir = self.topology.edge(joint_edge_id).unwrap().tangent(&self.topology);
        let edge_point = self.topology.vert(joint_edge_vert0).unwrap().coords;
        let joint_frame =Frame::from_origin_normal(edge_point, joint_edge_dir);

        // Everything in this scope is read-only.
        let (face0_edge_idx, face1_edge_idx, face0_reversed, face1_reversed, n0, n1, p01, p02, p11, p12, parallel_offset, layer0_id, layer1_id) = 
        {
            let face0 = self.topology.face(face0_id).unwrap();
            let group0 = self.construction_set.layer_group(face0.layer_group_id.unwrap()).unwrap();
            let layer0_id = group0.layers[layer0_idx];
            let layer0 = self.construction_set.layer(layer0_id).unwrap();
            let offset0 = group0.layer_offsets[layer0_idx];
            let thickness0 = layer0.thickness;
            let face0_normal = face0.frame.normal();
            let face0_edge_idx = face0.edges.iter().position(|id| *id == joint_edge_id).unwrap();
            let face0_vert0_idx = face0.verts.iter().position(|id| *id == joint_edge_vert0).unwrap();
            let face0_reversed =joint_edge_vert1 != face0.verts[(face0_vert0_idx + 1) % face0.verts.len()];
            let mut face0_edge_normal = cross(joint_edge_dir,face0_normal);

            if face0_reversed {
                face0_edge_normal = face0_edge_normal.multiply_scalar(-1.0);
            }

            let n0 = normalize_2d(project_vector_to_frame(&joint_frame, &face0_edge_normal));
            let p01_3d = face0_normal.clone().multiply_scalar(offset0).add(&edge_point);
            let p02_3d = face0_normal.clone().multiply_scalar(offset0 + thickness0).add(&edge_point);

            let p01 = project_to_frame(&joint_frame, &p01_3d);
            let p02 = project_to_frame(&joint_frame, &p02_3d);

            // face1
            let face1 = self.topology.face(face1_id).unwrap();
            let group1 = self.construction_set.layer_group(face1.layer_group_id.unwrap()).unwrap();
            let layer1_id = group1.layers[layer1_idx];
            let layer1 = self.construction_set.layer(layer1_id).unwrap();
            let offset1 = group1.layer_offsets[layer1_idx];
            let thickness1 = layer1.thickness;
            let face1_normal = face1.frame.normal();
            let face1_edge_idx = face1.edges.iter().position(|id| *id == joint_edge_id).unwrap();
            let face1_vert0_idx = face1.verts.iter().position(|id| *id == joint_edge_vert0).unwrap();
            let face1_reversed =joint_edge_vert1 != face1.verts[(face1_vert0_idx + 1) % face1.verts.len()];
            let mut face1_edge_normal =cross(joint_edge_dir,face1_normal);
            
            if face1_reversed {
                face1_edge_normal = face1_edge_normal.multiply_scalar(-1.0);
            }
            let n1 = normalize_2d(project_vector_to_frame(&joint_frame, &face1_edge_normal));

            let p11_3d = face1_normal.clone().multiply_scalar(offset1).add(&edge_point);
            let p12_3d = face1_normal.clone().multiply_scalar(offset1 + thickness1).add(&edge_point);

            let p11 = project_to_frame(&joint_frame, &p11_3d);
            let p12 = project_to_frame(&joint_frame, &p12_3d);

            let parallel_offset =
                if (n0.0 * n1.0 + n0.1 * n1.1).abs() > 0.999 {
                    Some(-face1.inner_edge_offsets[layer1_idx][face1_edge_idx])
                } else {
                    None
                };

            assert!((n0.0 * n0.0 + n0.1 * n0.1 - 1.0).abs() < 1e-9);
            assert!((n1.0 * n1.0 + n1.1 * n1.1 - 1.0).abs() < 1e-9);
            (face0_edge_idx, face1_edge_idx, face0_reversed, face1_reversed, n0, n1, p01, p02, p11, p12, parallel_offset, layer0_id, layer1_id)

        };

        let _dot = n0.0 * n1.0 + n0.1 * n1.1;
        let _cross = n0.0 * n1.1 - n0.1 * n1.0;

        let flip = _cross.atan2(_dot).rem_euclid(std::f64::consts::TAU)< std::f64::consts::PI;

        if _dot.abs() > 0.9 {
            if joint_type == JointType::DoubleButt {
                joint_type = JointType::Miter;
            }

            if _dot.abs() > 0.999 {
                let offset = parallel_offset.unwrap();

                let face0 = self.topology.faces.get_mut(face0_id).unwrap();

                face0.inner_edge_offsets[layer0_idx][face0_edge_idx] = offset;
                face0.outer_edge_offsets[layer0_idx][face0_edge_idx] = offset;

                return;
            }
        }

        let i11 = line_intersection_2d(p01, n0, p11, n1);
        let i22 = line_intersection_2d(p02, n0, p12, n1);
        let i12 = line_intersection_2d(p01, n0, p12, n1);
        let i21 = line_intersection_2d(p02, n0, p11, n1);

        let (i11, i22, i12, i21) = match (i11, i22, i12, i21) {
            (Some(i11), Some(i22), Some(i12), Some(i21)) => {
                (i11, i22, i12, i21)
            }
            _ => return,
        };

        let (face0_inner, face0_outer, face1_offsets) = match joint_type {
            JointType::Miter => {
                if face0_reversed == face1_reversed {
                    (
                        offset_from_intersection(i12, p01, n0),
                        offset_from_intersection(i21, p01, n0),
                        Some((
                            offset_from_intersection(i21, p11, n1),
                            offset_from_intersection(i12, p11, n1),
                        )),
                    )
                } else {
                    (
                        offset_from_intersection(i11, p01, n0),
                        offset_from_intersection(i22, p01, n0),
                        Some((
                            offset_from_intersection(i11, p11, n1),
                            offset_from_intersection(i22, p11, n1),
                        )),
                    )
                }
            }

            JointType::Butt => {
                if face0_reversed == face1_reversed {
                    (
                        offset_from_intersection(i11, p01, n0),
                        offset_from_intersection(i21, p01, n0),
                        None,
                    )
                } else {
                    let mut reversed0 = face0_reversed;
                    let mut reversed1 = face1_reversed;

                    if flip {
                        reversed0 = !reversed0;
                        reversed1 = !reversed1;
                    }

                    if reversed0 && !reversed1 {
                        (
                            offset_from_intersection(i22, p01, n0),
                            offset_from_intersection(i12, p01, n0),
                            None,
                        )
                    } else {
                        (
                            offset_from_intersection(i11, p01, n0),
                            offset_from_intersection(i21, p01, n0),
                            None,
                        )
                    }
                }
            }

            JointType::DoubleButt => {
                if face0_reversed == face1_reversed {
                    (
                        offset_from_intersection(i12, p01, n0),
                        offset_from_intersection(i22, p01, n0),
                        Some((
                            offset_from_intersection(i21, p11, n1),
                            offset_from_intersection(i22, p11, n1),
                        )),
                    )
                } else {
                    let mut reversed0 = face0_reversed;
                    let mut reversed1 = face1_reversed;

                    if flip {
                        reversed0 = !reversed0;
                        reversed1 = !reversed1;
                    }

                    if reversed0 && !reversed1 {
                        (
                            offset_from_intersection(i11, p01, n0),
                            offset_from_intersection(i21, p01, n0),
                            Some((
                                offset_from_intersection(i21, p11, n1),
                                offset_from_intersection(i22, p11, n1),
                            )),
                        )
                    } else {
                        (
                            offset_from_intersection(i12, p01, n0),
                            offset_from_intersection(i22, p01, n0),
                            Some((
                                offset_from_intersection(i11, p11, n1),
                                offset_from_intersection(i12, p11, n1),
                            )),
                        )
                    }
                }
            }
        };

        // Mutate only after every immutable borrow is gone.
        let face0 = self.topology.faces.get_mut(face0_id).unwrap();

        face0.inner_edge_offsets[layer0_idx][face0_edge_idx] = face0_inner;
        face0.outer_edge_offsets[layer0_idx][face0_edge_idx] = face0_outer;
        face0.joint_materials[layer0_idx][face0_edge_idx] = Some(layer1_id);

        if let Some((face1_inner, face1_outer)) = face1_offsets {
            let face1 = self.topology.faces.get_mut(face1_id).unwrap();
            face1.inner_edge_offsets[layer1_idx][face1_edge_idx] = face1_inner;
            face1.outer_edge_offsets[layer1_idx][face1_edge_idx] = face1_outer;
            face1.joint_materials[layer1_idx][face1_edge_idx] = Some(layer0_id);
        }
    }  

    pub fn solve_joint(&mut self,joint_edge_id: Id) -> HashMap<LayerRef, LayerRef> {
        
        let (joint_faces, edge_v0, edge_v1) = {
            let edge = self.topology.edge(joint_edge_id).unwrap();
            (
                edge.faces.clone(),
                edge.verts[0],
                edge.verts[1],
            )
        };

        if joint_faces.len() < 2 {return HashMap::new();}

        let joint_edge_dir = {
            let p0 = self.topology.vert(edge_v0).unwrap().coords;
            let p1 = self.topology.vert(edge_v1).unwrap().coords;
            normalize(Vector3::new(
                p1.x - p0.x,
                p1.y - p0.y,
                p1.z - p0.z,
            ))
        };
        
        let joint_frame = &Frame::from_origin_normal(self.topology.vert(edge_v0).unwrap().coords, joint_edge_dir);
        //order
        let mut assemblies = Vec::with_capacity(joint_faces.len());

        for &face_id in &joint_faces {
            let face = self.topology.face(face_id).unwrap();
            let v0_idx = face.verts.iter().position(|id| *id == edge_v0).expect("Joint edge vertex not found in face");
            let face_reversed =edge_v1 != face.verts[(v0_idx + 1) % face.verts.len()];

            let mut edge_normal =cross(joint_edge_dir, face.frame.normal());
            //let mut edge_normal =cross(face.frame.normal(),joint_edge_dir);
            if face_reversed {edge_normal = edge_normal.multiply_scalar(-1.0);}

            let angle = ccw_angle(&edge_normal, joint_frame);

            assemblies.push(JointAssembly {face_id,is_ccw: !face_reversed,direction: edge_normal,});
            let _ = angle;
        }

        assemblies.sort_by(|a, b| {
            let aa = {
                let face = self.topology.face(a.face_id).unwrap();
                let v0_idx = face.verts.iter().position(|id| *id == edge_v0).unwrap();
                
                let reversed =edge_v1 != face.verts[(v0_idx + 1) % face.verts.len()];
                let mut n = cross(joint_edge_dir, face.frame.normal());
                //let mut n = cross(face.frame.normal(),joint_edge_dir);
                if reversed {n = n.multiply_scalar(-1.0)}
                ccw_angle(&n, joint_frame)
            };

            let bb = {
                let face = self.topology.face(b.face_id).unwrap();
                let v0_idx = face.verts.iter().position(|id| *id == edge_v0).unwrap();

                let reversed = edge_v1 != face.verts[(v0_idx + 1) % face.verts.len()];
                let mut n =cross(joint_edge_dir, face.frame.normal());
                //let mut n =cross(face.frame.normal(),joint_edge_dir);
                if reversed {n = n.multiply_scalar(-1.0)}
                ccw_angle(&n, joint_frame)
            };

            aa.partial_cmp(&bb).unwrap()
        });
        
        let n = assemblies.len();
        if n > 1 {
            let mut avg = vec![0.0; n];
            for i in 0..n {
                let mut sum = 0.0;
                for j in 0..n {
                    if i != j {sum += dot(&assemblies[i].direction,&assemblies[j].direction,).clamp(-1.0, 1.0).acos()}
                }
                avg[i] = sum / (n - 1) as f64;
            }

            let best = avg.iter().enumerate().min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()).unwrap().0;
            assemblies.rotate_left(best);
        }

        let mut priority_layers = Vec::<(f64, LayerRef)>::new();

        for (assembly_idx, assembly) in assemblies.iter().enumerate() {
            let face = self.topology.face(assembly.face_id).unwrap();

            let group_id = face.layer_group_id.unwrap();
            let group = self.construction_set.layer_group(group_id).unwrap();

            for layer_idx in 0..group.layers.len() {
                let layer = self.construction_set.layer(group.layers[layer_idx]).unwrap();
                priority_layers.push((layer.priority, LayerRef {face_id: assembly.face_id, layer_idx,assembly_idx}));
            }
        }

        priority_layers.sort_by(|a, b| {
            b.0.partial_cmp(&a.0).unwrap()
        });

        let mut layer_joints = HashMap::<LayerRef, LayerRef>::new();
        let mut covered = HashMap::<LayerRef, LayerRef>::new();

        let visited = |map: &HashMap<LayerRef, LayerRef>, r: LayerRef| {
            map.contains_key(&r) || map.values().any(|x| *x == r)
        };

        let mut cursor = 0;

        while cursor < priority_layers.len() {
            let priority = priority_layers[cursor].0;
            let class = priority_class(priority);

            let mut end = cursor + 1;

            while end < priority_layers.len()
                && priority_class(priority_layers[end].0) == class
            {
                end += 1;
            }

            for &(layer_priority, layer_ref)
                in &priority_layers[cursor..end]
            {
                let index = layer_ref.assembly_idx;
                let scan_cw = {
                    let face = self.topology.face(layer_ref.face_id).unwrap();
                    let group = self.construction_set.layer_group(face.layer_group_id.unwrap()).unwrap();
                    group.layers[layer_ref.layer_idx + 1..].iter().any(|id| {
                        self.construction_set.layer(*id).unwrap().priority > layer_priority
                    })
                };

                let mut candidate_index = index;

                for _ in 0..4 {
                    if visited(&layer_joints, layer_ref) {
                        break;
                    }

                    let go_cw =
                        !assemblies[index].is_ccw ^ scan_cw;

                    candidate_index = (candidate_index as isize + if go_cw { -1 } else { 1 }).rem_euclid(assemblies.len() as isize) as usize;

                    let flip_layers = scan_cw ^ (assemblies[index].is_ccw != assemblies[candidate_index].is_ccw);

                    let candidate = assemblies[candidate_index];

                    let face = self.topology.face(candidate.face_id).unwrap();
                    let group = self.construction_set.layer_group(face.layer_group_id.unwrap()).unwrap();

                    let indices: Vec<usize> = if flip_layers {
                        (0..group.layers.len()).rev().collect()
                    } else {
                        (0..group.layers.len()).collect()
                    };

                    let mut found = false;

                    for candidate_layer_idx in indices {

                        let candidate_layer = self.construction_set.layer(group.layers[candidate_layer_idx]).unwrap();     
                        let candidate_ref = LayerRef {face_id: candidate.face_id, layer_idx: candidate_layer_idx,assembly_idx: candidate_index};

                        let face_idx = joint_faces.iter().position(|id| *id == layer_ref.face_id).unwrap();                    
                        let candidate_face_idx = joint_faces.iter().position(|id| *id == candidate_ref.face_id).unwrap();

                        // Higher priority -> butt.
                        if candidate_layer.priority > layer_priority {
                            self.apply_joint(
                                face_idx,
                                candidate_face_idx,
                                joint_edge_id,
                                layer_ref.layer_idx,
                                candidate_ref.layer_idx,
                                JointType::Butt,
                            );

                            layer_joints.insert(layer_ref, candidate_ref);

                            found = true;
                            break;
                        }

                        // Same priority class.
                        if priority_class(candidate_layer.priority)
                            != class
                        {
                            continue;
                        }

                        let mut candidate_ref = candidate_ref;

                        if visited(&layer_joints, candidate_ref) {
                            if let Some(&covered_ref) = covered.get(&candidate_ref) {
                                candidate_ref = covered_ref;
                            }

                            let face_idx = joint_faces.iter().position(|id| *id == layer_ref.face_id).unwrap();
                            let candidate_face_idx = joint_faces.iter().position(|id| *id == candidate_ref.face_id).unwrap();

                            self.apply_joint(
                                face_idx,
                                candidate_face_idx,
                                joint_edge_id,
                                layer_ref.layer_idx,
                                candidate_ref.layer_idx,
                                JointType::Butt,
                            );

                            layer_joints.insert(layer_ref, candidate_ref);

                            found = true;
                            break;
                        }

                        let layer = self.layer_from_ref(layer_ref);

                        if layer.joint_type == "miter_joint"
                            && candidate_layer.joint_type == "miter_joint"
                        {
                            self.apply_joint(
                                face_idx,
                                candidate_face_idx,
                                joint_edge_id,
                                layer_ref.layer_idx,
                                candidate_ref.layer_idx,
                                JointType::Miter,
                            );
                        } else {
                            self.apply_joint(
                                face_idx,
                                candidate_face_idx,
                                joint_edge_id,
                                layer_ref.layer_idx,
                                candidate_ref.layer_idx,
                                JointType::DoubleButt,
                            );

                            covered.insert(candidate_ref, layer_ref);
                        }

                        layer_joints.insert(layer_ref, candidate_ref);
                        found = true;
                        break;
                    }

                    if found {
                        break;
                    }
                }
            }
            cursor = end;
        }
        layer_joints
    }
    
    pub fn solve_joints(&mut self){
        for edge_id in self.topology.edges.ids().collect::<Vec<_>>(){
            self.solve_joint(edge_id);
        }
    }
}