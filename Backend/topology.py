import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# VERTS
@dataclass
class TopologyVert:
    uuid: str
    coords: tuple[float, float, float]
    dictionary: dict[str, Any]
    edges: list["TopologyEdge"] = field(default_factory=list)

# EDGES
@dataclass
class TopologyEdge:
    uuid: str
    dictionary: dict[str, Any]
    verts: list[TopologyVert] = field(default_factory=list)
    faces: list["TopologyFace"] = field(default_factory=list)

# FACES
@dataclass
class TopologyFace:
    uuid: str
    verts: list[TopologyVert]
    edges: list[TopologyEdge]
    construction: str
    type: str
    uvn: list[list[float]]
    layer_edge_offsets: list[list[list[float]]]
    dictionary: dict[str, Any]

    cells: list["TopologyCell"] = field(default_factory=list)
    assembly: Any = None

# CELLS
@dataclass
class TopologyCell:
    uuid: str
    dictionary: dict[str, Any]
    faces: list[TopologyFace] = field(default_factory=list)

# TOPOLOGY
class Topology:
    def __init__(self, data: dict[str, Any]):
        self.verts: dict[str, TopologyVert] = {}
        self.edges: dict[str, TopologyEdge] = {}
        self.faces: dict[str, TopologyFace] = {}
        self.cells: dict[str, TopologyCell] = {}

        # Verts
        for uuid, vert in data["verts"].items():
            self.verts[uuid] = TopologyVert(uuid=uuid,coords=tuple(vert["coords"]),dictionary=vert["dictionary"],)

        # Edges
        for uuid, edge in data["edges"].items():
            topology_edge = TopologyEdge(uuid=uuid,dictionary=edge["dictionary"],)
            topology_edge.verts = [self.verts[vert_uuid] for vert_uuid in edge["verts"]]
            self.edges[uuid] = topology_edge

        # Faces
        for uuid, face in data["faces"].items():
            self.faces[uuid] = TopologyFace(
                uuid=uuid,
                verts=[self.verts[vert_uuid] for vert_uuid in face["verts"]],
                edges=[self.edges[edge_uuid] for edge_uuid in face["edges"]],
                construction=face["construction"],
                type=face["type"],
                uvn=face["uvn"],
                layer_edge_offsets=face["layer_edge_offsets"],
                dictionary=face["dictionary"],
            )

        # Cells
        for uuid, cell in data["cells"].items():
            topology_cell = TopologyCell(uuid=uuid,dictionary=cell["dictionary"],)
            topology_cell.faces = [self.faces[face_uuid] for face_uuid in cell["faces"]]
            self.cells[uuid] = topology_cell

        self.build_topology()

    def build_topology(self):
        """Build the reverse topology relationships."""

        # Edge -> Vert, Vert -> Edge
        for edge in self.edges.values():
            for vert in edge.verts:
                vert.edges.append(edge)

        # Face -> Edge, Edge -> Face
        for face in self.faces.values():
            for edge in face.edges:
                edge.faces.append(face)

        # Cell -> Face, Face -> Cell
        for cell in self.cells.values():
            for face in cell.faces:
                face.cells.append(cell)

    def serialize(self) -> dict[str, Any]:
        return {
            "verts": {
                uuid: {
                    "coords": list(vert.coords),
                    "edges": [edge.uuid for edge in vert.edges], "dictionary": vert.dictionary,
                }
                for uuid, vert in self.verts.items()
            },

            "edges": {
                uuid: {
                    "verts": [vert.uuid for vert in edge.verts],
                    "faces": [face.uuid for face in edge.faces],
                    "dictionary": edge.dictionary,
                }
                for uuid, edge in self.edges.items()
            },

            "faces": {
                uuid: {
                    "cells": [cell.uuid for cell in face.cells],
                    "edges": [edge.uuid for edge in face.edges],
                    "verts": [vert.uuid for vert in face.verts],
                    "construction": face.construction,
                    "type": face.type,
                    "uvn": face.uvn,
                    "layer_edge_offsets": face.layer_edge_offsets,
                    "dictionary": face.dictionary,
                }
                for uuid, face in self.faces.items()
            },

            "cells": {
                uuid: {
                    "faces": [face.uuid for face in cell.faces],
                    "dictionary": cell.dictionary,
                }
                for uuid, cell in self.cells.items()
            },
        }

    def save_topology(self, filename: str = "topology.json"):
        path = Path(filename)

        with path.open("w", encoding="utf-8") as f:
            json.dump(
                self.serialize(),
                f,
                indent=2,
            )


def load_topology(filename: str = "topology.json") -> Topology:
    path = Path(filename)

    with path.open("r", encoding="utf-8") as f:
        topology_data = json.load(f)

    return Topology(topology_data)