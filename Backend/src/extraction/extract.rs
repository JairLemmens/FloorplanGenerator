use rayon::prelude::*;
use std::collections::HashMap;

use crate::extraction::douglas_peucker::{
    douglas_peucker,
    Point,
};

const NONE: i32 = -1;

/// 256-bit mask.
///
/// Channel n corresponds to:
///
///     bits[n / 64] bit (n % 64)
///
/// This is the complete identity of a pixel.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Mask256 {
    bits: [u64; 4],
}

impl Mask256 {
    #[inline]
    fn zero() -> Self {
        Self {
            bits: [0; 4],
        }
    }

    #[inline]
    fn popcount(self) -> u32 {
        self.bits[0].count_ones()
            + self.bits[1].count_ones()
            + self.bits[2].count_ones()
            + self.bits[3].count_ones()
    }

    #[inline]
    fn intersection(self, other: Self) -> Self {
        Self {
            bits: [
                self.bits[0] & other.bits[0],
                self.bits[1] & other.bits[1],
                self.bits[2] & other.bits[2],
                self.bits[3] & other.bits[3],
            ],
        }
    }

    /// Return the two set channel indices.
    ///
    /// Only valid when popcount() == 2.
    #[inline]
    fn two_bits(self) -> (u8, u8) {
        debug_assert_eq!(self.popcount(), 2);

        let mut first = 0u8;
        let mut second = 0u8;
        let mut found = 0;

        for word_idx in 0..4 {
            let mut word = self.bits[word_idx];

            while word != 0 {
                let bit = word.trailing_zeros() as u8;
                let channel = (word_idx as u8) * 64 + bit;

                if found == 0 {
                    first = channel;
                } else {
                    second = channel;
                }

                found += 1;
                word &= word - 1;
            }
        }

        (first, second)
    }

    /// Compact ID for a 2-bit boundary.
    ///
    /// Since channels are [0,255]:
    ///
    ///     id = first * 256 + second
    ///
    /// fits exactly in u16.
    #[inline]
    fn boundary_id(self) -> u16 {
        let (a, b) = self.two_bits();
        ((a as u16) << 8) | b as u16
    }
        
    #[inline]
    fn to_pair(self) -> [u8; 2] {
        self.two_bits().into()
    }
}

#[derive(Clone, Copy)]
struct Endpoint {row: i32, col: i32}

struct TracedEdge {points: Vec<[i32; 2]>, adjacency: [u8; 2]}


#[inline]
fn pixel_index(row: usize, col: usize, w: usize) -> usize {
    row * w + col
}

#[inline]
fn row_col(index: usize, w: usize) -> (usize, usize) {
    (index / w, index % w)
}

/// Calls f for the 4 neighbors in the same order as the
/// original Python implementation:
///
///     up, down, left, right
#[inline]
fn for_each_neighbor<F>(index: usize, h: usize, w: usize, mut f: F)
where
    F: FnMut(usize),
{
    let row = index / w;
    let col = index % w;

    if row > 0 {
        f(index - w);
    }

    if row + 1 < h {
        f(index + w);
    }

    if col > 0 {
        f(index - 1);
    }

    if col + 1 < w {
        f(index + 1);
    }
}

/// Pack one batch from PyTorch's contiguous B,N,H,W layout.
///
/// Input layout:
///
///     edge_map[b,n,r,c]
///
/// Output:
///
///     keys[r,c]
///
/// where each key contains all N channel bits.
fn pack_batch(input: &[u8], n: usize, h: usize, w: usize, batch: usize, output: &mut [Mask256]) {
    let hw = h * w;
    let batch_offset = batch * n * hw;

    debug_assert_eq!(output.len(), hw);

    for channel in 0..n {
        let input_offset =
            batch_offset + channel * hw;

        let word = channel >> 6;
        let bit = channel & 63;
        let mask = 1u64 << bit;

        for p in 0..hw {
            if input[input_offset + p] != 0 {
                output[p].bits[word] |= mask;
            }
        }
    }
}

