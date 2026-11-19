"""
Quad meshing with coarse layouts - stages that follow the segmentation.

    regions  = segment_domain(domain)              # (your code, unchanged)
    patches  = build_layout(regions)               # 3. template-based quadrilateralization
    patches  = remove_tjunctions(patches)          # 4. T-junction elimination
    counts   = choose_counts(patches)              # 5. subdivision counts for l_tar
    V, Q     = mesh_layout(patches, counts)        #    conforming initial quad mesh
    V, Q     = simplify(V, Q, domain)              # 6. poly-chord collapse/split

Layout data structure: a patch is a list of 4 polylines (np arrays, CCW, side i runs
corner i -> corner i+1). Everything below works on that representation.

Deliberate simplifications (see notes at the bottom of the file):
  * step 1 (boundary straightening) is not implemented, the pipeline runs on the input domain
  * k<4 templates use virtual corners / a 3-patch star, not the paper's exact template drawings
  * T-junction removal has an iteration cap instead of the singularity-insertion fallback
"""
import itertools
from collections import defaultdict
from dataclasses import dataclass

import numpy as np
import shapely
from shapely.geometry import Polygon, LineString, MultiLineString, MultiPolygon
from shapely.geometry.polygon import orient
from shapely.ops import substring, unary_union, polygonize

TWO_PI = 2.0 * np.pi


@dataclass
class QParams:
    l_tar: float = 1.0                    # target edge length
    alpha_th: float = 0.5                 # template acceptance threshold (radians)
    straight_tol: float = np.deg2rad(20)  # |angle-180| below this = not a corner
    max_corner_cands: int = 10
    max_combos: int = 40
    l_mer_factor: float = 1.0             # merge distance for T-junctions = factor * l_tar
    max_t_iter: int = 300
    hausdorff_tol: float = None           # default 1e-6 * domain size (exact boundary)
    max_outer_iter: int = 10
    collapse_ratio: float = 2.0 / 3.0     # collapse chords narrower than this * l_tar
    split_ratio: float = 4.0 / 3.0        # split chords wider than this * l_tar (see notes)
    max_ops_per_iter: int = 200


# --------------------------------------------------------------------------- #
# polyline helpers
# --------------------------------------------------------------------------- #
def _dedup(pl, tol=1e-12):
    out = [pl[0]]
    for q in pl[1:]:
        if np.hypot(*(q - out[-1])) > tol:
            out.append(q)
    if len(out) == 1:
        out.append(pl[-1])
    return np.array(out)


def _seglen(pl):
    return np.hypot(*np.diff(pl, axis=0).T)


def poly_len(pl):
    return float(_seglen(pl).sum())


def _cum(pl):
    return np.concatenate([[0.0], np.cumsum(_seglen(pl))])


def point_at(pl, d):
    s = _cum(pl)
    return np.array([np.interp(d, s, pl[:, 0]), np.interp(d, s, pl[:, 1])])


def sub_pl(pl, a, b):
    """Part of polyline between arc positions a<b."""
    s = _cum(pl)
    inner = pl[(s > a + 1e-12) & (s < b - 1e-12)]
    return _dedup(np.vstack([point_at(pl, a), inner, point_at(pl, b)]))


def _bends(pl, ang_tol=1e-4):
    """arc positions of interior polyline vertices where the polyline turns."""
    s = _cum(pl)
    out = []
    for i in range(1, len(pl) - 1):
        a, b = pl[i] - pl[i - 1], pl[i + 1] - pl[i]
        if abs(np.arctan2(a[0] * b[1] - a[1] * b[0], a @ b)) > ang_tol:
            out.append(s[i])
    return out


def _resample_c(pl, n):
    s = _cum(pl)
    L = s[-1]
    t = np.linspace(0, L, n + 1)
    bs = _bends(pl)
    if bs and n >= len(bs) + 1:
        idx, prev = [], 0
        for sb in bs:
            prev = max(int(round(sb / L * n)), prev + 1)
            idx.append(prev)
        m = len(bs)
        for j in range(m - 1, -1, -1):
            idx[j] = min(idx[j], n - (m - j))
        t = np.interp(np.arange(n + 1), [0] + idx + [n], [0] + list(bs) + [L])
    return np.stack([np.interp(t, s, pl[:, 0]), np.interp(t, s, pl[:, 1])], 1)


def resample(pl, n):
    """n segments, bends of the polyline are kept as vertices (exact boundary); direction-independent."""
    if tuple(pl[0].round(9)) > tuple(pl[-1].round(9)):
        return _resample_c(pl[::-1], n)[::-1]
    return _resample_c(pl, n)


def project(pl, P):
    """(distance, arc position, closest point) of P on polyline."""
    a = pl[:-1]
    ab = pl[1:] - a
    sl = np.hypot(ab[:, 0], ab[:, 1])
    l2 = np.where(sl == 0, 1e-300, sl ** 2)
    t = np.clip(((P - a) * ab).sum(1) / l2, 0, 1)
    q = a + t[:, None] * ab
    d = np.hypot(*(q - P).T)
    k = int(d.argmin())
    s = np.concatenate([[0.0], np.cumsum(sl)])
    return float(d[k]), float(s[k] + t[k] * sl[k]), q[k]


def split_pl(pl, d, exact_pt=None):
    L = poly_len(pl)
    p = point_at(pl, float(np.clip(d, 0, L)))
    if exact_pt is not None:
        p = np.asarray(exact_pt, float)
    s = _cum(pl)
    k = min(int(np.searchsorted(s, d, side="right")) - 1, len(pl) - 2)
    a = _dedup(np.vstack([pl[:k + 1], p]))
    b = _dedup(np.vstack([p, pl[k + 1:]]))
    a[-1] = p
    b[0] = p
    return a, b


def _ang(v):
    return np.arctan2(v[1], v[0])


def _nearest_distances(points, targets):
    """Distance from each point to its nearest target, using bounded memory."""
    points, targets = np.asarray(points, float), np.asarray(targets, float)
    if not len(targets):
        return np.full(len(points), np.inf)
    out = np.empty(len(points))
    for i in range(0, len(points), 1024):
        d = points[i:i + 1024, None, :] - targets[None, :, :]
        out[i:i + 1024] = np.sqrt(np.min(np.einsum("ijk,ijk->ij", d, d), axis=1))
    return out


