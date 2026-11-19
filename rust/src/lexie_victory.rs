//! Kaiju Showdown victory meltdown cutscene (`GameState::LexieVictoryCutscene`):
//! John Carpenter's *The Thing* (1982) practical-effects body horror in vivid color.
//!
//! Features procedural 256x256 PBR textures (necrotic veined flesh, striated viscera, boiling
//! fermented garlic brine, and bloodshot sclera) with manual orthogonal vertex tangents (because
//! `mikktspace` is disabled in `Cargo.toml`), a live CPU vertex-deforming biomass mound, an
//! articulated chest rib-maw that splits open to unleash tentacles and throbbing organs, a
//! detaching Spider-Head with telescoping eyestalks and unhinging jaw connected by dynamic
//! mucus/tendon strands, swelling gas blisters, and all 5 crew avatars witnessing the meltdown.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::audio::{is_voice_speaking, play_headphone_music, play_sfx, speak_voice};
use crate::state::GameState;
use crate::ui::HudOverlay;

#[derive(Component)]
pub struct LexieVictoryEntity;

#[derive(Component)]
pub struct LexieVictoryOverlay;

#[derive(Component)]
pub struct LexieVictorySpeakerText;

#[derive(Component)]
pub struct LexieVictorySubText;

#[derive(Component)]
pub struct LexieBannerLeftText;

#[derive(Component)]
pub struct LexieBannerRightText;

/// Handle to the dynamically vertex-deformed biomass mound/puddle mesh (`MAIN_WORLD | RENDER_WORLD`).
#[derive(Component)]
pub struct LexieMeltMoundMesh(pub Handle<Mesh>);

#[derive(Component, Clone, Copy)]
pub enum HorrorLightRole {
    FlickerOverhead,
    CrimsonCore,
    BileRim,
    CrewFill,
}

#[derive(Component)]
pub struct LexieHorrorLight {
    pub role: HorrorLightRole,
}

/// Unified component tagging every animated horror body part and practical-FX prop so a single
/// `Query<(&mut Transform, &ThingAnim)>` drives the entire scene with zero Bevy B0001 aliasing.
#[derive(Component)]
pub struct ThingAnim {
    pub kind: ThingPart,
}

#[derive(Clone, Copy)]
pub enum ThingPart {
    LowerBody,
    ChestHalf { side: f32 },
    InnerOrgan { lobe: usize },
    HeadAssembly,
    Jaw,
    Eyestalk { side: f32, seg: usize },
    SpiderLeg { idx: usize, side: f32 },
    SpiderShin { idx: usize, side: f32 },
    TentacleRoot { idx: usize, base_angle: f32 },
    TentacleSeg { idx: usize, seg: usize },
    TendonStrand { idx: usize, jaw_strand: bool },
    Boil { base_pos: Vec3, base_scale: f32, phase: f32 },
    BubbleDome { base_pos: Vec3, max_r: f32, phase: f32, speed: f32 },
    VisceraJet { base_pos: Vec3, vel: Vec3, phase: f32, speed: f32 },
    StinkCloud { base_pos: Vec3, phase: f32 },
    SpiralRing { speed: f32, base_scale: f32 },
}

#[derive(Resource, Default)]
pub struct LexieVictoryState {
    pub beat: u8,
    pub beat_t: f32,
    pub squelch_cd: f32,
    pub phase_sfx_step: u8,
}

/// (speaker, dialogue, tts_voice, left_banner_sfx, right_banner_sfx, min_duration_s)
const GROSS_BEATS: [(&str, &str, &str, &str, &str, f32); 5] = [
    (
        "LEXIE (CELLULAR CONTORTION)",
        "H-HEHEHEHAHAHA!! SOMETHING IN THE PICKLED GARLIC IS AWAKE!! MY SKIN IS CRAWLING - MY BONES ARE UNLOCKING!!",
        "lexie",
        "*CRACK-SNAP*  *THROB*",
        "GARLIC MUTATION!!",
        4.2,
    ),
    (
        "STAN (TO DEREK, TJ, NATE & GLENN)",
        "OH GOD STAY BACK!! HER FACE IS SPLITTING OPEN AND YELLOW BILE IS BOILING OUT OF HER NECK!! DEREK! TJ! NATE! GLENN! WATCH OUT!!",
        "stan",
        "*WET GURGLE*  *HISS*",
        "DO NOT BREATHE THIS!!",
        4.2,
    ),
    (
        "LEXIE-THING (CHEST RUPTURE)",
        "RRRAAAUUUGHHH!! *SQUELCH* DROWN IN MY RIB-TEETH!! TASTE MY PULSING FERMENTED ORGANS AND WRITHING TENDRILS!!",
        "lexie",
        "*RIP-TEAR*  *CRUNCH!!*",
        "RIBCAGE MAW!!  *SHRIEK*",
        5.0,
    ),
    (
        "DEREK, TJ, NATE & GLENN (HORRIFIED)",
        "HER HEAD JUST TORE OFF ON WET TENDONS AND GREW SPIDER LEGS!! SHE'S LIQUEFYING INTO BOILING PICKLED FEET JUICE!!",
        "tj",
        "*SQUELCH-POP*  *BOIL*",
        "THE GARLIC THING!!",
        4.8,
    ),
    (
        "STAN & THE PUB 217 CREW (SURVIVORS)",
        "Cedar Rapids is saved... those twitching garlic remains are nightmare fuel. Sprint back to Pub 217 and torch a six-foot beer tower!!",
        "stan",
        "*STEAMING OOZE*  *DRIP*",
        "WE SURVIVED THE THING!",
        4.2,
    ),
];

// =================================================================================================
// Procedural noise, PBR bio-textures & manual orthogonal tangents
// =================================================================================================

#[inline]
fn hash2(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix.wrapping_mul(374_761_393))
        .wrapping_add(iy.wrapping_mul(668_265_263))
        .wrapping_add(seed as i32 * 1_442_695_041) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

/// Seamless tileable value noise over period `period`.
fn tile_noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let gx = x.floor();
    let gy = y.floor();
    let tx = x - gx;
    let ty = y - gy;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let i0 = (gx as i32).rem_euclid(period);
    let j0 = (gy as i32).rem_euclid(period);
    let i1 = (i0 + 1) % period;
    let j1 = (j0 + 1) % period;
    let a = hash2(i0, j0, seed);
    let b = hash2(i1, j0, seed);
    let c = hash2(i0, j1, seed);
    let d = hash2(i1, j1, seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sy
}

fn tile_fbm(u: f32, v: f32, base_cells: i32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.54;
    let mut freq = base_cells;
    let mut norm = 0.0;
    for oct in 0..3 {
        sum += amp * tile_noise(u * freq as f32, v * freq as f32, freq, seed + oct * 97);
        norm += amp;
        amp *= 0.5;
        freq *= 2;
    }
    sum / norm
}

/// Ridged cellular vein network in [0, 1] (peaks = thin branching capillaries / scars).
fn tile_vein(u: f32, v: f32, cells: i32, seed: u32) -> f32 {
    let warp_u = u + (tile_fbm(u, v, cells, seed + 11) - 0.5) * 0.14;
    let warp_v = v + (tile_fbm(u + 0.37, v + 0.71, cells, seed + 29) - 0.5) * 0.14;
    let n1 = (tile_noise(warp_u * cells as f32, warp_v * cells as f32, cells, seed) * 2.0 - 1.0).abs();
    let v1 = (1.0 - n1).clamp(0.0, 1.0).powi(5);
    let c2 = cells * 2;
    let n2 = (tile_noise(warp_u * c2 as f32, warp_v * c2 as f32, c2, seed + 53) * 2.0 - 1.0).abs();
    let v2 = (1.0 - n2).clamp(0.0, 1.0).powi(6) * 0.65;
    (v1 + v2).clamp(0.0, 1.0)
}

fn repeat_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    })
}

fn make_rgba_image(size: u32, format: TextureFormat, data: Vec<u8>) -> Image {
    let mut img = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = repeat_sampler();
    img
}

