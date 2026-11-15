import bpy
import bmesh
import math
import os
import random

bpy.ops.wm.read_factory_settings(use_empty=True)

output_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "assets", "models"))
os.makedirs(output_dir, exist_ok=True)


def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    for mesh in list(bpy.data.meshes):
        if mesh.users == 0:
            bpy.data.meshes.remove(mesh)
    for mat in list(bpy.data.materials):
        if mat.users == 0:
            bpy.data.materials.remove(mat)
    for img in list(bpy.data.images):
        if img.users == 0:
            bpy.data.images.remove(img)


def apply_box_uv(obj, scale=1.0):
    """Assign clean box-projected UVs in bmesh without needing edit-mode context."""
    mesh = obj.data
    if not mesh.uv_layers:
        mesh.uv_layers.new(name="UVMap")
    uv_layer = mesh.uv_layers.active.data
    for poly in mesh.polygons:
        n = poly.normal
        ax, ay, az = abs(n.x), abs(n.y), abs(n.z)
        for loop_idx in poly.loop_indices:
            v = mesh.vertices[mesh.loops[loop_idx].vertex_index].co
            if az >= ax and az >= ay:
                u, w = v.x, v.y
            elif ax >= ay and ax >= az:
                u, w = v.y, v.z
            else:
                u, w = v.x, v.z
            uv_layer[loop_idx].uv = (u * scale + 0.5, w * scale + 0.5)