/// Trace one boundary group.
///
/// `pixels` is already in row-major order.
///
/// We intentionally preserve the Python neighbor ordering:
///
///     up, down, left, right
///
/// and dynamically determine endpoints from unvisited neighbors,
/// matching the behavior of the original trace_edge().
fn trace_group(pixels: &[usize], key: Mask256, h: usize, w: usize, keys: &[Mask256]) -> Vec<Vec<usize>> {
    if pixels.is_empty() {
        return Vec::new();
    }

    // Membership of this particular boundary.
    //
    // Because groups are processed independently, a local hash set
    // avoids a full H*W visited allocation per group.
    let membership: std::collections::HashSet<usize> =
        pixels.iter().copied().collect();

    let mut visited =
        std::collections::HashSet::<usize>::with_capacity(
            pixels.len(),
        );

    let mut remaining = pixels.len();

    let mut traces = Vec::new();

    while remaining > 0 {
        // Match Python:
        //
        //     endpoints = []
        //     for pixel in pixels:
        //         if not visited:
        //             cnt = sum(neighbors)
        //             if cnt == 1:
        //                 endpoints.append(pixel)
        //
        //     start = endpoints[0] if endpoints else pixels[0]
        let mut start = None;

        for &p in pixels {
            if visited.contains(&p) {
                continue;
            }

            let mut count = 0usize;

            for_each_neighbor(p, h, w, |q| {
                if count != 2
                    && membership.contains(&q)
                    && !visited.contains(&q)
                    && keys[q] == key
                {
                    count += 1;
                }
            });

            if count == 1 {
                start = Some(p);
                break;
            }
        }

        let start = start.unwrap_or_else(|| {
            pixels
                .iter()
                .copied()
                .find(|p| !visited.contains(p))
                .unwrap()
        });

        let mut ordered = Vec::new();

        let mut current = start;

        visited.insert(current);
        remaining -= 1;
        ordered.push(current);

        loop {
            let mut next = None;

            for_each_neighbor(current, h, w, |q| {
                if next.is_none()
                    && membership.contains(&q)
                    && !visited.contains(&q)
                    && keys[q] == key
                {
                    next = Some(q);
                }
            });

            match next {
                Some(q) => {
                    visited.insert(q);
                    remaining -= 1;

                    ordered.push(q);
                    current = q;
                }

                None => break,
            }
        }

        traces.push(ordered);
    }

    traces
}

/// Find endpoint IDs adjacent to a pixel.
///
/// Returns up to four endpoint IDs.
#[inline]
fn adjacent_endpoints(pixel: usize, h: usize, w: usize, endpoint_map: &[i32]) -> ([i32; 4], usize) {
    let mut result = [NONE; 4];
    let mut count = 0usize;

    for_each_neighbor(pixel, h, w, |q| {
        if count < 4 {
            let endpoint = endpoint_map[q];

            if endpoint != NONE {
                result[count] = endpoint;
                count += 1;
            }
        }
    });

    (result, count)
}

/// Find the first endpoint adjacent to a boundary pixel.
#[inline]
fn first_adjacent_endpoint(pixel: usize, h: usize, w: usize, endpoint_map: &[i32]) -> Option<usize> {
    let (ids, count) =
        adjacent_endpoints(pixel, h, w, endpoint_map);

    for &id in ids[..count].iter() {
        if id != NONE {
            return Some(id as usize);
        }
    }

    None
}