/// Builds a tangent-space normal map (`Rgba8Unorm`) from a scalar heightfield using Sobel gradients.
fn heightfield_to_normal_map(size: usize, heights: &[f32], strength: f32) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let sample = |x: isize, y: isize| -> f32 {
        let ix = x.rem_euclid(size as isize) as usize;
        let iy = y.rem_euclid(size as isize) as usize;
        heights[iy * size + ix]
    };
    for y in 0..size {
        for x in 0..size {
            let xi = x as isize;
            let yi = y as isize;
            let tl = sample(xi - 1, yi - 1);
            let t = sample(xi, yi - 1);
            let tr = sample(xi + 1, yi - 1);
            let l = sample(xi - 1, yi);
            let r = sample(xi + 1, yi);
            let bl = sample(xi - 1, yi + 1);
            let b = sample(xi, yi + 1);
            let br = sample(xi + 1, yi + 1);
            let dx = (tr + 2.0 * r + br) - (tl + 2.0 * l + bl);
            let dy = (bl + 2.0 * b + br) - (tl + 2.0 * t + tr);
            let n = Vec3::new(-dx * strength, -dy * strength, 1.0).normalize();
            let i = (y * size + x) * 4;
            out[i] = ((n.x * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            out[i + 1] = ((n.y * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            out[i + 2] = ((n.z * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            out[i + 3] = 255;
        }
    }
    out
}

struct BioTextures {
    flesh_albedo: Handle<Image>,
    flesh_normal: Handle<Image>,
    flesh_mr: Handle<Image>,
    viscera_albedo: Handle<Image>,
    viscera_normal: Handle<Image>,
    brine_albedo: Handle<Image>,
    brine_normal: Handle<Image>,
    eye_albedo: Handle<Image>,
}

fn make_bio_textures(images: &mut Assets<Image>) -> BioTextures {
    const N: usize = 256;
    let nf = N as f32;

    let mut flesh_rgb = vec![0u8; N * N * 4];
    let mut flesh_mr_bytes = vec![0u8; N * N * 4];
    let mut flesh_h = vec![0.0_f32; N * N];

    let mut visc_rgb = vec![0u8; N * N * 4];
    let mut visc_h = vec![0.0_f32; N * N];

    let mut brine_rgb = vec![0u8; N * N * 4];
    let mut brine_h = vec![0.0_f32; N * N];

    let mut eye_rgb = vec![0u8; N * N * 4];

    for y in 0..N {
        let v = y as f32 / nf;
        for x in 0..N {
            let u = x as f32 / nf;
            let idx = y * N + x;
            let p = idx * 4;

            // 1. Necrotic veined flesh: bruised plum/peach + jaundiced infection + crimson veins
            let mottle = tile_fbm(u, v, 5, 101);
            let bruise = tile_fbm(u + 0.21, v + 0.43, 3, 211);
            let pores = tile_fbm(u, v, 18, 307);
            let vein = tile_vein(u, v, 7, 409);
            let blister = (mottle - 0.60).max(0.0) * 2.5;

            let mut fr = 0.74 + mottle * 0.16 - bruise * 0.22;
            let mut fg = 0.56 + pores * 0.12 - bruise * 0.26 + blister * 0.24;
            let mut fb = 0.50 - bruise * 0.10 - blister * 0.18;
            // Crimson/violet arterial veins
            fr = fr * (1.0 - vein * 0.55) + 0.48 * vein;
            fg *= 1.0 - vein * 0.82;
            fb = fb * (1.0 - vein * 0.75) + 0.08 * vein;
            // Pus-yellow necrotic blisters
            let pus_mask = blister.clamp(0.0, 1.0);
            fr = fr * (1.0 - pus_mask * 0.4) + 0.88 * pus_mask * 0.4;
            fg = fg * (1.0 - pus_mask * 0.4) + 0.84 * pus_mask * 0.4;
            fb *= 1.0 - pus_mask * 0.55;

            flesh_rgb[p] = (fr.clamp(0.0, 1.0) * 255.0) as u8;
            flesh_rgb[p + 1] = (fg.clamp(0.0, 1.0) * 255.0) as u8;
            flesh_rgb[p + 2] = (fb.clamp(0.0, 1.0) * 255.0) as u8;
            flesh_rgb[p + 3] = 255;

            flesh_h[idx] = mottle * 0.45 + pores * 0.20 + vein * 0.35 + blister * 0.55;
            // GLTF metallic-roughness: G = roughness (wet mucus 0.05..0.18), B = metallic (0)
            let rough = (0.05 + (1.0 - pus_mask) * 0.09 + pores * 0.04).clamp(0.04, 0.20);
            flesh_mr_bytes[p] = 255;
            flesh_mr_bytes[p + 1] = (rough * 255.0) as u8;
            flesh_mr_bytes[p + 2] = 0;
            flesh_mr_bytes[p + 3] = 255;

            // 2. Striated arterial viscera: oxblood muscle fibers + pale wet fascia streaks
            let warp = tile_fbm(u, v, 6, 521);
            let fiber = ((u * 22.0 + warp * 4.5) * PI).sin() * 0.5 + 0.5;
            let clot = tile_fbm(u + 0.5, v + 0.17, 4, 631);
            let fascia = (fiber - 0.80).max(0.0) * 5.0;
            let vr = 0.30 + fiber * 0.38 - clot * 0.18 + fascia * 0.34;
            let vg = 0.02 + fiber * 0.04 + fascia * 0.46;
            let vb = 0.03 + clot * 0.04 + fascia * 0.42;
            visc_rgb[p] = (vr.clamp(0.0, 1.0) * 255.0) as u8;
            visc_rgb[p + 1] = (vg.clamp(0.0, 1.0) * 255.0) as u8;
            visc_rgb[p + 2] = (vb.clamp(0.0, 1.0) * 255.0) as u8;
            visc_rgb[p + 3] = 255;
            visc_h[idx] = fiber * 0.55 + warp * 0.35 + fascia * 0.25;

            // 3. Boiling fermented garlic brine: oily chartreuse slime + milky garlic curds
            let swirl = tile_fbm(u + (v * TAU).sin() * 0.06, v + (u * TAU).cos() * 0.06, 6, 743);
            let curd = (tile_fbm(u, v, 10, 853) - 0.64).max(0.0) * 2.8;
            let vein_b = tile_vein(u, v, 5, 967);
            let br_r = 0.24 + swirl * 0.28 + curd * 0.42 - vein_b * 0.14;
            let br_g = 0.46 + swirl * 0.36 + curd * 0.24 - vein_b * 0.26;
            let br_b = 0.04 + swirl * 0.06 + curd * 0.18;
            brine_rgb[p] = (br_r.clamp(0.0, 1.0) * 255.0) as u8;
            brine_rgb[p + 1] = (br_g.clamp(0.0, 1.0) * 255.0) as u8;
            brine_rgb[p + 2] = (br_b.clamp(0.0, 1.0) * 255.0) as u8;
            brine_rgb[p + 3] = 255;
            brine_h[idx] = swirl * 0.5 + curd * 0.7 - vein_b * 0.25;

            // 4. Bloodshot sclera: jaundiced white with dense radial scarlet capillaries
            let radial_v = (v - 0.5).abs() * 2.0; // equator/back-of-eye density
            let cap = tile_vein(u * 1.4, v, 8, 1091) * (0.35 + radial_v * 0.85);
            let er = 0.96 - cap * 0.22;
            let eg = 0.90 - radial_v * 0.12 - cap * 0.82;
            let eb = 0.76 - radial_v * 0.16 - cap * 0.76;
            eye_rgb[p] = (er.clamp(0.0, 1.0) * 255.0) as u8;
            eye_rgb[p + 1] = (eg.clamp(0.0, 1.0) * 255.0) as u8;
            eye_rgb[p + 2] = (eb.clamp(0.0, 1.0) * 255.0) as u8;
            eye_rgb[p + 3] = 255;
        }
    }

    let flesh_norm = heightfield_to_normal_map(N, &flesh_h, 3.6);
    let visc_norm = heightfield_to_normal_map(N, &visc_h, 4.4);
    let brine_norm = heightfield_to_normal_map(N, &brine_h, 4.0);

    BioTextures {
        flesh_albedo: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8UnormSrgb, flesh_rgb)),
        flesh_normal: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8Unorm, flesh_norm)),
        flesh_mr: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8Unorm, flesh_mr_bytes)),
        viscera_albedo: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8UnormSrgb, visc_rgb)),
        viscera_normal: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8Unorm, visc_norm)),
        brine_albedo: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8UnormSrgb, brine_rgb)),
        brine_normal: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8Unorm, brine_norm)),
        eye_albedo: images.add(make_rgba_image(N as u32, TextureFormat::Rgba8UnormSrgb, eye_rgb)),
    }
}