def make_image_material(name, size, pixel_fn, roughness=0.75, emission_strength=0.0):
    img = bpy.data.images.new(name=f"{name}_Tex", width=size, height=size, alpha=False)
    pixels = [0.0] * (size * size * 4)
    for y in range(size):
        for x in range(size):
            r, g, b = pixel_fn(x, y, size)
            idx = (y * size + x) * 4
            pixels[idx] = r
            pixels[idx + 1] = g
            pixels[idx + 2] = b
            pixels[idx + 3] = 1.0
    img.pixels.foreach_set(pixels)
    img.pack()

    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = roughness

    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img

    links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if emission_strength > 0.0:
        links.new(tex.outputs["Color"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = emission_strength
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    return mat


def create_plaid_material():
    def plaid_px(x, y, s):
        gx = (x * 4) // s
        gy = (y * 4) // s
        cx = gx % 2 == 0
        cy = gy % 2 == 0
        if cx and cy:
            return (0.76, 0.08, 0.10)  # Bright Buffalo Red
        elif not cx and not cy:
            return (0.05, 0.05, 0.06)  # Charcoal Black
        else:
            # Diagonal weave mix of dark red and black
            if (x + y) % 4 < 2:
                return (0.42, 0.04, 0.06)
            return (0.14, 0.03, 0.04)

    return make_image_material("PlaidOnesieMat", 128, plaid_px, roughness=0.85)


def create_tendy_material():
    rng = random.Random(42)
    noise = [[rng.uniform(-0.12, 0.12) for _ in range(64)] for _ in range(64)]

    def tendy_px(x, y, s):
        n = noise[y % 64][x % 64]
        return (
            min(1.0, max(0.0, 0.94 + n)),
            min(1.0, max(0.0, 0.58 + n * 0.9)),
            min(1.0, max(0.0, 0.12 + n * 0.4)),
        )

    return make_image_material("GoldenTendyMat", 64, tendy_px, roughness=0.45, emission_strength=0.35)


def create_solid_material(name, color, roughness=0.5, metallic=0.0, emission_strength=0.0):
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if emission_strength > 0.0:
        bsdf.inputs["Emission Color"].default_value = color
        bsdf.inputs["Emission Strength"].default_value = emission_strength
    return mat


def add_sphere(radius, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.5):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=20, ring_count=12, radius=radius, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_cylinder(radius, depth, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.5, vertices=20):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_cube(size, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.0):
    bpy.ops.mesh.primitive_cube_add(size=size, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_cone(r1, r2, depth, location, rotation=(0, 0, 0), mat=None, vertices=20):
    bpy.ops.mesh.primitive_cone_add(vertices=vertices, radius1=r1, radius2=r2, depth=depth, location=location, rotation=rotation)
    obj = bpy.context.active_object
    if mat:
        obj.data.materials.append(mat)
    return obj


def export_glb(filepath):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format='GLB',
        use_selection=True,
        export_apply=True
    )


def build_plaid_hero():
    """Hero facing +Y in Blender (-Z forward in glTF/Bevy)."""
    clear_scene()
    plaid = create_plaid_material()
    skin = create_solid_material("SkinMat", (0.89, 0.68, 0.54, 1.0), roughness=0.55)
    hair = create_solid_material("HairMat", (0.28, 0.17, 0.10, 1.0), roughness=0.8)
    shoe = create_solid_material("ShoeMat", (0.12, 0.12, 0.14, 1.0), roughness=0.65)
    sole = create_solid_material("SoleMat", (0.88, 0.88, 0.90, 1.0), roughness=0.5)
    flap = create_solid_material("FlapMat", (0.65, 0.45, 0.28, 1.0), roughness=0.85)
    eye_w = create_solid_material("EyeWhiteMat", (0.98, 0.98, 0.98, 1.0), roughness=0.2)
    eye_b = create_solid_material("EyeBlackMat", (0.04, 0.04, 0.05, 1.0), roughness=0.2)

    # Stocky plaid onesie torso
    add_sphere(0.44, (0, 0, 1.02), scale=(1.05, 0.85, 1.25), mat=plaid, uv_scale=1.8)
    # Little tan onesie butt-flap on the back (-Y)
    add_sphere(0.11, (0, -0.37, 0.82), scale=(1.1, 0.6, 1.0), mat=flap)

    # Head
    add_sphere(0.30, (0, 0.02, 1.80), scale=(0.98, 0.95, 1.04), mat=skin)
    # Messy brown hair cap + tufts
    add_sphere(0.32, (0, -0.02, 1.88), scale=(1.02, 1.04, 0.84), mat=hair)
    for tx, ty, tz, rot in [(-0.12, 0.12, 2.12, 0.3), (0.05, 0.14, 2.15, -0.2), (0.16, 0.04, 2.08, -0.4)]:
        add_cone(0.11, 0.01, 0.22, (tx, ty, tz), rotation=(0.3, rot, 0), mat=hair, vertices=10)

    # Short beard & mustache
    add_sphere(0.27, (0, 0.09, 1.69), scale=(0.96, 0.92, 0.68), mat=hair)
    # Eyes (+Y front)
    for sx in (-1, 1):
        add_sphere(0.055, (sx * 0.11, 0.27, 1.84), mat=eye_w)
        add_sphere(0.032, (sx * 0.11, 0.31, 1.84), mat=eye_b)

    # Arms (slightly angled down in heroic run-ready pose)
    for sx in (-1, 1):
        add_cylinder(0.12, 0.62, (sx * 0.56, 0.02, 1.24), rotation=(0.15, sx * -0.55, 0), mat=plaid, uv_scale=2.0)
        add_sphere(0.11, (sx * 0.72, 0.06, 0.98), mat=skin)

        # Legs
        add_cylinder(0.16, 0.58, (sx * 0.20, 0, 0.42), mat=plaid, uv_scale=2.0)
        # Slip-on shoes with white sole
        add_cube(0.24, (sx * 0.20, 0.07, 0.10), scale=(0.82, 1.35, 0.52), mat=shoe)
        add_cube(0.24, (sx * 0.20, 0.07, 0.03), scale=(0.86, 1.38, 0.16), mat=sole)

    export_glb(os.path.join(output_dir, "plaid_hero.glb"))


def build_golden_tendy():
    clear_scene()
    tendy = create_tendy_material()
    # Organic curved crispy tendy made of 4 overlapping textured lobes
    lobes = [
        (0.0, -0.26, 0.48, 0.20, (0.9, 1.2, 0.75)),
        (0.04, -0.05, 0.52, 0.24, (1.0, 1.25, 0.82)),
        (-0.02, 0.16, 0.50, 0.22, (0.92, 1.2, 0.78)),
        (0.03, 0.34, 0.46, 0.16, (0.85, 1.1, 0.70)),
    ]
    for x, y, z, r, sc in lobes:
        add_sphere(r, (x, y, z), scale=sc, mat=tendy, uv_scale=2.5)
    export_glb(os.path.join(output_dir, "golden_tendy.glb"))


def build_broccoli_brute():
    """Muscular grumpy Broccoli Brute facing +Y."""
    clear_scene()
    stalk = create_solid_material("StalkMat", (0.38, 0.68, 0.22, 1.0), roughness=0.55)
    crown = create_solid_material("CrownMat", (0.10, 0.38, 0.12, 1.0), roughness=0.88)
    brow = create_solid_material("BrowMat", (0.07, 0.26, 0.08, 1.0), roughness=0.8)
    eye_w = create_solid_material("BruteEyeMat", (0.98, 0.95, 0.78, 1.0), roughness=0.2)
    eye_b = create_solid_material("BrutePupilMat", (0.05, 0.05, 0.05, 1.0), roughness=0.2)
    teeth = create_solid_material("TeethMat", (0.96, 0.96, 0.92, 1.0), roughness=0.3)

    # Muscular torso
    add_cylinder(0.45, 0.95, (0, 0, 0.88), scale=(1.15, 0.85, 1.0), mat=stalk)
    # Big muscular arms & fists
    for sx in (-1, 1):
        add_cylinder(0.18, 0.72, (sx * 0.68, 0.08, 1.02), rotation=(0.3, sx * -0.4, 0), mat=stalk)
        add_sphere(0.22, (sx * 0.82, 0.22, 0.68), mat=stalk)
        # Legs
        add_cylinder(0.18, 0.48, (sx * 0.25, 0, 0.24), mat=stalk)

    # Dense floret crown
    puffs = [
        (0, 0, 1.68, 0.62), (-0.48, 0.05, 1.56, 0.46), (0.48, 0.05, 1.56, 0.46),
        (0, -0.38, 1.58, 0.46), (-0.28, 0.34, 1.62, 0.36), (0.28, 0.34, 1.62, 0.36),
        (0, 0.1, 1.98, 0.42),
    ]
    for x, y, z, r in puffs:
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=r, location=(x, y, z))
        p = bpy.context.active_object
        p.data.materials.append(crown)

    # Angry brow, eyes, and toothy grimace (+Y front)
    add_cube(0.2, (0, 0.42, 1.24), scale=(3.0, 0.6, 0.45), mat=brow)
    for sx in (-1, 1):
        add_sphere(0.08, (sx * 0.16, 0.40, 1.12), mat=eye_w)
        add_sphere(0.04, (sx * 0.16, 0.46, 1.12), mat=eye_b)
    add_cube(0.18, (0, 0.40, 0.90), scale=(2.2, 0.5, 0.45), mat=teeth)

    export_glb(os.path.join(output_dir, "broccoli_brute.glb"))


def build_carrot_lancer():
    """Angry Carrot holding a silver kitchen knife, facing +Y."""
    clear_scene()
    carrot = create_solid_material("CarrotMat", (0.96, 0.40, 0.05, 1.0), roughness=0.45)
    leaf = create_solid_material("CarrotLeafMat", (0.14, 0.62, 0.16, 1.0), roughness=0.7)
    eye_w = create_solid_material("EyeWMat", (0.98, 0.98, 0.98, 1.0), roughness=0.2)
    eye_b = create_solid_material("EyeBMat", (0.05, 0.05, 0.05, 1.0), roughness=0.2)
    blade = create_solid_material("BladeMat", (0.85, 0.88, 0.92, 1.0), roughness=0.18, metallic=0.9)
    handle = create_solid_material("HandleMat", (0.15, 0.15, 0.18, 1.0), roughness=0.5)

    # Tapered orange carrot body
    add_cone(0.38, 0.08, 1.35, (0, 0, 0.95), rotation=(math.pi, 0, 0), mat=carrot)
    # Leafy green hair tuft
    for ang in (-0.35, 0.0, 0.35):
        add_cone(0.13, 0.02, 0.52, (ang * 0.4, 0, 1.82), rotation=(0.15, ang, 0), mat=leaf, vertices=10)
    # Eyes + angry brow
    for sx in (-1, 1):
        add_sphere(0.075, (sx * 0.13, 0.28, 1.28), mat=eye_w)
        add_sphere(0.04, (sx * 0.13, 0.34, 1.28), mat=eye_b)
        add_cylinder(0.07, 0.38, (sx * 0.16, 0, 0.20), mat=carrot)
        add_cylinder(0.06, 0.48, (sx * 0.38, 0.12, 0.92), rotation=(0.9, 0, sx * -0.2), mat=carrot)

    # Silver kitchen knife held forward in right hand (+X, +Y)
    add_cube(0.12, (0.42, 0.36, 0.85), scale=(0.6, 2.2, 0.7), mat=handle)
    add_cube(0.12, (0.42, 0.90, 0.86), scale=(0.25, 6.8, 1.1), mat=blade)

    export_glb(os.path.join(output_dir, "carrot_lancer.glb"))


def build_kale_slime():
    clear_scene()
    kale = create_solid_material("KaleMat", (0.16, 0.52, 0.16, 1.0), roughness=0.35)
    kale_dark = create_solid_material("KaleDarkMat", (0.08, 0.32, 0.10, 1.0), roughness=0.45)
    eye_w = create_solid_material("SlimeEyeWMat", (0.98, 0.98, 0.95, 1.0), roughness=0.2)
    eye_b = create_solid_material("SlimeEyeBMat", (0.05, 0.05, 0.05, 1.0), roughness=0.2)

    puffs = [
        (0, 0, 0.28, 0.62, (1.2, 1.15, 0.62), kale),
        (-0.42, 0.16, 0.24, 0.36, (1.1, 1.0, 0.7), kale_dark),
        (0.42, 0.14, 0.24, 0.36, (1.0, 1.1, 0.7), kale_dark),
        (0, -0.38, 0.25, 0.38, (1.15, 0.95, 0.7), kale),
    ]
    for x, y, z, r, sc, mat in puffs:
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=r, location=(x, y, z))
        p = bpy.context.active_object
        p.scale = sc
        p.data.materials.append(mat)

    for sx in (-1, 1):
        add_sphere(0.10, (sx * 0.20, 0.52, 0.40), mat=eye_w)
        add_sphere(0.05, (sx * 0.20, 0.60, 0.40), mat=eye_b)

    export_glb(os.path.join(output_dir, "kale_slime.glb"))