def _pairs_within(points, tol):
    """Yield close point pairs via a grid hash (no SciPy spatial-tree dependency)."""
    cells = defaultdict(list)
    for i, point in enumerate(points):
        cell = tuple(np.floor(point / tol).astype(np.int64))
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for j in cells[(cell[0] + dx, cell[1] + dy)]:
                    if np.linalg.norm(point - points[j]) <= tol:
                        yield j, i
        cells[cell].append(i)


# --------------------------------------------------------------------------- #
# quad-mesh helpers
# --------------------------------------------------------------------------- #
def coons(sides, nu, nv):
    """Transfinite (Coons) grid (nu+1, nv+1, 2) for 4 CCW sides."""
    B = resample(sides[0], nu)
    R = resample(sides[1], nv)
    T = resample(sides[2], nu)[::-1]
    Lf = resample(sides[3], nv)[::-1]
    u = np.linspace(0, 1, nu + 1)[:, None]
    c0, c1, c2, c3 = B[0], B[-1], T[-1], T[0]
    X = np.empty((nu + 1, nv + 1, 2))
    for j in range(nv + 1):
        v = j / nv
        X[:, j] = ((1 - v) * B + v * T + (1 - u) * Lf[j] + u * R[j]
                   - ((1 - u) * (1 - v) * c0 + u * (1 - v) * c1 + u * v * c2 + (1 - u) * v * c3))
    return X


def grid_to_mesh(X):
    nu, nv = X.shape[0] - 1, X.shape[1] - 1
    V = X.reshape(-1, 2)
    idx = lambda i, j: i * (nv + 1) + j
    Q = [[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]
         for i in range(nu) for j in range(nv)]
    return V, np.array(Q, int)


def _corner_jac(V, Q):
    """yields (k, e1, e2, D) for each quad corner."""
    P = V[Q]
    for k in range(4):
        pk, pa, pb = P[:, k], P[:, (k + 1) % 4], P[:, (k - 1) % 4]
        e1, e2 = pa - pk, pb - pk
        D = e1[:, 0] * e2[:, 1] - e1[:, 1] * e2[:, 0]
        yield k, e1, e2, D


def min_det(V, Q):
    return min(float(D.min()) for _, _, _, D in _corner_jac(V, Q))


def alpha_error(V, Q):
    """(1 / 4Nq) * sum |alpha - 90deg|; inf if any quad is flipped / has a reflex corner."""
    tot = 0.0
    for _, e1, e2, D in _corner_jac(V, Q):
        if (D <= 0).any():
            return np.inf
        ang = np.arctan2(D, (e1 * e2).sum(1))
        tot += np.abs(ang - np.pi / 2).sum()
    return tot / (4 * len(Q))


def edge_map(Q):
    em = defaultdict(list)
    for qi, q in enumerate(Q):
        for k in range(4):
            a, b = int(q[k]), int(q[(k + 1) % 4])
            em[(min(a, b), max(a, b))].append(qi)
    return em


def boundary_info(V, Q):
    em = edge_map(Q)
    bnd = np.zeros(len(V), bool)
    for (a, b), qs in em.items():
        if len(qs) == 1:
            bnd[a] = bnd[b] = True
    return em, bnd


def feature_vertices(V, Q, em, tol):
    """boundary vertices where the boundary turns by more than tol (geometric corners)."""
    nb = defaultdict(list)
    for (a, b), qs in em.items():
        if len(qs) == 1:
            nb[a].append(b)
            nb[b].append(a)
    feat = np.zeros(len(V), bool)
    for v, n in nb.items():
        if len(n) != 2:
            feat[v] = True
            continue
        u1, u2 = V[n[0]] - V[v], V[n[1]] - V[v]
        c = np.dot(u1, u2) / (np.linalg.norm(u1) * np.linalg.norm(u2) + 1e-300)
        if np.arccos(np.clip(c, -1, 1)) < np.pi - tol:
            feat[v] = True
    return feat


# --------------------------------------------------------------------------- #
# 3. template-based quadrilateralization
# --------------------------------------------------------------------------- #
def _clean_ring(coords, micro):
    out = [coords[0]]
    for q in coords[1:]:
        if np.hypot(*(q - out[-1])) >= micro:
            out.append(q)
    while len(out) > 3 and np.hypot(*(out[0] - out[-1])) < micro:
        out.pop()
    return np.array(out)


def _interior_angles(ring):
    m = len(ring)
    return np.array([(_ang(ring[i - 1] - ring[i]) - _ang(ring[(i + 1) % m] - ring[i])) % TWO_PI
                     for i in range(m)])


def build_template(ring, corner_ids, k):
    """Return list of patches for the template with k selected ring corners (or None)."""
    cl = np.vstack([ring, ring[:1]])
    s = _cum(cl)
    L = s[-1]
    D = np.vstack([cl, cl[1:]])
    sc = [s[i] for i in corner_ids]
    if k == 4:
        arcs = sc
    elif k == 3:
        pass
    elif k == 2:
        arcs = [sc[0], 0.5 * (sc[0] + sc[1]), sc[1], 0.5 * (sc[1] + sc[0] + L)]
    elif k == 1:
        arcs = [sc[0] + i * L / 4 for i in range(4)]
    else:
        arcs = [i * L / 4 for i in range(4)]

    if k == 3:
        bounds = [(sc[0], sc[1]), (sc[1], sc[2]), (sc[2], sc[0] + L)]
        h1, h2, pts = [], [], []
        for a, b in bounds:
            m = 0.5 * (a + b)
            h1.append(sub_pl(D, a, m))
            h2.append(sub_pl(D, m, b))
            pts.append(point_at(D, a))
        c = np.mean(pts, axis=0)
        poly = Polygon(ring)
        if not poly.contains(shapely.Point(c)):
            c = np.array(poly.representative_point().coords[0])
        mids = [h[0] for h in h2]
        patches = []
        for i in range(3):
            j = (i + 1) % 3
            patches.append([h2[i], h1[j],
                            np.array([mids[j], c]), np.array([c, mids[i]])])
        return patches

    sides = []
    for i in range(4):
        a = arcs[i]
        b = arcs[i + 1] if i < 3 else arcs[0] + L
        if b - a < 1e-12:
            return None
        sides.append(sub_pl(D, a, b))
    return [sides]


