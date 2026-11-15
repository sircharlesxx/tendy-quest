import math
import os
import bpy
import numpy as np

bpy.ops.wm.read_factory_settings(use_empty=True)

OUTPUT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "assets", "models"))
os.makedirs(OUTPUT_DIR, exist_ok=True)


def clear_scene():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for col in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
        for item in list(col):
            if item.users == 0:
                col.remove(item)


def set_smooth(obj, smooth=True):
    if obj and obj.type == "MESH" and obj.data:
        for poly in obj.data.polygons:
            poly.use_smooth = smooth


def apply_box_uv(obj, scale=1.0, smooth=True):
    """Project UVs using world-scaled coordinates so scaled meshes keep true-scale textures."""
    mesh = obj.data
    if not mesh.uv_layers:
        mesh.uv_layers.new(name="UVMap")
    uv_layer = mesh.uv_layers.active.data
    sx, sy, sz = obj.scale.x, obj.scale.y, obj.scale.z
    ix = 1.0 / max(abs(sx), 1e-4)
    iy = 1.0 / max(abs(sy), 1e-4)
    iz = 1.0 / max(abs(sz), 1e-4)
    if isinstance(scale, (tuple, list)):
        su, sv = scale[0], scale[1]
    else:
        su = sv = scale

    for poly in mesh.polygons:
        n = poly.normal
        ax, ay, az = abs(n.x) * ix, abs(n.y) * iy, abs(n.z) * iz
        for loop_idx in poly.loop_indices:
            v = mesh.vertices[mesh.loops[loop_idx].vertex_index].co
            vx, vy, vz = v.x * sx, v.y * sy, v.z * sz
            if az >= ax and az >= ay:
                u, w = vx, vy
            elif ax >= ay and ax >= az:
                u, w = vy, vz
            else:
                u, w = vx, vz
            uv_layer[loop_idx].uv = (u * su + 0.5, w * sv + 0.5)
    set_smooth(obj, smooth=smooth)


def fbm_noise(size, freqs=(4, 8, 16, 32, 64), weights=(0.40, 0.27, 0.17, 0.10, 0.06), seed=0):
    """Seamless tileable multi-octave smoothstep noise in pure NumPy."""
    rng = np.random.default_rng(seed)
    acc = np.zeros((size, size), dtype=np.float32)
    coords = np.arange(size, dtype=np.float32)
    total_w = 0.0
    for f, w in zip(freqs, weights):
        grid = rng.random((f, f), dtype=np.float32)
        g = (coords / size) * f
        i0 = np.floor(g).astype(np.int32) % f
        i1 = (i0 + 1) % f
        t = g - np.floor(g)
        t = t * t * (3.0 - 2.0 * t)
        tx = t[None, :]
        ty = t[:, None]
        iy0, iy1 = i0[:, None], i1[:, None]
        ix0, ix1 = i0[None, :], i1[None, :]
        layer = (
            (grid[iy0, ix0] * (1.0 - tx) + grid[iy0, ix1] * tx) * (1.0 - ty)
            + (grid[iy1, ix0] * (1.0 - tx) + grid[iy1, ix1] * tx) * ty
        )
        acc += w * layer
        total_w += w
    return np.clip(acc / max(total_w, 1e-6), 0.0, 1.0)


def _pack_rgba_image(name, rgb, colorspace="sRGB"):
    h, w, _ = rgb.shape
    rgba = np.empty((h, w, 4), dtype=np.float32)
    rgba[..., :3] = np.clip(rgb, 0.0, 1.0)
    rgba[..., 3] = 1.0
    img = bpy.data.images.new(name=name, width=w, height=h, alpha=False)
    img.colorspace_settings.name = colorspace
    img.pixels.foreach_set(rgba.ravel())
    img.pack()
    return img


def make_numpy_pbr_material(
    name,
    rgb_arr,
    height_arr=None,
    roughness=0.5,
    metallic=0.0,
    normal_strength=2.5,
    emission_strength=0.0,
):
    """Build a Principled BSDF material with packed Base Color and tangent-space Normal Map."""
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic

    albedo_img = _pack_rgba_image(f"{name}_BaseColor", rgb_arr, colorspace="sRGB")
    tex_col = nodes.new("ShaderNodeTexImage")
    tex_col.image = albedo_img
    links.new(tex_col.outputs["Color"], bsdf.inputs["Base Color"])

    if emission_strength > 0.0:
        links.new(tex_col.outputs["Color"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = emission_strength

    if height_arr is not None:
        dx = np.roll(height_arr, -1, axis=1) - np.roll(height_arr, 1, axis=1)
        dy = np.roll(height_arr, -1, axis=0) - np.roll(height_arr, 1, axis=0)
        nx = -dx * normal_strength
        ny = -dy * normal_strength
        nz = np.ones_like(nx)
        inv_len = 1.0 / np.sqrt(nx * nx + ny * ny + nz * nz)
        norm_rgb = np.stack(
            [0.5 + 0.5 * nx * inv_len, 0.5 + 0.5 * ny * inv_len, 0.5 + 0.5 * nz * inv_len],
            axis=-1,
        )
        norm_img = _pack_rgba_image(f"{name}_Normal", norm_rgb, colorspace="Non-Color")
        tex_norm = nodes.new("ShaderNodeTexImage")
        tex_norm.image = norm_img
        norm_node = nodes.new("ShaderNodeNormalMap")
        norm_node.inputs["Strength"].default_value = 1.0
        links.new(tex_norm.outputs["Color"], norm_node.inputs["Color"])
        links.new(norm_node.outputs["Normal"], bsdf.inputs["Normal"])

    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    return mat


def create_solid_material(name, color, roughness=0.45, metallic=0.0, emission_strength=0.0):
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


# --- High-Resolution NumPy PBR Texture Materials ---

def mat_butcher_block():
    s = 1024
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n_coarse = fbm_noise(s, freqs=(6, 12, 24), weights=(0.5, 0.3, 0.2), seed=101)
    n_fine = fbm_noise(s, freqs=(24, 64, 128), weights=(0.4, 0.35, 0.25), seed=202)

    strips = 8
    strip_w = s / strips
    strip_id = np.floor(x / strip_w).astype(np.int32)
    stagger = ((strip_id * 373) % s).astype(np.float32)
    planks_y = 3
    plank_h = s / planks_y
    y_staggered = (y + stagger) % s
    plank_id = np.floor(y_staggered / plank_h).astype(np.int32)

    rng = np.random.default_rng(77)
    plank_tones = rng.uniform(-0.06, 0.06, size=(strips, planks_y)).astype(np.float32)
    tone = plank_tones[strip_id % strips, plank_id % planks_y]

    # Wood fibers running vertically along Y (countertop length)
    ring_phase = (
        (x / s) * 85.0
        + strip_id * 4.7
        + n_coarse * 7.5
        + np.sin((y_staggered / s) * 14.0 + plank_id) * 1.4
    )
    fibers = 0.5 + 0.5 * np.sin(ring_phase)
    pores = (n_fine - 0.5) * 0.14

    fx = (x % strip_w) / strip_w
    fy = (y_staggered % plank_h) / plank_h
    seam_mask = np.maximum(
        np.exp(-((np.minimum(fx, 1.0 - fx) * strip_w) ** 2) / 3.2),
        np.exp(-((np.minimum(fy, 1.0 - fy) * plank_h) ** 2) / 3.2),
    ).astype(np.float32)

    base = 0.78 + tone + (fibers - 0.5) * 0.09 + pores * 0.6 - seam_mask * 0.11
    rgb = np.stack(
        [
            base * 1.12,
            base * 0.84 - seam_mask * 0.02,
            base * 0.50 - seam_mask * 0.02,
        ],
        axis=-1,
    )
    height = (fibers * 0.12 + n_fine * 0.12 - seam_mask * 0.45).astype(np.float32)
    return make_numpy_pbr_material("ButcherBlockMat", rgb, height, roughness=0.26, normal_strength=1.8)


def mat_sage_tile():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(8, 16, 48), weights=(0.5, 0.3, 0.2), seed=11)
    tiles = 4
    tw = s / tiles
    fx = (x % tw) / tw
    fy = (y % tw) / tw
    edge_dist = np.minimum(np.minimum(fx, 1.0 - fx), np.minimum(fy, 1.0 - fy)) * tw
    grout = 1.0 / (1.0 + np.exp((edge_dist - 3.2) * 1.6))

    tile_idx = np.floor(x / tw).astype(np.int32) + np.floor(y / tw).astype(np.int32) * tiles
    rng = np.random.default_rng(19)
    tile_var = rng.uniform(-0.03, 0.03, size=tiles * tiles).astype(np.float32)[tile_idx % (tiles * tiles)]

    glaze = (n - 0.5) * 0.04 + tile_var
    tile_col = np.stack([0.50 + glaze, 0.61 + glaze, 0.50 + glaze], axis=-1)
    grout_col = np.array([0.84, 0.83, 0.79], dtype=np.float32)[None, None, :]
    rgb = tile_col * (1.0 - grout[..., None]) + grout_col * grout[..., None]
    height = (1.0 - grout) * 0.65 + n * 0.06
    return make_numpy_pbr_material("SageTileMat", rgb, height, roughness=0.16, normal_strength=6.0)


def mat_gingham():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    checks = 8
    cw = s / checks
    cx = (np.floor(x / cw).astype(np.int32) % 2) == 0
    cy = (np.floor(y / cw).astype(np.int32) % 2) == 0

    weave_u = 0.5 + 0.5 * np.sin(x * math.pi * 0.5)
    weave_v = 0.5 + 0.5 * np.sin(y * math.pi * 0.5)
    weave = 0.92 + 0.08 * ((weave_u + weave_v) * 0.5)

    rgb = np.empty((s, s, 3), dtype=np.float32)
    both = cx & cy
    neither = (~cx) & (~cy)
    mixed = cx ^ cy
    rgb[both] = [0.96, 0.95, 0.93]
    rgb[neither] = [0.78, 0.12, 0.13]
    rgb[mixed] = [0.89, 0.48, 0.46]
    rgb *= weave[..., None]
    height = (weave_u * 0.5 + weave_v * 0.5).astype(np.float32)
    return make_numpy_pbr_material("GinghamCurtainMat", rgb, height, roughness=0.86, normal_strength=2.2)


def mat_plaid_onesie():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(16, 48), weights=(0.55, 0.45), seed=5)
    sq = s / 4.0
    ix = (np.floor(x / sq).astype(np.int32) % 2) == 0
    iy = (np.floor(y / sq).astype(np.int32) % 2) == 0
    twill = ((np.floor((x + y) / 3.0).astype(np.int32)) % 2) == 0

    rgb = np.empty((s, s, 3), dtype=np.float32)
    rgb[ix & iy] = [0.76, 0.07, 0.10]
    rgb[(~ix) & (~iy)] = [0.05, 0.05, 0.06]
    rgb[(ix ^ iy) & twill] = [0.48, 0.05, 0.07]
    rgb[(ix ^ iy) & (~twill)] = [0.09, 0.03, 0.04]
    rgb *= (0.92 + 0.14 * n)[..., None]

    height = (0.35 * twill.astype(np.float32) + 0.35 * n).astype(np.float32)
    return make_numpy_pbr_material("PlaidOnesieMat", rgb, height, roughness=0.85, normal_strength=2.8)