def build_honey_mustard_cup():
    clear_scene()
    cup = create_solid_material("CupMat", (0.94, 0.94, 0.96, 1.0), roughness=0.25)
    mustard = create_solid_material("MustardMat", (0.96, 0.74, 0.08, 1.0), roughness=0.3, emission_strength=0.25)
    add_cone(0.28, 0.38, 0.45, (0, 0, 0.35), mat=cup)
    add_cylinder(0.35, 0.08, (0, 0, 0.56), mat=mustard)
    export_glb(os.path.join(output_dir, "honey_mustard_cup.glb"))


def build_ranch_shield():
    clear_scene()
    ranch = create_solid_material("RanchMat", (0.96, 0.98, 0.94, 1.0), roughness=0.3, emission_strength=0.25)
    herb = create_solid_material("HerbMat", (0.16, 0.56, 0.22, 1.0), roughness=0.6)
    add_sphere(0.40, (0, 0, 0.55), mat=ranch)
    bpy.ops.mesh.primitive_torus_add(major_radius=0.52, minor_radius=0.05, location=(0, 0, 0.55), rotation=(0.35, 0.2, 0))
    bpy.context.active_object.data.materials.append(herb)
    export_glb(os.path.join(output_dir, "ranch_shield.glb"))


def build_kitchen_sponge():
    """Yellow sponge with green scrub pad on top."""
    clear_scene()
    yellow = create_solid_material("SpongeYellowMat", (0.95, 0.80, 0.16, 1.0), roughness=0.92)
    green = create_solid_material("SpongeGreenMat", (0.12, 0.42, 0.14, 1.0), roughness=0.95)
    add_cube(1.0, (0, 0, 0.30), scale=(2.2, 1.4, 0.60), mat=yellow)
    add_cube(1.0, (0, 0, 0.70), scale=(2.2, 1.4, 0.22), mat=green)
    export_glb(os.path.join(output_dir, "kitchen_sponge.glb"))