def _local_counts(patch, l_tar):
    nb = [len(_bends(sd)) + 1 for sd in patch]
    n0 = max(nb[0], nb[2], int(round(0.5 * (poly_len(patch[0]) + poly_len(patch[2])) / l_tar)))
    n1 = max(nb[1], nb[3], int(round(0.5 * (poly_len(patch[1]) + poly_len(patch[3])) / l_tar)))
    return n0, n1


def template_error(patches, l_tar):
    Vs, Qs, off = [], [], 0
    for pt in patches:
        nu, nv = _local_counts(pt, l_tar)
        V, Q = grid_to_mesh(coons(pt, nu, nv))
        Vs.append(V)
        Qs.append(Q + off)
        off += len(V)
    return alpha_error(np.vstack(Vs), np.vstack(Qs))


def build_layout(regions, p=QParams()):
    rings = []
    scale = 0.0
    for r in regions:
        r = orient(r)
        rg_ = np.asarray(r.exterior.coords)[:-1]
        scale = max(scale, np.hypot(*(np.array(r.bounds[2:]) - np.array(r.bounds[:2]))))
        rings.append(rg_)
    micro = 1e-4 * scale
    rings = [_clean_ring(r, micro) for r in rings]
    angs = [_interior_angles(r) for r in rings]

    # points that are real corners in any region -> straight vertices there are dividing-line ends
    corner_pts = [r[i] for r, a in zip(rings, angs) for i in range(len(r))
                  if abs(a[i] - np.pi) > p.straight_tol]
    corner_pts = np.asarray(corner_pts, float).reshape(-1, 2)

    layout = []
    for ring, ang in zip(rings, angs):
        corner_distances = _nearest_distances(ring, corner_pts) if len(corner_pts) else None
        cand = []
        for i in range(len(ring)):
            if abs(ang[i] - np.pi) > p.straight_tol:
                cand.append(i)
            elif corner_distances is not None and corner_distances[i] < 10 * micro:
                cand.append(i)
        cand = sorted(sorted(cand, key=lambda i: abs(ang[i] - np.pi / 2))[:p.max_corner_cands])
        ns = [i for i in range(len(ring)) if abs(ang[i] - np.pi) > p.straight_tol]
        best = None
        if len(ns) == 3:                       # triangular region -> 3-quad template (paper-style)
            pats = build_template(ring, ns, 3)
            if pats is not None:
                layout += pats
                continue
        for k in (4, 3, 2, 1, 0):
            if len(cand) < k:
                continue
            combos = list(itertools.combinations(cand, k)) if k else [()]
            combos.sort(key=lambda c: sum(abs(ang[i] - np.pi / 2) for i in c))
            kbest = None
            for cmb in combos[:p.max_combos]:
                pats = build_template(ring, cmb, k)
                if pats is None:
                    continue
                err = template_error(pats, p.l_tar)
                if kbest is None or err < kbest[0]:
                    kbest = (err, pats)
            if kbest is not None and (best is None or kbest[0] < best[0]):
                best = kbest
            if best is not None and best[0] < p.alpha_th:
                break
        layout += best[1]
    return layout


# --------------------------------------------------------------------------- #
# 4. T-junction elimination
# --------------------------------------------------------------------------- #
def find_tjunction(patches, tol):
    for pi, P in enumerate(patches):
        for side in P:
            c = side[0]
            for qi, Q in enumerate(patches):
                if qi == pi:
                    continue
                for si, qs in enumerate(Q):
                    d, s, _ = project(qs, c)
                    if d < tol and tol < s < poly_len(qs) - tol:
                        return qi, si, c.copy(), s
    return None


def _split_patch(Q, si, cT, sT, others, tol, l_mer):
    sides = [Q[(si + i) % 4] for i in range(4)]
    s0, s1, s2, s3 = sides
    L0, L2 = poly_len(s0), poly_len(s2)
    s_o = (1.0 - sT / L0) * L2                     # opposite side runs in the other direction
    margin = min(0.5 * l_mer, 0.25 * L2)
    s_o = float(np.clip(s_o, margin, L2 - margin))
    snap_pt, best = None, l_mer                    # merge with a T-junction already on that side
    for P in others:
        for sd in P:
            c = sd[0]
            d, s, _ = project(s2, c)
            if d < tol and tol < s < L2 - tol and abs(s - s_o) < best:
                best, s_o, snap_pt = abs(s - s_o), s, c.copy()
    a1, a2 = split_pl(s0, sT, cT)
    b1, b2 = split_pl(s2, s_o, snap_pt)
    Tp = b1[-1]
    cut = np.array([cT, Tp])
    Q1 = [a1, cut, b2, s3]
    Q2 = [a2, s1, b1, cut[::-1].copy()]
    return Q1, Q2


def remove_tjunctions(patches, p=QParams(), scale=1.0):
    tol = 1e-5 * scale
    l_mer = p.l_mer_factor * p.l_tar
    patches = [list(pt) for pt in patches]
    for _ in range(p.max_t_iter):
        t = find_tjunction(patches, tol)
        if t is None:
            return patches, 0
        qi, si, cT, sT = t
        Q = patches.pop(qi)
        patches += list(_split_patch(Q, si, cT, sT, patches, tol, l_mer))
    n = 0
    while find_tjunction(patches, tol) is not None and n < 1000:
        n += 1
        break
    return patches, 1 if find_tjunction(patches, tol) is not None else 0


# --------------------------------------------------------------------------- #
# 5. subdivision counts
# --------------------------------------------------------------------------- #
def choose_counts(patches, p=QParams(), scale=1.0):
    """
    Opposite sides of a patch share one count (hard), boundary lengths approach l_tar (objective).
    Constraints are pure equalities, so edges group into independent classes; for each class the
    integer n minimising  sum_e (L_e / n / l_tar - 1)^2  is found by scanning n.
    """
    tol = 1e-5 * scale
    pool = []

    def pid(pt):
        if pool:
            d = np.hypot(*(np.asarray(pool) - pt).T)
            k = int(d.argmin())
            if d[k] < tol:
                return k
        pool.append(np.asarray(pt, float))
        return len(pool) - 1

    edges, elen, nbend, pedges = {}, [], [], []
    for P in patches:
        ids = []
        for sd in P:
            a, b = pid(sd[0]), pid(sd[-1])
            m = pid(point_at(sd, 0.5 * poly_len(sd)))
            key = (min(a, b), max(a, b), m)
            if key not in edges:
                edges[key] = len(edges)
                elen.append(poly_len(sd))
                nbend.append(len(_bends(sd)) + 1)
            ids.append(edges[key])
        pedges.append(ids)

    parent = list(range(len(edges)))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for ids in pedges:
        for a, b in ((0, 2), (1, 3)):
            parent[find(ids[a])] = find(ids[b])

    classes, nmin = defaultdict(list), defaultdict(lambda: 1)
    for e in range(len(edges)):
        classes[find(e)].append(elen[e])
        nmin[find(e)] = max(nmin[find(e)], nbend[e])
    n_of = {}
    for root, Ls in classes.items():
        Ls = np.array(Ls)
        best = None
        for n in range(nmin[root], max(nmin[root], int(Ls.max() / p.l_tar)) + 4):
            cost = float(np.sum((Ls / n / p.l_tar - 1.0) ** 2))
            if best is None or cost < best[0]:
                best = (cost, n)
        n_of[root] = best[1]
    return [(n_of[find(ids[0])], n_of[find(ids[1])]) for ids in pedges]