def mat_golden_tendy():
    s = 512
    n1 = fbm_noise(s, freqs=(8, 16, 32, 64, 128), weights=(0.15, 0.25, 0.28, 0.20, 0.12), seed=42)
    n2 = fbm_noise(s, freqs=(24, 48, 96, 160), weights=(0.30, 0.30, 0.25, 0.15), seed=99)
    crisp = np.clip((n1 * 0.55 + n2 * 0.45), 0.0, 1.0)
    dark_spots = np.clip((crisp - 0.58) * 2.4, 0.0, 1.0)
    light_flakes = np.clip((0.35 - crisp) * 2.2, 0.0, 1.0)

    r = np.clip(0.90 + light_flakes * 0.10 - dark_spots * 0.25, 0.0, 1.0)
    g = np.clip(0.55 + light_flakes * 0.20 - dark_spots * 0.28, 0.0, 1.0)
    b = np.clip(0.11 + light_flakes * 0.10 - dark_spots * 0.08, 0.0, 1.0)
    rgb = np.stack([r, g, b], axis=-1)
    return make_numpy_pbr_material(
        "GoldenTendyMat", rgb, crisp, roughness=0.42, normal_strength=7.5, emission_strength=0.22
    )


def mat_broccoli_crown():
    s = 512
    n1 = fbm_noise(s, freqs=(12, 24, 48, 96), weights=(0.25, 0.30, 0.28, 0.17), seed=303)
    n2 = fbm_noise(s, freqs=(32, 64, 128), weights=(0.35, 0.40, 0.25), seed=404)
    buds = np.clip(np.sin(n2 * 18.0) * 0.5 + 0.5, 0.0, 1.0)
    h = np.clip(n1 * 0.55 + buds * 0.45, 0.0, 1.0)
    rgb = np.stack(
        [
            0.07 + h * 0.14,
            0.26 + h * 0.30,
            0.06 + h * 0.09,
        ],
        axis=-1,
    )
    return make_numpy_pbr_material("BroccoliCrownMat", rgb, h, roughness=0.78, normal_strength=7.0)


def mat_broccoli_stalk():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(8, 24, 64), weights=(0.45, 0.35, 0.20), seed=55)
    fibers = 0.5 + 0.5 * np.sin((x / s) * 90.0 + n * 5.0)
    h = fibers * 0.35 + n * 0.65
    rgb = np.stack(
        [
            0.34 + h * 0.18,
            0.60 + h * 0.22,
            0.16 + h * 0.12,
        ],
        axis=-1,
    )
    return make_numpy_pbr_material("BroccoliStalkMat", rgb, h, roughness=0.48, normal_strength=3.5)


def mat_carrot_skin():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(8, 20, 48), weights=(0.45, 0.35, 0.20), seed=88)
    rings = np.sin((y / s) * 34.0 + n * 3.0)
    creases = np.clip(-rings - 0.55, 0.0, 1.0) * 2.2
    rgb = np.stack(
        [
            0.94 - creases * 0.18 + (n - 0.5) * 0.08,
            0.38 - creases * 0.16 + (n - 0.5) * 0.08,
            0.04 + (n - 0.5) * 0.03,
        ],
        axis=-1,
    )
    h = (0.7 - creases * 0.65 + n * 0.2).astype(np.float32)
    return make_numpy_pbr_material("CarrotSkinMat", rgb, h, roughness=0.42, normal_strength=4.5)


def mat_kale_slime():
    s = 512
    n1 = fbm_noise(s, freqs=(6, 14, 32, 64), weights=(0.35, 0.30, 0.22, 0.13), seed=61)
    bubbles = np.clip(n1 - 0.62, 0.0, 1.0) * 2.5
    rgb = np.stack(
        [
            0.14 + bubbles * 0.18,
            0.52 + bubbles * 0.28,
            0.10 + bubbles * 0.12,
        ],
        axis=-1,
    )
    h = np.clip(n1 * 0.5 + bubbles * 0.5, 0.0, 1.0)
    return make_numpy_pbr_material("KaleSlimeMat", rgb, h, roughness=0.10, normal_strength=5.5)


def mat_sponge_yellow():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(12, 32, 72), weights=(0.4, 0.35, 0.25), seed=77)
    pores = np.zeros((s, s), dtype=np.float32)
    rng = np.random.default_rng(77)
    for _ in range(170):
        px = rng.uniform(0, s)
        py = rng.uniform(0, s)
        pr = rng.uniform(3.5, 13.0)
        dx = np.minimum(np.abs(x - px), s - np.abs(x - px))
        dy = np.minimum(np.abs(y - py), s - np.abs(y - py))
        d2 = (dx * dx + dy * dy) / (pr * pr)
        pores = np.maximum(pores, np.clip(1.0 - d2, 0.0, 1.0))

    rgb = np.stack(
        [
            0.94 * (1.0 - pores * 0.45) + (n - 0.5) * 0.08,
            0.78 * (1.0 - pores * 0.50) + (n - 0.5) * 0.08,
            0.18 * (1.0 - pores * 0.65),
        ],
        axis=-1,
    )
    h = (0.75 - pores * 0.72 + n * 0.15).astype(np.float32)
    return make_numpy_pbr_material("SpongeYellowMat", rgb, h, roughness=0.92, normal_strength=6.5)