def build_pancake_stack():
    """Tall stack of golden pancakes with butter pat and maple syrup."""
    clear_scene()
    plate = create_solid_material("PlateMat", (0.95, 0.96, 0.98, 1.0), roughness=0.2)
    cake = create_solid_material("PancakeMat", (0.76, 0.46, 0.18, 1.0), roughness=0.75)
    butter = create_solid_material("ButterMat", (0.98, 0.86, 0.24, 1.0), roughness=0.35)
    syrup = create_solid_material("SyrupMat", (0.58, 0.24, 0.04, 1.0), roughness=0.15)

    add_cylinder(2.2, 0.18, (0, 0, 0.09), mat=plate, vertices=28)
    for i in range(6):
        add_cylinder(1.55 - i * 0.03, 0.26, (0, 0, 0.32 + i * 0.25), mat=cake, vertices=28)
    # Syrup cap & drips
    add_cylinder(1.1, 0.08, (0, 0, 1.72), mat=syrup, vertices=24)
    add_cube(0.55, (0, 0, 1.84), scale=(1.0, 1.0, 0.35), rotation=(0, 0, 0.4), mat=butter)
    export_glb(os.path.join(output_dir, "pancake_stack.glb"))


def build_plate_with_egg():
    clear_scene()
    plate = create_solid_material("PlateMat", (0.85, 0.90, 0.96, 1.0), roughness=0.2)
    white = create_solid_material("EggWhiteMat", (0.98, 0.98, 0.96, 1.0), roughness=0.35)
    yolk = create_solid_material("EggYolkMat", (0.98, 0.62, 0.06, 1.0), roughness=0.25)
    add_cylinder(1.6, 0.16, (0, 0, 0.08), mat=plate, vertices=28)
    add_cylinder(0.95, 0.08, (0.1, 0.05, 0.18), mat=white, vertices=24)
    add_sphere(0.36, (0.1, 0.05, 0.24), scale=(1.0, 1.0, 0.55), mat=yolk)
    export_glb(os.path.join(output_dir, "plate_with_egg.glb"))