def refine_bad_patches(patches, p, scale, rounds=4):
    """Patches whose transfinite grid has flipped quads (strongly non-convex) are cut in half."""
    tol = 1e-5 * scale
    for _ in range(rounds):
        counts = choose_counts(patches, p, scale)
        new, changed = [], False
        for P, c in zip(patches, counts):
            V, Q = grid_to_mesh(coons(P, *c))
            lens = [poly_len(sd) for sd in P]
            if min_det(V, Q) > 1e-9 * p.l_tar ** 2 or min(lens) < p.l_tar:
                new.append(P)
                continue
            si = 1 if lens[1] + lens[3] > lens[0] + lens[2] else 0
            Pr = [P[(si + i) % 4] for i in range(4)]
            L0 = poly_len(Pr[0])
            new += list(_split_patch(Pr, 0, point_at(Pr[0], 0.5 * L0), 0.5 * L0, [], tol, p.l_mer_factor * p.l_tar))
            changed = True
        if not changed:
            break
        patches, _ = remove_tjunctions(new, p, scale)
    return patches


# --------------------------------------------------------------------------- #
# conforming initial mesh
# --------------------------------------------------------------------------- #
def mesh_layout(patches, counts, p=QParams(), scale=1.0):
    Vs, Qs, off = [], [], 0
    for P, (nu, nv) in zip(patches, counts):
        V, Q = grid_to_mesh(coons(P, nu, nv))
        Vs.append(V)
        Qs.append(Q + off)
        off += len(V)
    V, Q = np.vstack(Vs), np.vstack(Qs)
    tol = min(1e-5 * scale, 0.01 * p.l_tar)
    parent = list(range(len(V)))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b in _pairs_within(V, tol):
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[max(ra, rb)] = min(ra, rb)
    root = np.array([find(i) for i in range(len(V))])
    used = np.unique(root)
    remap = -np.ones(len(V), int)
    remap[used] = np.arange(len(used))
    V = V[used]
    Q = remap[root][Q]
    keep, seen = [], set()
    for q in Q:
        key = frozenset(q.tolist())
        if len(key) == 4 and key not in seen:
            seen.add(key)
            keep.append(q)
    return V, np.array(keep, int)


# --------------------------------------------------------------------------- #
# 6a. boundary / validity helpers
# --------------------------------------------------------------------------- #
def domain_corners(domain, ang_tol=1e-4):
    pts = []
    for ring in [domain.exterior] + list(domain.interiors):
        c = np.asarray(ring.coords)[:-1]
        m = len(c)
        for i in range(m):
            a, b = c[i] - c[i - 1], c[(i + 1) % m] - c[i]
            if abs(np.arctan2(a[0] * b[1] - a[1] * b[0], a @ b)) > ang_tol:
                pts.append(c[i])
    return np.array(pts)


def hausdorff(V, Q, domain):
    em, _ = boundary_info(V, Q)
    segs = [LineString([V[a], V[b]]) for (a, b), qs in em.items() if len(qs) == 1]
    return float(shapely.hausdorff_distance(domain.boundary, MultiLineString(segs), densify=0.02))


# --------------------------------------------------------------------------- #
# 6b. poly-chord collapse / split
# --------------------------------------------------------------------------- #
def _trace_chord(Q, em, e0):
    rungs, quads, cur = [e0], [], e0
    q = em[e0][0]
    while True:
        if q in quads:
            return None
        quads.append(q)
        vs = Q[q]
        k = next(k for k in range(4)
                 if {int(vs[k]), int(vs[(k + 1) % 4])} == set(cur))
        a, b = int(vs[(k + 2) % 4]), int(vs[(k + 3) % 4])
        opp = (min(a, b), max(a, b))
        rungs.append(opp)
        nxt = [x for x in em[opp] if x != q]
        if not nxt:
            return rungs, quads
        q, cur = nxt[0], opp


def open_chords(V, Q):
    em = edge_map(Q)
    seen, out = set(), []
    for e, qs in em.items():
        if len(qs) != 1:
            continue
        r = _trace_chord(Q, em, e)
        if r is None:
            continue
        rungs, quads = r
        key = frozenset(quads)
        if key in seen:
            continue
        seen.add(key)
        width = float(np.mean([np.hypot(*(V[a] - V[b])) for a, b in rungs]))
        out.append((width, rungs, quads))
    return out


def _compress(V, Q):
    used = np.unique(Q)
    remap = -np.ones(len(V), int)
    remap[used] = np.arange(len(used))
    return V[used], remap[Q]


def valid_topology(Q):
    seen = set()
    for q in Q:
        if len(set(q.tolist())) < 4:
            return False
        for k in range(4):
            e = (int(q[k]), int(q[(k + 1) % 4]))
            if e in seen:
                return False
            seen.add(e)
    return True


def collapse_chord(V, Q, rungs, bnd, feat):
    parent = list(range(len(V)))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    touched = set()
    for a, b in rungs:
        if feat[a] and feat[b]:
            return None
        parent[find(a)] = find(b)
        touched |= {a, b}
    groups = defaultdict(list)
    for v in touched:
        groups[find(v)].append(v)
    Vn = V.copy()
    for r, g in groups.items():
        f = [v for v in g if feat[v]]
        if len(f) > 1:
            return None
        if f:
            pos = V[f[0]]
        else:
            b = [v for v in g if bnd[v]]
            pos = V[b].mean(0) if b else V[g].mean(0)
        Vn[r] = pos
    Qn = []
    for q in Q:
        qq = [find(int(v)) for v in q]
        if len(set(qq)) == 4:
            Qn.append(qq)
    if not Qn:
        return None
    return _compress(Vn, np.array(Qn, int))