def mat_sponge_green():
    s = 512
    n = fbm_noise(s, freqs=(20, 48, 96, 160), weights=(0.2, 0.3, 0.3, 0.2), seed=89)
    rgb = np.stack(
        [
            0.09 + n * 0.09,
            0.34 + n * 0.16,
            0.11 + n * 0.07,
        ],
        axis=-1,
    )
    return make_numpy_pbr_material("SpongeGreenMat", rgb, n, roughness=0.96, normal_strength=6.5)


def mat_pancake():
    s = 512
    n = fbm_noise(s, freqs=(8, 20, 48, 96), weights=(0.35, 0.28, 0.22, 0.15), seed=51)
    brown = np.clip((n - 0.42) * 1.9, 0.0, 1.0)
    rgb = np.stack(
        [
            0.84 - brown * 0.26,
            0.60 - brown * 0.26,
            0.28 - brown * 0.16,
        ],
        axis=-1,
    )
    return make_numpy_pbr_material("PancakeMat", rgb, n, roughness=0.72, normal_strength=3.2)


def mat_cutting_board():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    n = fbm_noise(s, freqs=(6, 18, 48), weights=(0.5, 0.3, 0.2), seed=14)
    grain = 0.5 + 0.5 * np.sin((x / s) * 45.0 + n * 6.0)
    rgb = np.stack(
        [
            0.76 + (grain - 0.5) * 0.14 + (n - 0.5) * 0.08,
            0.54 + (grain - 0.5) * 0.12 + (n - 0.5) * 0.06,
            0.30 + (grain - 0.5) * 0.07,
        ],
        axis=-1,
    )
    return make_numpy_pbr_material("CuttingBoardMat", rgb, grain * 0.4 + n * 0.6, roughness=0.45, normal_strength=2.5)


def mat_syrup_label():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    v = y / s
    # Warm cream parchment label with dark-red top/bottom trim borders & golden-orange center band
    rgb = np.full((s, s, 3), [0.96, 0.90, 0.74], dtype=np.float32)
    border = (v < 0.16) | (v > 0.84)
    rgb[border] = [0.76, 0.20, 0.06]
    center_band = (v > 0.38) & (v < 0.62)
    rgb[center_band] = [0.88, 0.46, 0.09]
    n = fbm_noise(s, freqs=(16, 48), weights=(0.6, 0.4), seed=31)
    rgb *= (0.94 + 0.1 * n)[..., None]
    return make_numpy_pbr_material("SyrupLabelMat", rgb, n, roughness=0.65, normal_strength=1.5)


def mat_window_view():
    s = 512
    y, x = np.mgrid[0:s, 0:s].astype(np.float32)
    u, v = x / s, y / s
    n = fbm_noise(s, freqs=(4, 8, 16, 32), weights=(0.45, 0.30, 0.15, 0.10), seed=19)

    # Sky gradient: soft light blue at top blending to warm sunlit cream near horizon
    t_sky = np.clip((v - 0.35) / 0.65, 0.0, 1.0)[..., None]
    sky_low = np.array([0.98, 0.96, 0.86], dtype=np.float32)[None, None, :]
    sky_high = np.array([0.84, 0.94, 1.00], dtype=np.float32)[None, None, :]
    rgb = sky_low * (1.0 - t_sky) + sky_high * t_sky

    # Soft out-of-focus sunlit green garden foliage on the lower half
    foliage_edge = 0.48 + 0.08 * np.sin(u * 10.0) + (n - 0.5) * 0.10
    in_foliage = np.clip((foliage_edge - v) * 18.0, 0.0, 1.0)[..., None]
    green_dark = np.array([0.46, 0.72, 0.36], dtype=np.float32)[None, None, :]
    green_sun = np.array([0.68, 0.86, 0.50], dtype=np.float32)[None, None, :]
    foliage = green_dark * (1.0 - n[..., None]) + green_sun * n[..., None]
    rgb = rgb * (1.0 - in_foliage) + foliage * in_foliage

    return make_numpy_pbr_material("WindowViewMat", rgb, None, roughness=0.2, emission_strength=0.85)


# --- Geometry Helpers ---

def add_sphere(radius, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.2, seg=28, ring=18):
    bpy.ops.mesh.primitive_uv_sphere_add(
        segments=seg, ring_count=ring, radius=radius, location=location, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=True)
    return obj


def add_ico(radius, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, subdiv=3, uv_scale=1.2):
    bpy.ops.mesh.primitive_ico_sphere_add(
        subdivisions=subdiv, radius=radius, location=location, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=True)
    return obj


def add_cylinder(radius, depth, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=1.2, vertices=28):
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=vertices, radius=radius, depth=depth, location=location, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=True)
    return obj


def add_cube(size, location, scale=(1, 1, 1), rotation=(0, 0, 0), mat=None, uv_scale=0.5, smooth=False, bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=size, location=location, rotation=rotation)
    obj = bpy.context.active_object
    obj.scale = scale
    if bevel > 0.0:
        mod = obj.modifiers.new(name="Bevel", type="BEVEL")
        mod.width = bevel
        mod.segments = 2
        bpy.ops.object.modifier_apply(modifier=mod.name)
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=smooth)
    return obj


def add_cone(r1, r2, depth, location, rotation=(0, 0, 0), scale=(1, 1, 1), mat=None, vertices=24, uv_scale=1.2):
    bpy.ops.mesh.primitive_cone_add(
        vertices=vertices, radius1=r1, radius2=r2, depth=depth, location=location, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=True)
    return obj


def add_torus(major_r, minor_r, location, rotation=(0, 0, 0), scale=(1, 1, 1), mat=None, uv_scale=1.2):
    bpy.ops.mesh.primitive_torus_add(
        major_segments=32, minor_segments=16, major_radius=major_r, minor_radius=minor_r, location=location, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.scale = scale
    if mat:
        obj.data.materials.append(mat)
    apply_box_uv(obj, uv_scale, smooth=True)
    return obj


def displace_organic(obj, freq=6.0, amp=0.10, seed=1):
    """Apply smooth seeded trigonometric + random harmonic vertex displacement."""
    mesh = obj.data
    rng = np.random.default_rng(seed)
    ph1, ph2, ph3 = rng.uniform(0, math.tau, 3)
    for v in mesh.vertices:
        p = v.co
        n = (
            math.sin(p.x * freq + ph1)
            * math.cos(p.y * freq + ph2)
            * math.sin(p.z * freq + ph3)
            + 0.5 * math.sin((p.x + p.y) * freq * 1.9 + ph2)
        )
        v.co *= 1.0 + amp * n
    mesh.update()
    set_smooth(obj, smooth=True)


def add_dinner_fork(x, y, z, rot_z=0.0, scale_s=1.0, mat=None, upright=False):
    """Detailed 4-tine stainless steel fork."""
    def pt(lx, ly, lz):
        ca, sa = math.cos(rot_z), math.sin(rot_z)
        rx = lx * ca - ly * sa
        ry = lx * sa + ly * ca
        return (x + rx * scale_s, y + ry * scale_s, z + lz * scale_s)

    if upright:
        # Held vertically in carrot hand pointing forward/up (+Z / +Y)
        add_cylinder(0.032 * scale_s, 0.65 * scale_s, (x, y, z + 0.30 * scale_s), mat=mat, vertices=16)
        add_cube(0.06 * scale_s, (x, y, z + 0.64 * scale_s), scale=(2.8, 0.35, 0.6), mat=mat, smooth=True)
        for tx in (-0.068, -0.023, 0.023, 0.068):
            add_cylinder(0.012 * scale_s, 0.24 * scale_s, (x + tx * scale_s, y, z + 0.77 * scale_s), mat=mat, vertices=10)
    else:
        # Lying horizontally on the countertop next to plate
        rot_x = math.pi / 2
        add_cylinder(0.035 * scale_s, 0.85 * scale_s, pt(0, -0.25, 0), rotation=(rot_x, 0, rot_z), mat=mat, vertices=16)
        add_cylinder(0.024 * scale_s, 0.30 * scale_s, pt(0, 0.28, 0), rotation=(rot_x, 0, rot_z), mat=mat, vertices=14)
        add_cube(0.08 * scale_s, pt(0, 0.45, 0), scale=(2.4, 0.8, 0.22), rotation=(0, 0, rot_z), mat=mat, smooth=True)
        for tx in (-0.068, -0.023, 0.023, 0.068):
            add_cylinder(0.012 * scale_s, 0.28 * scale_s, pt(tx, 0.60, 0), rotation=(rot_x, 0, rot_z), mat=mat, vertices=10)


def export_glb(filepath):
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
    )


