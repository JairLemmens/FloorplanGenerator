use crate::base_types::Id;
use std::collections::HashMap;

/// Small disjoint-set helper shared by mesh assembly and simplification.
pub(crate) struct UnionFind {
    parent: Vec<usize>,
}

pub(crate) struct IdUnionFind {
    parent: HashMap<Id, Id>,
}
impl IdUnionFind {
    pub(crate) fn new(ids: impl IntoIterator<Item = Id>) -> Self {
        Self {
            parent: ids.into_iter().map(|id| (id, id)).collect(),
        }
    }
    pub(crate) fn find(&mut self, id: Id) -> Id {
        let parent = self.parent[&id];
        if parent == id {
            id
        } else {
            let root = self.find(parent);
            self.parent.insert(id, root);
            root
        }
    }
    pub(crate) fn join(&mut self, a: Id, b: Id) {
        let ra = self.find(a);
        let rb = self.find(b);
        self.parent.insert(ra, rb);
    }
    pub(crate) fn join_min(&mut self, a: Id, b: Id) {
        let ra = self.find(a);
        let rb = self.find(b);
        self.parent.insert(ra.max(rb), ra.min(rb));
    }
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

    /// Join sets using the second root as representative, matching the historical
    /// behavior used for count constraints and chord collapses.
    pub(crate) fn join(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        self.parent[ra] = rb;
    }
}