def split_chord(V, Q, rungs, quads):
    mid, Vn = {}, [v for v in V]
    for a, b in rungs:
        mid[(a, b)] = len(Vn)
        Vn.append(0.5 * (V[a] + V[b]))
    rset = set(rungs)
    Qn = [q.tolist() for qi, q in enumerate(Q) if qi not in set(quads)]
    for qi in quads:
        vs = [int(v) for v in Q[qi]]
        for k in range(4):
            e_in = (min(vs[k], vs[(k + 1) % 4]), max(vs[k], vs[(k + 1) % 4]))
            e_out = (min(vs[(k + 2) % 4], vs[(k + 3) % 4]), max(vs[(k + 2) % 4], vs[(k + 3) % 4]))
            if e_in in rset and e_out in rset:
                break
        m_in, m_out = mid[e_in], mid[e_out]
        v0, v1, v2, v3 = vs[k], vs[(k + 1) % 4], vs[(k + 2) % 4], vs[(k + 3) % 4]
        Qn.append([v0, m_in, m_out, v3])
        Qn.append([m_in, v1, v2, m_out])
    return np.array(Vn), np.array(Qn, int)


def simplify(V, Q, domain, p=QParams(), verbose=True):
    h = p.l_tar
    scale = float(np.hypot(*(np.array(domain.bounds[2:]) - np.array(domain.bounds[:2]))))
    htol = p.hausdorff_tol if p.hausdorff_tol is not None else 1e-6 * scale   # boundary stays exact
    corners = domain_corners(domain)

    def try_accept(Vn, Qn):
        if not valid_topology(Qn) or min_det(Vn, Qn) <= 1e-9 * h * h:
            return None
        if hausdorff(Vn, Qn, domain) > htol:
            return None
        return Vn, Qn

    if verbose and min_det(V, Q) <= 0:
        print("warning: initial mesh contains flipped quads")
    for it in range(p.max_outer_iter):
        ops = 0
        for mode in ("collapse", "split"):
            while ops < p.max_ops_per_iter:
                em, bnd = boundary_info(V, Q)
                feat = feature_vertices(V, Q, em, p.straight_tol)
                feat |= bnd & (_nearest_distances(V, corners) < 1e-6 * scale)     # every original boundary corner is fixed
                chords = open_chords(V, Q)
                if mode == "collapse":
                    todo = sorted([c for c in chords if c[0] < p.collapse_ratio * h], key=lambda c: c[0])
                else:
                    todo = sorted([c for c in chords if c[0] > p.split_ratio * h], key=lambda c: -c[0])
                done = False
                for w, rungs, quads in todo:
                    if mode == "collapse":
                        res = collapse_chord(V, Q, rungs, bnd, feat)
                    else:
                        res = split_chord(V, Q, rungs, quads)
                    if res is None:
                        continue
                    res = try_accept(*res)
                    if res is None:
                        continue
                    V, Q = res
                    ops += 1
                    done = True
                    break
                if not done:
                    break
        if verbose:
            print(f"simplify iter {it}: {ops} topological ops, {len(Q)} quads")
        if ops == 0:
            break
    return V, Q


# --------------------------------------------------------------------------- #
# driver
# --------------------------------------------------------------------------- #
def quad_mesh_pipeline(domain, regions, p=QParams(), verbose=True):
    """domain: shapely Polygon; regions: output of segment_domain(domain)."""
    scale = float(np.hypot(*(np.array(domain.bounds[2:]) - np.array(domain.bounds[:2]))))
    patches = build_layout(regions, p)
    if verbose:
        print(f"layout: {len(regions)} regions -> {len(patches)} patches")
    patches, bad = remove_tjunctions(patches, p, scale)
    if verbose:
        print(f"T-junction removal: {len(patches)} patches" + ("  (UNRESOLVED T-junctions left)" if bad else ""))
    patches = refine_bad_patches(patches, p, scale)
    counts = choose_counts(patches, p, scale)
    V, Q = mesh_layout(patches, counts, p, scale)
    if verbose:
        print(f"initial mesh: {len(V)} vertices, {len(Q)} quads")
    V, Q = simplify(V, Q, domain, p, verbose)
    if verbose:
        bad = np.zeros(len(Q), bool)
        for _, _, _, D in _corner_jac(V, Q):
            bad |= D <= 0
        if bad.any():
            print(f"warning: {int(bad.sum())} flipped quads remain - the segmentation produced a region that is "
                  f"not rectangle-like (try a smaller beta_f / more cuts, or a smaller l_tar)")
    return patches, V, Q


def to_rhino_mesh(V, Q):
    import Rhino.Geometry as rg
    m = rg.Mesh()
    for x, y in V:
        m.Vertices.Add(float(x), float(y), 0.0)
    for a, b, c, d in Q:
        m.Faces.AddFace(int(a), int(b), int(c), int(d))
    m.Normals.ComputeNormals()
    m.Compact()
    return m


def patches_to_curves(patches):
    import Rhino.Geometry as rg
    out = []
    for P in patches:
        pts = []
        for sd in P:
            pts += [rg.Point3d(float(x), float(y), 0) for x, y in sd[:-1]]
        pts.append(pts[0])
        out.append(rg.Polyline(pts).ToNurbsCurve())
    return out

# --------------------------------------------------------------------------- #
# Notes
#  * Chord widths: the condensed description says "split if width > 2/3 l_tar", but with typical
#    meshes (rung length ~ l_tar) that would split every chord. The implementation collapses below
#    collapse_ratio*l_tar (2/3) and splits above split_ratio*l_tar (4/3); both are QParams fields.
#  * Only open poly-chords (boundary to boundary) are collapsed/split; closed chords are skipped.
#  * Step 1 (boundary straightening) would wrap segment_domain: straighten -> segment -> map the
#    regions back with the inverse deformation. It is a separate optimisation and is not included.
# --------------------------------------------------------------------------- #
 