# --- 13 Game Asset Builders ---

def build_plaid_hero():
    clear_scene()
    plaid = mat_plaid_onesie()
    skin = create_solid_material("SkinMat", (0.91, 0.71, 0.56, 1.0), roughness=0.48)
    cheek = create_solid_material("CheekMat", (0.94, 0.56, 0.48, 1.0), roughness=0.52)
    hair = create_solid_material("HairMat", (0.26, 0.14, 0.08, 1.0), roughness=0.68)
    hair_hi = create_solid_material("HairHiMat", (0.34, 0.19, 0.11, 1.0), roughness=0.62)
    cuff = create_solid_material("CuffMat", (0.09, 0.09, 0.10, 1.0), roughness=0.80)
    shoe = create_solid_material("ShoeMat", (0.10, 0.10, 0.12, 1.0), roughness=0.52)
    sole = create_solid_material("SoleMat", (0.48, 0.48, 0.50, 1.0), roughness=0.60)
    flap = create_solid_material("FlapMat", (0.70, 0.54, 0.37, 1.0), roughness=0.84)
    btn = create_solid_material("ButtonMat", (0.12, 0.08, 0.06, 1.0), roughness=0.38)
    zipper = create_solid_material("ZipperMat", (0.14, 0.14, 0.16, 1.0), roughness=0.32, metallic=0.65)
    eye_w = create_solid_material("EyeWMat", (0.99, 0.99, 0.99, 1.0), roughness=0.12)
    eye_iris = create_solid_material("EyeIrisMat", (0.24, 0.15, 0.08, 1.0), roughness=0.14)
    eye_b = create_solid_material("EyeBMat", (0.02, 0.02, 0.03, 1.0), roughness=0.10)
    mouth_in = create_solid_material("HeroMouthMat", (0.35, 0.08, 0.09, 1.0), roughness=0.55)
    teeth = create_solid_material("HeroTeethMat", (0.98, 0.98, 0.95, 1.0), roughness=0.22)

    # Stocky plaid torso + broad shoulders + folded plaid hoodie hood on upper back
    add_sphere(0.41, (0, 0, 1.00), scale=(1.02, 0.84, 1.24), mat=plaid, uv_scale=1.6)
    add_sphere(0.24, (0, 0.01, 1.36), scale=(1.58, 0.94, 0.56), mat=plaid, uv_scale=1.6)
    add_torus(0.21, 0.065, (0, 0.02, 1.48), rotation=(0.12, 0, 0), mat=plaid, uv_scale=1.6)
    # Draped plaid hood resting neatly on the upper back (-Y)
    add_sphere(0.27, (0, -0.22, 1.38), scale=(1.12, 0.68, 0.72), rotation=(-0.35, 0, 0), mat=plaid, uv_scale=1.6)
    # Front zipper
    add_cube(0.045, (0, 0.34, 1.10), scale=(0.65, 0.4, 12.5), mat=zipper, smooth=True)
    # Iconic buttoned tan butt-flap on the lower back (-Y)
    add_cube(0.30, (0, -0.33, 0.80), scale=(1.05, 0.20, 0.85), rotation=(0.14, 0, 0), mat=flap, smooth=True, bevel=0.03)
    for bx in (-0.10, 0.10):
        add_cylinder(0.032, 0.025, (bx, -0.37, 0.90), rotation=(math.pi / 2, 0, 0), mat=btn)

    # Sculpted head, nose, rosy cheeks, ears
    add_sphere(0.29, (0, 0.03, 1.79), scale=(0.96, 0.96, 1.06), mat=skin)
    add_sphere(0.052, (0, 0.31, 1.78), scale=(0.88, 1.15, 0.82), mat=skin)
    for sx in (-1, 1):
        add_sphere(0.062, (sx * 0.28, 0.01, 1.78), scale=(0.50, 0.82, 1.08), mat=skin)
        add_sphere(0.055, (sx * 0.16, 0.24, 1.75), scale=(1.1, 0.65, 0.85), mat=cheek)

    # Full neatly groomed short brown hair cap + layered crown volume + nape taper (NO pointy cones!)
    add_sphere(0.305, (0, -0.02, 1.88), scale=(1.01, 1.04, 0.84), mat=hair)
    add_sphere(0.255, (0, 0.02, 1.98), scale=(1.06, 1.08, 0.56), rotation=(0.18, 0, 0), mat=hair_hi)
    add_sphere(0.22, (0, -0.14, 1.75), scale=(1.12, 0.72, 0.86), mat=hair)  # clean nape
    hair_waves = [
        (-0.12, 0.08, 2.02, (0.28, 0.22, 0.32), (0.1, 0, 0.3), hair),
        (0.0, 0.12, 2.03, (0.30, 0.24, 0.30), (0.12, 0, 0.0), hair_hi),
        (0.12, 0.07, 2.01, (0.26, 0.22, 0.30), (0.1, 0, -0.3), hair),
        (-0.22, -0.02, 1.90, (0.16, 0.25, 0.28), (0, 0.2, 0), hair),
        (0.22, -0.02, 1.90, (0.16, 0.25, 0.28), (0, -0.2, 0), hair),
    ]
    for hx, hy, hz, hsc, hrot, hm in hair_waves:
        add_sphere(0.42, (hx, hy, hz), scale=hsc, rotation=hrot, mat=hm)

    # Full warm beard, mustache, cheerful smile & detailed eyes
    add_sphere(0.265, (0, 0.09, 1.68), scale=(0.94, 0.90, 0.64), mat=hair)
    add_sphere(0.11, (0, 0.28, 1.72), scale=(1.25, 0.52, 0.34), mat=hair)
    add_sphere(0.085, (0, 0.27, 1.66), scale=(1.3, 0.5, 0.65), mat=mouth_in)
    add_cube(0.05, (0, 0.30, 1.68), scale=(2.0, 0.4, 0.32), mat=teeth, smooth=True)
    for sx in (-1, 1):
        add_sphere(0.056, (sx * 0.10, 0.27, 1.84), mat=eye_w)
        add_sphere(0.034, (sx * 0.10, 0.30, 1.84), mat=eye_iris)
        add_sphere(0.022, (sx * 0.10, 0.315, 1.84), mat=eye_b)
        add_cube(0.038, (sx * 0.10, 0.29, 1.91), scale=(2.1, 0.55, 0.42), rotation=(0, sx * -0.10, 0), mat=hair, smooth=True)

    # Arms, ribbed dark cuffs, hands, legs & rounded slip-on sneakers
    for sx in (-1, 1):
        add_cylinder(0.112, 0.56, (sx * 0.50, 0.02, 1.21), rotation=(0.12, sx * -0.45, 0), mat=plaid, uv_scale=1.8)
        add_sphere(0.112, (sx * 0.62, 0.04, 0.98), mat=plaid, uv_scale=1.8)
        add_torus(0.095, 0.025, (sx * 0.64, 0.05, 0.95), rotation=(0.12, sx * -0.45, 0), mat=cuff)
        add_sphere(0.095, (sx * 0.67, 0.07, 0.89), mat=skin)

        add_cylinder(0.15, 0.56, (sx * 0.19, 0.0, 0.41), mat=plaid, uv_scale=1.8)
        add_sphere(0.15, (sx * 0.19, 0.0, 0.64), mat=plaid, uv_scale=1.8)
        add_torus(0.13, 0.03, (sx * 0.19, 0.0, 0.16), mat=cuff)

        add_sphere(0.125, (sx * 0.19, 0.07, 0.10), scale=(0.86, 1.42, 0.54), mat=shoe)
        add_cylinder(0.115, 0.038, (sx * 0.19, 0.07, 0.025), scale=(0.90, 1.46, 1.0), mat=sole)

    export_glb(os.path.join(OUTPUT_DIR, "plaid_hero.glb"))