def build_syrup_bottle():
    clear_scene()
    glass = create_solid_material("BottleGlassMat", (0.45, 0.18, 0.04, 1.0), roughness=0.18)
    cap = create_solid_material("BottleCapMat", (0.82, 0.10, 0.10, 1.0), roughness=0.4)
    label = create_solid_material("BottleLabelMat", (0.94, 0.88, 0.72, 1.0), roughness=0.7)
    add_cylinder(0.65, 2.0, (0, 0, 1.0), mat=glass, vertices=20)
    add_cylinder(0.32, 0.7, (0, 0, 2.3), mat=glass, vertices=20)
    add_cylinder(0.36, 0.35, (0, 0, 2.75), mat=cap, vertices=20)
    add_cylinder(0.67, 0.9, (0, 0, 1.0), mat=label, vertices=20)
    export_glb(os.path.join(output_dir, "syrup_bottle.glb"))


def build_salt_shaker():
    clear_scene()
    glass = create_solid_material("ShakerGlassMat", (0.92, 0.94, 0.96, 1.0), roughness=0.2)
    cap = create_solid_material("ShakerCapMat", (0.82, 0.84, 0.88, 1.0), roughness=0.18, metallic=0.9)
    add_cylinder(0.42, 1.1, (0, 0, 0.55), mat=glass, vertices=16)
    add_sphere(0.40, (0, 0, 1.12), scale=(1.0, 1.0, 0.7), mat=cap)
    export_glb(os.path.join(output_dir, "salt_shaker.glb"))


if __name__ == "__main__":
    build_plaid_hero()
    build_golden_tendy()
    build_broccoli_brute()
    build_carrot_lancer()
    build_kale_slime()
    build_honey_mustard_cup()
    build_ranch_shield()
    build_kitchen_sponge()
    build_pancake_stack()
    build_plate_with_egg()
    build_syrup_bottle()
    build_salt_shaker()
    print("Successfully exported all 12 Tendy Quest .glb assets to:", output_dir)