# --------------------------------------------------------------------------- #
# parameters
# --------------------------------------------------------------------------- #
@dataclass
class Params:
    ray_step: float = np.pi / 36          # One-ECV angular sampling
    theta_min: float = np.pi / 3
    theta_max: float = 2 * np.pi / 3
    beta_f: float = np.pi / 12            # max misalignment
    ecv_tol: float = np.deg2rad(15)       # reflex threshold: angle > pi + ecv_tol
    n_samples: int = 25                   # samples along a line for misalignment
    eps_rel: float = 1e-7                 # geometric tolerance / bbox diagonal
    relax_reflex: bool = True             # allow 90/180-type splits at reflex ECVs
    max_hole_samples: int = 16            # fallback when a hole has no ECV
    edge_rays: bool = True                # also try the extensions of the two edges at an ECV
    relax_if_stuck: bool = True           # if no candidate passes the filters, relax them in stages
    relax_top_k: int = 40                 # relaxed stages: evaluate the resulting pieces of the best k lines
 
 
# --------------------------------------------------------------------------- #
# mesh <-> polygon / cross field helpers
# --------------------------------------------------------------------------- #
def mesh_to_polygon(verts, faces):
    """Union of triangles/quads of a 2D mesh -> shapely (Multi)Polygon."""
    verts = np.asarray(verts, float)
    polys = [Polygon(verts[f]) for f in faces]
    u = unary_union(polys)
    return u
 
 
def cross_field_from_faces(verts, faces, face_angles):
    """Cross field given as one angle per face (mod pi/2); nearest-face lookup."""
    verts = np.asarray(verts, float)
    centers = np.array([verts[f].mean(axis=0) for f in faces])
    face_angles = np.asarray(face_angles, float)
 
    def field(pts):
        pts = np.atleast_2d(pts)
        idx = np.argmin(((pts[:, None, :] - centers[None, :, :]) ** 2).sum(axis=2), axis=1)
        return face_angles[idx]
    return field
 
 
def cross_field_from_boundary(poly):
    """
    Default cross field when none is supplied: at each point, use the direction
    of the nearest boundary edge (the cross field is defined mod pi/2 anyway).
    """
    rings = _rings(poly)
    segs, angs = [], []
    for ring in rings:
        m = len(ring)
        for i in range(m):
            a, b = ring[i], ring[(i + 1) % m]
            if np.hypot(*(b - a)) > 0:
                segs.append(LineString([a, b]))
                angs.append(_ang(b - a))
    tree = shapely.STRtree(segs)
    angs = np.array(angs)
 
    def field(pts):
        pts = np.atleast_2d(pts)
        idx = tree.nearest(shapely.points(pts))
        return angs[idx]
    return field
 
 
 
def label_faces(regions, verts, faces):
    """Assign every mesh face to a region index (-1 if none) by centroid."""
    verts = np.asarray(verts, float)
    c = np.array([verts[f].mean(axis=0) for f in faces])
    lab = -np.ones(len(faces), int)
    for i, r in enumerate(regions):
        lab[shapely.contains_xy(r, c[:, 0], c[:, 1])] = i
    return lab
 
 
# --------------------------------------------------------------------------- #
# geometry helpers
# --------------------------------------------------------------------------- #
def _clean(ring, micro):
    """Drop vertices closer than `micro` to their predecessor (hairline-cut artefacts)."""
    if micro <= 0:
        return ring
    out = [ring[0]]
    for q in ring[1:]:
        if np.hypot(*(q - out[-1])) >= micro:
            out.append(q)
    while len(out) > 3 and np.hypot(*(out[0] - out[-1])) < micro:
        out.pop()
    return np.array(out)
 
 
def _rings(poly, micro=0.0):
    """Exterior CCW, holes CW  ->  interior is always on the left."""
    poly = orient(poly)
    rs = [np.asarray(poly.exterior.coords)[:-1]] + \
         [np.asarray(r.coords)[:-1] for r in poly.interiors]
    return [_clean(r, micro) for r in rs]
 
 
def find_ecvs(rings, p):
    """Reflex vertices. Returns list of (ring_index, point, prev, next)."""
    out = []
    for ri, ring in enumerate(rings):
        m = len(ring)
        for i in range(m):
            prev, P, nxt = ring[i - 1], ring[i], ring[(i + 1) % m]
            total = (_ang(prev - P) - _ang(nxt - P)) % TWO_PI
            if np.pi + p.ecv_tol < total < TWO_PI - 1e-9:
                out.append((ri, P, prev, nxt))
    return out
 
 
def _endpoint_angles(rings, P, u, tol):
    """
    Angles on both sides of a line leaving boundary point P in direction u.
    Returns (sub1, sub2, total, is_vertex) or None.
    """
    # pass 1: vertices
    for ring in rings:
        m = len(ring)
        for i in range(m):
            if np.hypot(*(ring[i] - P)) < tol:
                prev, nxt = ring[i - 1], ring[(i + 1) % m]
                return _split(prev, nxt, P, u, True)
    # pass 2: edge interiors
    for ring in rings:
        m = len(ring)
        for i in range(m):
            a, b = ring[i], ring[(i + 1) % m]
            ab = b - a
            t = np.dot(P - a, ab) / np.dot(ab, ab)
            if 0 < t < 1 and np.hypot(*(a + t * ab - P)) < tol:
                return _split(a, b, P, u, False)
    return None
 
 
def _split(prev, nxt, P, u, is_vertex):
    total = (_ang(prev - P) - _ang(nxt - P)) % TWO_PI
    if total < 1e-9:
        total = TWO_PI
    s1 = (_ang(u) - _ang(nxt - P)) % TWO_PI
    return s1, total - s1, total, is_vertex
 
 
def misalignment(field, A, B, n):
    """(max, mean) angular distance between line direction and the cross field."""
    t = np.linspace(0.05, 0.95, n)[:, None]
    pts = A + t * (B - A)
    d = _ang(B - A)
    th = np.asarray(field(pts), float)
    diff = np.abs((d - th + np.pi / 4) % (np.pi / 2) - np.pi / 4)
    return diff.max(), diff.mean()
 
 
def _angles_ok(rings, A, B, p, tol):
    u = B - A
    for P, dirv in ((A, u), (B, -u)):
        r = _endpoint_angles(rings, P, dirv, tol)
        if r is None:
            return False
        s1, s2, total, is_vertex = r
        if s1 <= 1e-9 or s2 <= 1e-9:
            return False                      # line leaves the domain
        reflex = is_vertex and total > np.pi + p.ecv_tol
        hi = np.pi + p.ecv_tol if (reflex and p.relax_reflex) else p.theta_max
        for s in (s1, s2):
            if not (p.theta_min <= s <= hi):
                return False
    return True
 
 
