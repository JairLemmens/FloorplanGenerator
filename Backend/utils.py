# Copyright 2026 Jair Lemmens JairLemmens@gmail.com

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at

# http://www.apache.org/licenses/LICENSE-2.0

# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import numpy as np
import matplotlib.pyplot as plt
import floorplan_backend 
from collections import defaultdict

def sample_to_img(sample,colours =None,imsize=64):
    
    out = np.zeros((imsize,imsize,3))

    if colours is None:
        colours = plt.get_cmap('hsv', sample.shape[0]+1)
        for n,layer in enumerate(sample):
            out += np.einsum('k,ij->ijk',colours(n)[:3],np.where(layer.round()==1,1,0))
    else:
        colours = np.array(colours)
        if (colours>1).any():
            colours = colours/255
        for n,layer in enumerate(sample):
                out += np.einsum('k,ij->ijk',colours[n][:3],np.where(layer.round()==1,1,0))
    return(out.clip(0,1))

def depthwise_conv2x2(arr):
    # arr: (num_layers, H, W)
    N, H, W = arr.shape
    out_H, out_W = H - 1, W - 1  # kernel_size=2, stride=1
    
    # Correct strides for 2x2 sliding windows
    shape = (N, out_H, out_W, 2, 2)
    strides = (arr.strides[0], arr.strides[1], arr.strides[2], arr.strides[1], arr.strides[2])
    
    windows = np.lib.stride_tricks.as_strided(arr, shape=shape, strides=strides)
    
    # Sum over 2x2 windows
    conv = windows.sum(axis=(3, 4))

    mask = conv > 0
    return mask
    
def extract_boundaries(edge_map, smoothing=2.0):
    if edge_map.dtype != np.uint8:
        raise TypeError("edge_map must be np.uint8")

    if edge_map.ndim != 4:
        raise ValueError("edge_map must have shape (B,N,H,W)")

    if edge_map.shape[1] > 256:
        raise ValueError("N must be <= 256")

    if not edge_map.flags.c_contiguous:
        edge_map = np.ascontiguousarray(edge_map)

    b, n, h, w = edge_map.shape

    return floorplan_backend.extract_boundaries(edge_map.ctypes.data, b, n, h, w, smoothing)

def boundaries_to_mesh(edges_raw, face_adjacencies_raw, z=0,scale = 1):
    """
    Convert polyline edges and face adjacency data into a mesh representation.
 
    Parameters
    ----------
    edges_raw : list
        List of polylines, where each polyline is a list of [x, y] points.
        e.g. [[[31, 74], [22, 76], [56, 83]], [[81, 76], [114, 73]], ...]
    face_adjacencies_raw : list
        List of [edge_index, face_index] pairs describing which polylines
        bound which faces.
        e.g. [[0, 5], [0, 6], [1, 4], ...]
    z : float, optional
        Z-coordinate to assign to all vertices (default: 0).
 
    Returns
    -------
    vertices : list
        List of vertices in the form [x, y, z].
    edges : list
        List of edges in the form [i, j] where i and j are vertex indices.
    faces : list
        List of faces in the form [i, j, k, ...] where items are vertex indices.
        Faces are assumed closed (last vertex connects back to first).
    """
    vertex_list = []
    vertex_index = {}
 
    def get_or_add_vertex(pt):
        key = tuple(pt)
        if key not in vertex_index:
            vertex_index[key] = len(vertex_list)
            vertex_list.append(key)
        return vertex_index[key]
 
    # Register all vertices
    for polyline in edges_raw:
        for pt in polyline:
            get_or_add_vertex(pt)
 
    # Convert polylines to vertex index sequences
    polyline_verts = [
        [get_or_add_vertex(pt) for pt in polyline]
        for polyline in edges_raw
    ]
 
    # Build deduplicated edges from consecutive pairs in each polyline
    edge_set = set()
    edges_out = []
    for verts in polyline_verts:
        for i in range(len(verts) - 1):
            key = frozenset((verts[i], verts[i + 1]))
            if key not in edge_set:
                edge_set.add(key)
                edges_out.append([verts[i], verts[i + 1]])
 
    # Group polyline indices by face
    face_to_polylines = defaultdict(list)
    for edge_idx, face_idx in face_adjacencies_raw:
        face_to_polylines[face_idx].append(edge_idx)
 
    def chain_polylines(polyline_indices):
        """Chain a set of polylines into a single ordered vertex loop."""
        segs = [list(polyline_verts[i]) for i in polyline_indices]
        result = list(segs[0])
        used = {0}
 
        for _ in range(len(segs) - 1):
            tail = result[-1]
            head = result[0]
            found = False
            for j, seg in enumerate(segs):
                if j in used:
                    continue
                if seg[0] == tail:
                    result.extend(seg[1:])
                    used.add(j)
                    found = True
                    break
                elif seg[-1] == tail:
                    result.extend(reversed(seg[:-1]))
                    used.add(j)
                    found = True
                    break
                elif seg[0] == head:
                    result = list(reversed(seg[1:])) + result
                    used.add(j)
                    found = True
                    break
                elif seg[-1] == head:
                    result = list(seg[:-1]) + result
                    used.add(j)
                    found = True
                    break
            if not found:
                break  # incomplete chain; return what we have
 
        # Drop closing duplicate vertex if present
        if result and result[0] == result[-1]:
            result = result[:-1]
 
        return result
 
    faces_out = [
        chain_polylines(face_to_polylines[face_idx])
        for face_idx in sorted(face_to_polylines)
    ]
 
    vertices_out = [[x*scale, y*scale, z] for x, y in vertex_list]
 
    return vertices_out, edges_out, faces_out
 
