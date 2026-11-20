/// Disjoint-set over `0..n`, used for edge-count classes, chord collapses and
/// vertex welding.
pub(crate) struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    pub(crate) fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Join sets; the second root becomes the representative.
    pub(crate) fn join(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        self.parent[ra] = rb;
    }

    /// Join sets; the smallest index becomes the representative.
    pub(crate) fn join_min(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        self.parent[ra.max(rb)] = ra.min(rb);
    }
}