def build_golden_tendy():
    clear_scene()
    tendy = mat_golden_tendy()
    star_mat = create_solid_material("SparkleStarMat", (1.0, 0.92, 0.34, 1.0), roughness=0.18, emission_strength=0.95)

    # Crispy craggy breaded tender / drumstick silhouette tilted playfully
    lobes = [
        (-0.04, -0.32, 0.44, 0.17, (0.85, 1.18, 0.72), 1),
        (0.03, -0.14, 0.51, 0.25, (1.08, 1.32, 0.88), 2),
        (-0.02, 0.08, 0.55, 0.26, (1.04, 1.28, 0.86), 3),
        (0.04, 0.27, 0.49, 0.20, (0.90, 1.15, 0.78), 4),
        (0.01, 0.43, 0.44, 0.13, (0.78, 1.05, 0.68), 5),
    ]
    for x, y, z, r, sc, seed in lobes:
        obj = add_ico(r, (x, y, z), scale=sc, rotation=(0.2, 0.0, 0.3), mat=tendy, subdiv=3, uv_scale=2.2)
        displace_organic(obj, freq=14.0, amp=0.16, seed=seed)

    # 8 sharp 4-pointed golden star sparkles orbiting the tendy
    for i in range(8):
        ang = i * (math.tau / 8.0)
        sx = math.cos(ang) * 0.62
        sy = math.sin(ang) * 0.62
        sz = 0.52 + (0.16 if i % 2 == 0 else -0.14)
        add_ico(0.048, (sx, sy, sz), scale=(1.0, 0.35, 1.9), mat=star_mat, subdiv=1)
        add_ico(0.048, (sx, sy, sz), scale=(1.9, 0.35, 1.0), mat=star_mat, subdiv=1)

    export_glb(os.path.join(OUTPUT_DIR, "golden_tendy.glb"))


def build_broccoli_brute():
    clear_scene()
    stalk = mat_broccoli_stalk()
    crown = mat_broccoli_crown()
    brow = create_solid_material("BrowMat", (0.06, 0.24, 0.07, 1.0), roughness=0.72)
    shorts = create_solid_material("LeafShortsMat", (0.14, 0.36, 0.08, 1.0), roughness=0.78)
    eye_w = create_solid_material("BruteEyeMat", (0.98, 0.96, 0.74, 1.0), roughness=0.15)
    eye_b = create_solid_material("BrutePupilMat", (0.04, 0.04, 0.04, 1.0), roughness=0.12)
    mouth_bg = create_solid_material("MouthBgMat", (0.12, 0.03, 0.03, 1.0), roughness=0.8)
    teeth = create_solid_material("TeethMat", (0.98, 0.98, 0.94, 1.0), roughness=0.22)

    # Muscular torso, defined pecs, 6-pack abs, leaf loincloth & broad delts
    add_cylinder(0.45, 0.96, (0, 0, 0.94), scale=(1.22, 0.85, 1.0), mat=stalk, uv_scale=1.4)
    add_sphere(0.46, (0, -0.02, 0.48), scale=(1.12, 0.92, 0.45), mat=shorts)
    for row in range(3):
        for sx in (-1, 1):
            add_sphere(0.11, (sx * 0.13, 0.35, 0.90 - row * 0.14), scale=(1.1, 0.45, 0.7), mat=stalk)

    for sx in (-1, 1):
        add_sphere(0.26, (sx * 0.22, 0.24, 1.12), scale=(1.05, 0.72, 0.84), mat=stalk)
        add_sphere(0.25, (sx * 0.64, 0.02, 1.24), mat=stalk)  # deltoid
        add_cylinder(0.18, 0.56, (sx * 0.78, 0.08, 0.96), rotation=(0.25, sx * -0.35, 0), mat=stalk)
        # Leafy wrist spikes (just like the video!)
        for sp_a in (-0.4, 0.0, 0.4):
            add_cone(0.06, 0.008, 0.24, (sx * 0.84, 0.14 + sp_a * 0.12, 0.80), rotation=(-0.4, sx * 0.7, sp_a), mat=crown, vertices=10)
        add_sphere(0.23, (sx * 0.88, 0.22, 0.62), scale=(1.0, 1.15, 0.95), mat=stalk)  # clenched fist
        add_cylinder(0.20, 0.46, (sx * 0.26, 0, 0.24), mat=stalk)  # thigh/leg
        add_sphere(0.19, (sx * 0.26, 0.06, 0.08), scale=(0.95, 1.35, 0.52), mat=stalk)

    # Rich 28-cluster floret afro crown with organic displacement
    rng = np.random.default_rng(7)
    base_puffs = [
        (0, 0, 1.74, 0.56), (-0.54, 0.04, 1.64, 0.43), (0.54, 0.04, 1.64, 0.43),
        (0, -0.45, 1.66, 0.44), (-0.38, -0.34, 1.72, 0.37), (0.38, -0.34, 1.72, 0.37),
        (-0.34, 0.32, 1.66, 0.34), (0.34, 0.32, 1.66, 0.34),
        (0, 0.14, 2.04, 0.44), (-0.40, 0.05, 1.98, 0.35), (0.40, 0.05, 1.98, 0.35),
        (0, -0.24, 1.98, 0.36), (-0.60, -0.14, 1.78, 0.33), (0.60, -0.14, 1.78, 0.33),
        (-0.20, 0.34, 1.88, 0.30), (0.20, 0.34, 1.88, 0.30),
        (0, 0.28, 1.62, 0.30), (0.0, 0.0, 2.22, 0.32),
    ]
    for idx, (px, py, pz, pr) in enumerate(base_puffs):
        p = add_ico(pr, (px, py, pz), mat=crown, subdiv=3, uv_scale=2.2)
        displace_organic(p, freq=9.0, amp=0.14, seed=idx + 10)
    for idx in range(10):
        ang = rng.uniform(0, math.tau)
        rad = rng.uniform(0.35, 0.65)
        pz = rng.uniform(1.62, 2.08)
        p = add_ico(rng.uniform(0.20, 0.28), (math.cos(ang) * rad, math.sin(ang) * rad, pz), mat=crown, subdiv=2, uv_scale=2.2)
        displace_organic(p, freq=11.0, amp=0.15, seed=idx + 50)

    # Scowling furrowed V-brow, angry eyes & clenched toothy grimace (+Y)
    for sx in (-1, 1):
        add_cube(0.16, (sx * 0.18, 0.42, 1.31), scale=(1.6, 0.65, 0.40), rotation=(0, sx * -0.22, 0), mat=brow, smooth=True)
        add_sphere(0.082, (sx * 0.16, 0.41, 1.19), mat=eye_w)
        add_sphere(0.042, (sx * 0.16, 0.48, 1.19), mat=eye_b)
    add_cube(0.18, (0, 0.39, 0.94), scale=(2.3, 0.45, 0.62), mat=mouth_bg, smooth=True)
    for tx in (-0.14, -0.05, 0.05, 0.14):
        add_cone(0.032, 0.005, 0.068, (tx, 0.43, 0.90), mat=teeth, vertices=8)
        add_cone(0.030, 0.005, 0.062, (tx, 0.43, 0.98), rotation=(math.pi, 0, 0), mat=teeth, vertices=8)

    export_glb(os.path.join(OUTPUT_DIR, "broccoli_brute.glb"))