def _valid_chord(poly, boundary, A, B, eps):
    L = np.hypot(*(B - A))
    if L < 40 * eps:
        return False
    inner = substring(LineString([A, B]), 10 * eps, L - 10 * eps)
    return poly.contains(inner) and not inner.intersects(boundary)
 
 
def _cut(poly, A, B, eps, min_area):
    """Hairline slit A-B (used for hole elimination). Returns resulting polygons."""
    blade = LineString([A, B]).buffer(eps, cap_style=2)
    res = poly.difference(blade)
    geoms = getattr(res, "geoms", [res])
    return [g for g in geoms if g.geom_type == "Polygon" and g.area > min_area]
 
 
def _prepare(poly, P, tol):
    """Return (poly2, P2) where P2 is an exact vertex of poly2 (inserted if needed)."""
    poly = orient(poly)
    rings = [[tuple(c) for c in np.asarray(poly.exterior.coords)[:-1]]] + \
            [[tuple(c) for c in np.asarray(r.coords)[:-1]] for r in poly.interiors]
    for r in rings:
        for q in r:
            if np.hypot(q[0] - P[0], q[1] - P[1]) < tol:
                return poly, np.array(q)
    best = None
    for ri, r in enumerate(rings):
        n = len(r)
        for i in range(n):
            a, b = np.array(r[i]), np.array(r[(i + 1) % n])
            ab = b - a
            t = np.dot(P - a, ab) / np.dot(ab, ab)
            if 0 <= t <= 1:
                d = np.hypot(*(a + t * ab - P))
                if best is None or d < best[0]:
                    best = (d, ri, i)
    _, ri, i = best
    rings[ri].insert(i + 1, (float(P[0]), float(P[1])))
    return Polygon(rings[0], rings[1:]), np.array(P, float)
 
 
def _split_exact(poly, A, B, eps, min_area):
    """Exact split by chord A-B (shared vertices, no gap)."""
    poly2, A2 = _prepare(poly, A, 10 * eps)
    poly2, B2 = _prepare(poly2, B, 10 * eps)
    noded = unary_union([poly2.boundary, LineString([A2, B2])])
    return [f for f in polygonize(noded)
            if f.area > min_area and poly2.contains(f.representative_point())]
 
 
# --------------------------------------------------------------------------- #
# candidate generation
# --------------------------------------------------------------------------- #
def _two_ecv_candidates(poly, rings, ecvs, p, eps):
    boundary = poly.boundary
    pts = [e[1] for e in ecvs]
    for i in range(len(pts)):
        for j in range(i + 1, len(pts)):
            if _valid_chord(poly, boundary, pts[i], pts[j], eps):
                yield pts[i], pts[j]
 
 
def _one_ecv_candidates(poly, ecvs, p, eps):
    """ecvs: list of (P, prev, next)."""
    boundary = poly.boundary
    minx, miny, maxx, maxy = poly.bounds
    R = 2.0 * np.hypot(maxx - minx, maxy - miny)
    n_rays = int(round(TWO_PI / p.ray_step))
    for E, prev, nxt in ecvs:
        angles = [k * p.ray_step for k in range(n_rays)]
        if p.edge_rays:                           # extensions of the two incident edges
            angles += [_ang(E - prev), _ang(E - nxt)]
        for a in angles:
            d = np.array([np.cos(a), np.sin(a)])
            ray = LineString([E, E + R * d])
            inter = ray.intersection(boundary)
            pts = [np.array(g.coords[0]) for g in getattr(inter, "geoms", [inter])
                   if not g.is_empty] if not inter.is_empty else []
            pts = [q for q in pts if np.dot(q - E, d) > 10 * eps]
            if not pts:
                continue
            B = min(pts, key=lambda q: np.dot(q - E, d))
            if _valid_chord(poly, boundary, E, B, eps):
                yield E, B
 
 
def _angle_dev(rings, A, B, tol):
    """(mean deviation of the 4 endpoint angles from multiples of pi/2, smallest angle) or None."""
    devs, subs = [], []
    for P, u in ((A, B - A), (B, A - B)):
        r = _endpoint_angles(rings, P, u, tol)
        if r is None or r[0] <= 1e-9 or r[1] <= 1e-9:
            return None
        for sa in r[:2]:
            subs.append(sa)
            devs.append(abs((sa + np.pi / 4) % (np.pi / 2) - np.pi / 4))
    return float(np.mean(devs)), min(subs)
 
 
def _rank(rings, cands, field, p, eps, stage=0):
    """
    Filter + score candidates; returns sorted list of (score, A, B).
      stage 0: the paper's filters (angle range AND misalignment), score = mean misalignment
      stage 1: angle range only (misalignment ignored), score = misalignment + angle deviation
      stage 2: only 'line stays inside and every angle >= 20 deg', same score
    """
    out = []
    for A, B in cands:
        mmax, mmean = misalignment(field, A, B, p.n_samples)
        if stage == 0:
            if mmax >= p.beta_f or not _angles_ok(rings, A, B, p, 10 * eps):
                continue
            score = mmean
        else:
            r = _angle_dev(rings, A, B, 10 * eps)
            if r is None:
                continue
            dev, minsub = r
            if stage == 1 and not _angles_ok(rings, A, B, p, 10 * eps):
                continue
            if stage == 2 and minsub < np.pi / 9:
                continue
            score = mmean + dev
        out.append((score, A, B))
    out.sort(key=lambda t: t[0])
    return out
 
 
# --------------------------------------------------------------------------- #
# preprocess: make simply connected
# --------------------------------------------------------------------------- #
def eliminate_holes(poly, field=None, p=Params()):
    poly = orient(poly)
    if field is None:
        field = cross_field_from_boundary(poly)
    scale = np.hypot(*(np.array(poly.bounds[2:]) - np.array(poly.bounds[:2])))
    eps = p.eps_rel * scale
    min_area = eps * eps          # numerical degenerate-face guard only
    n_cuts = 0
    while len(poly.interiors) > 0:
        rings = _rings(poly, 1e3 * eps)
        ecvs = [(P, pv, nx) for ri, P, pv, nx in find_ecvs(rings, p) if ri > 0]
        if not ecvs:                               # smooth hole(s): subsample
            for ring in rings[1:]:
                m = len(ring)
                idx = np.linspace(0, m, p.max_hole_samples, endpoint=False).astype(int)
                ecvs += [(ring[i], ring[i - 1], ring[(i + 1) % m]) for i in idx]
        cands = list(_one_ecv_candidates(poly, ecvs, p, eps))
        done = False
        for stage in ((0, 1, 2) if p.relax_if_stuck else (0,)):
            ranked = _rank(rings, cands, field, p, eps, stage)
            for _, A, B in ranked:
                parts = _cut(poly, A, B, eps, min_area)
                if len(parts) == 1 and len(parts[0].interiors) < len(poly.interiors):
                    poly, done = orient(parts[0]), True
                    break
            if done:
                break
        if not done:
            raise RuntimeError("could not eliminate remaining holes")
        n_cuts += 1
    return poly
 
 
