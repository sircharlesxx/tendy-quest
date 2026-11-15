import bpy
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


def set_smooth(obj):
    if obj and obj.type == 'MESH' and obj.data:
        for poly in obj.data.polygons:
            poly.use_smooth = True


def apply_box_uv(obj, scale=1.0):
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
    set_smooth(obj)


def make_image_material(name, size, pixel_fn, roughness=0.75, metallic=0.0, emission_strength=0.0):
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
    bsdf.inputs["Metallic"].default_value = metallic

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
            return (0.78, 0.09, 0.13)  # #C81822 crimson red
        elif not cx and not cy:
            return (0.06, 0.06, 0.07)  # #101012 jet black
        elif (x + y) % 4 < 2:
            return (0.45, 0.05, 0.07)
        else:
            return (0.12, 0.03, 0.04)

    return make_image_material("PlaidOnesieMat", 256, plaid_px, roughness=0.82)


def create_tendy_material():
    rng = random.Random(42)
    noise = [[rng.uniform(-0.14, 0.14) for _ in range(128)] for _ in range(128)]

    def tendy_px(x, y, s):
        n = noise[y % 128][x % 128]
        crumb = 0.08 if ((x * 7 + y * 13) % 19 == 0) else 0.0
        return (
            min(1.0, max(0.0, 0.94 + n + crumb)),
            min(1.0, max(0.0, 0.58 + n * 0.85 + crumb * 0.6)),
            min(1.0, max(0.0, 0.11 + n * 0.35)),
        )

    return make_image_material("GoldenTendyMat", 128, tendy_px, roughness=0.40, emission_strength=0.40)


def create_butcher_block_material():
    rng = random.Random(101)
    plank_tints = [rng.uniform(-0.07, 0.07) for _ in range(16)]

    def wood_px(x, y, s):
        strip = (x * 8) // s
        stagger = (strip * 57) % s
        plank = (((y + stagger) % s) * 4) // s
        tint = plank_tints[(strip * 3 + plank) % len(plank_tints)]
        grain = math.sin((y + strip * 11) * 0.25 + math.sin(x * 0.4) * 1.5) * 0.025
        seam_x = (x * 8) % s < 2
        seam_y = ((y + stagger) * 4) % s < 2
        seam = -0.14 if (seam_x or seam_y) else 0.0
        return (
            min(1.0, max(0.0, 0.78 + tint + grain + seam)),
            min(1.0, max(0.0, 0.56 + tint * 0.8 + grain * 0.7 + seam * 0.8)),
            min(1.0, max(0.0, 0.33 + tint * 0.5 + grain * 0.4 + seam * 0.5)),
        )

    return make_image_material("ButcherBlockMat", 256, wood_px, roughness=0.32)