def build_carrot_lancer():
    clear_scene()
    carrot = mat_carrot_skin()
    ridge = create_solid_material("CarrotRidgeMat", (0.76, 0.24, 0.03, 1.0), roughness=0.52)
    leaf = create_solid_material("CarrotLeafMat", (0.14, 0.60, 0.15, 1.0), roughness=0.62)
    eye_w = create_solid_material("EyeWMat", (0.99, 0.99, 0.99, 1.0), roughness=0.14)
    eye_b = create_solid_material("EyeBMat", (0.03, 0.03, 0.04, 1.0), roughness=0.12)
    mouth_bg = create_solid_material("CarrotMouthMat", (0.18, 0.04, 0.03, 1.0), roughness=0.7)
    teeth = create_solid_material("CarrotTeethMat", (0.98, 0.98, 0.94, 1.0), roughness=0.22)
    silver = create_solid_material("SilverForkMat", (0.90, 0.92, 0.95, 1.0), roughness=0.12, metallic=0.95)

    # Tapered organic carrot body + root rings
    body = add_cone(0.38, 0.08, 1.42, (0, 0, 0.98), rotation=(math.pi, 0, 0), mat=carrot, vertices=32, uv_scale=1.6)
    displace_organic(body, freq=8.0, amp=0.04, seed=21)
    for rz, rr in [(0.74, 0.20), (1.02, 0.27), (1.30, 0.33)]:
        add_torus(rr, 0.018, (0, 0, rz), mat=ridge)

    # Lush 8-frond leafy carrot crown
    for i in range(8):
        ang = i * (math.tau / 8.0)
        add_cone(
            0.10, 0.012, 0.58,
            (math.cos(ang) * 0.11, math.sin(ang) * 0.11, 1.88),
            rotation=(math.sin(ang) * 0.42, -math.cos(ang) * 0.42, 0),
            mat=leaf, vertices=12,
        )

    # Angry brow, eyes, grimacing teeth, springy carrot arms & legs
    for sx in (-1, 1):
        add_sphere(0.085, (sx * 0.13, 0.28, 1.32), mat=eye_w)
        add_sphere(0.042, (sx * 0.13, 0.35, 1.32), mat=eye_b)
        add_cube(0.038, (sx * 0.13, 0.33, 1.42), scale=(2.8, 0.6, 0.42), rotation=(0, sx * -0.25, 0), mat=eye_b, smooth=True)
        add_cylinder(0.060, 0.38, (sx * 0.16, 0, 0.19), mat=carrot)
        add_cylinder(0.048, 0.48, (sx * 0.37, 0.14, 0.94), rotation=(0.95, 0, sx * -0.22), mat=carrot)

    add_cube(0.11, (0, 0.31, 1.12), scale=(2.0, 0.5, 0.7), mat=mouth_bg, smooth=True)
    for tx in (-0.07, -0.02, 0.03, 0.08):
        add_cone(0.022, 0.004, 0.045, (tx, 0.34, 1.09), mat=teeth, vertices=8)

    # Gleaming 4-pronged silver dinner fork held in right hand!
    add_dinner_fork(0.44, 0.36, 0.68, rot_z=0.0, scale_s=1.25, mat=silver, upright=True)

    export_glb(os.path.join(OUTPUT_DIR, "carrot_lancer.glb"))


def build_kale_slime():
    clear_scene()
    kale = mat_kale_slime()
    eye_w = create_solid_material("SlimeEyeWMat", (0.99, 0.99, 0.97, 1.0), roughness=0.10)
    eye_b = create_solid_material("SlimeEyeBMat", (0.03, 0.03, 0.04, 1.0), roughness=0.10)
    mouth = create_solid_material("SlimeMouthMat", (0.03, 0.14, 0.04, 1.0), roughness=0.35)

    puddle = add_cylinder(0.98, 0.08, (0, 0, 0.04), scale=(1.12, 1.16, 1.0), mat=kale, vertices=32, uv_scale=1.5)
    displace_organic(puddle, freq=5.0, amp=0.08, seed=3)

    mounds = [
        (0, 0, 0.31, 0.58, (1.15, 1.10, 0.66), 1),
        (-0.42, 0.18, 0.24, 0.35, (1.10, 1.00, 0.70), 2),
        (0.42, 0.16, 0.24, 0.35, (1.00, 1.10, 0.70), 3),
        (0, -0.40, 0.26, 0.38, (1.15, 0.95, 0.72), 4),
        (-0.30, -0.28, 0.24, 0.30, (1.00, 1.00, 0.70), 5),
        (0.32, -0.28, 0.24, 0.30, (1.00, 1.00, 0.70), 6),
    ]
    for x, y, z, r, sc, seed in mounds:
        m = add_ico(r, (x, y, z), scale=sc, mat=kale, subdiv=3, uv_scale=1.8)
        displace_organic(m, freq=7.5, amp=0.12, seed=seed)

    for sx in (-1, 1):
        add_sphere(0.11, (sx * 0.20, 0.52, 0.42), mat=eye_w)
        add_sphere(0.055, (sx * 0.20, 0.61, 0.42), mat=eye_b)
    add_sphere(0.14, (0, 0.54, 0.24), scale=(1.35, 0.45, 0.65), mat=mouth)

    export_glb(os.path.join(OUTPUT_DIR, "kale_slime.glb"))


def build_honey_mustard_cup():
    clear_scene()
    cup = create_solid_material("CupMat", (0.95, 0.95, 0.97, 1.0), roughness=0.18)
    mustard = create_solid_material("MustardMat", (0.96, 0.74, 0.08, 1.0), roughness=0.22, emission_strength=0.25)
    add_cone(0.28, 0.38, 0.46, (0, 0, 0.35), mat=cup, vertices=32)
    add_torus(0.38, 0.025, (0, 0, 0.57), mat=cup)
    add_cylinder(0.35, 0.08, (0, 0, 0.56), mat=mustard, vertices=32)
    export_glb(os.path.join(OUTPUT_DIR, "honey_mustard_cup.glb"))


def build_ranch_shield():
    clear_scene()
    ranch = create_solid_material("RanchMat", (0.97, 0.98, 0.95, 1.0), roughness=0.20, emission_strength=0.22)
    herb = create_solid_material("HerbMat", (0.16, 0.62, 0.24, 1.0), roughness=0.40, emission_strength=0.18)
    add_sphere(0.38, (0, 0, 0.55), mat=ranch)
    add_torus(0.52, 0.045, (0, 0, 0.55), rotation=(0.35, 0.2, 0), mat=herb)
    add_torus(0.52, 0.045, (0, 0, 0.55), rotation=(-0.35, -0.2, 0.6), mat=herb)
    export_glb(os.path.join(OUTPUT_DIR, "ranch_shield.glb"))


def build_kitchen_sponge():
    clear_scene()
    yellow = mat_sponge_yellow()
    green = mat_sponge_green()
    add_cube(1.0, (0, 0, 0.30), scale=(2.2, 1.4, 0.60), mat=yellow, uv_scale=0.55, smooth=True, bevel=0.05)
    add_cube(1.0, (0, 0, 0.70), scale=(2.2, 1.4, 0.22), mat=green, uv_scale=0.65, smooth=True, bevel=0.04)
    export_glb(os.path.join(OUTPUT_DIR, "kitchen_sponge.glb"))


def build_pancake_stack():
    clear_scene()
    plate = create_solid_material("PlateMat", (0.95, 0.96, 0.98, 1.0), roughness=0.14)
    cake = mat_pancake()
    edge = create_solid_material("PancakeEdgeMat", (0.87, 0.66, 0.34, 1.0), roughness=0.76)
    butter = create_solid_material("ButterMat", (0.99, 0.88, 0.25, 1.0), roughness=0.24)
    syrup = create_solid_material("SyrupMat", (0.50, 0.16, 0.02, 1.0), roughness=0.06)

    add_cylinder(2.28, 0.14, (0, 0, 0.07), mat=plate, vertices=36)
    add_torus(2.18, 0.10, (0, 0, 0.14), mat=plate)
    for i in range(6):
        z = 0.28 + i * 0.24
        r = 1.58 - i * 0.03
        add_cylinder(r, 0.22, (0, 0, z), mat=cake, vertices=36, uv_scale=0.45)
        add_torus(r - 0.04, 0.075, (0, 0, z), mat=edge)

    # Rich glossy syrup pool & 6 thick maple ribbons cascading down onto the plate
    add_cylinder(1.22, 0.06, (0, 0, 1.62), mat=syrup, vertices=32)
    for i in range(6):
        ang = i * (math.tau / 6.0) + 0.25
        dx = math.cos(ang) * 1.45
        dy = math.sin(ang) * 1.45
        add_cylinder(0.11, 0.95, (dx, dy, 1.12), mat=syrup, vertices=14)
        add_sphere(0.13, (dx, dy, 0.65), scale=(1.0, 1.0, 0.8), mat=syrup)
        add_cylinder(0.28, 0.04, (dx * 1.10, dy * 1.10, 0.16), mat=syrup, vertices=16)

    add_cube(0.54, (0, 0, 1.73), scale=(1.0, 1.0, 0.32), rotation=(0, 0, 0.42), mat=butter, smooth=True, bevel=0.05)
    export_glb(os.path.join(OUTPUT_DIR, "pancake_stack.glb"))


