"""Prepare a downloaded car model for use as traffic.

Run with Blender (tested with 3.4 and 5.2):

    blender -b --factory-startup --python tools/convert_traffic_car.py -- \
        scene.gltf assets/models/traffic/car.glb --length 4.81 --turn

It joins the model into one body, reduces it to about --triangles
triangles, scales it so it is --length metres long, centres it, stands it
on its tyres and saves a .glb that faces -Z in Bevy. The four wheels become
separate nodes (wheel_front_left, ...) centred on their axles, so the game
can spin them. Pass --turn if the car comes out facing backwards.

It prints each wheel's radius, which goes into src/traffic.rs.
"""

import argparse
import math
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

WHEEL_NAMES = {
    "fl": "wheel_front_left",
    "fr": "wheel_front_right",
    "rl": "wheel_rear_left",
    "rr": "wheel_rear_right",
}


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("source", help="the model to convert (.gltf or .glb)")
    parser.add_argument("output", help="the .glb to write")
    parser.add_argument("--length", type=float, required=True, help="real length in metres")
    parser.add_argument("--triangles", type=int, default=50_000)
    parser.add_argument("--turn", action="store_true", help="turn the car around")
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def select_only(objects, active):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = active


def bounds(points):
    lo = Vector([min(p[i] for p in points) for i in range(3)])
    hi = Vector([max(p[i] for p in points) for i in range(3)])
    return lo, hi


def triangle_count(objects):
    return sum(len(p.vertices) - 2 for o in objects for p in o.data.polygons)


def import_as_one_mesh(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=path)

    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in meshes:
        world = o.matrix_world.copy()
        o.parent = None
        o.matrix_world = world
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)

    select_only(meshes, meshes[0])
    bpy.ops.object.join()
    car = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return car


def normalise(car, length, turn):
    """Scale to metres, centre, and point the front towards Blender +Y,
    which the glTF export turns into -Z."""
    lo, hi = bounds([v.co for v in car.data.vertices])
    size = hi - lo

    rotation = Matrix.Identity(4)
    if size.x > size.y:
        rotation = Matrix.Rotation(math.pi / 2, 4, "Z")
    if turn:
        rotation = Matrix.Rotation(math.pi, 4, "Z") @ rotation

    bottom_centre = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
    scale = Matrix.Scale(length / max(size.x, size.y), 4)
    car.data.transform(scale @ rotation @ Matrix.Translation(-bottom_centre))


def pieces(car):
    """The connected pieces of the mesh, as (vertex indices, lo, hi).

    Done with bmesh rather than "separate by loose parts", which creates
    one object per piece and is very slow with thousands of them."""
    bm = bmesh.new()
    bm.from_mesh(car.data)
    bm.verts.ensure_lookup_table()
    positions = [v.co.copy() for v in bm.verts]
    seen = [False] * len(positions)
    result = []

    for start in bm.verts:
        if seen[start.index]:
            continue
        seen[start.index] = True
        stack = [start]
        indices = []
        while stack:
            v = stack.pop()
            indices.append(v.index)
            for edge in v.link_edges:
                other = edge.other_vert(v)
                if not seen[other.index]:
                    seen[other.index] = True
                    stack.append(other)
        lo, hi = bounds([positions[i] for i in indices])
        result.append((indices, lo, hi))

    bm.free()
    return result


def find_tyres(car):
    """Tyres are pieces near the bottom, about as tall as they are long,
    away from the middle. Returns the bounds of each corner's tyre."""
    wheels = {}
    for _, lo, hi in pieces(car):
        height = hi.z - lo.z
        length = hi.y - lo.y
        x = (lo.x + hi.x) / 2
        y = (lo.y + hi.y) / 2
        if lo.z < 0.15 and 0.4 < height < 1.1 and abs(height - length) < 0.15 and abs(x) > 0.3:
            key = ("f" if y > 0 else "r") + ("l" if x < 0 else "r")
            wheel = wheels.setdefault(key, [lo.copy(), hi.copy()])
            for i in range(3):
                wheel[0][i] = min(wheel[0][i], lo[i])
                wheel[1][i] = max(wheel[1][i], hi[i])

    if len(wheels) != 4:
        sys.exit(f"Found tyres at {sorted(wheels)} instead of all four corners.")
    return wheels


def inside_wheel(lo, hi, wheel_lo, wheel_hi, margin=0.03):
    centre = (wheel_lo + wheel_hi) / 2
    radius = (wheel_hi.z - wheel_lo.z) / 2
    return (
        wheel_lo.x - margin <= lo.x
        and hi.x <= wheel_hi.x + margin
        and centre.y - radius - margin <= lo.y
        and hi.y <= centre.y + radius + margin
        and centre.z - radius - margin <= lo.z
        and hi.z <= centre.z + radius + margin
    )


def split_off_wheel(car, wheel_lo, wheel_hi, name):
    """Moves every piece inside the wheel (tyre, rim, bolts, ...) into its
    own object, with its origin at the wheel centre."""
    chosen = set()
    for indices, lo, hi in pieces(car):
        if inside_wheel(lo, hi, wheel_lo, wheel_hi):
            chosen.update(indices)

    mesh = car.data
    for v in mesh.vertices:
        v.select = v.index in chosen
    for e in mesh.edges:
        e.select = e.vertices[0] in chosen
    for f in mesh.polygons:
        f.select = f.vertices[0] in chosen

    select_only([car], car)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.separate(type="SELECTED")
    bpy.ops.object.mode_set(mode="OBJECT")

    wheel = next(o for o in bpy.context.selected_objects if o != car)
    centre = (wheel_lo + wheel_hi) / 2
    wheel.data.transform(Matrix.Translation(-centre))
    wheel.location = centre
    wheel.name = name

    radius = (wheel_hi.z - wheel_lo.z) / 2
    print(f"{name}: centre {[round(v, 3) for v in centre]}, radius {radius:.3f}")


def reduce(objects, target):
    ratio = min(1.0, target / triangle_count(objects))
    for o in objects:
        select_only([o], o)
        modifier = o.modifiers.new("decimate", "DECIMATE")
        modifier.ratio = ratio
        bpy.ops.object.modifier_apply(modifier=modifier.name)


def main():
    args = parse_args()

    car = import_as_one_mesh(args.source)
    original = triangle_count([car])
    normalise(car, args.length, args.turn)

    # Stand the car on its tyres, as some parts may hang lower.
    wheels = find_tyres(car)
    ground = min(lo.z for lo, _ in wheels.values())
    car.data.transform(Matrix.Translation(Vector((0, 0, -ground))))
    for lo, hi in wheels.values():
        lo.z -= ground
        hi.z -= ground

    for key, (lo, hi) in wheels.items():
        split_off_wheel(car, lo, hi, WHEEL_NAMES[key])
    car.name = "body"

    objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    reduce(objects, args.triangles)
    print(f"Triangles: {original} -> {triangle_count(objects)}")

    bpy.ops.export_scene.gltf(filepath=args.output, export_format="GLB", export_apply=True)


main()
