import bpy
import math
import os

# Clear existing scene
bpy.ops.wm.read_factory_settings(use_empty=True)

output_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "assets", "models"))
os.makedirs(output_dir, exist_ok=True)


def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)


def create_plaid_material(name="PlaidOnesieMat"):
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.85

    tex_coord = nodes.new("ShaderNodeTexCoord")
    checker1 = nodes.new("ShaderNodeTexChecker")
    checker1.inputs["Scale"].default_value = 14.0
    checker1.inputs["Color1"].default_value = (0.65, 0.04, 0.06, 1.0)  # Crimson Red
    checker1.inputs["Color2"].default_value = (0.04, 0.04, 0.04, 1.0)  # Charcoal Black

    links.new(tex_coord.outputs["Object"], checker1.inputs["Vector"])
    links.new(checker1.outputs["Color"], bsdf.inputs["Base Color"])
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    return mat


def create_solid_material(name, color, roughness=0.5, emission_strength=0.0):
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = roughness
    if emission_strength > 0.0:
        bsdf.inputs["Emission Color"].default_value = color
        bsdf.inputs["Emission Strength"].default_value = emission_strength
    return mat


def export_glb(filepath):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format='GLB',
        use_selection=True,
        export_apply=True
    )


def build_plaid_hero():
    clear_scene()
    plaid_mat = create_plaid_material()
    skin_mat = create_solid_material("SkinMat", (0.87, 0.67, 0.54, 1.0), roughness=0.6)
    hair_mat = create_solid_material("HairMat", (0.28, 0.19, 0.12, 1.0), roughness=0.8)
    shoe_mat = create_solid_material("ShoeMat", (0.12, 0.12, 0.14, 1.0), roughness=0.7)

    # Torso (stocky capsule/cylinder)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.55, location=(0, 0, 1.05))
    torso = bpy.context.active_object
    torso.scale = (1.05, 0.85, 1.25)
    torso.data.materials.append(plaid_mat)

    # Head
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.32, location=(0, 0, 1.92))
    head = bpy.context.active_object
    head.data.materials.append(skin_mat)

    # Hair cap
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.34, location=(0, -0.03, 1.98))
    hair = bpy.context.active_object
    hair.scale = (1.02, 1.05, 0.85)
    hair.data.materials.append(hair_mat)

    # Beard
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.29, location=(0, 0.08, 1.82))
    beard = bpy.context.active_object
    beard.scale = (0.98, 0.95, 0.72)
    beard.data.materials.append(hair_mat)

    # Arms (T-pose)
    for side in (-1, 1):
        bpy.ops.mesh.primitive_cylinder_add(radius=0.14, depth=0.75, location=(side * 0.85, 0, 1.38), rotation=(0, math.pi / 2, 0))
        arm = bpy.context.active_object
        arm.data.materials.append(plaid_mat)

        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.13, location=(side * 1.26, 0, 1.38))
        hand = bpy.context.active_object
        hand.data.materials.append(skin_mat)

        # Legs
        bpy.ops.mesh.primitive_cylinder_add(radius=0.18, depth=0.65, location=(side * 0.25, 0, 0.42))
        leg = bpy.context.active_object
        leg.data.materials.append(plaid_mat)

        # Shoes
        bpy.ops.mesh.primitive_cube_add(size=0.3, location=(side * 0.25, 0.08, 0.08))
        shoe = bpy.context.active_object
        shoe.scale = (0.7, 1.25, 0.45)
        shoe.data.materials.append(shoe_mat)

    export_glb(os.path.join(output_dir, "plaid_hero.glb"))


def build_golden_tendy():
    clear_scene()
    tendy_mat = create_solid_material("GoldenTendyMat", (0.95, 0.62, 0.12, 1.0), roughness=0.45, emission_strength=0.25)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.35, location=(0, 0, 0.5))
    tendy = bpy.context.active_object
    tendy.scale = (0.65, 1.65, 0.48)
    tendy.rotation_euler = (0.25, 0.2, 0.4)
    tendy.data.materials.append(tendy_mat)
    export_glb(os.path.join(output_dir, "golden_tendy.glb"))