def create_gingham_material():
    def gingham_px(x, y, s):
        cx = ((x * 8) // s) % 2 == 0
        cy = ((y * 8) // s) % 2 == 0
        if cx and cy:
            return (0.96, 0.95, 0.93)
        elif not cx and not cy:
            return (0.82, 0.14, 0.16)
        else:
            return (0.91, 0.52, 0.50)

    return make_image_material("GinghamCurtainMat", 128, gingham_px, roughness=0.85)


def create_sponge_yellow_material():
    rng = random.Random(77)
    pores = [(rng.randint(4, 123), rng.randint(4, 123), rng.randint(2, 5)) for _ in range(70)]

    def sponge_px(x, y, s):
        for px, py, pr in pores:
            if (x - px) ** 2 + (y - py) ** 2 <= pr ** 2:
                return (0.58, 0.44, 0.05)
        return (0.96, 0.82, 0.18)

    return make_image_material("SpongeYellowMat", 128, sponge_px, roughness=0.92)


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


def add_sphere(radius, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.5, seg=24, ring=16):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=seg, ring_count=ring, radius=radius, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_ico(radius, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, subdiv=3, uv_scale=1.5):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdiv, radius=radius, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_cylinder(radius, depth, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.5, vertices=24):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    return obj


def add_cube(size, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.0, smooth=False):
    bpy.ops.mesh.primitive_cube_add(size=size, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale)
    if not smooth:
        for poly in obj.data.polygons:
            poly.use_smooth = False
    return obj


def add_cone(r1, r2, depth, location, rotation=(0, 0, 0), scale=(1, 1, 1), mat=None, vertices=24):
    bpy.ops.mesh.primitive_cone_add(vertices=vertices, radius1=r1, radius2=r2, depth=depth, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, 1.0)
    return obj


def add_torus(major_r, minor_r, location, rotation=(0, 0, 0), scale=(1, 1, 1), mat=None):
    bpy.ops.mesh.primitive_torus_add(major_segments=28, minor_segments=14, major_radius=major_r, minor_radius=minor_r, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    set_smooth(obj)
    return obj


def export_glb(filepath):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format='GLB',
        use_selection=True,
        export_apply=True,
    )


def build_plaid_hero():
    """Stocky cheerful hero facing +Y in Blender (-Z forward in glTF/Bevy)."""
    clear_scene()
    plaid = create_plaid_material()
    skin = create_solid_material("SkinMat", (0.90, 0.70, 0.56, 1.0), roughness=0.52)
    hair = create_solid_material("HairMat", (0.28, 0.16, 0.09, 1.0), roughness=0.78)
    shoe = create_solid_material("ShoeMat", (0.11, 0.11, 0.13, 1.0), roughness=0.60)
    sole = create_solid_material("SoleMat", (0.92, 0.92, 0.94, 1.0), roughness=0.45)
    flap = create_solid_material("FlapMat", (0.72, 0.54, 0.35, 1.0), roughness=0.82)
    btn = create_solid_material("ButtonMat", (0.14, 0.10, 0.08, 1.0), roughness=0.45)
    zipper = create_solid_material("ZipperMat", (0.16, 0.16, 0.18, 1.0), roughness=0.35, metallic=0.6)
    eye_w = create_solid_material("EyeWMat", (0.99, 0.99, 0.99, 1.0), roughness=0.18)
    eye_b = create_solid_material("EyeBMat", (0.04, 0.04, 0.05, 1.0), roughness=0.18)

    # Stocky plaid torso + shoulders + collar
    add_sphere(0.42, (0, 0, 1.02), scale=(1.04, 0.84, 1.24), mat=plaid, uv_scale=1.9)
    add_sphere(0.24, (0, 0.02, 1.38), scale=(1.65, 0.95, 0.55), mat=plaid, uv_scale=1.9)
    add_torus(0.20, 0.06, (0, 0.02, 1.50), rotation=(0.1, 0, 0), mat=plaid)
    # Front zipper strip (+Y)
    add_cube(0.05, (0, 0.34, 1.12), scale=(0.8, 0.6, 12.0), mat=zipper, smooth=True)
    # Buttoned tan butt-flap on the back (-Y)
    add_cube(0.32, (0, -0.34, 0.82), scale=(1.1, 0.25, 0.85), rotation=(0.15, 0, 0), mat=flap, smooth=True)
    for bx in (-0.11, 0.11):
        add_cylinder(0.035, 0.03, (bx, -0.38, 0.92), rotation=(math.pi / 2, 0, 0), mat=btn)

    # Head + nose + ears
    add_sphere(0.29, (0, 0.03, 1.80), scale=(0.96, 0.95, 1.05), mat=skin)
    add_sphere(0.055, (0, 0.30, 1.78), scale=(0.9, 1.1, 0.8), mat=skin)
    for sx in (-1, 1):
        add_sphere(0.065, (sx * 0.28, 0.01, 1.79), scale=(0.55, 0.85, 1.1), mat=skin)

    # Sculpted swept-back brown hair (cap + 6 layered locks)
    add_sphere(0.31, (0, -0.02, 1.89), scale=(1.02, 1.06, 0.82), mat=hair)
    locks = [
        (-0.14, 0.05, 2.13, (0.35, -0.25, 0)),
        (0.0, 0.07, 2.16, (0.40, 0.0, 0)),
        (0.14, 0.04, 2.12, (0.35, 0.28, 0)),
        (-0.10, -0.14, 2.08, (-0.35, -0.2, 0)),
        (0.10, -0.14, 2.08, (-0.35, 0.2, 0)),
        (0.0, -0.22, 1.98, (-0.6, 0.0, 0)),
    ]
    for lx, ly, lz, lrot in locks:
        add_cone(0.11, 0.015, 0.24, (lx, ly, lz), rotation=lrot, mat=hair, vertices=14)

    # Beard, mustache, eyebrows, eyes
    add_sphere(0.26, (0, 0.10, 1.69), scale=(0.94, 0.90, 0.65), mat=hair)
    add_sphere(0.12, (0, 0.28, 1.72), scale=(1.35, 0.55, 0.38), mat=hair)
    for sx in (-1, 1):
        add_sphere(0.055, (sx * 0.10, 0.27, 1.84), mat=eye_w)
        add_sphere(0.032, (sx * 0.10, 0.31, 1.84), mat=eye_b)
        add_cube(0.04, (sx * 0.10, 0.29, 1.91), scale=(2.2, 0.6, 0.45), rotation=(0, sx * -0.12, 0), mat=hair, smooth=True)

    # Arms, elbows, hands, legs & slip-on loafers
    for sx in (-1, 1):
        add_cylinder(0.115, 0.58, (sx * 0.52, 0.02, 1.22), rotation=(0.12, sx * -0.45, 0), mat=plaid, uv_scale=2.0)
        add_sphere(0.115, (sx * 0.64, 0.04, 0.99), mat=plaid, uv_scale=2.0)
        add_sphere(0.10, (sx * 0.68, 0.07, 0.91), mat=skin)

        add_cylinder(0.155, 0.58, (sx * 0.19, 0.0, 0.42), mat=plaid, uv_scale=2.0)
        add_sphere(0.155, (sx * 0.19, 0.0, 0.65), mat=plaid, uv_scale=2.0)

        add_sphere(0.13, (sx * 0.19, 0.08, 0.11), scale=(0.88, 1.45, 0.55), mat=shoe)
        add_cylinder(0.12, 0.04, (sx * 0.19, 0.08, 0.03), scale=(0.92, 1.48, 1.0), mat=sole)

    export_glb(os.path.join(output_dir, "plaid_hero.glb"))


def build_golden_tendy():
    clear_scene()
    tendy = create_tendy_material()
    star_mat = create_solid_material("SparkleStarMat", (1.0, 0.92, 0.30, 1.0), roughness=0.2, emission_strength=0.85)

    # Organic curved bumpy golden tender
    rng = random.Random(12)
    lobes = [
        (-0.04, -0.30, 0.46, 0.19, (0.95, 1.25, 0.78)),
        (0.03, -0.12, 0.52, 0.24, (1.05, 1.3, 0.85)),
        (-0.02, 0.10, 0.54, 0.23, (1.0, 1.25, 0.82)),
        (0.04, 0.28, 0.48, 0.18, (0.88, 1.15, 0.74)),
        (0.01, 0.42, 0.44, 0.12, (0.80, 1.0, 0.68)),
    ]
    for x, y, z, r, sc in lobes:
        obj = add_ico(r, (x, y, z), scale=sc, mat=tendy, subdiv=3, uv_scale=2.5)
        for v in obj.data.vertices:
            v.co *= 1.0 + rng.uniform(-0.09, 0.11)
        set_smooth(obj)

    # 6 floating golden 3D star sparkles orbiting the tendy
    for i in range(6):
        ang = i * (math.tau / 6.0)
        sx = math.cos(ang) * 0.58
        sy = math.sin(ang) * 0.58
        sz = 0.52 + (0.14 if i % 2 == 0 else -0.12)
        add_ico(0.065, (sx, sy, sz), scale=(1.0, 1.0, 1.0), mat=star_mat, subdiv=1)

    export_glb(os.path.join(output_dir, "golden_tendy.glb"))


def build_broccoli_brute():
    """Hulking muscular grumpy Broccoli Brute facing +Y."""
    clear_scene()
    stalk = create_solid_material("StalkMat", (0.38, 0.72, 0.22, 1.0), roughness=0.52)
    crown = create_solid_material("CrownMat", (0.09, 0.38, 0.11, 1.0), roughness=0.88)
    crown_hi = create_solid_material("CrownHiMat", (0.14, 0.48, 0.14, 1.0), roughness=0.85)
    brow = create_solid_material("BrowMat", (0.06, 0.25, 0.08, 1.0), roughness=0.75)
    eye_w = create_solid_material("BruteEyeMat", (0.98, 0.96, 0.76, 1.0), roughness=0.2)
    eye_b = create_solid_material("BrutePupilMat", (0.05, 0.05, 0.05, 1.0), roughness=0.2)
    mouth_bg = create_solid_material("MouthBgMat", (0.12, 0.04, 0.04, 1.0), roughness=0.8)
    teeth = create_solid_material("TeethMat", (0.97, 0.97, 0.93, 1.0), roughness=0.28)

    # Muscular V-tapered torso + pecs & abs
    add_cylinder(0.46, 0.98, (0, 0, 0.92), scale=(1.22, 0.86, 1.0), mat=stalk)
    for sx in (-1, 1):
        add_sphere(0.26, (sx * 0.22, 0.24, 1.10), scale=(1.0, 0.75, 0.85), mat=stalk)
        add_sphere(0.24, (sx * 0.64, 0.02, 1.24), mat=stalk)  # deltoid
        add_cylinder(0.18, 0.56, (sx * 0.78, 0.08, 0.96), rotation=(0.25, sx * -0.35, 0), mat=stalk)
        add_sphere(0.24, (sx * 0.88, 0.22, 0.62), scale=(1.0, 1.15, 0.95), mat=stalk)  # fist
        add_cylinder(0.20, 0.48, (sx * 0.26, 0, 0.24), mat=stalk)  # stalk leg
        add_sphere(0.20, (sx * 0.26, 0.06, 0.08), scale=(0.95, 1.35, 0.55), mat=stalk)

    # Dense 16-cluster floret afro crown
    puffs = [
        (0, 0, 1.72, 0.58, crown), (-0.52, 0.04, 1.62, 0.44, crown), (0.52, 0.04, 1.62, 0.44, crown),
        (0, -0.44, 1.64, 0.44, crown), (-0.36, -0.34, 1.72, 0.38, crown_hi), (0.36, -0.34, 1.72, 0.38, crown_hi),
        (-0.32, 0.34, 1.66, 0.36, crown_hi), (0.32, 0.34, 1.66, 0.36, crown_hi),
        (0, 0.14, 2.04, 0.44, crown_hi), (-0.38, 0.05, 1.98, 0.36, crown), (0.38, 0.05, 1.98, 0.36, crown),
        (0, -0.22, 1.98, 0.36, crown), (-0.58, -0.12, 1.76, 0.34, crown_hi), (0.58, -0.12, 1.76, 0.34, crown_hi),
        (-0.18, 0.36, 1.88, 0.30, crown), (0.18, 0.36, 1.88, 0.30, crown),
    ]
    for x, y, z, r, m in puffs:
        add_ico(r, (x, y, z), mat=m, subdiv=3)

    # Heavy brow ridge, angry eyes & toothy grimace (+Y)
    add_cube(0.18, (0, 0.42, 1.30), scale=(3.2, 0.65, 0.45), mat=brow, smooth=True)
    for sx in (-1, 1):
        add_sphere(0.085, (sx * 0.16, 0.41, 1.18), mat=eye_w)
        add_sphere(0.042, (sx * 0.16, 0.48, 1.18), mat=eye_b)
    add_cube(0.18, (0, 0.39, 0.94), scale=(2.4, 0.45, 0.65), mat=mouth_bg, smooth=True)
    for tx in (-0.14, -0.05, 0.05, 0.14):
        add_cone(0.035, 0.005, 0.07, (tx, 0.43, 0.90), mat=teeth, vertices=8)

    export_glb(os.path.join(output_dir, "broccoli_brute.glb"))


def build_carrot_lancer():
    """Tall tapered orange Carrot Lancer holding a silver butter-knife, facing +Y."""
    clear_scene()
    carrot = create_solid_material("CarrotMat", (0.96, 0.40, 0.05, 1.0), roughness=0.45)
    ridge = create_solid_material("CarrotRidgeMat", (0.82, 0.28, 0.03, 1.0), roughness=0.55)
    leaf = create_solid_material("CarrotLeafMat", (0.14, 0.62, 0.16, 1.0), roughness=0.68)
    eye_w = create_solid_material("EyeWMat", (0.98, 0.98, 0.98, 1.0), roughness=0.18)
    eye_b = create_solid_material("EyeBMat", (0.04, 0.04, 0.05, 1.0), roughness=0.18)
    blade = create_solid_material("BladeMat", (0.88, 0.90, 0.94, 1.0), roughness=0.15, metallic=0.92)
    handle = create_solid_material("HandleMat", (0.14, 0.14, 0.17, 1.0), roughness=0.45)

    # Tapered orange body + horizontal carrot root rings
    add_cone(0.38, 0.08, 1.40, (0, 0, 0.98), rotation=(math.pi, 0, 0), mat=carrot, vertices=28)
    for rz, rr in [(0.75, 0.21), (1.02, 0.28), (1.28, 0.33)]:
        add_torus(rr, 0.018, (0, 0, rz), mat=ridge)

    # Fountain of 6 curved green leaf stalks
    for i in range(6):
        ang = i * (math.tau / 6.0)
        add_cone(0.11, 0.015, 0.56, (math.cos(ang) * 0.12, math.sin(ang) * 0.12, 1.88),
                 rotation=(math.sin(ang) * 0.35, -math.cos(ang) * 0.35, 0), mat=leaf, vertices=12)

    # Angry eyes + slanted black eyebrows
    for sx in (-1, 1):
        add_sphere(0.085, (sx * 0.13, 0.28, 1.32), mat=eye_w)
        add_sphere(0.042, (sx * 0.13, 0.35, 1.32), mat=eye_b)
        add_cube(0.04, (sx * 0.13, 0.33, 1.42), scale=(2.8, 0.6, 0.45), rotation=(0, sx * -0.25, 0), mat=eye_b, smooth=True)
        add_cylinder(0.065, 0.38, (sx * 0.16, 0, 0.19), mat=carrot)
        add_cylinder(0.055, 0.50, (sx * 0.38, 0.12, 0.94), rotation=(0.95, 0, sx * -0.2), mat=carrot)

    # Gleaming silver butter-knife held forward (+X, +Y)
    add_cylinder(0.045, 0.28, (0.42, 0.38, 0.86), rotation=(math.pi / 2, 0, 0), mat=handle)
    add_cube(0.12, (0.42, 0.92, 0.87), scale=(0.18, 6.8, 1.05), mat=blade, smooth=True)

    export_glb(os.path.join(output_dir, "carrot_lancer.glb"))


def build_kale_slime():
    clear_scene()
    kale = create_solid_material("KaleMat", (0.18, 0.58, 0.16, 1.0), roughness=0.25)
    kale_dark = create_solid_material("KaleDarkMat", (0.08, 0.34, 0.10, 1.0), roughness=0.35)
    eye_w = create_solid_material("SlimeEyeWMat", (0.99, 0.99, 0.96, 1.0), roughness=0.15)
    eye_b = create_solid_material("SlimeEyeBMat", (0.04, 0.04, 0.05, 1.0), roughness=0.15)
    mouth = create_solid_material("SlimeMouthMat", (0.04, 0.18, 0.06, 1.0), roughness=0.5)

    # Wide glossy slime puddle base
    add_cylinder(0.95, 0.08, (0, 0, 0.04), scale=(1.1, 1.15, 1.0), mat=kale_dark, vertices=28)

    # 8 undulating leafy mound clusters + dripping blobs
    puffs = [
        (0, 0, 0.30, 0.58, (1.15, 1.1, 0.65), kale),
        (-0.42, 0.18, 0.25, 0.36, (1.1, 1.0, 0.72), kale_dark),
        (0.42, 0.16, 0.25, 0.36, (1.0, 1.1, 0.72), kale_dark),
        (0, -0.40, 0.26, 0.38, (1.15, 0.95, 0.72), kale),
        (-0.30, -0.28, 0.24, 0.30, (1.0, 1.0, 0.7), kale_dark),
        (0.32, -0.28, 0.24, 0.30, (1.0, 1.0, 0.7), kale_dark),
        (-0.55, -0.10, 0.12, 0.18, (1.2, 1.0, 0.5), kale),
        (0.55, -0.12, 0.12, 0.18, (1.2, 1.0, 0.5), kale),
    ]
    for x, y, z, r, sc, m in puffs:
        add_ico(r, (x, y, z), scale=sc, mat=m, subdiv=3)

    for sx in (-1, 1):
        add_sphere(0.11, (sx * 0.20, 0.50, 0.42), mat=eye_w)
        add_sphere(0.055, (sx * 0.20, 0.59, 0.42), mat=eye_b)
    add_sphere(0.14, (0, 0.54, 0.24), scale=(1.35, 0.45, 0.65), mat=mouth)

    export_glb(os.path.join(output_dir, "kale_slime.glb"))


def build_honey_mustard_cup():
    clear_scene()
    cup = create_solid_material("CupMat", (0.94, 0.94, 0.96, 1.0), roughness=0.22)
    mustard = create_solid_material("MustardMat", (0.96, 0.74, 0.08, 1.0), roughness=0.28, emission_strength=0.25)
    add_cone(0.28, 0.38, 0.45, (0, 0, 0.35), mat=cup, vertices=28)
    add_cylinder(0.35, 0.08, (0, 0, 0.56), mat=mustard, vertices=28)
    export_glb(os.path.join(output_dir, "honey_mustard_cup.glb"))


def build_ranch_shield():
    clear_scene()
    ranch = create_solid_material("RanchMat", (0.96, 0.98, 0.94, 1.0), roughness=0.25, emission_strength=0.25)
    herb = create_solid_material("HerbMat", (0.16, 0.62, 0.24, 1.0), roughness=0.45, emission_strength=0.2)
    add_sphere(0.38, (0, 0, 0.55), mat=ranch)
    add_torus(0.52, 0.045, (0, 0, 0.55), rotation=(0.35, 0.2, 0), mat=herb)
    add_torus(0.52, 0.045, (0, 0, 0.55), rotation=(-0.35, -0.2, 0.6), mat=herb)
    export_glb(os.path.join(output_dir, "ranch_shield.glb"))


def build_kitchen_sponge():
    clear_scene()
    yellow = create_sponge_yellow_material()
    green = create_solid_material("SpongeGreenMat", (0.11, 0.42, 0.14, 1.0), roughness=0.95)
    add_cube(1.0, (0, 0, 0.30), scale=(2.2, 1.4, 0.60), mat=yellow, uv_scale=1.2)
    add_cube(1.0, (0, 0, 0.70), scale=(2.2, 1.4, 0.22), mat=green)
    export_glb(os.path.join(output_dir, "kitchen_sponge.glb"))


def build_pancake_stack():
    clear_scene()
    plate = create_solid_material("PlateMat", (0.95, 0.96, 0.98, 1.0), roughness=0.18)
    cake = create_solid_material("PancakeMat", (0.76, 0.46, 0.18, 1.0), roughness=0.72)
    edge = create_solid_material("PancakeEdgeMat", (0.86, 0.64, 0.30, 1.0), roughness=0.78)
    butter = create_solid_material("ButterMat", (0.98, 0.86, 0.22, 1.0), roughness=0.32)
    syrup = create_solid_material("SyrupMat", (0.56, 0.20, 0.03, 1.0), roughness=0.10)

    add_cylinder(2.25, 0.14, (0, 0, 0.07), mat=plate, vertices=32)
    add_torus(2.15, 0.10, (0, 0, 0.14), mat=plate)
    for i in range(6):
        z = 0.28 + i * 0.24
        r = 1.56 - i * 0.03
        add_cylinder(r, 0.22, (0, 0, z), mat=cake, vertices=32)
        add_torus(r - 0.04, 0.07, (0, 0, z), mat=edge)

    # Pooling glossy maple syrup + 5 drips down sides
    add_cylinder(1.15, 0.07, (0, 0, 1.62), mat=syrup, vertices=28)
    for i in range(5):
        ang = i * (math.tau / 5.0) + 0.3
        dx = math.cos(ang) * 1.42
        dy = math.sin(ang) * 1.42
        add_cylinder(0.11, 0.85, (dx, dy, 1.15), mat=syrup, vertices=12)
        add_sphere(0.12, (dx, dy, 0.72), mat=syrup)
    # Melting yellow butter pat on top
    add_cube(0.55, (0, 0, 1.74), scale=(1.0, 1.0, 0.35), rotation=(0, 0, 0.4), mat=butter, smooth=True)
    export_glb(os.path.join(output_dir, "pancake_stack.glb"))


def build_plate_with_egg():
    clear_scene()
    plate = create_solid_material("BluePlateMat", (0.76, 0.87, 0.96, 1.0), roughness=0.18)
    white = create_solid_material("EggWhiteMat", (0.98, 0.98, 0.96, 1.0), roughness=0.32)
    yolk = create_solid_material("EggYolkMat", (0.98, 0.62, 0.06, 1.0), roughness=0.20)
    silver = create_solid_material("SilverMat", (0.86, 0.88, 0.92, 1.0), roughness=0.18, metallic=0.92)

    add_cylinder(1.55, 0.12, (0, 0, 0.06), mat=plate, vertices=32)
    add_torus(1.48, 0.09, (0, 0, 0.12), mat=plate)
    add_cylinder(0.88, 0.07, (0.08, 0.04, 0.14), mat=white, vertices=28)
    add_sphere(0.35, (0.08, 0.04, 0.20), scale=(1.0, 1.0, 0.55), mat=yolk)
    # Silver fork & butter knife resting beside the plate
    add_cylinder(0.045, 1.5, (-1.85, 0, 0.06), rotation=(math.pi / 2, 0, 0.08), mat=silver)
    add_cylinder(0.05, 1.6, (1.85, 0, 0.06), rotation=(math.pi / 2, 0, -0.05), mat=silver)
    export_glb(os.path.join(output_dir, "plate_with_egg.glb"))


def build_syrup_bottle():
    clear_scene()
    glass = create_solid_material("BottleGlassMat", (0.48, 0.18, 0.03, 1.0), roughness=0.15)
    cap = create_solid_material("BottleCapMat", (0.84, 0.10, 0.10, 1.0), roughness=0.35)
    label = create_solid_material("BottleLabelMat", (0.95, 0.89, 0.72, 1.0), roughness=0.65)
    add_cylinder(0.65, 1.95, (0, 0, 0.98), mat=glass, vertices=28)
    add_sphere(0.65, (0, 0, 1.95), scale=(1.0, 1.0, 0.45), mat=glass)
    add_cylinder(0.30, 0.65, (0, 0, 2.35), mat=glass, vertices=24)
    add_torus(0.28, 0.07, (0.45, 0, 2.05), rotation=(math.pi / 2, 0, 0), mat=glass)
    add_cylinder(0.34, 0.32, (0, 0, 2.78), mat=cap, vertices=24)
    add_cylinder(0.67, 0.90, (0, 0, 1.00), mat=label, vertices=28)
    export_glb(os.path.join(output_dir, "syrup_bottle.glb"))


def build_salt_shaker():
    clear_scene()
    glass = create_solid_material("ShakerGlassMat", (0.84, 0.90, 0.94, 1.0), roughness=0.15)
    salt = create_solid_material("SaltFillMat", (0.98, 0.98, 0.98, 1.0), roughness=0.85)
    cap = create_solid_material("ShakerCapMat", (0.84, 0.86, 0.90, 1.0), roughness=0.16, metallic=0.92)
    add_cylinder(0.42, 1.08, (0, 0, 0.54), mat=glass, vertices=24)
    add_cylinder(0.36, 0.78, (0, 0, 0.42), mat=salt, vertices=24)
    add_sphere(0.40, (0, 0, 1.10), scale=(1.0, 1.0, 0.72), mat=cap)
    export_glb(os.path.join(output_dir, "salt_shaker.glb"))


def build_kitchen_environment():
    """Complete Giant Kitchen countertop & sunny backdrop matching the video."""
    clear_scene()
    wood = create_butcher_block_material()
    gingham = create_gingham_material()
    wall = create_solid_material("KitchenWallMat", (0.94, 0.91, 0.85, 1.0), roughness=0.72)
    cab = create_solid_material("CabinetWoodMat", (0.48, 0.31, 0.17, 1.0), roughness=0.58)
    steel = create_solid_material("StainlessMat", (0.78, 0.80, 0.84, 1.0), roughness=0.22, metallic=0.88)
    sky = create_solid_material("WindowSkyMat", (0.76, 0.91, 1.0, 1.0), roughness=0.2, emission_strength=0.65)
    red_bowl = create_solid_material("RedBowlMat", (0.84, 0.16, 0.14, 1.0), roughness=0.3)
    blue_bowl = create_solid_material("BlueBowlMat", (0.22, 0.48, 0.82, 1.0), roughness=0.3)
    board_wood = create_solid_material("CuttingBoardMat", (0.72, 0.48, 0.25, 1.0), roughness=0.52)
    mill_wood = create_solid_material("PepperMillMat", (0.58, 0.36, 0.16, 1.0), roughness=0.45)

    # 1. Butcher-block wooden countertop: 14m wide, 44m long, 1m thick, top at Z=0
    add_cube(1.0, (0, 0, -0.5), scale=(14.0, 44.0, 1.0), mat=wood, uv_scale=0.18)

    # 2. Cream backsplash wall at far end (+Y = 22.8 -> Bevy -Z = -22.8)
    add_cube(1.0, (0, 22.8, 6.0), scale=(32.0, 1.0, 14.0), mat=wall)
    # Left backsplash wall ledge
    add_cube(1.0, (-7.8, 0.0, 4.0), scale=(1.0, 46.0, 10.0), mat=wall)

    # 3. Sunny window above the sink
    add_cube(1.0, (0, 22.25, 6.4), scale=(9.6, 0.18, 5.8), mat=sky)
    for mx in (-4.8, 0.0, 4.8):
        add_cube(1.0, (mx, 22.1, 6.4), scale=(0.28, 0.32, 5.9), mat=cab)
    for mz in (3.5, 6.4, 9.3):
        add_cube(1.0, (0, 22.1, mz), scale=(9.8, 0.32, 0.28), mat=cab)

    # Gingham cafe curtains framing the window
    for sx in (-1, 1):
        add_cube(1.0, (sx * 4.1, 21.95, 6.4), scale=(1.9, 0.22, 5.6), mat=gingham, uv_scale=0.6)
    add_cube(1.0, (0, 21.9, 9.0), scale=(9.8, 0.26, 1.1), mat=gingham, uv_scale=0.6)

    # 4. Stainless steel sink & curved chrome gooseneck faucet in front of window
    add_cube(1.0, (0, 20.8, 0.12), scale=(6.2, 2.8, 0.24), mat=steel)
    add_cylinder(0.18, 2.2, (0, 21.8, 1.1), mat=steel)
    add_torus(0.65, 0.14, (0, 21.15, 2.2), rotation=(0, math.pi / 2, 0), mat=steel)

    # 5. Upper kitchen cabinets flanking the window + open shelf bowls/jars
    for sx in (-1, 1):
        add_cube(1.0, (sx * 9.5, 21.5, 6.8), scale=(6.8, 2.4, 6.6), mat=cab)
        add_cube(1.0, (sx * 6.0, 21.5, 3.8), scale=(4.5, 1.6, 0.25), mat=cab)
    add_sphere(0.65, (-6.2, 21.4, 4.3), scale=(1.0, 1.0, 0.65), mat=red_bowl)
    add_sphere(0.55, (-4.8, 21.4, 4.25), scale=(1.0, 1.0, 0.65), mat=blue_bowl)
    add_cylinder(0.45, 1.3, (5.5, 21.4, 4.55), mat=red_bowl)

    # 6. Leaning wooden cutting boards & tall pepper mill along left edge (-X)
    for by, bh in [(14.0, 3.8), (-6.0, 4.4)]:
        add_cube(1.0, (-6.7, by, bh * 0.5), scale=(0.28, 2.6, bh), rotation=(0, -0.14, 0), mat=board_wood)
    # Tall wooden pepper mill
    add_cylinder(0.55, 2.8, (-5.9, 10.5, 1.4), mat=mill_wood)
    add_sphere(0.62, (-5.9, 10.5, 3.0), mat=mill_wood)

    export_glb(os.path.join(output_dir, "kitchen_environment.glb"))


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
    build_kitchen_environment()
    print("Successfully exported all 13 Tendy Quest .glb assets to:", output_dir)