/// Populates `Mesh::ATTRIBUTE_TANGENT` orthogonally from `ATTRIBUTE_NORMAL` without requiring
/// `mikktspace` in `Cargo.toml`, so normal maps shade crisply on WebGL2.
fn add_tangents(mesh: &mut Mesh) {
    let Some(VertexAttributeValues::Float32x3(norms)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else {
        return;
    };
    let tangents: Vec<[f32; 4]> = norms
        .iter()
        .map(|n_arr| {
            let n = Vec3::from_array(*n_arr).normalize_or(Vec3::Y);
            let up = if n.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
            let t = up.cross(n).normalize_or(Vec3::X);
            [t.x, t.y, t.z, 1.0]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
}

fn add_mesh_t(meshes: &mut Assets<Mesh>, mut mesh: Mesh) -> Handle<Mesh> {
    add_tangents(&mut mesh);
    meshes.add(mesh)
}

fn add_geo_t(meshes: &mut Assets<Mesh>, g: crate::shapes::Geo) -> Handle<Mesh> {
    add_mesh_t(meshes, g.mesh())
}

// =================================================================================================
// Live CPU vertex-deformed boiling biomass mound (`LexieMeltMoundMesh`)
// =================================================================================================

const MOUND_RINGS: usize = 14;
const MOUND_SEGS: usize = 28;
const MOUND_RADIUS: f32 = 4.9;

#[inline]
fn mound_height_and_grad(x: f32, z: f32, t: f32, melt_amt: f32, slump_amt: f32) -> (f32, f32, f32) {
    let r2 = x * x + z * z;
    let r = r2.sqrt();
    // Spread radius starts compact (2.1m) during Beats 0-1, then engorges to 4.6m during melt
    let spread = 2.1 + melt_amt * 2.55;
    let env = (-r2 / (spread * spread * 0.42)).exp();
    // Central heaped body carcass rises in Beat 2 and slumps into a heaving soup by Beat 3-4
    let dome_h = 0.14 + melt_amt * 0.88 - slump_amt * 0.42;
    let core_env = (-r2 / (1.45 * 1.45)).exp();
    let wave_a = (r * 3.8 - t * 4.4 + (z * 1.7 + x * 1.2).sin() * 1.1).sin();
    let wave_b = (x * 4.2 + t * 2.6).cos() * (z * 3.9 - t * 3.1).sin();
    let bubble_bump = ((r * 5.2 - t * 6.0).sin() * 0.5 + 0.5).powi(3);
    let amp = 0.04 + melt_amt * 0.11;
    let y = (dome_h * core_env + 0.12 * env + (wave_a * 0.55 + wave_b * 0.45 + bubble_bump * 0.5) * amp * env)
        .max(0.015);
    // Analytic-style finite gradient using fast local differences
    let eps = 0.10;
    let sample_y = |px: f32, pz: f32| -> f32 {
        let pr2 = px * px + pz * pz;
        let pr = pr2.sqrt();
        let penv = (-pr2 / (spread * spread * 0.42)).exp();
        let pc = (-pr2 / (1.45 * 1.45)).exp();
        let pwa = (pr * 3.8 - t * 4.4 + (pz * 1.7 + px * 1.2).sin() * 1.1).sin();
        let pwb = (px * 4.2 + t * 2.6).cos() * (pz * 3.9 - t * 3.1).sin();
        let pbb = ((pr * 5.2 - t * 6.0).sin() * 0.5 + 0.5).powi(3);
        (dome_h * pc + 0.12 * penv + (pwa * 0.55 + pwb * 0.45 + pbb * 0.5) * amp * penv).max(0.015)
    };
    let dhdx = (sample_y(x + eps, z) - y) / eps;
    let dhdz = (sample_y(x, z + eps) - y) / eps;
    (y, dhdx, dhdz)
}

fn build_melt_mound_mesh() -> Mesh {
    let verts = (MOUND_RINGS + 1) * (MOUND_SEGS + 1);
    let mut pos = Vec::with_capacity(verts);
    let mut norm = Vec::with_capacity(verts);
    let mut tang = Vec::with_capacity(verts);
    let mut uv = Vec::with_capacity(verts);
    let mut idx = Vec::with_capacity(MOUND_RINGS * MOUND_SEGS * 6);

    for ir in 0..=MOUND_RINGS {
        let fr = ir as f32 / MOUND_RINGS as f32;
        let r = fr.powf(0.92) * MOUND_RADIUS;
        for ia in 0..=MOUND_SEGS {
            let fa = ia as f32 / MOUND_SEGS as f32;
            let ang = fa * TAU;
            let x = ang.cos() * r;
            let z = ang.sin() * r;
            pos.push([x, 0.04, z]);
            norm.push([0.0, 1.0, 0.0]);
            tang.push([1.0, 0.0, 0.0, 1.0]);
            uv.push([x * 0.45 + 0.5, z * 0.45 + 0.5]);
        }
    }
    let stride = (MOUND_SEGS + 1) as u32;
    for ir in 0..MOUND_RINGS as u32 {
        for ia in 0..MOUND_SEGS as u32 {
            let a = ir * stride + ia;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    // Uses MAIN_WORLD | RENDER_WORLD so update_lexie_victory_cutscene can deform vertices live.
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, norm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tang)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

fn update_melt_mound_mesh(mesh: &mut Mesh, t: f32, melt_amt: f32, slump_amt: f32) {
    let mut new_norms: Vec<[f32; 3]> = Vec::new();
    let mut new_tangs: Vec<[f32; 4]> = Vec::new();
    if let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        new_norms.reserve(pos.len());
        new_tangs.reserve(pos.len());
        for p in pos.iter_mut() {
            let (y, dhdx, dhdz) = mound_height_and_grad(p[0], p[2], t, melt_amt, slump_amt);
            p[1] = y;
            let n = Vec3::new(-dhdx, 1.0, -dhdz).normalize_or(Vec3::Y);
            let tan = Vec3::new(1.0, dhdx, 0.0).normalize_or(Vec3::X);
            new_norms.push([n.x, n.y, n.z]);
            new_tangs.push([tan.x, tan.y, tan.z, 1.0]);
        }
    }
    if !new_norms.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, new_norms);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, new_tangs);
    }
}

// =================================================================================================
// Deterministic per-frame kinematic rig pose solver (shared by Head, Chest, Eyestalks & Tendons)
// =================================================================================================

struct RigPose {
    lower_pos: Vec3,
    lower_scale: Vec3,
    chest_spread: f32,
    chest_open_angle: f32,
    chest_sink_y: f32,
    head_pos: Vec3,
    head_rot: Quat,
    head_scale: f32,
    jaw_open: f32,
    eye_extend: f32,
    spider_deploy: f32,
    spider_crawl_phase: f32,
    tentacle_reach: f32,
    melt_amt: f32,
    slump_amt: f32,
    tendon_stretch: f32,
    collar_anchor: Vec3,
    head_neck_anchor: Vec3,
}

fn solve_horror_rig(stage: Vec3, beat: u8, bt: f32, t: f32) -> RigPose {
    let base_xz = Vec3::new(stage.x, stage.y, stage.z - 2.2);
    let shake_x = (t * 26.0).sin() * 0.032;
    let shake_z = (t * 19.0).cos() * 0.025;

    let (
        lower_sink,
        lower_bloat,
        chest_spread,
        chest_open_angle,
        chest_sink_y,
        head_offset,
        head_pitch,
        head_yaw,
        head_roll,
        head_scale,
        jaw_open,
        eye_extend,
        spider_deploy,
        spider_crawl_phase,
        tentacle_reach,
        melt_amt,
        slump_amt,
        tendon_stretch,
    ) = match beat {
        0 => {
            // Beat 0: shuddering contortion close-up — jaw starts unhinging, eyes asymmetrically twitch
            let u = (bt / 4.0).clamp(0.0, 1.0);
            (
                0.0,
                Vec3::new(1.0 + u * 0.14, 1.0 - u * 0.04, 1.0 + u * 0.14),
                0.02 * u + (t * 28.0).sin().abs() * 0.02,
                0.08 * u,
                0.0,
                Vec3::new(shake_x, 2.38 + (t * 18.0).sin() * 0.03, 0.05),
                -0.12 - (t * 15.0).sin().abs() * 0.14,
                (t * 11.0).sin() * 0.18,
                (t * 22.0).sin() * 0.14,
                0.62 * (1.0 + u * 0.10),
                0.28 + 0.42 * u + (t * 14.0).sin().abs() * 0.12,
                0.10 * u,
                0.02,
                0.0,
                0.05,
                0.15 + u * 0.15,
                0.0,
                0.2,
            )
        }
        1 => {
            // Beat 1: crew recoils — torso swells ominously ready to burst
            let u = (bt / 4.0).clamp(0.0, 1.0);
            let heave = (t * 9.0).sin() * 0.05;
            (
                -0.06 * u,
                Vec3::new(1.16 + heave, 0.95, 1.18 + heave),
                0.07 + heave.abs(),
                0.22 + heave.abs() * 0.4,
                -0.05 * u,
                Vec3::new(shake_x * 1.4, 2.40 + u * 0.12, 0.06),
                -0.25 + (t * 14.0).sin() * 0.16,
                (t * 8.0).sin() * 0.25,
                (t * 19.0).sin() * 0.22,
                0.68,
                0.65 + (t * 10.0).sin().abs() * 0.18,
                0.18 + u * 0.14,
                0.12 * u,
                0.0,
                0.18,
                0.35 + u * 0.20,
                0.0,
                0.55,
            )
        }
        2 => {
            // Beat 2: THE RUPTURE — chest splits wide into snapping rib-maw, tentacles & neck tendons stretch!
            let u = (bt / 3.2).clamp(0.0, 1.0);
            let snap = (u * 1.6).clamp(0.0, 1.0);
            let neck_y = 2.45 + snap * 0.72 + (t * 11.0).sin() * 0.12;
            (
                -0.22 * u,
                Vec3::new(1.25 + u * 0.20, 0.85 - u * 0.15, 1.25 + u * 0.20),
                0.18 + snap * 0.36,
                0.25 + snap * 0.88,
                -0.12 * u,
                Vec3::new((t * 7.5).sin() * 0.22, neck_y, 0.08 + snap * 0.32),
                -0.42 + (t * 9.5).sin() * 0.22,
                (t * 6.5).sin() * 0.48,
                (t * 13.0).sin() * 0.32,
                0.72,
                0.88 + (t * 16.0).sin() * 0.15,
                0.32 + snap * 0.50,
                0.15 + snap * 0.85,
                t * 10.0,
                0.20 + snap * 0.95,
                0.55 + u * 0.35,
                0.05 * u,
                1.0,
            )
        }
        3 => {
            // Beat 3: TOTAL LIQUEFACTION — torso melts into boiling biomass while detached Spider-Head crawls forward!
            let u = (bt / 4.2).clamp(0.0, 1.0);
            let crawl_z = 0.55 + u * 1.35 + (t * 14.0).sin() * 0.05;
            let crawl_x = (t * 2.8).sin() * 0.34;
            let crawl_y = 0.46 + (t * 14.0).cos().abs() * 0.06;
            (
                -0.35 - u * 0.35,
                Vec3::new(1.48 + u * 0.25, 0.55 - u * 0.28, 1.48 + u * 0.25),
                0.58 + u * 0.25,
                1.18 + u * 0.35,
                -0.28 - u * 0.52,
                Vec3::new(crawl_x, crawl_y, crawl_z),
                0.16 + (t * 12.0).sin() * 0.14,
                (t * 3.4).sin() * 0.36,
                (t * 9.0).sin() * 0.14,
                0.70,
                0.55 + (t * 18.0).sin().abs() * 0.38,
                0.72 + (t * 4.0).sin() * 0.14,
                1.0,
                t * 17.0,
                1.05 - u * 0.25,
                1.0,
                0.35 + u * 0.45,
                0.22,
            )
        }
        _ => {
            // Beat 4: Twitching liquefied remains & skittered Spider-Head panting on the slick puddle
            let u = (bt / 4.0).clamp(0.0, 1.0);
            (
                -0.72,
                Vec3::new(1.70, 0.25, 1.70),
                0.82,
                1.48,
                -0.82,
                Vec3::new(-0.22 + (t * 1.8).sin() * 0.12, 0.42 + (t * 5.0).sin() * 0.025, 1.72),
                0.10 + (t * 4.5).sin() * 0.08,
                (t * 2.2).sin() * 0.30,
                (t * 3.5).sin() * 0.10,
                0.68,
                0.32 + (t * 6.0).sin().abs() * 0.22,
                0.68 - u * 0.12,
                0.96,
                t * 5.5,
                0.52 - u * 0.20,
                1.0,
                0.90,
                0.08,
            )
        }
    };

    let head_pos = base_xz + head_offset;
    let head_rot = Quat::from_euler(EulerRot::YXZ, head_yaw, head_pitch, head_roll);
    let collar_anchor = base_xz + Vec3::new(0.0, (2.15 + chest_sink_y).max(0.28), 0.04);
    let head_neck_anchor = head_pos + head_rot * Vec3::new(0.0, -0.26 * head_scale, -0.04 * head_scale);

    RigPose {
        lower_pos: base_xz + Vec3::new(shake_x * 0.4, lower_sink, shake_z * 0.4),
        lower_scale: lower_bloat * 0.62,
        chest_spread,
        chest_open_angle,
        chest_sink_y,
        head_pos,
        head_rot,
        head_scale,
        jaw_open,
        eye_extend,
        spider_deploy,
        spider_crawl_phase,
        tentacle_reach,
        melt_amt,
        slump_amt,
        tendon_stretch,
        collar_anchor,
        head_neck_anchor,
    }
}

pub fn enter_lexie_victory_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut v_state: ResMut<LexieVictoryState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut clear_col: ResMut<ClearColor>,
) {
    use crate::shapes as sh;

    *v_state = LexieVictoryState::default();
    for mut hv in &mut hud_q {
        *hv = Visibility::Hidden;
    }
    // Deep cold dark-slate night horizon contrasting with vivid crimson viscera & acid-green bile.
    clear_col.0 = Color::srgb(0.02, 0.015, 0.035);
    let stage = Vec3::new(0.0, 0.0, -420.0);

    // 4-point practical John Carpenter horror lighting rig
    commands.spawn((
        LexieVictoryEntity,
        LexieHorrorLight { role: HorrorLightRole::FlickerOverhead },
        PointLight {
            color: Color::srgb(0.86, 0.94, 1.0),
            intensity: 640_000.0,
            range: 36.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.4, 5.4, -0.6)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        LexieHorrorLight { role: HorrorLightRole::CrimsonCore },
        PointLight {
            color: Color::srgb(1.0, 0.14, 0.10),
            intensity: 240_000.0,
            range: 18.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 1.35, -1.65)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        LexieHorrorLight { role: HorrorLightRole::BileRim },
        PointLight {
            color: Color::srgb(0.55, 1.0, 0.18),
            intensity: 210_000.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(-2.4, 1.4, -3.3)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        LexieHorrorLight { role: HorrorLightRole::CrewFill },
        PointLight {
            color: Color::srgb(1.0, 0.92, 0.84),
            intensity: 380_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 4.6, 3.8)),
    ));

    // Procedural 256x256 PBR textures + ultra-wet practical-effects materials
    let tex = make_bio_textures(&mut images);
    let flesh_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.90, 0.88),
        base_color_texture: Some(tex.flesh_albedo),
        normal_map_texture: Some(tex.flesh_normal),
        metallic_roughness_texture: Some(tex.flesh_mr),
        emissive: LinearRgba::new(0.09, 0.015, 0.015, 1.0),
        reflectance: 0.94,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.04,
        ..default()
    });
    let viscera_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex.viscera_albedo),
        normal_map_texture: Some(tex.viscera_normal),
        emissive: LinearRgba::new(0.20, 0.012, 0.016, 1.0),
        perceptual_roughness: 0.05,
        reflectance: 0.96,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.03,
        ..default()
    });
    let brine_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex.brine_albedo),
        normal_map_texture: Some(tex.brine_normal),
        emissive: LinearRgba::new(0.14, 0.34, 0.025, 1.0),
        perceptual_roughness: 0.04,
        reflectance: 1.0,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.03,
        ..default()
    });
    let tendon_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.70, 0.66, 0.82),
        emissive: LinearRgba::new(0.16, 0.04, 0.04, 1.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.04,
        reflectance: 0.96,
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.02,
        ..default()
    });
    let bone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.84, 0.68),
        perceptual_roughness: 0.18,
        reflectance: 0.8,
        clearcoat: 0.70,
        clearcoat_perceptual_roughness: 0.08,
        ..default()
    });
    let pus_boil_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.78, 0.24, 0.86),
        emissive: LinearRgba::new(0.45, 0.34, 0.04, 1.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.05,
        clearcoat: 1.0,
        ..default()
    });
    let blood_drop_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.03, 0.04),
        emissive: LinearRgba::new(0.28, 0.01, 0.015, 1.0),
        perceptual_roughness: 0.04,
        reflectance: 1.0,
        clearcoat: 1.0,
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex.eye_albedo),
        emissive: LinearRgba::new(0.32, 0.18, 0.16, 1.0),
        perceptual_roughness: 0.04,
        clearcoat: 1.0,
        ..default()
    });
    let street_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.06, 0.08),
        perceptual_roughness: 0.22,
        reflectance: 0.75,
        ..default()
    });
    let stink_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.48, 0.78, 0.16, 0.24),
        emissive: LinearRgba::new(0.12, 0.28, 0.03, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let pink_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.20, 0.50),
        perceptual_roughness: 0.38,
        clearcoat: 0.45,
        ..default()
    });
    let denim_shorts = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.26, 0.46),
        perceptual_roughness: 0.52,
        clearcoat: 0.35,
        ..default()
    });
    let blonde_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.28),
        perceptual_roughness: 0.35,
        clearcoat: 0.45,
        ..default()
    });
    let sneaker_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.82, 0.78),
        perceptual_roughness: 0.45,
        ..default()
    });
    let iris_spiral = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.12, 0.95),
        emissive: LinearRgba::new(2.1, 0.18, 2.5, 1.0),
        ..default()
    });
    let pupil_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        ..default()
    });
    let mouth_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.01, 0.02),
        emissive: LinearRgba::new(0.18, 0.01, 0.01, 1.0),
        perceptual_roughness: 0.05,
        ..default()
    });

    // Moody colored Uzumaki spiral rings behind the horror stage
    let spiral_colors = [
        LinearRgba::new(0.28, 1.65, 0.10, 1.0),
        LinearRgba::new(1.75, 0.10, 0.55, 1.0),
        LinearRgba::new(1.55, 1.15, 0.08, 1.0),
    ];
    let ring_mesh = meshes.add(Torus::new(0.055, 0.90));
    for ring_i in 0..7 {
        let sc = 1.05 + ring_i as f32 * 0.72;
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: spiral_colors[ring_i % spiral_colors.len()],
            unlit: true,
            ..default()
        });
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim {
                kind: ThingPart::SpiralRing {
                    speed: if ring_i % 2 == 0 { 1.4 } else { -1.8 },
                    base_scale: sc,
                },
            },
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(ring_mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.9, -5.2))
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::splat(sc)),
        ));
    }

    // Wet asphalt stage + live CPU vertex-deformed boiling biomass puddle/mound
    commands.spawn((
        LexieVictoryEntity,
        Mesh3d(meshes.add(Cuboid::new(32.0, 0.2, 32.0))),
        MeshMaterial3d(street_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    let mound_handle = meshes.add(build_melt_mound_mesh());
    commands.spawn((
        LexieVictoryEntity,
        LexieMeltMoundMesh(mound_handle.clone()),
        Mesh3d(mound_handle),
        MeshMaterial3d(brine_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 0.0, -2.2)),
    ));

    // Shared real Lexie body meshes (`city::lexie_meshes`) + populate tangents on meshes using normal maps
    let lm = crate::city::lexie_meshes(&mut meshes);
    for h in [&lm.leg, &lm.head, &lm.sleeve, &lm.hand_l, &lm.torso] {
        if let Some(m) = meshes.get_mut(h) {
            add_tangents(m);
        }
    }

    // 1. LexieLowerBody: sneakers, legs, shorts, lower hoodie
    commands
        .spawn((
            LexieVictoryEntity,
            ThingAnim { kind: ThingPart::LowerBody },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -2.2)).with_scale(Vec3::splat(0.62)),
            Visibility::default(),
        ))
        .with_children(|lb| {
            for sx in [-1.0_f32, 1.0_f32] {
                lb.spawn((
                    Mesh3d(lm.shoe.clone()),
                    MeshMaterial3d(sneaker_mat.clone()),
                    Transform::from_xyz(sx * 0.46, 0.18, 0.10),
                ));
                lb.spawn((
                    Mesh3d(lm.sole.clone()),
                    MeshMaterial3d(pink_hoodie.clone()),
                    Transform::from_xyz(sx * 0.46, 0.18, 0.10),
                ));
                lb.spawn((
                    Mesh3d(lm.leg.clone()),
                    MeshMaterial3d(flesh_mat.clone()),
                    Transform::from_xyz(sx * 0.46, 0.92, 0.0).with_scale(Vec3::new(0.75, 1.0, 0.75)),
                ));
                lb.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.32))),
                    MeshMaterial3d(denim_shorts.clone()),
                    Transform::from_xyz(sx * 0.44, 1.56, 0.0),
                ));
            }
            lb.spawn((
                Mesh3d(meshes.add(Sphere::new(0.66))),
                MeshMaterial3d(denim_shorts.clone()),
                Transform::from_xyz(0.0, 1.82, 0.04).with_scale(Vec3::new(1.32, 0.70, 1.12)),
            ));
        });

    // 2. LexieChestHalf (-1.0 Left / +1.0 Right): split pink hoodie torso + visceral lining + 12 jagged bone rib-teeth + flung arms
    let rib_tooth_mesh = add_geo_t(
        &mut meshes,
        sh::limb_profile(0.46, &[0.06, 0.048, 0.032, 0.012], 0.85).at(0.0, 0.23, 0.0),
    );
    let chest_outer_mesh = add_geo_t(&mut meshes, sh::superellipsoid(Vec3::new(0.36, 0.56, 0.46), 0.75, 0.85, 16, 12));
    let chest_inner_mesh = add_geo_t(&mut meshes, sh::superellipsoid(Vec3::new(0.24, 0.50, 0.36), 0.75, 0.85, 14, 10));
    for side in [-1.0_f32, 1.0_f32] {
        commands
            .spawn((
                LexieVictoryEntity,
                ThingAnim { kind: ThingPart::ChestHalf { side } },
                Transform::from_translation(stage + Vec3::new(side * 0.16, 1.16, -2.2)),
                Visibility::default(),
            ))
            .with_children(|ch| {
                ch.spawn((
                    Mesh3d(chest_outer_mesh.clone()),
                    MeshMaterial3d(pink_hoodie.clone()),
                    Transform::from_xyz(side * 0.16, 0.48, 0.0),
                ));
                ch.spawn((
                    Mesh3d(chest_inner_mesh.clone()),
                    MeshMaterial3d(viscera_mat.clone()),
                    Transform::from_xyz(-side * 0.04, 0.48, 0.06),
                ));
                // 6 curved jagged ivory rib-teeth along the sternum split
                for ri in 0..6 {
                    let ry = 0.12 + ri as f32 * 0.15;
                    let fwd = 0.24 - (ri as f32 - 2.5).abs() * 0.025;
                    ch.spawn((
                        Mesh3d(rib_tooth_mesh.clone()),
                        MeshMaterial3d(bone_mat.clone()),
                        Transform::from_xyz(-side * 0.06, ry, fwd)
                            .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.18, 0.0, side * 1.18)),
                    ));
                }
                // Flung pink-hoodie sleeve + veined flesh hand
                let arm_tf = Transform::from_xyz(side * 0.42, 0.72, 0.08)
                    .with_rotation(Quat::from_euler(EulerRot::XYZ, -0.78, 0.0, side * 0.52));
                ch.spawn((
                    Mesh3d(lm.sleeve.clone()),
                    MeshMaterial3d(pink_hoodie.clone()),
                    arm_tf.with_scale(Vec3::new(0.32, 0.50, 0.32)),
                ));
                ch.spawn((
                    Mesh3d(lm.hand_l.clone()),
                    MeshMaterial3d(flesh_mat.clone()),
                    Transform::from_translation(arm_tf.transform_point(Vec3::new(0.0, -0.38, 0.0)))
                        .with_rotation(arm_tf.rotation)
                        .with_scale(Vec3::splat(0.46)),
                ));
            });
    }

    // 3. Exposed throbbing inner organs inside the chest maw
    let organ_mesh = add_geo_t(&mut meshes, sh::superellipsoid(Vec3::new(0.22, 0.28, 0.20), 0.72, 0.82, 16, 12));
    for lobe in 0..4 {
        let mat = if lobe % 2 == 0 { viscera_mat.clone() } else { flesh_mat.clone() };
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim { kind: ThingPart::InnerOrgan { lobe } },
            Mesh3d(organ_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.56, -2.18)),
        ));
    }

    // 4. Articulated Mutating Head Assembly (face, boils, open gullet, unhinging jaw, telescoping eyestalks & spider legs!)
    let eye_ball_mesh = add_mesh_t(&mut meshes, Mesh::from(Sphere::new(0.135).mesh().uv(20, 14)));
    // Unit-length (0..1 along +Y) eyestalk & base-pivoted spider leg segments so joints connect cleanly
    let eyestalk_mesh = add_geo_t(&mut meshes, sh::limb(1.0, 0.060, 0.038).at(0.0, 0.5, 0.0));
    let jaw_mesh = add_geo_t(&mut meshes, sh::superellipsoid(Vec3::new(0.36, 0.20, 0.38), 0.72, 0.85, 16, 10));
    let tongue_mesh = add_geo_t(&mut meshes, sh::limb_profile(0.42, &[0.09, 0.11, 0.08, 0.03], 0.55));
    let tooth_mesh = add_geo_t(&mut meshes, sh::limb_profile(0.11, &[0.026, 0.018, 0.005], 0.85));
    let spider_femur_mesh = add_geo_t(
        &mut meshes,
        sh::limb_profile(0.86, &[0.068, 0.056, 0.044, 0.036], 0.85).at(0.0, 0.43, 0.0),
    );
    let spider_tibia_mesh = add_geo_t(
        &mut meshes,
        sh::limb_profile(1.02, &[0.044, 0.032, 0.016, 0.004], 0.85).at(0.0, 0.51, 0.0),
    );

    commands
        .spawn((
            LexieVictoryEntity,
            ThingAnim { kind: ThingPart::HeadAssembly },
            Transform::from_translation(stage + Vec3::new(0.0, 2.38, -2.15)).with_scale(Vec3::splat(0.62)),
            Visibility::default(),
        ))
        .with_children(|hd| {
            // Vein-swollen necrotic skull + double chin + blonde hair/mane/bun
            hd.spawn((
                Mesh3d(lm.head.clone()),
                MeshMaterial3d(flesh_mat.clone()),
                Transform::default(),
            ));
            hd.spawn((
                Mesh3d(lm.hair.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::default(),
            ));
            hd.spawn((
                Mesh3d(lm.mane.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, -0.30, -0.41),
            ));
            hd.spawn((
                Mesh3d(meshes.add(Sphere::new(0.36))),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 0.77, -0.24),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                hd.spawn((
                    Mesh3d(if sx < 0.0 { lm.lock_l.clone() } else { lm.lock_r.clone() }),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(sx * 0.56, -0.27, -0.02),
                ));
            }
            // Gaping esophageal cavity + upper jagged teeth
            hd.spawn((
                Mesh3d(meshes.add(Sphere::new(0.30))),
                MeshMaterial3d(mouth_dark.clone()),
                Transform::from_xyz(0.0, -0.18, 0.40).with_scale(Vec3::new(1.15, 0.95, 0.75)),
            ));
            for ti in 0..7 {
                let tx = (ti as f32 - 3.0) * 0.078;
                let tz = 0.58 - tx * tx * 0.85;
                hd.spawn((
                    Mesh3d(tooth_mesh.clone()),
                    MeshMaterial3d(bone_mat.clone()),
                    Transform::from_xyz(tx, -0.10, tz)
                        .with_rotation(Quat::from_rotation_x(PI - 0.14)),
                ));
            }

            // Articulated unhinging lower jaw with tongue + lower teeth
            hd.spawn((
                ThingAnim { kind: ThingPart::Jaw },
                Transform::from_xyz(0.0, -0.24, 0.16),
                Visibility::default(),
            ))
            .with_children(|jw| {
                jw.spawn((
                    Mesh3d(jaw_mesh.clone()),
                    MeshMaterial3d(flesh_mat.clone()),
                    Transform::from_xyz(0.0, -0.12, 0.24),
                ));
                jw.spawn((
                    Mesh3d(tongue_mesh.clone()),
                    MeshMaterial3d(viscera_mat.clone()),
                    Transform::from_xyz(0.0, -0.02, 0.30)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2 + 0.18)),
                ));
                for ti in 0..7 {
                    let tx = (ti as f32 - 3.0) * 0.072;
                    let tz = 0.46 - tx * tx * 0.80;
                    jw.spawn((
                        Mesh3d(tooth_mesh.clone()),
                        MeshMaterial3d(bone_mat.clone()),
                        Transform::from_xyz(tx, -0.02, tz)
                            .with_rotation(Quat::from_rotation_x(0.14)),
                    ));
                }
            });

            // 2 telescoping bloodshot spiral eyestalks (base stalk + tip eyeball)
            for (idx, side) in [-1.0_f32, 1.0_f32].iter().enumerate() {
                let sc = if idx == 0 { 1.18 } else { 0.94 };
                hd.spawn((
                    ThingAnim { kind: ThingPart::Eyestalk { side: *side, seg: 0 } },
                    Mesh3d(eyestalk_mesh.clone()),
                    MeshMaterial3d(viscera_mat.clone()),
                    Transform::from_xyz(side * 0.21, 0.11, 0.38)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                ));
                hd.spawn((
                    ThingAnim { kind: ThingPart::Eyestalk { side: *side, seg: 1 } },
                    Transform::from_xyz(side * 0.21, 0.11, 0.56).with_scale(Vec3::splat(sc)),
                    Visibility::default(),
                ))
                .with_children(|eye| {
                    eye.spawn((
                        Mesh3d(eye_ball_mesh.clone()),
                        MeshMaterial3d(eye_white.clone()),
                    ));
                    eye.spawn((
                        Mesh3d(meshes.add(Torus::new(0.026, 0.072))),
                        MeshMaterial3d(iris_spiral.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.105)
                            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                    ));
                    eye.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.036))),
                        MeshMaterial3d(pupil_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.135),
                    ));
                });
            }

            // Swollen cheek & forehead pus boils
            for (i, (bx, by, bz)) in [
                (-0.44_f32, -0.06_f32, 0.36_f32),
                (0.45, -0.04, 0.35),
                (-0.24, 0.34, 0.40),
                (0.26, 0.32, 0.38),
                (-0.36, 0.14, 0.42),
                (0.35, -0.20, 0.34),
            ]
            .iter()
            .enumerate()
            {
                hd.spawn((
                    ThingAnim {
                        kind: ThingPart::Boil {
                            base_pos: Vec3::new(*bx, *by, *bz),
                            base_scale: 0.085 + (i % 2) as f32 * 0.03,
                            phase: i as f32 * 1.15,
                        },
                    },
                    Mesh3d(meshes.add(Sphere::new(1.0))),
                    MeshMaterial3d(if i % 2 == 0 { pus_boil_mat.clone() } else { viscera_mat.clone() }),
                    Transform::from_xyz(*bx, *by, *bz).with_scale(Vec3::splat(0.09)),
                ));
            }

            // 6 multi-jointed arachnid Spider-Legs bursting from the neck/cheeks
            for side in [-1.0_f32, 1.0_f32] {
                for leg_i in 0..3 {
                    let socket = Vec3::new(side * 0.38, -0.20 + leg_i as f32 * 0.06, -0.08 + leg_i as f32 * 0.16);
                    hd.spawn((
                        ThingAnim { kind: ThingPart::SpiderLeg { idx: leg_i, side } },
                        Mesh3d(spider_femur_mesh.clone()),
                        MeshMaterial3d(viscera_mat.clone()),
                        Transform::from_translation(socket),
                    ))
                    .with_children(|fem| {
                        fem.spawn((
                            ThingAnim { kind: ThingPart::SpiderShin { idx: leg_i, side } },
                            Mesh3d(spider_tibia_mesh.clone()),
                            MeshMaterial3d(bone_mat.clone()),
                            Transform::from_xyz(0.0, 0.84, 0.0),
                        ));
                    });
                }
            }
        });

    // 5. 10 Multi-segment writhing tentacles erupting out of the ruptured torso cavity (base-pivoted 0..len)
    let tent_seg_meshes: [Handle<Mesh>; 3] = [
        add_geo_t(&mut meshes, sh::limb_profile(0.95, &[0.14, 0.18, 0.24, 0.20], 0.88).at(0.0, 0.475, 0.0)),
        add_geo_t(&mut meshes, sh::limb_profile(0.88, &[0.09, 0.12, 0.17, 0.14], 0.88).at(0.0, 0.44, 0.0)),
        add_geo_t(&mut meshes, sh::limb_profile(0.82, &[0.015, 0.05, 0.11, 0.09], 0.85).at(0.0, 0.41, 0.0)),
    ];
    let cyst_mesh = add_mesh_t(&mut meshes, Mesh::from(Sphere::new(0.11).mesh().uv(12, 8)));
    for t_i in 0..10 {
        let ang = (t_i as f32 / 10.0) * TAU + 0.18;
        let seg0_mat = if t_i % 2 == 0 { viscera_mat.clone() } else { flesh_mat.clone() };
        let seg1_mat = if t_i % 2 == 0 { flesh_mat.clone() } else { viscera_mat.clone() };
        commands
            .spawn((
                ThingAnim { kind: ThingPart::TentacleRoot { idx: t_i, base_angle: ang } },
                LexieVictoryEntity,
                Transform::from_translation(stage + Vec3::new(0.0, 1.32, -2.18)),
                Visibility::default(),
            ))
            .with_children(|root| {
                root.spawn((
                    ThingAnim { kind: ThingPart::TentacleSeg { idx: t_i, seg: 0 } },
                    Mesh3d(tent_seg_meshes[0].clone()),
                    MeshMaterial3d(seg0_mat),
                    Transform::default(),
                ))
                .with_children(|s0| {
                    s0.spawn((
                        Mesh3d(cyst_mesh.clone()),
                        MeshMaterial3d(pus_boil_mat.clone()),
                        Transform::from_xyz(0.14, 0.45, 0.04).with_scale(Vec3::splat(0.95)),
                    ));
                    s0.spawn((
                        ThingAnim { kind: ThingPart::TentacleSeg { idx: t_i, seg: 1 } },
                        Mesh3d(tent_seg_meshes[1].clone()),
                        MeshMaterial3d(seg1_mat.clone()),
                        Transform::from_xyz(0.0, 0.90, 0.0),
                    ))
                    .with_children(|s1| {
                        s1.spawn((
                            Mesh3d(cyst_mesh.clone()),
                            MeshMaterial3d(pus_boil_mat.clone()),
                            Transform::from_xyz(-0.10, 0.42, 0.06).with_scale(Vec3::splat(0.75)),
                        ));
                        s1.spawn((
                            ThingAnim { kind: ThingPart::TentacleSeg { idx: t_i, seg: 2 } },
                            Mesh3d(tent_seg_meshes[2].clone()),
                            MeshMaterial3d(viscera_mat.clone()),
                            Transform::from_xyz(0.0, 0.84, 0.0),
                        ));
                    });
                });
            });
    }

    // 6. 8 Glistening stretching mucus/sinew tendon cables
    let tendon_mesh = add_mesh_t(&mut meshes, Mesh::from(Cylinder::new(0.032, 1.0)));
    for s_i in 0..8 {
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim { kind: ThingPart::TendonStrand { idx: s_i, jaw_strand: s_i >= 5 } },
            Mesh3d(tendon_mesh.clone()),
            MeshMaterial3d(if s_i % 3 == 0 { viscera_mat.clone() } else { tendon_mat.clone() }),
            Transform::from_translation(stage + Vec3::new(0.0, 2.0, -2.2)),
        ));
    }

    // 7. 18 Boiling pus/bile blisters across the biomass mound
    let blister_mesh = add_mesh_t(&mut meshes, Mesh::from(Sphere::new(1.0).mesh().uv(14, 10)));
    for b_i in 0..18 {
        let a = (b_i as f32 * 2.399) % TAU;
        let rad = 0.55 + ((b_i as f32 * 1.37) % 1.0) * 3.1;
        let bpos = stage + Vec3::new(a.cos() * rad, 0.08, -2.2 + a.sin() * rad);
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim {
                kind: ThingPart::BubbleDome {
                    base_pos: bpos,
                    max_r: 0.14 + (b_i % 4) as f32 * 0.065,
                    phase: b_i as f32 * 0.53,
                    speed: 0.55 + (b_i % 3) as f32 * 0.24,
                },
            },
            Mesh3d(blister_mesh.clone()),
            MeshMaterial3d(if b_i % 3 == 0 { pus_boil_mat.clone() } else { brine_mat.clone() }),
            Transform::from_translation(bpos).with_scale(Vec3::splat(0.02)),
        ));
    }

    // 8. 36 Velocity-stretched arterial blood, acid-bile, and garlic-clot spray droplets
    let jet_mesh = meshes.add(Sphere::new(0.085));
    for j_i in 0..36 {
        let ang = (j_i as f32 / 36.0) * TAU * 3.0;
        let spd = 0.85 + (j_i % 5) as f32 * 0.42;
        let mat = match j_i % 4 {
            0 | 1 => blood_drop_mat.clone(),
            2 => brine_mat.clone(),
            _ => bone_mat.clone(),
        };
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim {
                kind: ThingPart::VisceraJet {
                    base_pos: stage + Vec3::new(0.0, 1.55, -2.05),
                    vel: Vec3::new(ang.cos() * spd, 2.8 + (j_i % 4) as f32 * 0.85, ang.sin() * spd * 0.72),
                    phase: j_i as f32 * 0.19,
                    speed: 1.05 + (j_i % 3) as f32 * 0.22,
                },
            },
            Mesh3d(jet_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.55, -2.05)),
        ));
    }

    // 9. 10 Hot steamy toxic miasma plumes rising off the boiling remains
    let cloud_mesh = meshes.add(Sphere::new(0.38));
    for c_i in 0..10 {
        let ang = PI + (c_i as f32 / 9.0) * PI;
        let cpos = stage + Vec3::new(ang.cos() * 2.4, 0.28, -2.2 + ang.sin() * 1.4);
        commands.spawn((
            LexieVictoryEntity,
            ThingAnim { kind: ThingPart::StinkCloud { base_pos: cpos, phase: c_i as f32 * 0.41 } },
            Mesh3d(cloud_mesh.clone()),
            MeshMaterial3d(stink_mat.clone()),
            Transform::from_translation(cpos),
        ));
    }

    // 10. All 5 REAL CREW AVATARS witnessing the meltdown from z = +2.5..+2.9 facing -Z
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        LexieVictoryEntity,
        SceneRoot(hero_scene),
        Transform::from_translation(stage + Vec3::new(0.0, 0.0, 2.5)).with_scale(Vec3::splat(1.25)),
    ));
    let derek_mats = crate::derek::make_derek_materials(&mut materials);
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(-1.35, 0.0, 2.65))
                .with_rotation(Quat::from_rotation_y(PI - 0.18))
                .with_scale(Vec3::splat(1.08)),
            Visibility::default(),
        ))
        .with_children(|dk| {
            crate::derek::spawn_derek_rig(dk, &mut meshes, &derek_mats, false);
        });
    let tj_ent = crate::tj::spawn_tj_character(
        &mut commands,
        &mut meshes,
        &mut materials,
        stage + Vec3::new(1.35, 0.0, 2.65),
        PI + 0.18,
        1.08,
        false,
    );
    commands.entity(tj_ent).insert(LexieVictoryEntity);

    let nate_mats = crate::nate::make_nate_materials(&mut materials);
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(-2.45, 0.0, 2.90))
                .with_rotation(Quat::from_rotation_y(PI - 0.28))
                .with_scale(Vec3::splat(1.08)),
            Visibility::default(),
        ))
        .with_children(|nt| {
            crate::nate::spawn_nate_rig(nt, &mut meshes, &nate_mats, false);
        });
    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(2.45, 0.0, 2.90))
                .with_rotation(Quat::from_rotation_y(PI + 0.28))
                .with_scale(Vec3::splat(1.08)),
            Visibility::default(),
        ))
        .with_children(|gl| {
            crate::glenn::spawn_glenn_rig(gl, &mut meshes, &glenn_mats);
        });

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.45, 2.38, -0.05);
        c_tf.look_at(stage + Vec3::new(0.0, 2.34, -2.18), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.62;
        }
    }

    let (spk, line, voice, b_left, b_right, _) = GROSS_BEATS[0];
    commands
        .spawn((
            LexieVictoryOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(20.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(92.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
            ))
            .with_children(|sfx_row| {
                sfx_row.spawn((
                    LexieBannerLeftText,
                    Text::new(b_left),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.58, 1.0, 0.20)),
                ));
                sfx_row.spawn((
                    LexieBannerRightText,
                    Text::new(b_right),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.24, 0.34)),
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(88.0),
                    max_width: Val::Px(820.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.015, 0.035, 0.90)),
                BorderColor(Color::srgb(0.85, 0.22, 0.24)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    LexieVictorySpeakerText,
                    Text::new(spk),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 1.0, 0.30)),
                ));
                card.spawn((
                    LexieVictorySubText,
                    Text::new(format!("\"{}\"", line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    play_headphone_music(2);
    play_sfx("bone_crack");
    play_sfx("thing_shriek");
    speak_voice(line, voice);
}

#[allow(clippy::type_complexity)]
pub fn update_lexie_victory_cutscene(
    time: Res<Time>,
    mut v_state: ResMut<LexieVictoryState>,
    mut next_state: crate::transition::SceneTransition,
    mut meshes: ResMut<Assets<Mesh>>,
    mound_q: Query<&LexieMeltMoundMesh>,
    mut part_q: Query<(&mut Transform, &ThingAnim), Without<Camera3d>>,
    mut light_q: Query<(&mut PointLight, &LexieHorrorLight)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<ThingAnim>)>,
    mut spk_q: Query<
        &mut Text,
        (
            With<LexieVictorySpeakerText>,
            Without<LexieVictorySubText>,
            Without<LexieBannerLeftText>,
            Without<LexieBannerRightText>,
        ),
    >,
    mut sub_q: Query<
        &mut Text,
        (
            With<LexieVictorySubText>,
            Without<LexieVictorySpeakerText>,
            Without<LexieBannerLeftText>,
            Without<LexieBannerRightText>,
        ),
    >,
    mut bl_q: Query<
        &mut Text,
        (
            With<LexieBannerLeftText>,
            Without<LexieVictorySpeakerText>,
            Without<LexieVictorySubText>,
            Without<LexieBannerRightText>,
        ),
    >,
    mut br_q: Query<
        &mut Text,
        (
            With<LexieBannerRightText>,
            Without<LexieVictorySpeakerText>,
            Without<LexieVictorySubText>,
            Without<LexieBannerLeftText>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    v_state.beat_t += dt;
    v_state.squelch_cd = (v_state.squelch_cd - dt).max(0.0);

    // Hold-to-skip: jump to the last beat with its wait elapsed so StageClear transitions cleanly.
    let skipping = crate::skip::skipping();
    if skipping {
        v_state.beat = (GROSS_BEATS.len() as u8).saturating_sub(1);
        v_state.beat_t = v_state.beat_t.max(4.5);
    }

    let beat_idx = (v_state.beat as usize).min(GROSS_BEATS.len() - 1);
    let min_t = GROSS_BEATS[beat_idx].5;
    if v_state.beat_t >= min_t && (skipping || !is_voice_speaking() || v_state.beat_t >= 18.0) {
        if v_state.beat + 1 >= GROSS_BEATS.len() as u8 {
            next_state.set(GameState::StageClear);
            return;
        }
        v_state.beat += 1;
        v_state.beat_t = 0.0;
        v_state.phase_sfx_step = 0;
        let (spk, line, voice, b_left, b_right, _) = GROSS_BEATS[v_state.beat as usize];
        match v_state.beat {
            1 => {
                play_sfx("wet_squelch");
            }
            2 => {
                play_sfx("flesh_tear");
                play_sfx("bone_crack");
                play_sfx("thing_shriek");
            }
            3 => {
                play_sfx("wet_squelch");
                play_sfx("flesh_tear");
            }
            _ => {
                play_sfx("wet_squelch");
            }
        }
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
        if let Ok(mut bl) = bl_q.get_single_mut() {
            bl.0 = b_left.to_string();
        }
        if let Ok(mut br) = br_q.get_single_mut() {
            br.0 = b_right.to_string();
        }
    }

    // Mid-beat secondary horror Foley accents (chest snap & wet tentacle eruption)
    if v_state.beat == 2 && v_state.beat_t >= 1.25 && v_state.phase_sfx_step == 0 {
        v_state.phase_sfx_step = 1;
        play_sfx("bone_crack");
        play_sfx("wet_squelch");
    } else if v_state.beat == 3 && v_state.beat_t >= 1.4 && v_state.phase_sfx_step == 0 {
        v_state.phase_sfx_step = 1;
        play_sfx("thing_shriek");
    }

    let beat = v_state.beat;
    let bt = v_state.beat_t;
    let t = time.elapsed_secs();
    let stage = Vec3::new(0.0, 0.0, -420.0);
    let pose = solve_horror_rig(stage, beat, bt, t);

    // Deform live CPU biomass mound vertices, normals & tangents
    for mound in &mound_q {
        if let Some(m) = meshes.get_mut(&mound.0) {
            update_melt_mound_mesh(m, t, pose.melt_amt, pose.slump_amt);
        }
    }

    // Animate practical horror lights (cold overhead fluorescent flicker + pulsing crimson cavity glow)
    let flicker = if matches!(beat, 0 | 2 | 3) {
        let f = (t * 37.0).sin() * (t * 61.0).cos();
        if f > 0.52 { 0.25 } else { 1.0 }
    } else {
        1.0
    };
    for (mut pl, role) in &mut light_q {
        match role.role {
            HorrorLightRole::FlickerOverhead => {
                pl.intensity = 640_000.0 * flicker;
            }
            HorrorLightRole::CrimsonCore => {
                let open_boost = if beat >= 2 { 2.2 } else { 0.85 };
                pl.intensity = (240_000.0 * open_boost) * (0.8 + 0.45 * (t * 11.0).sin());
            }
            HorrorLightRole::BileRim => {
                pl.intensity = 210_000.0 + pose.melt_amt * 140_000.0 * (0.75 + 0.3 * (t * 7.0).cos());
            }
            HorrorLightRole::CrewFill => {
                pl.intensity = 380_000.0;
            }
        }
    }

    // Animate every horror rig part, tentacle segment, tendon strand, boil, blister & viscera jet
    let mut trigger_pop_sfx = false;
    for (mut tf, anim) in &mut part_q {
        match anim.kind {
            ThingPart::LowerBody => {
                tf.translation = pose.lower_pos;
                tf.scale = pose.lower_scale;
            }
            ThingPart::ChestHalf { side } => {
                tf.translation = stage + Vec3::new(side * (0.16 + pose.chest_spread), (1.16 + pose.chest_sink_y).max(0.20), -2.2);
                let droop_x = if beat >= 3 { pose.slump_amt * 0.65 } else { 0.0 };
                let jitter = (t * 21.0 + side).sin() * 0.04;
                tf.rotation = Quat::from_euler(EulerRot::YXZ, side * pose.chest_open_angle, droop_x, -side * droop_x * 0.5 + jitter);
                tf.scale = Vec3::new(1.0 + pose.melt_amt * 0.18, (1.0 - pose.slump_amt * 0.48).max(0.45), 1.0);
            }
            ThingPart::InnerOrgan { lobe } => {
                let throb = 1.0 + (t * 15.0 + lobe as f32 * 1.4).sin() * 0.24;
                let a = lobe as f32 * 1.57 + t * 0.9;
                let sink = if beat >= 3 { pose.chest_sink_y * 0.72 } else { 0.0 };
                tf.translation = stage
                    + Vec3::new(a.cos() * 0.14, (1.52 + sink + (lobe as f32 - 1.5) * 0.09).max(0.28), -2.18 + a.sin() * 0.12);
                tf.scale = Vec3::splat(throb * if beat >= 2 { 1.18 } else { 0.65 });
            }
            ThingPart::HeadAssembly => {
                tf.translation = pose.head_pos;
                tf.rotation = pose.head_rot;
                tf.scale = Vec3::splat(pose.head_scale);
            }
            ThingPart::Jaw => {
                let snap = (t * 24.0).sin() * 0.06;
                tf.rotation = Quat::from_rotation_x(pose.jaw_open + snap);
            }
            ThingPart::Eyestalk { side, seg } => {
                let whip_x = (t * 9.5 + side * 2.1).sin() * pose.eye_extend * 0.24;
                let whip_y = (t * 11.2 + side * 1.3).cos() * pose.eye_extend * 0.20;
                let socket = Vec3::new(side * 0.21, 0.11, 0.34);
                let eye_pos = Vec3::new(side * 0.21 + whip_x, 0.11 + whip_y, 0.56 + pose.eye_extend * 0.85);
                if seg == 0 {
                    let delta = eye_pos - socket;
                    let len = delta.length().max(0.02);
                    let dir = delta / len;
                    tf.translation = socket;
                    tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
                    tf.scale = Vec3::new(1.0, len, 1.0);
                } else {
                    let bulge = if side < 0.0 { 1.22 } else { 0.98 };
                    tf.translation = eye_pos;
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, (t * 14.0).sin() * 0.28, (t * 12.0 + side).cos() * 0.35, 0.0);
                    tf.scale = Vec3::splat(bulge * (1.0 + (t * 16.0).sin() * 0.08));
                }
            }
            ThingPart::SpiderLeg { idx, side } => {
                if pose.spider_deploy < 0.05 {
                    tf.scale = Vec3::splat(0.01);
                } else {
                    let s = pose.spider_deploy;
                    tf.scale = Vec3::new(s, s, s);
                    let tripod = ((idx % 2) as f32 + if side > 0.0 { 1.0 } else { 0.0 }) * PI;
                    let swing = (pose.spider_crawl_phase + tripod).sin() * 0.42 * if beat == 3 { 1.0 } else { 0.28 };
                    let lift = (pose.spider_crawl_phase + tripod).cos().max(0.0) * 0.28;
                    let spread_y = (idx as f32 - 1.0) * 0.42;
                    tf.rotation = Quat::from_euler(
                        EulerRot::YXZ,
                        side * (-0.35 + spread_y) + swing,
                        0.15 + lift,
                        -side * (1.05 + lift * 0.4),
                    );
                }
            }
            ThingPart::SpiderShin { idx, side } => {
                let tripod = ((idx % 2) as f32 + if side > 0.0 { 1.0 } else { 0.0 }) * PI;
                let bend = (pose.spider_crawl_phase + tripod).sin() * 0.25;
                tf.rotation = Quat::from_euler(EulerRot::XYZ, bend * 0.3, 0.0, -side * (1.58 + bend * 0.3));
            }
            ThingPart::TentacleRoot { idx, base_angle } => {
                let reach = pose.tentacle_reach;
                let sink = if beat >= 3 { pose.slump_amt * 0.85 } else { 0.0 };
                let r_off = 0.18 + reach * 0.14;
                tf.translation = stage + Vec3::new(base_angle.cos() * r_off, (1.35 - sink).max(0.22), -2.18 + base_angle.sin() * r_off * 0.65);
                tf.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    -base_angle + (t * 2.2 + idx as f32).sin() * 0.35,
                    0.0,
                    -0.78 - pose.slump_amt * 0.45 + (t * 6.5 + idx as f32).sin() * 0.25,
                );
                let sc = if reach < 0.08 { 0.02 } else { reach.clamp(0.05, 1.12) };
                tf.scale = Vec3::splat(sc);
            }
            ThingPart::TentacleSeg { idx, seg } => {
                let phase = t * 7.8 + idx as f32 * 0.9 - seg as f32 * 0.85;
                let curl = (0.28 + seg as f32 * 0.14) * phase.sin() + pose.slump_amt * 0.25;
                let roll = 0.22 * (phase * 0.8 + 1.1).cos();
                tf.rotation = Quat::from_euler(EulerRot::XYZ, curl, 0.0, roll);
            }
            ThingPart::TendonStrand { idx, jaw_strand } => {
                if !jaw_strand && (beat >= 4 || pose.tendon_stretch < 0.04) {
                    tf.scale = Vec3::ZERO;
                } else {
                    let spread_a = (idx as f32 / 5.0) * TAU;
                    let off = Vec3::new(spread_a.cos() * 0.16, 0.0, spread_a.sin() * 0.12);
                    let (a, b) = if jaw_strand {
                        // Sticky mucus & saliva strings stretching across the unhinged jaw
                        let jx = (idx as f32 - 6.0) * 0.09;
                        let top = pose.head_pos + pose.head_rot * Vec3::new(jx, -0.06 * pose.head_scale, 0.48 * pose.head_scale);
                        let jaw_rot = pose.head_rot * Quat::from_rotation_x(pose.jaw_open);
                        let bot = pose.head_pos + pose.head_rot * Vec3::new(0.0, -0.24 * pose.head_scale, 0.16 * pose.head_scale)
                            + jaw_rot * Vec3::new(jx * 0.9, -0.06 * pose.head_scale, 0.38 * pose.head_scale);
                        (top, bot)
                    } else if beat == 3 {
                        // Torn trailing sinew whips hanging from the chest collar after the head breaks free
                        let a = pose.collar_anchor + off;
                        let whip = Vec3::new((t * 9.0 + idx as f32).sin() * 0.28, 0.32, (t * 7.0).cos() * 0.24);
                        (a, a + whip)
                    } else {
                        (pose.collar_anchor + off, pose.head_neck_anchor + off * 0.75)
                    };
                    let d = b - a;
                    let len = d.length().max(0.01);
                    tf.translation = (a + b) * 0.5;
                    let dir = d / len;
                    tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
                    let thick = (1.15 - (len * 0.25).min(0.65)) * if jaw_strand { 0.45 } else { 1.0 };
                    tf.scale = Vec3::new(thick, len, thick);
                }
            }
            ThingPart::Boil { base_pos, base_scale, phase } => {
                let s = base_scale * (0.9 + (t * 9.5 + phase).sin().abs() * 0.85);
                tf.translation = base_pos;
                tf.scale = Vec3::splat(s);
            }
            ThingPart::BubbleDome { base_pos, max_r, phase, speed } => {
                let show = 0.25 + pose.melt_amt * 0.75;
                let cyc = (t * speed + phase) % 1.0;
                // Slow inflation, violent shudder near peak, sudden burst pop
                let r = if cyc < 0.82 {
                    let grow = (cyc / 0.82).powf(0.7);
                    let shudder = if cyc > 0.68 { (t * 48.0).sin() * 0.08 } else { 0.0 };
                    (max_r * grow * (1.0 + shudder)) * show
                } else {
                    if beat >= 2 && cyc < 0.86 {
                        trigger_pop_sfx = true;
                    }
                    0.015
                };
                let (mh, _, _) = mound_height_and_grad(base_pos.x - stage.x, base_pos.z - (stage.z - 2.2), t, pose.melt_amt, pose.slump_amt);
                tf.translation = Vec3::new(base_pos.x, mh * 0.85, base_pos.z);
                tf.scale = Vec3::new(r, r * 0.82, r);
            }
            ThingPart::VisceraJet { base_pos, vel, phase, speed } => {
                if beat == 0 && bt < 0.5 {
                    tf.scale = Vec3::ZERO;
                } else {
                    let cyc = (t * speed + phase) % 1.25;
                    let p = base_pos + Vec3::new(vel.x * cyc, vel.y * cyc - 4.9 * cyc * cyc, vel.z * cyc);
                    let v = Vec3::new(vel.x, vel.y - 9.8 * cyc, vel.z);
                    let v_len = v.length().max(0.1);
                    let pos = Vec3::new(p.x, p.y.max(stage.y + 0.08), p.z);
                    tf.translation = pos;
                    let dir = v / v_len;
                    let up_ref = if dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
                    tf.look_at(pos + dir, up_ref);
                    let stretch = (0.7 + v_len * 0.24).min(2.6);
                    tf.scale = Vec3::new(0.85, 0.85, stretch);
                }
            }
            ThingPart::StinkCloud { base_pos, phase } => {
                let rise = (t * 0.52 + phase) % 2.5;
                tf.translation = base_pos + Vec3::new((t + phase).sin() * 0.42, rise * 1.45, (t + phase).cos() * 0.42);
                tf.scale = Vec3::splat(0.65 + rise * 0.60);
            }
            ThingPart::SpiralRing { speed, base_scale } => {
                tf.rotate_z(speed * dt);
                let pulse = 1.0 + (t * 2.4 + base_scale).sin() * 0.07;
                tf.scale = Vec3::splat(base_scale * pulse);
            }
        }
    }

    if trigger_pop_sfx && v_state.squelch_cd <= 0.0 {
        v_state.squelch_cd = 1.35;
        play_sfx("wet_squelch");
    }

    // Dynamic practical horror camera choreography across the 5 beats
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 4.5).min(1.0);
        let shake_amp = match beat {
            2 => (1.0 - (bt / 1.4).min(1.0)) * 0.055 + 0.012,
            3 => 0.018,
            _ => 0.004,
        };
        let cam_shake = Vec3::new((t * 33.0).sin() * shake_amp, (t * 29.0).cos() * shake_amp, 0.0);

        match beat {
            0 => {
                // Beat 0: Claustrophobic close-up on Lexie's twitching face, unhinging jaw & cheek boils
                c_tf.translation = stage + Vec3::new(0.45 - push * 0.22, 2.38, -0.05 - push * 0.22) + cam_shake;
                c_tf.look_at(pose.head_pos + Vec3::Y * 0.06, Vec3::Y);
                c_tf.rotate_local_z((t * 1.9).sin() * 0.045);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62 - push * 0.07;
                }
            }
            1 => {
                // Beat 1: Wide low-angle reaction shot framing all 5 crew avatars!
                c_tf.translation = stage + Vec3::new(0.0, 1.92, -1.25 + push * 0.22) + cam_shake;
                c_tf.look_at(stage + Vec3::new(0.0, 1.55, 2.75), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.88;
                }
            }
            2 => {
                // Beat 2: Dramatic 3/4 dolly showing the chest rib-maw tear wide open, tentacles & taut neck tendons
                c_tf.translation = stage + Vec3::new(1.55 - push * 0.45, 1.85 + push * 0.25, 0.45 - push * 0.20) + cam_shake;
                c_tf.look_at(stage + Vec3::new(0.0, 1.68, -2.05), Vec3::Y);
                c_tf.rotate_local_z((t * 3.1).sin() * 0.06);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.70 - push * 0.06;
                }
            }
            3 => {
                // Beat 3: Skimming low-angle macro shot across the boiling biomass puddle & approaching Spider-Head!
                c_tf.translation = stage + Vec3::new(-1.15 + push * 0.65, 0.82, 1.35 - push * 0.30) + cam_shake;
                c_tf.look_at(pose.head_pos * 0.65 + (stage + Vec3::new(0.0, 0.55, -2.0)) * 0.35, Vec3::Y);
                c_tf.rotate_local_z((t * 2.2).sin() * 0.04);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
                }
            }
            _ => {
                // Beat 4: Wide 3/4 survivor shot showing all 5 crew members & the steaming liquefied remains
                c_tf.translation = stage + Vec3::new(4.5 - push * 0.4, 2.55, 0.65);
                c_tf.look_at(stage + Vec3::new(-0.15, 1.35, 0.85), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.76;
                }
            }
        }
    }
}

pub fn exit_lexie_victory_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<LexieVictoryEntity>>,
    ov_q: Query<Entity, With<LexieVictoryOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    for mut hv in &mut hud_q {
        *hv = Visibility::Inherited;
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
}