def build_broccoli_brute():
    clear_scene()
    stalk_mat = create_solid_material("StalkMat", (0.45, 0.72, 0.28, 1.0), roughness=0.6)
    crown_mat = create_solid_material("CrownMat", (0.09, 0.36, 0.11, 1.0), roughness=0.9)

    # Thick stalk torso
    bpy.ops.mesh.primitive_cylinder_add(radius=0.42, depth=1.1, location=(0, 0, 0.85))
    stalk = bpy.context.active_object
    stalk.data.materials.append(stalk_mat)

    # Leafy floret crown puffs
    offsets = [(0, 0, 1.65, 0.65), (-0.48, 0.1, 1.5, 0.45), (0.48, 0.1, 1.5, 0.45), (0, -0.38, 1.55, 0.45)]
    for x, y, z, r in offsets:
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=r, location=(x, y, z))
        puff = bpy.context.active_object
        puff.data.materials.append(crown_mat)

    # Stalk legs
    for side in (-1, 1):
        bpy.ops.mesh.primitive_cylinder_add(radius=0.16, depth=0.45, location=(side * 0.22, 0, 0.22))
        leg = bpy.context.active_object
        leg.data.materials.append(stalk_mat)

    export_glb(os.path.join(output_dir, "broccoli_brute.glb"))


def build_honey_mustard_cup():
    clear_scene()
    cup_mat = create_solid_material("CupMat", (0.92, 0.92, 0.95, 1.0), roughness=0.25)
    mustard_mat = create_solid_material("HoneyMustardMat", (0.95, 0.72, 0.08, 1.0), roughness=0.3, emission_strength=0.2)

    # Cup body
    bpy.ops.mesh.primitive_cone_add(vertices=24, radius1=0.28, radius2=0.38, depth=0.45, location=(0, 0, 0.35))
    cup = bpy.context.active_object
    cup.data.materials.append(cup_mat)

    # Honey mustard fill
    bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.35, depth=0.08, location=(0, 0, 0.56))
    fill = bpy.context.active_object
    fill.data.materials.append(mustard_mat)

    export_glb(os.path.join(output_dir, "honey_mustard_cup.glb"))


def build_ranch_shield():
    clear_scene()
    ranch_mat = create_solid_material("RanchMat", (0.96, 0.97, 0.94, 1.0), roughness=0.35, emission_strength=0.25)
    herb_mat = create_solid_material("HerbMat", (0.18, 0.52, 0.22, 1.0), roughness=0.7)

    # Creamy orb
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.42, location=(0, 0, 0.55))
    orb = bpy.context.active_object
    orb.data.materials.append(ranch_mat)

    # Ring / shield halo
    bpy.ops.mesh.primitive_torus_add(major_radius=0.52, minor_radius=0.05, location=(0, 0, 0.55), rotation=(0.35, 0.2, 0))
    ring = bpy.context.active_object
    ring.data.materials.append(herb_mat)

    export_glb(os.path.join(output_dir, "ranch_shield.glb"))


def build_carrot_lancer():
    clear_scene()
    carrot_mat = create_solid_material("CarrotMat", (0.96, 0.42, 0.06, 1.0), roughness=0.45)
    leaf_mat = create_solid_material("CarrotLeafMat", (0.14, 0.62, 0.18, 1.0), roughness=0.7)
    eye_mat = create_solid_material("EyeMat", (0.05, 0.05, 0.05, 1.0), roughness=0.3)

    # Tapered carrot cone body (pointing slightly forward/up)
    bpy.ops.mesh.primitive_cone_add(vertices=20, radius1=0.42, radius2=0.05, depth=1.6, location=(0, 0, 0.95), rotation=(math.pi, 0, 0))
    body = bpy.context.active_object
    body.data.materials.append(carrot_mat)

    # Leafy green hair tuft on top
    for angle in (-0.35, 0.0, 0.35):
        bpy.ops.mesh.primitive_cone_add(vertices=10, radius1=0.14, radius2=0.02, depth=0.55, location=(angle * 0.4, 0, 1.92), rotation=(0.15, angle, 0))
        leaf = bpy.context.active_object
        leaf.data.materials.append(leaf_mat)

    # Eyes
    for side in (-1, 1):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.06, location=(side * 0.14, 0.32, 1.35))
        eye = bpy.context.active_object
        eye.data.materials.append(eye_mat)

    export_glb(os.path.join(output_dir, "carrot_lancer.glb"))


