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
import cv2 as cv
import matplotlib.pyplot as plt

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


def trace_edge(edge_map):
    """
    edge_map: 2D binary array (single-pixel, 0/1)
    returns: list of (row, col) coordinates in order along the edge
    """
    # Find all edge pixels
    pixels = np.argwhere(edge_map)
    visited = np.zeros_like(edge_map, dtype=bool)
    added_pixels = 0
    edges = []

    # Start at one endpoint (pixel with only one neighbor) or first pixel
    def neighbors(r, c):
        for dr, dc in [(-1,0),(1,0),(0,-1),(0,1)]:  # 4-connectivity
            nr, nc = r+dr, c+dc
            if 0 <= nr < edge_map.shape[0] and 0 <= nc < edge_map.shape[1]:
                if edge_map[nr, nc] and not visited[nr, nc]:
                    yield nr, nc
    
    #instead of while True just in case, 100 should never be reached
    for n in range(100):
        # Find endpoint (pixel with only 1 neighbor)
        endpoints = []
        for r,c in pixels:
            if visited[r, c] == False:
                cnt = sum(1 for _ in neighbors(r,c))
                if cnt == 1:
                    endpoints.append((r,c))
        start = endpoints[0] if endpoints else tuple(pixels[0])
        added_pixels +=1
        # Sequential walk
        ordered_edge = [start]
        visited[start] = True
        r, c = start
    
        while True:
            next_pixels = list(neighbors(r, c))
            if not next_pixels:
                break
            r, c = next_pixels[0]  # there’s only 1 unvisited neighbor
            ordered_edge.append((r, c))
            visited[r, c] = True
            added_pixels +=1
        edges.append(np.array(ordered_edge))
        
        if added_pixels == len(pixels):
            break
    return edges

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


def extract_boundaries(onehot,smoothing =2):
    edge_map = depthwise_conv2x2(onehot)
    ends = np.argwhere(edge_map.sum(0)>2)
    boundaries = np.unique(edge_map.reshape(edge_map.shape[0], -1).T, axis=0)

    edges = []
    adjacencies = []
    for boundary in boundaries:
        if boundary.sum() != 2:
            continue
        
        new_edges = trace_edge(np.all(edge_map.transpose(1, 2, 0) == boundary, axis=-1))
        for edge in new_edges:
            if len(edge)>1:
                edge = np.stack([ends[np.argmin(np.abs((edge[0]-ends)).sum(1))],*edge,ends[np.argmin(np.abs((edge[-1]-ends)).sum(1))]]).astype(np.int32)
            else:
                closest_ends = np.argsort(np.abs((edge[0]-ends)).sum(1))
                edge = np.stack([ends[closest_ends[0]],*edge,ends[closest_ends[1]]]).astype(np.int32)
            approx = cv.approxPolyDP(edge, smoothing, closed=False)
            edges.append(approx.squeeze().tolist())
            adjacencies.append(boundary.tolist())
    diff = ends[:, np.newaxis, :] - ends[np.newaxis, :, :]  # shape (N, N, 2)
    dists = np.linalg.norm(diff, axis=2) 
    dists = dists==1
    N = len(ends)
    mask = np.tril(np.ones((N, N), dtype=bool))
    dists[mask] = False
    for end,dist in zip(ends,dists):
        if dist.any() == True:
            edge =[end.tolist(),ends[dist][0].tolist()]
            edges.append(edge)
            adjacencies.append((edge_map[:,edge[0][0],edge[0][1]]*edge_map[:,edge[1][0],edge[1][1]]).tolist())
    return(edges,adjacencies)


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
 
