import numpy as np
import cv2 as cv

def sample_to_img(sample,colours):
        colours = np.array(colours)
        if (colours>1).any():
                colours = colours/255
        out = np.zeros((64,64,3))
        for n,layer in enumerate(sample):
                out += np.einsum('k,ij->ijk',colours[n][:3],layer)
        return(out.clip(0,1))


def trace_edge(edge_map):
    """
    edge_map: 2D binary array (single-pixel, 0/1)
    returns: list of (row, col) coordinates in order along the edge
    """
    # Find all edge pixels
    pixels = np.argwhere(edge_map)
    visited = np.zeros_like(edge_map, dtype=bool)
    
    # Start at one endpoint (pixel with only one neighbor) or first pixel
    def neighbors(r, c):
        for dr, dc in [(-1,0),(1,0),(0,-1),(0,1)]:  # 4-connectivity
            nr, nc = r+dr, c+dc
            if 0 <= nr < edge_map.shape[0] and 0 <= nc < edge_map.shape[1]:
                if edge_map[nr, nc] and not visited[nr, nc]:
                    yield nr, nc

    # Find endpoint (pixel with only 1 neighbor)
    endpoints = []
    for r,c in pixels:
        cnt = sum(1 for _ in neighbors(r,c))
        if cnt == 1:
            endpoints.append((r,c))
    start = endpoints[0] if endpoints else tuple(pixels[0])

    # Sequential walk
    ordered = [start]
    visited[start] = True
    r, c = start

    while True:
        next_pixels = list(neighbors(r, c))
        if not next_pixels:
            break
        r, c = next_pixels[0]  # there’s only 1 unvisited neighbor
        ordered.append((r, c))
        visited[r, c] = True

    return np.array(ordered)

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

def extract_boundaries(onehot_array,smoothing =2):
    edge_map = depthwise_conv2x2(onehot_array)
    ends = np.argwhere(edge_map.sum(0)>2)
    N = onehot_array.shape[0]
    boundaries = np.unique(edge_map.reshape(N, -1).T, axis=0)

    edges = []
    adjacencies = []
    for boundary in boundaries:
        if boundary.sum() != 2:
            continue
        edge = trace_edge(np.all(edge_map.transpose(1, 2, 0) == boundary, axis=-1))
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