def build_kale_slime():
    clear_scene()
    kale_dark = create_solid_material("KaleDarkMat", (0.07, 0.24, 0.1, 1.0), roughness=0.85)
    kale_mid = create_solid_material("KaleMidMat", (0.14, 0.38, 0.15, 1.0), roughness=0.8)
    eye_mat = create_solid_material("SlimeEyeMat", (0.95, 0.92, 0.25, 1.0), roughness=0.3, emission_strength=0.3)

    # Undulating mound of leafy green clusters
    puffs = [
        (0.0, 0.0, 0.32, 0.65, (1.2, 1.15, 0.65), kale_dark),
        (-0.42, 0.18, 0.28, 0.38, (1.1, 1.0, 0.75), kale_mid),
        (0.45, 0.12, 0.26, 0.36, (1.0, 1.1, 0.7), kale_mid),
        (0.05, -0.4, 0.3, 0.4, (1.15, 0.95, 0.75), kale_dark),
        (-0.2, 0.38, 0.25, 0.3, (1.0, 1.0, 0.8), kale_mid),
    ]
    for x, y, z, r, sc, mat in puffs:
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=r, location=(x, y, z))
        puff = bpy.context.active_object
        puff.scale = sc
        puff.data.materials.append(mat)

    # Glowing grumpy eyes
    for side in (-1, 1):
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.07, location=(side * 0.2, 0.58, 0.38))
        eye = bpy.context.active_object
        eye.data.materials.append(eye_mat)

    export_glb(os.path.join(output_dir, "kale_slime.glb"))


def build_kitchen_stool():
    clear_scene()
    cushion_mat = create_solid_material("StoolCushionMat", (0.78, 0.14, 0.16, 1.0), roughness=0.5)
    chrome_mat = create_solid_material("ChromeMat", (0.78, 0.8, 0.84, 1.0), roughness=0.2)

    # Seat cushion
    bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.95, depth=0.28, location=(0, 0, 0.96))
    seat = bpy.context.active_object
    seat.data.materials.append(cushion_mat)

    # 4 legs
    for sx in (-1, 1):
        for sy in (-1, 1):
            bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.08, depth=0.88, location=(sx * 0.55, sy * 0.55, 0.44))
            leg = bpy.context.active_object
            leg.data.materials.append(chrome_mat)

    export_glb(os.path.join(output_dir, "kitchen_stool.glb"))


def build_giant_toaster():
    clear_scene()
    chrome_mat = create_solid_material("ToasterChromeMat", (0.82, 0.84, 0.88, 1.0), roughness=0.2)
    slot_mat = create_solid_material("ToasterSlotMat", (0.08, 0.08, 0.09, 1.0), roughness=0.9)
    toast_mat = create_solid_material("ToastMat", (0.76, 0.52, 0.24, 1.0), roughness=0.85)

    # Toaster body
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 0.7))
    body = bpy.context.active_object
    body.scale = (2.2, 1.4, 1.4)
    body.data.materials.append(chrome_mat)

    # Two toast slots & toast slices popping out
    for sy in (-0.32, 0.32):
        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, sy, 1.38))
        slot = bpy.context.active_object
        slot.scale = (1.5, 0.22, 0.08)
        slot.data.materials.append(slot_mat)

        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, sy, 1.55))
        toast = bpy.context.active_object
        toast.scale = (1.35, 0.14, 0.65)
        toast.data.materials.append(toast_mat)

    export_glb(os.path.join(output_dir, "giant_toaster.glb"))


def build_giant_stove():
    clear_scene()
    body_mat = create_solid_material("StoveBodyMat", (0.22, 0.23, 0.26, 1.0), roughness=0.35)
    burner_mat = create_solid_material("BurnerMat", (0.06, 0.06, 0.06, 1.0), roughness=0.7)
    glass_mat = create_solid_material("OvenGlassMat", (0.1, 0.12, 0.16, 1.0), roughness=0.15)

    # Main stove body
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 1.25))
    body = bpy.context.active_object
    body.scale = (3.8, 3.6, 2.5)
    body.data.materials.append(body_mat)

    # 4 stovetop burners
    for sx in (-0.95, 0.95):
        for sy in (-0.85, 0.85):
            bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.55, depth=0.08, location=(sx, sy, 2.54))
            burner = bpy.context.active_object
            burner.data.materials.append(burner_mat)

    # Oven window on front (+Y)
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 1.82, 1.1))
    window = bpy.context.active_object
    window.scale = (2.6, 0.08, 1.2)
    window.data.materials.append(glass_mat)

    export_glb(os.path.join(output_dir, "giant_stove.glb"))


if __name__ == "__main__":
    build_plaid_hero()
    build_golden_tendy()
    build_broccoli_brute()
    build_honey_mustard_cup()
    build_ranch_shield()
    build_carrot_lancer()
    build_kale_slime()
    build_kitchen_stool()
    build_giant_toaster()
    build_giant_stove()
    print("Successfully exported Tendy Quest .glb assets to:", output_dir)