/// Attach endpoints to a traced boundary.
///
/// Structural invariant:
///
///     The closest endpoint to each boundary endpoint pixel
///     is guaranteed to be one of its 4-neighbors.
///
/// Therefore no distance calculation / KD-tree is needed.
fn attach_endpoints(trace: &[usize], h: usize, w: usize, endpoint_map: &[i32], endpoints: &[Endpoint]) -> Vec<[i32; 2]> {
    if trace.is_empty() {
        return Vec::new();
    }

    if trace.len() == 1 {
        let pixel = trace[0];

        let (ids, count) =
            adjacent_endpoints(
                pixel,
                h,
                w,
                endpoint_map,
            );

        let mut first = None;
        let mut second = None;

        for &id in ids[..count].iter() {
            if id == NONE {
                continue;
            }

            let id = id as usize;

            if first.is_none() {
                first = Some(id);
            } else if Some(id) != first {
                second = Some(id);
                break;
            }
        }

        // Structural invariant should normally guarantee both.
        if let (Some(a), Some(b)) = (first, second) {
            let mut result = Vec::with_capacity(3);

            result.push([
                endpoints[a].row,
                endpoints[a].col,
            ]);

            let (r, c) = row_col(pixel, w);

            result.push([
                r as i32,
                c as i32,
            ]);

            result.push([
                endpoints[b].row,
                endpoints[b].col,
            ]);

            return result;
        }

        // Defensive fallback.
        let (r, c) = row_col(pixel, w);

        let mut result = Vec::with_capacity(3);

        if let Some(a) = first {
            result.push([
                endpoints[a].row,
                endpoints[a].col,
            ]);
        }

        result.push([
            r as i32,
            c as i32,
        ]);

        if let Some(b) = second {
            result.push([
                endpoints[b].row,
                endpoints[b].col,
            ]);
        }

        return result;
    }

    let first_endpoint =
        first_adjacent_endpoint(
            trace[0],
            h,
            w,
            endpoint_map,
        );

    let last_endpoint =
        first_adjacent_endpoint(
            *trace.last().unwrap(),
            h,
            w,
            endpoint_map,
        );

    let mut result =
        Vec::with_capacity(trace.len() + 2);

    if let Some(id) = first_endpoint {
        result.push([
            endpoints[id].row,
            endpoints[id].col,
        ]);
    }

    for &pixel in trace {
        let (r, c) = row_col(pixel, w);

        result.push([
            r as i32,
            c as i32,
        ]);
    }

    if let Some(id) = last_endpoint {
        result.push([
            endpoints[id].row,
            endpoints[id].col,
        ]);
    }

    result
}

/// Simplify one edge using Douglas-Peucker.
fn simplify_edge(
    edge: &[[i32; 2]],
    smoothing: f64,
) -> Vec<[i32; 2]> {
    if edge.len() <= 2 || smoothing <= 0.0 {
        return edge.to_vec();
    }

    let points: Vec<Point> = edge
        .iter()
        .map(|p| Point {
            x: p[1] as f64,
            y: p[0] as f64,
        })
        .collect();

    let simplified =
        douglas_peucker(&points, smoothing);

    simplified
        .into_iter()
        .map(|p| [
            p.y as i32,
            p.x as i32,
        ])
        .collect()
}

