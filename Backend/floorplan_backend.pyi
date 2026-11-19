"""Type stubs for the ``floorplan_backend`` pyo3 extension module."""

from typing import Any, Mapping, Optional, Sequence, TypedDict

# Input points/rings accept any 2-sequence of floats, e.g. ``(x, y)`` or ``[x, y]``.
Point2 = Sequence[float]
# ``[x, y, cos(4*theta), sin(4*theta)]``
FieldSample = Sequence[float]

def extract_boundaries(
    edge_ptr: int,
    b: int,
    n: int,
    h: int,
    w: int,
    smoothing: float = 2.0,
) -> tuple[list[list[list[int]]], list[list[int]]]:
    """Extract boundaries from a raw ``u8`` edge buffer.

    ``edge_ptr`` is the address of a contiguous buffer (must be non-null);
    ``b > 0``, ``0 < n <= 256``, ``smoothing`` finite and ``>= 0``.
    Raises ``ValueError`` on invalid arguments. Returns ``([], [])`` if ``h`` or ``w`` is 0.
    """
    ...

def segment_domain(
    outer: Sequence[Point2],
    field_samples: Optional[Sequence[FieldSample]] = None,
) -> list[list[list[list[float]]]]:
    """Segment a domain into regions.

    Each region is a list of rings of ``[x, y]`` points (outer ring first, then holes).
    """
    ...

def mesh_domain(
    outer: Sequence[Point2],
    holes: Optional[Sequence[Sequence[Point2]]] = None,
    l_tar: float = 30.0,
    field_samples: Optional[Sequence[FieldSample]] = None,
) -> tuple[list[list[float]], list[list[int]]]:
    """Build a quad mesh. Returns ``(vertices, faces)`` where faces are 4 vertex indices.

    ``holes`` must currently be ``None`` or empty (``ValueError`` otherwise).
    """
    ...

class Id:
    index: int
    generation: int
    def __init__(self, index: int, generation: int) -> None: ...

class JointRef(TypedDict):
    face_Id: str
    layer_idx: int

class LayerGeometry(TypedDict):
    name: str
    uuid: str
    area: float
    joints: list[Optional[JointRef]]
    inner: list[tuple[float, float, float]]
    outer: list[tuple[float, float, float]]

class Building:
    @staticmethod
    def from_json(topology_json: str, construction_json: str) -> Building: ...
    @staticmethod
    def from_data(topology: Mapping[str, Any], construction: Mapping[str, Any]) -> Building: ...
    def face_ids(self) -> list[Id]: ...
    def face_neighbors(self, face_id: Id) -> list[Id]: ...
    def layer_geometry(self, face_id: Id) -> dict[str, list[LayerGeometry]]:
        """Return ``{face_uuid: [layer, ...]}`` for the given face."""
        ...
    def solve_joints(self) -> None: ...