def build_plate_with_egg():
    clear_scene()
    plate = create_solid_material("BluePlateMat", (0.72, 0.85, 0.95, 1.0), roughness=0.12)
    white = create_solid_material("EggWhiteMat", (0.98, 0.98, 0.96, 1.0), roughness=0.25)
    egg_crisp = create_solid_material("EggCrispMat", (0.92, 0.86, 0.72, 1.0), roughness=0.45)
    yolk = create_solid_material("EggYolkMat", (0.98, 0.62, 0.05, 1.0), roughness=0.08)
    silver = create_solid_material("SilverMat", (0.89, 0.91, 0.95, 1.0), roughness=0.12, metallic=0.95)

    add_cylinder(1.56, 0.12, (0, 0, 0.06), mat=plate, vertices=36)
    add_torus(1.48, 0.10, (0, 0, 0.13), mat=plate)
    add_cylinder(0.92, 0.06, (0.06, 0.04, 0.14), mat=egg_crisp, vertices=32)
    add_cylinder(0.86, 0.07, (0.06, 0.04, 0.16), mat=white, vertices=32)
    add_sphere(0.36, (0.06, 0.04, 0.22), scale=(1.0, 1.0, 0.54), mat=yolk)

    # Detailed 4-tine stainless steel fork on left + butter knife on right
    add_dinner_fork(-1.88, -0.05, 0.06, rot_z=0.08, scale_s=1.05, mat=silver, upright=False)
    add_cylinder(0.042, 0.78, (1.88, -0.35, 0.055), rotation=(math.pi / 2, 0, -0.06), mat=silver, vertices=16)
    add_cube(0.12, (1.92, 0.42, 0.06), scale=(0.24, 6.6, 0.95), rotation=(0, 0, -0.06), mat=silver, smooth=True)

    export_glb(os.path.join(OUTPUT_DIR, "plate_with_egg.glb"))


def build_syrup_bottle():
    clear_scene()
    glass = create_solid_material("BottleGlassMat", (0.42, 0.15, 0.02, 1.0), roughness=0.08)
    cap = create_solid_material("BottleCapMat", (0.82, 0.56, 0.14, 1.0), roughness=0.25, metallic=0.5)
    label = mat_syrup_label()
    add_cylinder(0.65, 1.95, (0, 0, 0.98), mat=glass, vertices=32)
    add_sphere(0.65, (0, 0, 1.95), scale=(1.0, 1.0, 0.48), mat=glass)
    add_cylinder(0.30, 0.65, (0, 0, 2.36), mat=glass, vertices=28)
    add_torus(0.28, 0.072, (0.45, 0, 2.06), rotation=(math.pi / 2, 0, 0), mat=glass)
    add_cylinder(0.34, 0.34, (0, 0, 2.78), mat=cap, vertices=28)
    add_cylinder(0.67, 0.92, (0, 0, 1.00), mat=label, vertices=32, uv_scale=0.95)
    export_glb(os.path.join(OUTPUT_DIR, "syrup_bottle.glb"))


def build_salt_shaker():
    clear_scene()
    glass = create_solid_material("ShakerGlassMat", (0.84, 0.91, 0.95, 1.0), roughness=0.09)
    salt = create_solid_material("SaltFillMat", (0.98, 0.98, 0.98, 1.0), roughness=0.88)
    cap = create_solid_material("ShakerCapMat", (0.88, 0.90, 0.94, 1.0), roughness=0.12, metallic=0.95)
    add_cone(0.46, 0.36, 1.10, (0, 0, 0.55), mat=glass, vertices=28)
    add_cone(0.39, 0.30, 0.80, (0, 0, 0.42), mat=salt, vertices=24)
    add_sphere(0.38, (0, 0, 1.12), scale=(1.0, 1.0, 0.72), mat=cap)
    export_glb(os.path.join(OUTPUT_DIR, "salt_shaker.glb"))


