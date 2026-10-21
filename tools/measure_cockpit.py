"""Find where things in the cockpit are, in car space.

Take a screenshot from the driver's seat (see AGENT.md), note the pixels
of what you want to measure, then cast rays from the driver's eyes through
those pixels against the car model:

    python tools/measure_cockpit.py assets/models/porsche_911.glb 1022,685 1125,750

Prints the first point each ray hits. This is how the infotainment
screen and the head-up display in src/hud.rs were placed. The defaults
match the game: the model's root transform in src/car/mod.rs, the eye
position (DRIVER_HEAD) and the camera's field of view.
"""

import argparse
import json
import math
import struct


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("model", help="the car's .glb")
    parser.add_argument("pixels", nargs="+", help="screenshot pixels as x,y")
    parser.add_argument("--eye", default="-0.335,1.1,0.4", help="eye position in car space")
    parser.add_argument("--fov", type=float, default=70.0, help="vertical field of view in degrees")
    parser.add_argument("--screenshot", default="1600x900", help="screenshot size")
    parser.add_argument("--scale", type=float, default=100.0, help="scale on the model's root")
    parser.add_argument("--no-turn", action="store_true", help="the root isn't turned 180 degrees")
    return parser.parse_args()


def multiply(a, b):
    """4x4 column-major matrices, as in glTF."""
    return [sum(a[k * 4 + r] * b[c * 4 + k] for k in range(4)) for c in range(4) for r in range(4)]


def node_matrix(node):
    if "matrix" in node:
        return node["matrix"]

    tx, ty, tz = node.get("translation", [0, 0, 0])
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    sx, sy, sz = node.get("scale", [1, 1, 1])
    return [
        (1 - 2 * (y * y + z * z)) * sx, 2 * (x * y + z * w) * sx, 2 * (x * z - y * w) * sx, 0,
        2 * (x * y - z * w) * sy, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z + x * w) * sy, 0,
        2 * (x * z + y * w) * sz, 2 * (y * z - x * w) * sz, (1 - 2 * (x * x + y * y)) * sz, 0,
        tx, ty, tz, 1,
    ]


def transform(m, p):
    return tuple(m[i] * p[0] + m[4 + i] * p[1] + m[8 + i] * p[2] + m[12 + i] for i in range(3))


def load_triangles(path, root):
    data = open(path, "rb").read()
    json_length = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20 : 20 + json_length])
    binary = 20 + json_length + 8

    def read(accessor_index):
        accessor = gltf["accessors"][accessor_index]
        view = gltf["bufferViews"][accessor["bufferView"]]
        offset = binary + view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
        count = accessor["count"]
        if accessor["type"] == "VEC3":
            stride = view.get("byteStride", 12)
            return [struct.unpack_from("<fff", data, offset + i * stride) for i in range(count)]
        fmt = {5121: "<B", 5123: "<H", 5125: "<I"}[accessor["componentType"]]
        size = struct.calcsize(fmt)
        return [struct.unpack_from(fmt, data, offset + i * size)[0] for i in range(count)]

    triangles = []

    def walk(index, parent):
        node = gltf["nodes"][index]
        matrix = multiply(parent, node_matrix(node))
        if "mesh" in node:
            for primitive in gltf["meshes"][node["mesh"]]["primitives"]:
                positions = [transform(matrix, p) for p in read(primitive["attributes"]["POSITION"])]
                indices = read(primitive["indices"])
                for k in range(0, len(indices), 3):
                    triangles.append(tuple(positions[indices[k + j]] for j in range(3)))
        for child in node.get("children", []):
            walk(child, matrix)

    for index in gltf["scenes"][gltf.get("scene", 0)]["nodes"]:
        walk(index, root)
    return triangles


def ray_hit(origin, direction, triangles, min_distance=0.05):
    """Distance to the nearest triangle (Moller-Trumbore), or None."""
    def sub(a, b):
        return [a[i] - b[i] for i in range(3)]

    def cross(a, b):
        return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])

    def dot(a, b):
        return sum(a[i] * b[i] for i in range(3))

    nearest = None
    for a, b, c in triangles:
        e1, e2 = sub(b, a), sub(c, a)
        p = cross(direction, e2)
        det = dot(e1, p)
        if abs(det) < 1e-12:
            continue
        t = sub(origin, a)
        u = dot(t, p) / det
        if u < 0 or u > 1:
            continue
        q = cross(t, e1)
        v = dot(direction, q) / det
        if v < 0 or u + v > 1:
            continue
        distance = dot(e2, q) / det
        if distance > min_distance and (nearest is None or distance < nearest):
            nearest = distance
    return nearest


def main():
    args = parse_args()
    eye = tuple(float(v) for v in args.eye.split(","))
    width, height = (int(v) for v in args.screenshot.split("x"))
    turn = 1 if args.no_turn else -1
    root = [turn * args.scale, 0, 0, 0, 0, args.scale, 0, 0, 0, 0, turn * args.scale, 0, 0, 0, 0, 1]

    triangles = load_triangles(args.model, root)
    tan = math.tan(math.radians(args.fov) / 2)

    for pixel in args.pixels:
        px, py = (float(v) for v in pixel.split(","))
        # The camera looks along -Z with +Y up.
        x = (px / (width / 2) - 1) * tan * width / height
        y = (1 - py / (height / 2)) * tan
        length = math.sqrt(x * x + y * y + 1)
        direction = (x / length, y / length, -1 / length)

        distance = ray_hit(eye, direction, triangles)
        if distance is None:
            print(f"{pixel}: nothing")
        else:
            point = [round(eye[i] + direction[i] * distance, 4) for i in range(3)]
            print(f"{pixel}: {point} ({distance:.3f} m from the eyes)")


main()