# --------------------------------------------------------------------------- #
# segmentation
# --------------------------------------------------------------------------- #
def _piece_cost(piece, micro):
    """Area-independent corner quality of a region: mean deviation of its corners from 90 deg
    (reflex corners count as deviation from 90 too, so they are penalised strongly)."""
    ring = _rings(piece, micro)[0]
    m = len(ring)
    devs, min_corner = [], np.pi
    for i in range(m):
        prev, P, nxt = ring[i - 1], ring[i], ring[(i + 1) % m]
        a = (_ang(prev - P) - _ang(nxt - P)) % TWO_PI
        if abs(a - np.pi) < np.deg2rad(10):
            continue                              # (nearly) straight: not a corner
        devs.append(abs(a - np.pi / 2))
        min_corner = min(min_corner, a)
    # rectangle-like = 4 corners: triangles / pentagons are penalised (0.3 rad per corner off);
    # sharp corners (slivers) are penalised through their angle, never through area
    sharp = max(0.0, np.pi / 4 - min_corner) if devs else 0.0
    return (float(np.mean(devs)) if devs else 0.0) + 0.3 * abs(len(devs) - 4) + 0.5 * sharp
 
 
def _try_split(poly, field, p, method, eps, min_area):
    rings = _rings(poly, 1e3 * eps)
    ecvs = find_ecvs(rings, p)
    if not ecvs:
        return None
    if method == "two":
        if len(ecvs) < 2:
            return None
        cands = list(_two_ecv_candidates(poly, rings, ecvs, p, eps))
    else:
        cands = list(_one_ecv_candidates(poly, [e[1:] for e in ecvs], p, eps))
    stages = (0, 1, 2) if (method == "one" and p.relax_if_stuck) else (0,)
    for stage in stages:
        ranked = _rank(rings, cands, field, p, eps, stage)
        if stage == 0:                            # paper: greedy best
            for _, A, B in ranked:
                parts = _split_exact(poly, A, B, eps, min_area)
                if len(parts) >= 2:
                    return [orient(g) for g in parts]
        else:                                     # relaxed: judge the resulting pieces
            best = None
            for score, A, B in ranked[:p.relax_top_k]:
                parts = _split_exact(poly, A, B, eps, min_area)
                if len(parts) < 2:
                    continue
                # every piece counts equally, so a small ugly piece is not hidden by a big one
                cost = float(np.mean([_piece_cost(g, 1e3 * eps) for g in parts])) + score
                if best is None or cost < best[0]:
                    best = (cost, parts)
            if best is not None:
                return [orient(g) for g in best[1]]
    return None
 
 
def _recurse(polys, field, p, method, eps, min_area):
    stack, final = list(polys), []
    while stack:
        r = stack.pop()
        parts = _try_split(r, field, p, method, eps, min_area)
        if parts is None:
            final.append(r)
        else:
            stack.extend(parts)
    return final
 
 
def segment_domain(domain, field=None, p=Params()):
    """
    domain : shapely Polygon / MultiPolygon (use mesh_to_polygon on your mesh)
    field  : optional callable (M,2)->(M,) cross-field angle. If None, the field is
             derived from the nearest boundary edge direction (mod pi/2).
    returns: list of shapely Polygons (rectangle-like regions)
    """
    comps = list(domain.geoms) if isinstance(domain, MultiPolygon) else [domain]
    result = []
    for dom in comps:
        scale = np.hypot(*(np.array(dom.bounds[2:]) - np.array(dom.bounds[:2])))
        eps = p.eps_rel * scale
        min_area = eps * eps
        fld = field if field is not None else cross_field_from_boundary(dom)
        simple = eliminate_holes(dom, fld, p)                      # preprocess
        regs = _recurse([simple], fld, p, "two", eps, min_area)    # Two-ECV
        regs = _recurse(regs, fld, p, "one", eps, min_area)        # One-ECV
        result += regs
    return result
# --------------------------------------------------------------------------- #
# Notes
#  * Chord widths: the condensed description says "split if width > 2/3 l_tar", but with typical
#    meshes (rung length ~ l_tar) that would split every chord. The implementation collapses below
#    collapse_ratio*l_tar (2/3) and splits above split_ratio*l_tar (4/3); both are QParams fields.
#  * Only open poly-chords (boundary to boundary) are collapsed/split; closed chords are skipped.
#  * Step 1 (boundary straightening) would wrap segment_domain: straighten -> segment -> map the
#    regions back with the inverse deformation. It is a separate optimisation and is not included.
# --------------------------------------------------------------------------- #



def quad_mesh_from_coords(coords, l_tar=30.0, verbose=False):
    """Build a quad mesh from an exterior coordinate ring; return patches, V, Q."""
    domain = Polygon(coords)
    if not domain.is_valid or domain.is_empty:
        raise ValueError("coords must define a non-empty, valid polygon")
    regions = segment_domain(domain)
    return quad_mesh_pipeline(domain, regions, QParams(l_tar=float(l_tar)), verbose)


# Grasshopper supplies `polygon` as an implicit input. Keeping the adapter guarded
# makes this file importable as a normal Python module for batch processing.
if "polygon" in globals():
    coords = [[pt.X, pt.Y] for pt in polygon]
    patches, V, Q = quad_mesh_from_coords(coords, globals().get("l_tar", 30.0))
    mesh = to_rhino_mesh(V, Q)
    layout = patches_to_curves(patches)

# data = {"coords":coords,"V":V.tolist(),"Q":Q.tolist()}

# import json
# import uuid

# with open(f"C:/Users/jairl/Documents/ChatGPT/quads/data/{uuid.uuid4()}.json", "w") as f:
#     json.dump(data, f, indent=4)