def build_kitchen_environment():
    """Complete photorealistic sunlit kitchen enclosure matching all video details."""
    clear_scene()
    wood = mat_butcher_block()
    tile = mat_sage_tile()
    gingham = mat_gingham()
    board_mat = mat_cutting_board()
    wall = create_solid_material("KitchenWallMat", (0.79, 0.83, 0.76, 1.0), roughness=0.72)
    cab = create_solid_material("CabinetWoodMat", (0.46, 0.29, 0.16, 1.0), roughness=0.45)
    cab_trim = create_solid_material("CabinetTrimMat", (0.38, 0.23, 0.12, 1.0), roughness=0.52)
    floor = create_solid_material("KitchenFloorMat", (0.58, 0.43, 0.28, 1.0), roughness=0.48)
    stone = create_solid_material("SideStoneMat", (0.88, 0.86, 0.81, 1.0), roughness=0.28)
    steel = create_solid_material("StainlessMat", (0.86, 0.88, 0.92, 1.0), roughness=0.16, metallic=0.92)
    dark_sink = create_solid_material("SinkBasinMat", (0.30, 0.32, 0.34, 1.0), roughness=0.35, metallic=0.85)
    sky = mat_window_view()
    red_cer = create_solid_material("RedCeramicMat", (0.80, 0.14, 0.12, 1.0), roughness=0.18)
    tomato = create_solid_material("TomatoMat", (0.88, 0.12, 0.08, 1.0), roughness=0.15)
    blue_cer = create_solid_material("BlueBowlMat", (0.55, 0.74, 0.84, 1.0), roughness=0.22)
    yellow_cer = create_solid_material("YellowJarMat", (0.92, 0.74, 0.24, 1.0), roughness=0.25)
    white_cer = create_solid_material("WhiteCeramicMat", (0.95, 0.95, 0.93, 1.0), roughness=0.20)
    mill_wood = create_solid_material("PepperMillMat", (0.68, 0.46, 0.23, 1.0), roughness=0.36)
    terracotta = create_solid_material("TerracottaMat", (0.74, 0.36, 0.22, 1.0), roughness=0.72)
    plant_g = create_solid_material("PlantMat", (0.18, 0.52, 0.16, 1.0), roughness=0.62)

    # 1. Varnished Butcher-Block Countertop (14m x 44m, anisotropic world-space UVs) & oak under-cabinet base
    add_cube(1.0, (0, 0, -0.5), scale=(14.0, 44.0, 1.0), mat=wood, uv_scale=(0.14, 0.032))
    add_cube(1.0, (0, 0, -2.75), scale=(13.5, 43.5, 3.5), mat=cab)
    add_cube(1.0, (1.5, -2.0, -4.75), scale=(32.0, 54.0, 0.5), mat=floor)

    # 2. Glossy Sage-Green Ceramic Tile Left Wall (X = -7.3) with cream grout lines
    add_cube(1.0, (-7.35, -0.5, 3.4), scale=(0.45, 48.0, 7.6), mat=tile, uv_scale=0.24)
    # Left overhanging wooden upper kitchen cabinets (stays left of X = -3.8 so view stays wide open!)
    add_cube(1.0, (-5.7, 3.5, 8.6), scale=(3.4, 34.0, 5.2), mat=cab)
    for cz in (6.1, 11.1):
        add_cube(1.0, (-3.95, 3.5, cz), scale=(0.15, 34.0, 0.24), mat=cab_trim)

    # 3. Warm Sage Back Wall & Low Right-Side Kitchen Wing (unblocked ceiling/sky for sunlit countertop)
    add_cube(1.0, (2.0, 22.8, 5.2), scale=(30.0, 0.6, 17.5), mat=wall)

    # Right kitchen low wainscoting wall + lower stone counter wing at X = +11.6 (top at Z=4.45 so sun streams over)
    add_cube(1.0, (13.6, -1.0, 1.2), scale=(0.8, 50.0, 6.5), mat=wall)
    add_cube(1.0, (11.6, -1.0, -0.6), scale=(3.6, 46.0, 0.8), mat=stone)
    add_cube(1.0, (11.6, -1.0, -2.75), scale=(3.4, 45.5, 3.5), mat=cab)

    # 4. Sunny Kitchen Window (Back-Right: X = +5.0, Y = 22.3) + Garden Foliage Texture + Potted Plant + Gingham Curtains
    add_cube(1.0, (5.0, 22.35, 6.2), scale=(7.6, 0.2, 5.6), mat=sky, uv_scale=0.16)
    # Wood window frame + mullions + deep sill
    for mx in (1.2, 5.0, 8.8):
        add_cube(1.0, (mx, 22.18, 6.2), scale=(0.28, 0.35, 5.8), mat=cab)
    for mz in (3.4, 6.2, 9.0):
        add_cube(1.0, (5.0, 22.18, mz), scale=(7.8, 0.42, 0.28), mat=cab)
    add_cube(1.0, (5.0, 21.85, 3.35), scale=(8.2, 1.1, 0.24), mat=cab)

    # Terracotta plant pot + leafy plant on window sill
    add_cone(0.32, 0.44, 0.58, (3.2, 21.75, 3.76), mat=terracotta, vertices=20)
    for lx, ly, lz, rot in [(0, 0, 4.32, (0, 0, 0)), (-0.18, 0.05, 4.24, (0.3, -0.4, 0)), (0.18, -0.05, 4.24, (-0.3, 0.4, 0))]:
        add_sphere(0.24, (3.2 + lx, 21.75 + ly, lz), scale=(0.75, 1.2, 0.42), rotation=rot, mat=plant_g)

    # Pleated Red-and-White Gingham Cafe Curtains
    add_cube(1.0, (5.0, 21.9, 8.6), scale=(8.0, 0.3, 1.25), mat=gingham, uv_scale=0.55)
    for fold_x in np.linspace(1.3, 8.7, 13):
        add_cylinder(0.20, 1.28, (float(fold_x), 21.85, 8.6), mat=gingham, uv_scale=0.55, vertices=14)
    for sx in (1.75, 8.25):
        add_cube(1.0, (sx, 21.92, 6.1), scale=(1.1, 0.25, 5.3), mat=gingham, uv_scale=0.55)

    # 5. Stainless Steel Sink & Chrome Gooseneck Faucet in front of Window
    add_cube(1.0, (5.0, 20.6, 0.10), scale=(5.8, 2.8, 0.22), mat=steel)
    add_cube(1.0, (5.0, 20.6, 0.18), scale=(5.0, 2.0, 0.10), mat=dark_sink)
    add_cylinder(0.16, 2.1, (5.0, 21.75, 1.05), mat=steel)
    add_torus(0.60, 0.13, (5.0, 21.15, 2.05), rotation=(0, math.pi / 2, 0), mat=steel)

    # 6. Open Wooden Back-Left Shelves & Props (X = -5.5..+0.6, Y = 21.6)
    for sz in (3.8, 6.8):
        add_cube(1.0, (-2.6, 21.7, sz), scale=(6.6, 1.4, 0.24), mat=board_mat, uv_scale=0.4)
    # Props on lower shelf (z ~ 4.0)
    add_sphere(0.55, (-4.8, 21.65, 4.30), scale=(1.0, 1.0, 0.62), mat=blue_cer)
    add_cylinder(0.42, 0.82, (-3.5, 21.65, 4.35), mat=yellow_cer)
    for pi in range(5):
        add_cylinder(0.68, 0.08, (-2.1, 21.65, 3.98 + pi * 0.11), mat=white_cer)
    add_cylinder(0.32, 0.65, (-0.7, 21.65, 4.26), mat=white_cer)
    # Props on upper shelf (z ~ 7.0)
    add_cylinder(0.75, 0.34, (-3.6, 21.65, 7.12), mat=plant_g)
    add_cylinder(0.44, 0.95, (-1.6, 21.65, 7.42), mat=white_cer)

    # 7. Left-Side Foreground/Midground Kitchen Props (visible right next to Hero & down the run!)
    # Leaning Paddle Cutting Board near player start (Blender Y = -12.0 -> Bevy Z = +12.0)
    add_cube(1.0, (-6.55, -12.0, 2.05), scale=(0.24, 2.35, 3.9), rotation=(0, -0.14, 0), mat=board_mat, uv_scale=0.45, smooth=True, bevel=0.04)
    add_cube(1.0, (-6.82, -12.0, 4.45), scale=(0.24, 0.68, 1.15), rotation=(0, -0.14, 0), mat=board_mat, uv_scale=0.45, smooth=True, bevel=0.04)

    # Contoured Wooden Pepper Mill (Blender Y = -10.0)
    add_cylinder(0.46, 2.1, (-5.9, -10.0, 1.05), mat=mill_wood)
    add_sphere(0.50, (-5.9, -10.0, 2.25), scale=(1.0, 1.0, 0.85), mat=mill_wood)
    add_sphere(0.14, (-5.9, -10.0, 2.74), mat=steel)

    # Wall Hanging Rail + Stainless Utensils on Left Tile Wall (Blender Y = -10.5..-6.0, Z = 4.6)
    add_cylinder(0.06, 6.2, (-7.0, -8.5, 4.85), rotation=(math.pi / 2, 0, 0), mat=steel)
    for uy, handle_mat in [(-10.2, red_cer), (-8.8, steel), (-7.2, cab)]:
        add_cylinder(0.06, 1.5, (-6.98, uy, 4.05), mat=handle_mat)
        add_cube(0.32, (-6.98, uy, 3.15), scale=(0.15, 1.15, 1.35), mat=steel, smooth=True)

    # White Ceramic Utensil Crock + Wooden Spoons (Blender Y = -6.8)
    add_cylinder(0.72, 1.65, (-6.0, -6.8, 0.82), mat=white_cer)
    add_torus(0.72, 0.08, (-6.0, -6.8, 1.64), mat=white_cer)
    for sx, sy, rot in [(-0.18, -0.1, (0.25, -0.2, 0)), (0.15, 0.14, (-0.22, 0.25, 0)), (0.0, -0.2, (0.3, 0.1, 0))]:
        add_cylinder(0.06, 2.1, (-6.0 + sx, -6.8 + sy, 2.05), rotation=rot, mat=board_mat, uv_scale=0.8)
        add_sphere(0.22, (-6.0 + sx * 1.6, -6.8 + sy * 1.6, 3.05), scale=(0.9, 1.3, 0.35), rotation=rot, mat=board_mat, uv_scale=0.8)

    # Glossy Red Ceramic Bowl with Bright Red Tomatoes (Blender Y = -3.8)
    add_sphere(1.05, (-5.85, -3.8, 0.58), scale=(1.0, 1.0, 0.62), mat=red_cer)
    add_cylinder(0.96, 0.22, (-5.85, -3.8, 0.88), mat=red_cer)
    for tx, ty, tz in [(-0.28, -0.15, 1.10), (0.26, -0.10, 1.12), (-0.02, 0.26, 1.10), (0.04, -0.02, 1.30)]:
        add_sphere(0.32, (-5.85 + tx, -3.8 + ty, tz), scale=(1.08, 1.08, 0.88), mat=tomato)

    # Second distant cutting board on left wall (Blender Y = +13.5) for mid-course detail
    add_cube(1.0, (-6.6, 13.5, 1.9), scale=(0.24, 2.4, 3.6), rotation=(0, -0.14, 0), mat=board_mat, uv_scale=0.45, smooth=True, bevel=0.04)

    export_glb(os.path.join(OUTPUT_DIR, "kitchen_environment.glb"))


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
    print("Successfully exported all 13 photorealistic PBR Tendy Quest .glb assets to:", OUTPUT_DIR)