/// Main extraction implementation.
///
/// Input layout:
///
///     B,N,H,W
///
/// PyTorch contiguous row-major storage.
pub unsafe fn extract_boundaries_raw(edge_ptr: *const u8, b: usize, n: usize, h: usize, w: usize, smoothing: f64) -> (Vec<Vec<[i32; 2]>>, Vec<[i64; 2]>) 
{
    let hw = h * w;
    let total = b * n * hw;

    let input = unsafe {std::slice::from_raw_parts(edge_ptr, total)};

    let mut all_edges: Vec<TracedEdge> =
        Vec::new();

    /*
     * Process each batch independently.
     *
     * Packing is parallel across batches.
     */
    let mut all_keys =
        vec![Mask256::zero(); b * hw];

    all_keys
        .par_chunks_mut(hw)
        .enumerate()
        .for_each(|(batch, output)| {
            pack_batch(input, n, h, w, batch, output,);
        });

    /*
     * Process each batch independently.
     */
    for batch in 0..b {
        let keys =
            &all_keys[batch * hw..(batch + 1) * hw];

        /*
         * Endpoint pixels:
         *
         * popcount(mask) > 2
         */
        let mut endpoints =
            Vec::<Endpoint>::new();

        let mut endpoint_map =
            vec![NONE; hw];

        for p in 0..hw {
            if keys[p].popcount() > 2 {
                let (r, c) = row_col(p, w);

                let id = endpoints.len();

                endpoints.push(Endpoint {
                    row: r as i32,
                    col: c as i32,
                });

                endpoint_map[p] = id as i32;
            }
        }

        /*
         * Group all pixels having exactly two
         * active channels.
         *
         * key:
         *
         *     pair of channels
         *
         * is represented by u16.
         */
        let mut groups:
            HashMap<u16, Vec<usize>> =
            HashMap::new();

        for p in 0..hw {
            let key = keys[p];

            if key.popcount() == 2 {
                let boundary_id =
                    key.boundary_id();

                groups
                    .entry(boundary_id)
                    .or_default()
                    .push(p);
            }
        }

        /*
         * Sort groups by boundary ID so output ordering
         * is deterministic.
         */
        let mut ordered_groups:
            Vec<(u16, Vec<usize>)> =
            groups.into_iter().collect();

        ordered_groups.sort_unstable_by_key(
            |(id, _)| *id,
        );

        /*
         * Trace boundary groups in parallel.
         */
        let traced: Vec<TracedEdge> = ordered_groups
            .par_iter()
            .flat_map_iter(|(boundary_id, pixels)| {
                let a = (*boundary_id >> 8) as usize;
                let bb = (*boundary_id & 0xff) as usize;

                let mut key = Mask256::zero();

                key.bits[a >> 6] |= 1u64 << (a & 63);
                key.bits[bb >> 6] |= 1u64 << (bb & 63);

                let traces = trace_group(
                    pixels,
                    key,
                    h,
                    w,
                    keys,
                );

                let endpoint_map = &endpoint_map;
                let endpoints = &endpoints;

                traces.into_iter().filter_map(move |trace| {
                    if trace.is_empty() {
                        return None;
                    }

                    let attached = attach_endpoints(
                        &trace,
                        h,
                        w,
                        endpoint_map,
                        endpoints,
                    );

                    if attached.len() <= 1 {
                        return None;
                    }

                    let simplified =
                        simplify_edge(&attached, smoothing);

                    Some(TracedEdge {
                        points: simplified,
                        adjacency: key.to_pair(),
                    })
                })
            })
            .collect();

        all_edges.extend(traced);

        /*
         * Direct endpoint-to-endpoint adjacency.
         */
        for endpoint_id in 0..endpoints.len() {
            let endpoint =
                endpoints[endpoint_id];

            let p = pixel_index(
                endpoint.row as usize,
                endpoint.col as usize,
                w,
            );

            for_each_neighbor(p, h, w, |q| {
                let other_id =
                    endpoint_map[q];

                if other_id == NONE {
                    return;
                }

                let other_id =
                    other_id as usize;

                if other_id <= endpoint_id {
                    return;
                }

                let other =
                    endpoints[other_id];

                let adjacency =
                    keys[p]
                        .intersection(keys[q])
                        .to_pair();

                all_edges.push(TracedEdge {
                    points: vec![
                        [
                            endpoint.row,
                            endpoint.col,
                        ],
                        [
                            other.row,
                            other.col,
                        ],
                    ],
                    adjacency,
                });
            });
        }
    }

    /*
     * Return the same conceptual structure as the Python version:
     *
     *     edges
     *     adjacencies
     *
     * Batch is intentionally not added to the coordinate output.
     */
    let mut edges = Vec::with_capacity(all_edges.len());
    let mut adjacencies = Vec::with_capacity(all_edges.len() * 2);

    for (edge_idx, edge) in all_edges.iter().enumerate() {
        edges.push(edge.points.clone());

        adjacencies.push([
            edge_idx as i64,
            edge.adjacency[0] as i64,
        ]);

        adjacencies.push([
            edge_idx as i64,
            edge.adjacency[1] as i64,
        ]);
    }

    (edges, adjacencies)
}