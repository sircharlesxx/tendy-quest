//! Rendering & lighting pass: HDR camera pipeline, CPU mipmaps, per-scene light rigs,
//! gradient sky dome, distance fog, shadow cascade tuning and emissive hygiene.
//!
//! Everything here is driven by *observing* the existing scene code (GameState, PlayerStats,
//! ClearColor, AmbientLight) rather than by editing the scene enter/exit systems, so the
//! gameplay modules keep owning their own mood colours and this module only refines how
//! they are rendered.

use bevy::core_pipeline::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::core_pipeline::fxaa::Fxaa;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::render::camera::Exposure;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap, DistanceFog,
    FogFalloff, NotShadowCaster, NotShadowReceiver, ShadowFilteringMethod,
};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::primitives::Aabb;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    Extent3d, Face, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
use bevy::transform::TransformSystem;

use crate::state::{GameState, PlayerStats};

/// Marks the six warm overhead apartment lights so they only shine in apartment scenes.
#[derive(Component)]
pub struct KitchenLight;

/// Tag on the city's window band boxes; swapped onto a lit/unlit window material once spawned.
#[derive(Component)]
pub struct CityWindowBand;

/// Root of a city building (its penthouse, spire, window bands and billboard are children).
/// Buildings that swallow the camera or block the hero are swapped to a see-through ghost.
#[derive(Component)]
pub struct CityBuilding;

/// Root whose direct children are each ghosted independently (Pub 217 shell parts).
#[derive(Component)]
pub struct CityOccluderGroup;

/// Seconds a ghosted building stays ghosted after it stops blocking (hysteresis vs popping).
#[derive(Component)]
struct GhostHold(f32);

/// Original material of a mesh currently drawn with the ghost material.
#[derive(Component)]
struct Ghosted(Handle<StandardMaterial>);

#[derive(Resource)]
struct GhostMaterial(Handle<StandardMaterial>);

/// Camera-following gradient sky sphere.
#[derive(Component)]
struct SkyDome;

/// Shared generated materials (city windows, detail textures) created once at startup.
#[derive(Resource, Clone)]
pub struct GfxMaterials {
    /// Dark glazing plus a few lit-window variants; the city picks one per window band.
    pub city_windows: Vec<Handle<StandardMaterial>>,
    /// Tileable, near-white grain multiplied into big untextured flat surfaces.
    detail: Handle<Image>,
}

/// Cache of detail-textured clones keyed by (source material, tiling in x, tiling in z).
#[derive(Resource, Default)]
struct DetailCache(std::collections::HashMap<(AssetId<StandardMaterial>, u32, u32), Handle<StandardMaterial>>);

/// Which lighting rig the current frame should use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SceneKind {
    /// Level 1 apartment (exploration, boss fight, menus over it).
    Apartment,
    /// Level 2 Cedar Rapids streets and the city cutscenes staged in them.
    City,
    /// Mom victory anime sunset cutscene.
    Anime,
    /// Hand-styled cutscene sets (bedroom, memory void, manga, TJ, finale): leave their look alone.
    Stylized,
    /// Tendy Palace Casino: obsidian mirror floor, neon bloom, and chandelier reflections.
    Casino,
}

#[derive(Resource)]
struct SkyState {
    mesh: Handle<Mesh>,
    last_key: [u32; 9],
}

/// Linear-light sky gradient for one scene.
#[derive(Clone, Copy)]
struct SkyColors {
    zenith: Vec3,
    horizon: Vec3,
    ground: Vec3,
}

pub struct GfxPlugin;

impl Plugin for GfxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectionalLightShadowMap { size: 2048 })
            .init_resource::<DetailCache>()
            .add_systems(Startup, (setup_gfx, setup_city_textures))
            .add_systems(PostStartup, apply_quality_setting)
            .add_systems(
                Update,
                (
                    generate_image_mipmaps,
                    tame_emissive_materials,
                    mark_flat_meshes_no_shadow,
                    apply_scene_rig,
                    assign_city_windows,
                    add_surface_detail,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    follow_sky_dome.before(TransformSystem::TransformPropagate),
                    ghost_city_occluders.after(TransformSystem::TransformPropagate),
                ),
            );
    }
}

/// Extra components for the main 3D camera: HDR target, restrained bloom, a deliberate
/// tonemapper and a gentle grade. Fog starts disabled (alpha 0) and is driven per scene.
pub fn camera_components() -> impl Bundle {
    (
        Camera {
            hdr: true,
            ..default()
        },
        // HDR + 4x MSAA hung the WebGL2 (SwiftShader) renderer outright; FXAA is the safe AA.
        Msaa::Off,
        Fxaa::default(),
        // Crisp, cheap 2x2 hardware PCF instead of the wide Gaussian kernel (also kinder to mobile).
        ShadowFilteringMethod::Hardware2x2,
        Tonemapping::TonyMcMapface,
        Exposure::default(),
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.12,
                ..default()
            },
            shadows: ColorGradingSection {
                contrast: 1.06,
                ..default()
            },
            midtones: ColorGradingSection {
                saturation: 1.06,
                contrast: 1.04,
                ..default()
            },
            highlights: ColorGradingSection {
                saturation: 1.02,
                ..default()
            },
        },
        Bloom {
            intensity: 0.06,
            low_frequency_boost: 0.4,
            prefilter: BloomPrefilter {
                threshold: 0.9,
                threshold_softness: 0.35,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            falloff: FogFalloff::Linear {
                start: 1000.0,
                end: 2000.0,
            },
            ..default()
        },
    )
}

/// `?gfx=low` in the page URL drops bloom and halves the shadow map for weak devices.
#[cfg(target_arch = "wasm32")]
pub(crate) fn low_quality_requested() -> bool {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function tq_gfx_query() { try { return String(window.location.search || ''); } catch (e) { return ''; } }")]
    extern "C" {
        fn tq_gfx_query() -> String;
    }
    tq_gfx_query()
        .trim_start_matches('?')
        .split('&')
        .any(|kv| kv == "gfx=low")
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn low_quality_requested() -> bool {
    false
}

fn apply_quality_setting(
    mut commands: Commands,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    cam_q: Query<Entity, With<Bloom>>,
) {
    if !low_quality_requested() {
        return;
    }
    shadow_map.size = 1024;
    for e in &cam_q {
        commands.entity(e).remove::<Bloom>();
    }
}

// ---------------------------------------------------------------------------------------------
// Startup: sky dome + generated materials
// ---------------------------------------------------------------------------------------------

fn setup_gfx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Inverted gradient sphere; colours are written into vertex colours per scene.
    let mut sky_mesh = Sphere::new(1.0).mesh().uv(32, 18);
    let count = sky_mesh.count_vertices();
    sky_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32, 1.0, 1.0, 1.0]; count]);
    let mesh = meshes.add(sky_mesh);
    let sky_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: Some(Face::Front),
        ..default()
    });
    commands.spawn((
        SkyDome,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(sky_mat),
        Transform::from_scale(Vec3::splat(900.0)),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.insert_resource(SkyState {
        mesh,
        last_key: [u32::MAX; 9],
    });

    // City window bands: one dark glazing material plus three lit patterns (warm, warm-dim, cool)
    // so the skyline twinkles at dusk without per-window materials.
    let mut city_windows = Vec::new();
    city_windows.push(materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.20, 0.26),
        metallic: 0.55,
        perceptual_roughness: 0.22,
        ..default()
    }));
    let variants: [(u64, f32, Vec3); 3] = [
        (11, 0.55, Vec3::new(1.0, 0.72, 0.38)),
        (23, 0.35, Vec3::new(1.0, 0.82, 0.55)),
        (37, 0.30, Vec3::new(0.70, 0.85, 1.0)),
    ];
    for (seed, lit_frac, warm) in variants {
        let (base_img, emis_img) = window_band_textures(seed, lit_frac, warm);
        let base = images.add(base_img);
        let emis = images.add(emis_img);
        city_windows.push(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(base),
            emissive: LinearRgba::rgb(1.6, 1.6, 1.6),
            emissive_texture: Some(emis),
            metallic: 0.35,
            perceptual_roughness: 0.3,
            ..default()
        }));
    }
    commands.insert_resource(GhostMaterial(materials.add(StandardMaterial {
        base_color: Color::srgba(0.70, 0.78, 0.92, 0.10),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.5,
        ..default()
    })));
    let detail = images.add(detail_noise_texture());
    commands.insert_resource(GfxMaterials {
        city_windows,
        detail,
    });
}

/// Small deterministic hash used for procedural patterns (no RNG state needed).
fn hash_u32(mut x: u64) -> u32 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^= x >> 33;
    x as u32
}

/// Builds a 128x8 band of 16 windows (2 rows) as albedo + emissive textures with mip chains.
fn window_band_textures(seed: u64, lit_frac: f32, warm: Vec3) -> (Image, Image) {
    const W: u32 = 128;
    const H: u32 = 8;
    let mut base = vec![0u8; (W * H * 4) as usize];
    let mut emis = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let cell_x = x / 8;
            let in_x = x % 8;
            let frame = in_x == 0 || in_x == 7 || y == 0 || y == H - 1;
            let i = ((y * W + x) * 4) as usize;
            let h = hash_u32(seed * 1000 + cell_x as u64);
            let lit = (h % 1000) as f32 / 1000.0 < lit_frac;
            let (b, e) = if frame {
                ([96u8, 92, 88], [0u8, 0, 0])
            } else if lit {
                let k = 0.75 + ((h >> 10) % 100) as f32 / 400.0;
                let c = warm * k;
                (
                    [60, 56, 50],
                    [
                        (c.x.min(1.0) * 255.0) as u8,
                        (c.y.min(1.0) * 255.0) as u8,
                        (c.z.min(1.0) * 255.0) as u8,
                    ],
                )
            } else {
                ([38, 50, 66], [0, 0, 0])
            };
            base[i..i + 3].copy_from_slice(&b);
            base[i + 3] = 255;
            emis[i..i + 3].copy_from_slice(&e);
            emis[i + 3] = 255;
        }
    }
    let mk = |data: Vec<u8>| {
        let mut img = Image::new(
            Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        if build_mip_chain(&mut img) {
            img.sampler = trilinear_sampler(ImageAddressMode::Repeat);
        }
        img
    };
    (mk(base), mk(emis))
}

/// Gives each city window band one of the shared window materials (about 1 in 4 stays dark).
fn assign_city_windows(
    mut commands: Commands,
    mats: Option<Res<GfxMaterials>>,
    q: Query<Entity, Added<CityWindowBand>>,
) {
    let Some(mats) = mats else {
        return;
    };
    for e in &q {
        let h = hash_u32(e.index() as u64 * 7919 + 13);
        let pick = match h % 8 {
            0 | 1 => 0,
            2 | 3 | 4 => 1,
            5 | 6 => 2,
            _ => 3,
        };
        if let Some(m) = mats.city_windows.get(pick) {
            commands.entity(e).insert(MeshMaterial3d(m.clone()));
        }
    }
}

/// 64x64 tileable value noise (two octaves + fine grain), values roughly 0.80..1.0.
fn detail_noise_texture() -> Image {
    const N: u32 = 64;
    let lattice = |cells: u32, x: u32, y: u32, seed: u64| -> f32 {
        let fx = x as f32 * cells as f32 / N as f32;
        let fy = y as f32 * cells as f32 / N as f32;
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let v = |i: u32, j: u32| {
            let h = hash_u32(seed + ((i % cells) as u64) * 131 + ((j % cells) as u64) * 7_919);
            (h % 1000) as f32 / 1000.0
        };
        let a = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
        let b = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
        a + (b - a) * sy
    };
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let n = lattice(4, x, y, 1) * 0.5
                + lattice(16, x, y, 2) * 0.3
                + (hash_u32(3 + (y * N + x) as u64) % 1000) as f32 / 1000.0 * 0.2;
            let v = (0.70 + n * 0.30).clamp(0.0, 1.0);
            let b = (v * 255.0) as u8;
            data.extend_from_slice(&[b, b, b, 255]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    if build_mip_chain(&mut img) {
        img.sampler = trilinear_sampler(ImageAddressMode::Repeat);
    }
    img
}

/// Big untextured horizontal slabs (floors, counters, roads, sidewalks) get a faint tiled grain
/// so they stop reading as flat CG fills. Colour is preserved on average.
fn add_surface_detail(
    mut commands: Commands,
    gfx: Option<Res<GfxMaterials>>,
    mut cache: ResMut<DetailCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<
        (Entity, &Aabb, &GlobalTransform, &MeshMaterial3d<StandardMaterial>),
        (Added<Aabb>, Without<CityWindowBand>, Without<SkyDome>),
    >,
) {
    let Some(gfx) = gfx else {
        return;
    };
    const TILE: f32 = 3.0;
    for (e, aabb, gt, mat) in &q {
        let m = gt.affine().matrix3;
        let he = aabb.half_extents;
        let world = Vec3::new(
            m.row(0).abs().dot(he),
            m.row(1).abs().dot(he),
            m.row(2).abs().dot(he),
        ) * 2.0;
        let min_h = world.x.min(world.z);
        if min_h < 6.0 || world.y > min_h * 0.25 {
            continue;
        }
        let Some(src) = materials.get(&mat.0) else {
            continue;
        };
        if src.base_color_texture.is_some()
            || src.unlit
            || src.alpha_mode != AlphaMode::Opaque
            || src.emissive.red.max(src.emissive.green).max(src.emissive.blue) > 0.0
        {
            continue;
        }
        // Tiling in mesh-local axes (box top face: u along local X, v along local Z).
        let s = gt.compute_transform().scale.abs();
        let reps_x = ((aabb.half_extents.x * 2.0 * s.x / TILE).round() as u32).max(1);
        let reps_z = ((aabb.half_extents.z * 2.0 * s.z / TILE).round() as u32).max(1);
        let key = (mat.0.id(), reps_x, reps_z);
        let handle = if let Some(h) = cache.0.get(&key) {
            h.clone()
        } else {
            let mut clone = src.clone();
            let c = LinearRgba::from(clone.base_color);
            // Texture averages ~0.85; lift albedo so the surface keeps its original brightness.
            clone.base_color = Color::LinearRgba(LinearRgba::new(
                c.red * 1.17,
                c.green * 1.17,
                c.blue * 1.17,
                c.alpha,
            ));
            clone.base_color_texture = Some(gfx.detail.clone());
            clone.uv_transform =
                bevy::math::Affine2::from_scale(Vec2::new(reps_x as f32, reps_z as f32));
            let h = materials.add(clone);
            cache.0.insert(key, h.clone());
            h
        };
        commands.entity(e).insert(MeshMaterial3d(handle));
    }
}

// ---------------------------------------------------------------------------------------------
// Mipmaps
// ---------------------------------------------------------------------------------------------

pub(crate) fn trilinear_sampler(address: ImageAddressMode) -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    })
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Appends a box-filtered mip chain to a single-mip 2D RGBA8 image. Returns false (leaving the
/// image untouched) for anything it does not understand.
pub(crate) fn build_mip_chain(image: &mut Image) -> bool {
    let desc = &image.texture_descriptor;
    let srgb = match desc.format {
        TextureFormat::Rgba8UnormSrgb => true,
        TextureFormat::Rgba8Unorm => false,
        _ => return false,
    };
    if desc.dimension != TextureDimension::D2
        || desc.mip_level_count != 1
        || desc.size.depth_or_array_layers != 1
        || desc
            .usage
            .intersects(TextureUsages::RENDER_ATTACHMENT | TextureUsages::STORAGE_BINDING)
    {
        return false;
    }
    let (mut w, mut h) = (desc.size.width as usize, desc.size.height as usize);
    if w < 2 && h < 2 || image.data.len() != w * h * 4 {
        return false;
    }

    // sRGB decode table; encode through a 4096-entry table to keep big textures cheap on WASM.
    let to_lin: Vec<f32> = (0..256)
        .map(|v| {
            let c = v as f32 / 255.0;
            if srgb {
                srgb_to_linear(c)
            } else {
                c
            }
        })
        .collect();
    let from_lin: Vec<u8> = (0..4096)
        .map(|i| {
            let c = i as f32 / 4095.0;
            let v = if srgb { linear_to_srgb(c) } else { c };
            (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
        })
        .collect();
    let enc = |v: f32| from_lin[((v * 4095.0 + 0.5) as usize).min(4095)];

    let mut levels = 1u32;
    let mut prev_start = 0usize;
    let mut out = std::mem::take(&mut image.data);
    while w > 1 || h > 1 {
        let nw = (w / 2).max(1);
        let nh = (h / 2).max(1);
        let mut next = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            let y0 = (y * 2).min(h - 1);
            let y1 = (y * 2 + 1).min(h - 1);
            for x in 0..nw {
                let x0 = (x * 2).min(w - 1);
                let x1 = (x * 2 + 1).min(w - 1);
                let idx = [
                    prev_start + (y0 * w + x0) * 4,
                    prev_start + (y0 * w + x1) * 4,
                    prev_start + (y1 * w + x0) * 4,
                    prev_start + (y1 * w + x1) * 4,
                ];
                for c in 0..3 {
                    let s: f32 = idx.iter().map(|&i| to_lin[out[i + c] as usize]).sum();
                    next.push(enc(s * 0.25));
                }
                // Alpha is linear in every format.
                let a: u32 = idx.iter().map(|&i| out[i + 3] as u32).sum();
                next.push(((a + 2) / 4) as u8);
            }
        }
        prev_start = out.len();
        out.extend_from_slice(&next);
        w = nw;
        h = nh;
        levels += 1;
    }
    image.data = out;
    image.texture_descriptor.mip_level_count = levels;
    true
}

/// Gives every loaded 2D RGBA8 texture (glTF albedo, normal maps, sprites) a mip chain and
/// trilinear sampling so distant plaid/broccoli textures stop shimmering.
fn generate_image_mipmaps(
    mut events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mut ids = Vec::new();
    for ev in events.read() {
        if let AssetEvent::Added { id } = ev {
            ids.push(*id);
        }
    }
    for id in ids {
        // Only file-backed images: font atlases and render targets are runtime-updated.
        if asset_server.get_path(id).is_none() {
            continue;
        }
        let Some(img) = images.get(id) else {
            continue;
        };
        if img.texture_descriptor.mip_level_count != 1 {
            continue;
        }
        let address = match &img.sampler {
            ImageSampler::Descriptor(d) => d.address_mode_u,
            ImageSampler::Default => ImageAddressMode::ClampToEdge,
        };
        let address_v = match &img.sampler {
            ImageSampler::Descriptor(d) => d.address_mode_v,
            ImageSampler::Default => ImageAddressMode::ClampToEdge,
        };
        let Some(img) = images.get_mut(id) else {
            continue;
        };
        if build_mip_chain(img) {
            let mut sampler = trilinear_sampler(address);
            if let ImageSampler::Descriptor(d) = &mut sampler {
                d.address_mode_v = address_v;
            }
            img.sampler = sampler;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Emissive hygiene & shadow casters
// ---------------------------------------------------------------------------------------------

/// With bloom, emissive values of 4-7 explode into white blobs. Scale any emissive above the cap
/// down (hue preserved). Idempotent, so per-frame animated materials just stay capped.
fn tame_emissive_materials(
    mut events: EventReader<AssetEvent<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut ids = Vec::new();
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => ids.push(*id),
            _ => {}
        }
    }
    for id in ids {
        let Some(m) = materials.get(id) else {
            continue;
        };
        let cap = match m.alpha_mode {
            AlphaMode::Blend | AlphaMode::Add | AlphaMode::Premultiplied => 2.0,
            _ => 3.0,
        };
        let e = m.emissive;
        let peak = e.red.max(e.green).max(e.blue);
        if peak > cap + 1e-3 {
            if let Some(m) = materials.get_mut(id) {
                let k = cap / peak;
                m.emissive = LinearRgba::new(e.red * k, e.green * k, e.blue * k, e.alpha);
            }
        }
    }
}

/// Paper-thin horizontal meshes (road stripes, rugs, decals, floor slabs) gain nothing from
/// casting shadows but cost shadow-pass draws and cause acne; opt them out automatically.
fn mark_flat_meshes_no_shadow(
    mut commands: Commands,
    q: Query<(Entity, &Aabb, &GlobalTransform), (Added<Aabb>, With<Mesh3d>, Without<NotShadowCaster>)>,
) {
    for (e, aabb, gt) in &q {
        let m = gt.affine().matrix3;
        let he = aabb.half_extents;
        let world = Vec3::new(
            m.row(0).abs().dot(he),
            m.row(1).abs().dot(he),
            m.row(2).abs().dot(he),
        );
        let horiz = world.x.max(world.z);
        if horiz > 0.0 && (world.y < 0.03 || world.y < horiz * 0.012) {
            commands.entity(e).insert(NotShadowCaster);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Per-scene rig
// ---------------------------------------------------------------------------------------------

fn scene_kind(state: GameState, stats: &PlayerStats) -> SceneKind {
    match state {
        GameState::Playing | GameState::BossFight => SceneKind::Apartment,
        GameState::StageClear | GameState::GameOver => {
            if stats.in_city {
                SceneKind::City
            } else {
                SceneKind::Apartment
            }
        }
        GameState::MomVictoryCutscene => SceneKind::Anime,
        GameState::CityIntroCutscene
        | GameState::CityPlay
        | GameState::DerekCutscene
        | GameState::NateCutscene
        | GameState::GlennCutscene
        // Why: `LexieBattle` & `LexieVictoryCutscene` need the 900m Cedar Rapids twilight `SkyDome`, warm
        // atmospheric fog, and city sun light so the burning Quaker Oats plant and surrounding
        // downtown skyline sit against dusk sky instead of black void. Gameplay impact: visual only.
        | GameState::LexieBattle
        | GameState::LexieVictoryCutscene => SceneKind::City,
        GameState::Tutorial
        | GameState::Cutscene
        | GameState::HeadphoneCutscene
        | GameState::MikeCutscene
        | GameState::TjCutscene
        // Why: Chillary's cutscene takes place outdoors along the bright sunlit Cedar River waterfront
        // (`SceneKind::Stylized` gives crisp 20k noon sunlight and bright blue ClearColor sky instead
        // of `SceneKind::City`'s dusk sunset dome). Gameplay impact: visual lighting/sky only.
        | GameState::ChillaryCutscene
        | GameState::PubPortalCutscene
        | GameState::StanCrashoutCutscene
        | GameState::HouseCraftCutscene
        | GameState::BlackHoleCutscene
        | GameState::CraftPlay => SceneKind::Stylized,
        GameState::CasinoPlay | GameState::CoasterCutscene | GameState::ClubPlay => SceneKind::Casino,
    }
}

fn lin(c: Color) -> Vec3 {
    let l = LinearRgba::from(c);
    Vec3::new(l.red, l.green, l.blue)
}

fn sky_for(kind: SceneKind, clear: Color) -> Option<SkyColors> {
    let c = lin(clear);
    match kind {
        SceneKind::City => {
            // The scene's navy ClearColor becomes the zenith (lifted to a readable dusk blue),
            // with a warm horizon glow matching the low warm sun.
            let zenith = (c * 2.2 + Vec3::new(0.05, 0.09, 0.26)).min(Vec3::ONE);
            let horizon = Vec3::new(0.95, 0.52, 0.30);
            Some(SkyColors {
                zenith,
                horizon,
                ground: horizon * 0.55 + zenith * 0.25,
            })
        }
        SceneKind::Anime => {
            // Keep the ClearColor sunset as the horizon; deepen towards a violet zenith.
            let horizon = (c * 1.05).min(Vec3::ONE);
            let zenith = c * 0.25 + Vec3::new(0.10, 0.06, 0.24);
            Some(SkyColors {
                zenith,
                horizon,
                ground: horizon * 0.6,
            })
        }
        SceneKind::Apartment | SceneKind::Stylized | SceneKind::Casino => None,
    }
}

/// Lights, ambient, fog, bloom, cascades and sky visibility for the current scene.
#[allow(clippy::too_many_arguments)]
fn apply_scene_rig(
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    clear: Res<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    mut ambient_track: Local<(f32, f32)>,
    mut last_sig: Local<Option<(SceneKind, i32)>>,
    mut sun_q: Query<(&mut DirectionalLight, &mut Transform, &mut CascadeShadowConfig), Without<Camera3d>>,
    mut kitchen_q: Query<&mut Visibility, (With<KitchenLight>, Without<SkyDome>)>,
    mut sky_q: Query<&mut Visibility, (With<SkyDome>, Without<KitchenLight>)>,
    mut cam_q: Query<
        (
            &mut DistanceFog,
            Option<&mut Bloom>,
            &mut ColorGrading,
            &mut Exposure,
            &Transform,
        ),
        (With<Camera3d>, Without<DirectionalLight>),
    >,
    sky_state: Option<ResMut<SkyState>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let kind = scene_kind(*state.get(), &stats);
    let scale = stats.player_scale.max(0.1);

    // Ambient: each scene inserts its own AmbientLight on enter; re-base whenever the scene
    // writes a new value, then apply this pass's multiplier exactly once.
    let amb_mult = match kind {
        SceneKind::Apartment => 0.70,
        // Strong cool fill so shadowed faces keep their own colour (~40-50% of the lit side).
        SceneKind::City => 3.6,
        SceneKind::Anime => 1.0,
        SceneKind::Stylized => 1.0,
        SceneKind::Casino => 0.48,
    };
    let (base, written) = *ambient_track;
    let new_base = if (ambient.brightness - written).abs() > 1e-3 {
        ambient.brightness
    } else {
        base
    };
    let want = new_base * amb_mult;
    if (ambient.brightness - want).abs() > 1e-3 {
        ambient.brightness = want;
    }
    *ambient_track = (new_base, want);

    // Sun: direction (pointing from), colour, illuminance and a cascade fitted to the camera's
    // working distance so a single WebGL2 cascade stays crisp at any hero scale.
    // Why: pin `city_sf = 1.0` during `LexieVictoryCutscene` so a 6500cm finale run (`scale = 18.0`)
    // keeps identical exposure, shadow cascade (70m), and dusk fog range (60..420m) to `?lexie_victory=1`
    // rather than washing out the burning factory and cityscape. Gameplay impact: purely visual.
    let city_sf = if *state.get() == GameState::LexieVictoryCutscene {
        1.0
    } else {
        (scale / 1.8).max(1.0)
    };
    let (illum, color, from, shadow_far) = match kind {
        SceneKind::Apartment => (
            11_000.0,
            Color::srgb(1.0, 0.95, 0.86),
            Vec3::new(12.0, 42.0, 28.0),
            (45.0 + scale * 12.0).min(110.0),
        ),
        SceneKind::City => (
            7_000.0,
            Color::srgb(1.0, 0.82, 0.62),
            Vec3::new(20.0, 36.0, 30.0),
            70.0 * city_sf.powf(0.65),
        ),
        SceneKind::Anime => (
            15_000.0,
            Color::srgb(1.0, 0.84, 0.68),
            Vec3::new(12.0, 26.0, 30.0),
            32.0,
        ),
        SceneKind::Stylized => (
            20_000.0,
            Color::srgb(1.0, 0.96, 0.86),
            Vec3::new(12.0, 42.0, 28.0),
            80.0,
        ),
        SceneKind::Casino => (
            1_400.0,
            Color::srgb(0.68, 0.58, 1.0),
            Vec3::new(8.0, 34.0, 18.0),
            60.0,
        ),
    };
    let sig = (kind, (shadow_far * 2.0) as i32);
    let rig_changed = *last_sig != Some(sig);
    if rig_changed {
        *last_sig = Some(sig);
        for (mut sun, mut tf, mut cascade) in &mut sun_q {
            sun.illuminance = illum;
            sun.color = color;
            sun.shadows_enabled = true;
            *tf = Transform::from_translation(from).looking_at(Vec3::ZERO, Vec3::Y);
            *cascade = CascadeShadowConfigBuilder {
                num_cascades: 1,
                minimum_distance: 0.1,
                maximum_distance: shadow_far,
                first_cascade_far_bound: shadow_far,
                overlap_proportion: 0.2,
            }
            .build();
        }
        let kitchen_on = kind == SceneKind::Apartment;
        for mut v in &mut kitchen_q {
            let want = if kitchen_on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != want {
                *v = want;
            }
        }
    }

    // Sky dome + fog.
    let sky = sky_for(kind, clear.0);
    for mut v in &mut sky_q {
        let want = if sky.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if let (Some(sky), Some(mut st)) = (sky, sky_state) {
        let key = [
            sky.zenith.x, sky.zenith.y, sky.zenith.z, sky.horizon.x, sky.horizon.y,
            sky.horizon.z, sky.ground.x, sky.ground.y, sky.ground.z,
        ]
        .map(|f| (f * 1000.0) as u32);
        if key != st.last_key {
            st.last_key = key;
            if let Some(mesh) = meshes.get_mut(&st.mesh) {
                write_sky_colors(mesh, sky);
            }
        }
    }
    for (mut fog, bloom, mut grading, mut exposure, cam_tf) in &mut cam_q {
        // Cutscene cranes and the kaiju follow cam sit far above the street: push fog out with them.
        let fog_sf = city_sf.max(cam_tf.translation.y / 14.0);
        match (kind, sky) {
            (SceneKind::City, Some(s)) => {
                let fog_col = s.horizon * 0.5 + s.zenith * 0.5;
                fog.color = Color::LinearRgba(LinearRgba::new(fog_col.x, fog_col.y, fog_col.z, 0.38));
                fog.falloff = FogFalloff::Linear {
                    start: 60.0 * fog_sf.powf(0.8),
                    end: 420.0 * fog_sf.powf(0.6),
                };
            }
            (SceneKind::Anime, Some(s)) => {
                fog.color = Color::LinearRgba(LinearRgba::new(s.horizon.x, s.horizon.y, s.horizon.z, 0.45));
                fog.falloff = FogFalloff::Linear {
                    start: 30.0,
                    end: 220.0,
                };
            }
            _ => {
                if fog.color.alpha() != 0.0 {
                    fog.color = Color::srgba(0.0, 0.0, 0.0, 0.0);
                }
            }
        }
        let want_bloom = match kind {
            SceneKind::City | SceneKind::Anime => 0.07,
            SceneKind::Apartment => 0.05,
            SceneKind::Stylized => 0.06,
            SceneKind::Casino => 0.15,
        };
        if let Some(mut bloom) = bloom {
            if (bloom.intensity - want_bloom).abs() > 1e-4 {
                bloom.intensity = want_bloom;
            }
        }
        // The city palette is already saturated (sidewalk orange, grass green): grade it neutral.
        let (post_sat, mid_sat) = if kind == SceneKind::City {
            (0.97, 1.0)
        } else if kind == SceneKind::Casino {
            (1.24, 1.14)
        } else {
            (1.12, 1.06)
        };
        if (grading.global.post_saturation - post_sat).abs() > 1e-4 {
            grading.global.post_saturation = post_sat;
            grading.midtones.saturation = mid_sat;
        }
        // Lift exposure as the kaiju camera pulls back over the rooftops (up to +0.7 EV).
        let ev = if kind == SceneKind::City {
            Exposure::EV100_BLENDER - 0.7 * ((city_sf - 1.0) / 4.0).clamp(0.0, 1.0)
        } else {
            Exposure::EV100_BLENDER
        };
        if (exposure.ev100 - ev).abs() > 1e-3 {
            exposure.ev100 = ev;
        }
    }
}

fn write_sky_colors(mesh: &mut Mesh, sky: SkyColors) {
    let ys: Vec<f32> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| v[1]).collect(),
        _ => return,
    };
    let cols: Vec<[f32; 4]> = ys
        .iter()
        .map(|&y| {
            let c = if y >= 0.0 {
                // Horizon band hugs the bottom; zenith takes over smoothly.
                let t = (y / 0.3).clamp(0.0, 1.0).powf(0.6);
                sky.horizon.lerp(sky.zenith, t)
            } else {
                let t = (-y / 0.25).clamp(0.0, 1.0);
                sky.horizon.lerp(sky.ground, t)
            };
            [c.x, c.y, c.z, 1.0]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
}

// ---------------------------------------------------------------------------------------------
// City occluders
// ---------------------------------------------------------------------------------------------

/// Segment a->b (only the part with t < t_max) against an AABB, slab method.
fn segment_hits_aabb(a: Vec3, b: Vec3, min: Vec3, max: Vec3, t_max: f32) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f32, t_max);
    for i in 0..3 {
        if d[i].abs() < 1e-6 {
            if a[i] < min[i] || a[i] > max[i] {
                return false;
            }
        } else {
            let inv = 1.0 / d[i];
            let (mut ta, mut tb) = ((min[i] - a[i]) * inv, (max[i] - a[i]) * inv);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// In city gameplay, buildings that contain the camera or sit between the camera and the hero
/// are drawn with one shared translucent ghost material (visual only: colliders are separate
/// `PlatformBox` data and untouched). They come back after a short hold once clear.
/// `CityBuilding` roots ghost as one unit; each direct child of a `CityOccluderGroup` (Pub 217,
/// which scales up with the hero) ghosts on its own.
#[allow(clippy::too_many_arguments)]
fn ghost_city_occluders(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    ghost: Option<Res<GhostMaterial>>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    player_q: Query<&GlobalTransform, With<crate::player::Player>>,
    building_q: Query<Entity, With<CityBuilding>>,
    group_q: Query<&Children, With<CityOccluderGroup>>,
    mut hold_q: Query<&mut GhostHold>,
    children_q: Query<&Children>,
    bounds_q: Query<(&Aabb, &GlobalTransform)>,
    mut mat_q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&Ghosted>)>,
) {
    let Some(ghost) = ghost else {
        return;
    };
    const HOLD_SECS: f32 = 0.45;
    let active = *state.get() == GameState::CityPlay && stats.in_city;
    let cam_gt = cam_q.get_single().ok();
    let cam = cam_gt.map(|t| t.translation());
    let cam_right = cam_gt.map(|t| t.right().as_vec3()).unwrap_or(Vec3::X);
    let hero = player_q.get_single().ok().map(|t| t.translation());
    let scale = stats.player_scale.max(0.1);
    let dt = time.delta_secs();
    // Inside Pub 217 the camera is deliberately boxed in by walls: leave them alone.
    let hero_in_pub = hero.is_some_and(|h| crate::city::is_in_pub217(h, scale));

    let mut units: Vec<(Entity, bool)> = building_q.iter().map(|e| (e, true)).collect();
    for children in &group_q {
        units.extend(children.iter().map(|&c| (c, !hero_in_pub)));
    }

    for (unit, allowed) in units {
        let mut occluding = false;
        if let (true, true, Some(cam), Some(hero)) = (active, allowed, cam, hero) {
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for e in std::iter::once(unit).chain(children_q.iter_descendants(unit)) {
                if let Ok((aabb, gt)) = bounds_q.get(e) {
                    let m = gt.affine().matrix3;
                    let he = aabb.half_extents;
                    let ext = Vec3::new(
                        m.row(0).abs().dot(he),
                        m.row(1).abs().dot(he),
                        m.row(2).abs().dot(he),
                    );
                    let c = Vec3::from(gt.affine().transform_point3a(aabb.center));
                    min = min.min(c - ext);
                    max = max.max(c + ext);
                }
            }
            let size = max - min;
            // Skip empty units and flat slabs (floors, rugs): they never block the view.
            if min.x <= max.x && size.y > 0.1 * size.x.min(size.z) {
                let margin = Vec3::splat(0.6);
                let inside = cam.cmpge(min - margin).all() && cam.cmple(max + margin).all();
                // Sight lines to the hero's hips, chest and head plus a little to either side, so
                // a wall hugging the frame next to the hero also counts. Stop short of the hero so
                // walls the hero is touching stay solid.
                let chest = hero + Vec3::Y * scale * 0.9;
                let side = cam_right * scale * 0.9;
                occluding = inside
                    || [
                        hero + Vec3::Y * scale * 0.4,
                        chest,
                        hero + Vec3::Y * scale * 1.5,
                        chest + side,
                        chest - side,
                    ]
                    .iter()
                    .any(|&p| segment_hits_aabb(cam, p, min, max, 0.96));
            }
        }

        let remaining = match hold_q.get_mut(unit) {
            Ok(mut h) => {
                h.0 = if occluding { HOLD_SECS } else { h.0 - dt };
                h.0
            }
            Err(_) => {
                if occluding {
                    commands.entity(unit).insert(GhostHold(HOLD_SECS));
                    HOLD_SECS
                } else {
                    0.0
                }
            }
        };
        let want_ghost = remaining > 0.0;

        for e in std::iter::once(unit).chain(children_q.iter_descendants(unit)) {
            let Ok((mut mat, ghosted)) = mat_q.get_mut(e) else {
                continue;
            };
            match (want_ghost, ghosted) {
                (true, None) => {
                    commands.entity(e).insert(Ghosted(mat.0.clone()));
                    mat.0 = ghost.0.clone();
                }
                (false, Some(orig)) => {
                    mat.0 = orig.0.clone();
                    commands.entity(e).remove::<Ghosted>();
                }
                _ => {}
            }
        }
    }
}

/// Keep the sky centred on the camera so it reads as infinitely far away.
fn follow_sky_dome(
    cam_q: Query<&Transform, (With<Camera3d>, Without<SkyDome>)>,
    mut sky_q: Query<&mut Transform, (With<SkyDome>, Without<Camera3d>)>,
) {
    let Ok(cam) = cam_q.get_single() else {
        return;
    };
    for mut tf in &mut sky_q {
        tf.translation = cam.translation;
    }
}

// ---------------------------------------------------------------------------------------------
// City surface textures: procedural, tileable detail maps baked once at startup
// ---------------------------------------------------------------------------------------------
//
// Every map is a near-white multiplier (Rgba8Unorm, full mip chain, repeat sampling), so a
// material keeps its own cartoon base colour and only gains detail. `city_textured` lifts the
// base colour by the map's median so the surface's average brightness stays where it was.
// Tiling is solved in geometry: `tiled_box` builds box meshes whose UVs are in world metres
// divided by the texture's tile size, so a 17 m facade and a 0.2 m lane stripe both tile at
// their real scale. Handles are fixed weak ids, so the city builder can reference them without
// any new system parameters.

/// One generated city detail map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CityTex {
    /// Facade (2.6 m x 5.2 m, one window bay pair by one floor): brick courses + windows.
    FacadeBrick = 0,
    /// Facade: concrete/stone panels with seams + windows.
    FacadeConcrete,
    /// Facade: glass curtain wall grid with floor spandrels.
    FacadeGlass,
    /// Tar and gravel roof membrane with seams and patches (6 m tile).
    Roof,
    /// Asphalt with aggregate, patches and cracks (12 m tile).
    Asphalt,
    /// Worn road paint (1.5 m tile).
    Paint,
    /// Square sidewalk pavers with joints (3 m tile).
    Pavers,
    /// Running-bond brick, for brick walks and the pub (1.2 m tile).
    Brick,
    /// Grass with dry and lush patches (4 m tile).
    Grass,
    /// Gentle wave pattern (10 m tile).
    Water,
    /// Wooden planks with grain (2 m tile).
    Wood,
    /// Vertical bark ridges.
    Bark,
    /// Clumped leaf clusters.
    Foliage,
    /// Billboard poster print with frame (whole face, not tiled).
    Billboard,
}

const CITY_TEX_COUNT: usize = 14;

/// Facade texture tile in metres (u along the wall, v up). Floors repeat every 5.2 m, matching
/// the 5.2 m pitch of the lit `CityWindowBand` rows, so facade windows line up with the bands.
pub const FACADE_TILE: Vec2 = Vec2::new(2.6, 5.2);
/// UV that lands on the plain stringcourse band of every facade map (used for box tops).
pub const FACADE_FLAT_UV: [f32; 2] = [0.05, -0.07 / 5.2];

/// Median-based brightness compensation for each map (1.0 until the maps exist).
static CITY_TEX_GAIN: std::sync::OnceLock<[f32; CITY_TEX_COUNT]> = std::sync::OnceLock::new();

impl CityTex {
    const ALL: [CityTex; CITY_TEX_COUNT] = [
        CityTex::FacadeBrick,
        CityTex::FacadeConcrete,
        CityTex::FacadeGlass,
        CityTex::Roof,
        CityTex::Asphalt,
        CityTex::Paint,
        CityTex::Pavers,
        CityTex::Brick,
        CityTex::Grass,
        CityTex::Water,
        CityTex::Wood,
        CityTex::Bark,
        CityTex::Foliage,
        CityTex::Billboard,
    ];

    /// Fixed handle the startup system inserts the generated image under.
    pub const fn handle(self) -> Handle<Image> {
        Handle::weak_from_u128(0x6c1f_7e47_a2d9_4b3e_9e15_c17e_7e70_0000 + self as u128)
    }
}

/// Applies a city detail map to a material, lifting its base colour so the textured surface
/// averages the same brightness (and hue) as the original flat colour.
pub fn city_textured(mut m: StandardMaterial, tex: CityTex) -> StandardMaterial {
    let gain = CITY_TEX_GAIN
        .get()
        .map(|g| g[tex as usize])
        .unwrap_or(1.0);
    let c = LinearRgba::from(m.base_color);
    m.base_color = Color::LinearRgba(LinearRgba::new(
        c.red * gain,
        c.green * gain,
        c.blue * gain,
        c.alpha,
    ));
    m.base_color_texture = Some(tex.handle());
    m
}

/// Roof membrane material in a building's own colour (slightly darker than its walls).
pub fn city_roof_material(wall: Color) -> StandardMaterial {
    let c = LinearRgba::from(wall);
    city_textured(
        StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::new(c.red * 0.72, c.green * 0.72, c.blue * 0.74, 1.0)),
            perceptual_roughness: 0.92,
            ..default()
        },
        CityTex::Roof,
    )
}

#[derive(Default)]
struct MeshBuf {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl MeshBuf {
    /// Adds a box (centre, geometric size) whose UVs are world metres / tile. `world` is the
    /// box's real size (differs from `geom` when the entity transform scales a unit box).
    /// Side faces: u runs along the wall from its left edge, v runs up from `base_y`.
    /// Top/bottom: square `tile.x` grid, or one constant UV when `flat_top` is given.
    fn push_box(&mut self, center: Vec3, geom: Vec3, world: Vec3, tile: Vec2, base_y: f32, flat_top: Option<[f32; 2]>) {
        let faces = [
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
            (Vec3::X, Vec3::NEG_Z, Vec3::Y),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::X, Vec3::NEG_Z),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        ];
        for (n, t, b) in faces {
            let base = self.pos.len() as u32;
            let ext_t = (t.abs() * world).element_sum();
            let ext_b = (b.abs() * world).element_sum();
            for (st, sb) in [(-0.5_f32, -0.5_f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                let p = n * 0.5 + t * st + b * sb;
                self.pos.push((center + p * geom).to_array());
                self.nrm.push(n.to_array());
                let u = (st + 0.5) * ext_t / tile.x;
                let uv = if n.y.abs() > 0.5 {
                    flat_top.unwrap_or([u, (sb + 0.5) * ext_b / tile.x])
                } else {
                    [u, -((sb + 0.5) * ext_b + base_y) / tile.y]
                };
                self.uv.push(uv);
            }
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_indices(bevy::render::mesh::Indices::U32(self.idx))
    }
}

/// Box mesh with world-scaled UVs. Pass `geom = Vec3::ONE` for a unit box that the entity's
/// transform scales to `world`, or `geom = world` for a box built at its real size.
/// `base_y` is the world height of the box bottom (keeps facade floors on one global rhythm).
pub fn tiled_box(geom: Vec3, world: Vec3, tile: Vec2, base_y: f32, flat_top: Option<[f32; 2]>) -> Mesh {
    let mut b = MeshBuf::default();
    b.push_box(Vec3::ZERO, geom, world, tile, base_y, flat_top);
    b.into_mesh()
}

/// One mesh holding the four kerb stones around a square sidewalk slab (outer edge `outer`,
/// stones `width` wide, `height` tall, bottom at y = 0 in mesh space).
pub fn kerb_ring_mesh(outer: f32, width: f32, height: f32, tile: f32) -> Mesh {
    let mut b = MeshBuf::default();
    let off = (outer - width) * 0.5;
    let tile = Vec2::splat(tile);
    for (c, s) in [
        (Vec3::new(0.0, height * 0.5, off), Vec3::new(outer, height, width)),
        (Vec3::new(0.0, height * 0.5, -off), Vec3::new(outer, height, width)),
        (Vec3::new(off, height * 0.5, 0.0), Vec3::new(width, height, outer - 2.0 * width)),
        (Vec3::new(-off, height * 0.5, 0.0), Vec3::new(width, height, outer - 2.0 * width)),
    ] {
        b.push_box(c, s, s, tile, 0.0, None);
    }
    b.into_mesh()
}

/// Hash to [0, 1) from a seed and two lattice coordinates.
fn h01(seed: u64, a: i64, b: i64) -> f32 {
    let k = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((a as u64).wrapping_mul(73_856_093))
        .wrapping_add((b as u64).wrapping_mul(19_349_663_000_013));
    (hash_u32(k) % 10_000) as f32 / 10_000.0
}

/// Tileable value noise on the unit square with `cx` x `cy` lattice cells per tile.
fn tnoise(u: f32, v: f32, cx: u32, cy: u32, seed: u64) -> f32 {
    let fx = u.rem_euclid(1.0) * cx as f32;
    let fy = v.rem_euclid(1.0) * cy as f32;
    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let g = |i: i64, j: i64| h01(seed, i.rem_euclid(cx as i64), j.rem_euclid(cy as i64));
    let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * sx;
    let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * sx;
    a + (b - a) * sy
}

/// Tileable fractal noise, roughly 0..1.
fn tfbm(u: f32, v: f32, cx: u32, cy: u32, octaves: u32, seed: u64) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
    let (mut x, mut y) = (cx, cy);
    for o in 0..octaves {
        sum += tnoise(u, v, x, y, seed + o as u64 * 101) * amp;
        norm += amp;
        amp *= 0.5;
        x *= 2;
        y *= 2;
    }
    sum / norm
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Bakes `f(u, v)` (v = 0 at the image top) into a mipmapped, repeating detail map and
/// returns it with its brightness gain (1 / median luminance, clamped).
pub(crate) fn bake_detail(w: u32, h: u32, aniso: u16, f: impl Fn(f32, f32) -> Vec3) -> (Image, f32) {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    let mut lum = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32;
            let c = f(u, v).clamp(Vec3::ZERO, Vec3::ONE);
            lum.push((c.x + c.y + c.z) / 3.0);
            data.extend_from_slice(&[
                (c.x * 255.0 + 0.5) as u8,
                (c.y * 255.0 + 0.5) as u8,
                (c.z * 255.0 + 0.5) as u8,
                255,
            ]);
        }
    }
    lum.sort_by(|a, b| a.total_cmp(b));
    let median = lum[lum.len() / 2].max(0.05);
    let gain = (1.0 / median).clamp(1.0, 1.35);
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    if build_mip_chain(&mut img) {
        let mut sampler = trilinear_sampler(ImageAddressMode::Repeat);
        if let ImageSampler::Descriptor(d) = &mut sampler {
            // Ignored by WebGL2 drivers without the extension; sharpens grazing street views.
            d.anisotropy_clamp = aniso;
        }
        img.sampler = sampler;
    }
    (img, gain)
}

/// Window openings shared by the facade maps (x, y in metres within the 2.6 x 5.2 tile).
/// Rows sit at y 3.85..5.15, i.e. exactly behind the lit window bands (centres 4.5 + 5.2 k).
fn facade_window(x: f32, y: f32) -> Option<Vec3> {
    let win_x = [0.25_f32, 1.55];
    for x0 in win_x {
        let lx = x - x0;
        if (-0.08..0.88).contains(&lx) && (3.72..3.85).contains(&y) {
            return Some(Vec3::splat(1.0)); // stone sill
        }
        if (0.0..0.8).contains(&lx) && (3.85..5.15).contains(&y) {
            let frame = lx < 0.06 || lx > 0.74 || y < 3.91 || y > 5.09 || (lx - 0.4).abs() < 0.03;
            if frame {
                return Some(Vec3::splat(0.55));
            }
            let sheen = 0.20 + 0.10 * ((y - 3.85) / 1.3);
            return Some(Vec3::new(sheen, sheen * 1.12, sheen * 1.35));
        }
    }
    None
}

/// Soft grime: darker streaks running down from each window sill, plus broad blotches.
fn facade_grime(u: f32, v: f32, x: f32, y: f32, seed: u64) -> f32 {
    let mut g = 0.93 + 0.07 * tfbm(u, v, 2, 4, 3, seed);
    for x0 in [0.25_f32, 1.55] {
        if (x0..x0 + 0.8).contains(&x) && (2.4..3.72).contains(&y) {
            let streak = tnoise(u, 0.0, 64, 1, seed + 9);
            g *= 1.0 - 0.10 * ((y - 2.4) / 1.32) * streak;
        }
    }
    g
}

fn facade_stringcourse(y: f32) -> bool {
    y < 0.14
}

fn tex_facade_brick(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let (x, y) = (u * FACADE_TILE.x, (1.0 - v) * FACADE_TILE.y);
        if facade_stringcourse(y) {
            return Vec3::splat(0.95);
        }
        if let Some(c) = facade_window(x, y) {
            return c;
        }
        let course = FACADE_TILE.y / 24.0;
        let row = (y / course).floor();
        let yr = y - row * course;
        let blen = FACADE_TILE.x / 6.0;
        let off = if (row as i64) % 2 == 1 { blen * 0.5 } else { 0.0 };
        let xs = (x + off).rem_euclid(FACADE_TILE.x);
        let bi = (xs / blen).floor();
        let xr = xs - bi * blen;
        let mortar = yr < 0.028 || xr < 0.028;
        let val = if mortar {
            1.0
        } else {
            let b = h01(41, bi as i64, row as i64);
            0.80 + (b - 0.5) * 0.18 + (tnoise(u, v, 64, 128, 43) - 0.5) * 0.08
        };
        Vec3::splat(val * facade_grime(u, v, x, y, 47))
    })
}

fn tex_facade_concrete(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let (x, y) = (u * FACADE_TILE.x, (1.0 - v) * FACADE_TILE.y);
        if facade_stringcourse(y) {
            return Vec3::splat(0.95);
        }
        if let Some(c) = facade_window(x, y) {
            return c;
        }
        let (pw, ph) = (1.3_f32, 2.6_f32);
        let (px, py) = ((x / pw).floor(), (y / ph).floor());
        let (xr, yr) = (x - px * pw, y - py * ph);
        let seam = xr < 0.03 || yr < 0.03;
        let val = if seam {
            0.62
        } else {
            let tone = h01(53, px as i64, py as i64);
            // Form-tie holes on a 0.43 m grid inside each panel.
            let (hx, hy) = ((xr - 0.217).rem_euclid(0.433), (yr - 0.217).rem_euclid(0.433));
            let hole = (hx - 0.216).abs() < 0.025 && (hy - 0.216).abs() < 0.025;
            let base = 0.86 + (tone - 0.5) * 0.10 + (tfbm(u, v, 8, 16, 3, 55) - 0.5) * 0.12;
            if hole {
                base * 0.72
            } else {
                base
            }
        };
        Vec3::splat(val * facade_grime(u, v, x, y, 59))
    })
}

fn tex_facade_glass(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let (x, y) = (u * FACADE_TILE.x, (1.0 - v) * FACADE_TILE.y);
        if facade_stringcourse(y) {
            return Vec3::splat(0.95);
        }
        // Spandrel panels hide each floor slab; mullions every 0.65 m, transoms at 1.3 m.
        if y < 0.6 {
            return Vec3::splat(0.62 + 0.05 * tnoise(u, v, 16, 32, 61));
        }
        let (mw, mh) = (0.65_f32, 1.3_f32);
        let (px, py) = ((x / mw).floor(), ((y - 0.6) / mh).floor());
        let (xr, yr) = (x - px * mw, (y - 0.6) - py * mh);
        if xr < 0.05 || yr < 0.05 {
            return Vec3::splat(1.0);
        }
        // Each pane reflects a slightly different patch of sky, with a diagonal sheen.
        let tone = h01(67, px as i64, py as i64);
        let sheen = ((xr / mw + yr / mh) * 0.5 - 0.5).abs();
        let val = 0.70 + (tone - 0.5) * 0.18 + (0.5 - sheen) * 0.10;
        Vec3::new(val, val, val * 1.04)
    })
}

fn tex_roof(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let px = (u * w as f32) as i64;
        let py = (v * h as f32) as i64;
        let gravel = h01(71, px, py);
        let mut val = 0.78 + 0.12 * tfbm(u, v, 4, 4, 2, 73) + (gravel - 0.5) * 0.14;
        // Membrane seams every 1.5 m (4 per 6 m tile).
        let (sx, sy) = ((u * 4.0).fract(), (v * 4.0).fract());
        if sx < 0.012 || sy < 0.012 {
            val *= 0.78;
        }
        // Occasional repair patches.
        let (cx, cy) = ((u * 3.0).floor() as i64, (v * 3.0).floor() as i64);
        if h01(79, cx, cy) < 0.35 {
            let (fx, fy) = ((u * 3.0).fract(), (v * 3.0).fract());
            let (ox, oy) = (0.2 + 0.3 * h01(83, cx, cy), 0.2 + 0.3 * h01(89, cx, cy));
            if (ox..ox + 0.32).contains(&fx) && (oy..oy + 0.24).contains(&fy) {
                val *= 0.84;
            }
        }
        Vec3::splat(val)
    })
}

fn tex_asphalt(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    // Cracks: wandering random walks drawn into a wrapping mask first.
    let (wi, hi) = (w as i64, h as i64);
    let mut crack = vec![0.0_f32; (w * h) as usize];
    let scale = w as f32 / 256.0;
    for c in 0..7_i64 {
        let mut p = Vec2::new(h01(101, c, 0) * w as f32, h01(103, c, 0) * h as f32);
        let mut ang = h01(107, c, 0) * std::f32::consts::TAU;
        let steps = ((40.0 + 60.0 * h01(109, c, 0)) * scale) as i64;
        for s in 0..steps {
            ang += (h01(113, c, s) - 0.5) * 1.1;
            p += Vec2::new(ang.cos(), ang.sin());
            let (ix, iy) = ((p.x as i64).rem_euclid(wi), (p.y as i64).rem_euclid(hi));
            crack[(iy * wi + ix) as usize] = 1.0;
            // Branch now and then.
            if h01(127, c, s) < 0.04 {
                ang += 0.9;
            }
        }
    }
    bake_detail(w, h, aniso, move |u, v| {
        let px = ((u * w as f32) as i64).min(wi - 1);
        let py = ((v * h as f32) as i64).min(hi - 1);
        let grain = h01(131, px, py);
        let mut val = 0.80 + 0.12 * tfbm(u, v, 4, 4, 3, 137) + (grain - 0.5) * 0.10;
        if grain > 0.95 {
            val += 0.10; // light aggregate
        }
        // Patched rectangles in a 4 x 4 cell grid (3 m cells on the 12 m tile).
        let (cx, cy) = ((u * 4.0).floor() as i64, (v * 4.0).floor() as i64);
        let (fx, fy) = ((u * 4.0).fract(), (v * 4.0).fract());
        let k = h01(139, cx, cy);
        if k < 0.3 {
            let (ox, oy) = (0.1 + 0.3 * h01(149, cx, cy), 0.1 + 0.3 * h01(151, cx, cy));
            let (ew, eh) = (0.3 + 0.3 * h01(157, cx, cy), 0.25 + 0.3 * h01(163, cx, cy));
            if (ox..ox + ew).contains(&fx) && (oy..oy + eh).contains(&fy) {
                let edge = (fx - ox).min(ox + ew - fx).min(fy - oy).min(oy + eh - fy);
                val = if edge < 0.012 { val * 0.70 } else { 0.70 + (grain - 0.5) * 0.05 + if k < 0.15 { 0.12 } else { 0.0 } };
            }
        }
        if crack[(py * wi + px) as usize] > 0.0 {
            val *= 0.66;
        }
        Vec3::splat(val)
    })
}

fn tex_paint(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let n = tfbm(u, v, 4, 4, 4, 173);
        let speck = h01(179, (u * w as f32) as i64, (v * h as f32) as i64);
        let wear = smoothstep(0.55, 0.68, n);
        let val = 0.96 - wear * 0.38 - if speck > 0.9 { 0.15 } else { 0.0 };
        Vec3::splat(val)
    })
}

fn tex_pavers(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let (cx, cy) = ((u * 4.0).floor() as i64, (v * 4.0).floor() as i64);
        let (fx, fy) = ((u * 4.0).fract(), (v * 4.0).fract());
        // Some slabs are split in two.
        let split = h01(181, cx, cy) < 0.25 && (fx - 0.5).abs() < 0.012;
        let joint = fx < 0.025 || fy < 0.025 || split;
        if joint {
            return Vec3::splat(0.58);
        }
        let tone = h01(191, cx, cy);
        let n = tnoise(u, v, 32, 32, 193);
        let speck = h01(197, (u * w as f32) as i64, (v * h as f32) as i64);
        let edge = fx.min(fy).min(1.0 - fx).min(1.0 - fy);
        let bevel = if edge < 0.05 { 0.94 } else { 1.0 };
        Vec3::splat((0.86 + (tone - 0.5) * 0.10 + (n - 0.5) * 0.08 + (speck - 0.5) * 0.05) * bevel)
    })
}

fn tex_brick(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let row = (v * 8.0).floor();
        let yr = (v * 8.0).fract();
        let off = if (row as i64) % 2 == 1 { 0.5 } else { 0.0 };
        let xs = (u * 4.0 + off).rem_euclid(4.0);
        let bi = xs.floor();
        let xr = xs.fract();
        if yr < 0.12 || xr < 0.06 {
            return Vec3::splat(1.0);
        }
        let b = h01(211, bi as i64, row as i64);
        let n = tnoise(u, v, 32, 32, 223);
        Vec3::splat(0.78 + (b - 0.5) * 0.22 + (n - 0.5) * 0.10)
    })
}

fn tex_grass(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let dry = smoothstep(0.55, 0.75, tfbm(u, v, 3, 3, 3, 227));
        let clump = tfbm(u, v, 8, 8, 2, 229);
        let px = (u * w as f32) as i64;
        let py = (v * h as f32) as i64;
        // Short vertical blade strokes: hash per column, stretched over 3 pixels.
        let blade = h01(233, px, py.div_euclid(3) + px * 7);
        let val = 0.72 + 0.18 * clump + if blade > 0.85 { 0.12 } else if blade < 0.1 { -0.10 } else { 0.0 };
        let tint = Vec3::new(0.86, 1.0, 0.88).lerp(Vec3::new(1.0, 0.94, 0.66), dry);
        tint * val
    })
}

fn tex_water(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    use std::f32::consts::TAU;
    bake_detail(w, h, aniso, |u, v| {
        // Integer wave numbers keep the sum tileable.
        let waves = (TAU * (3.0 * u + 1.0 * v) + 0.4).sin() * 0.35
            + (TAU * (-2.0 * u + 4.0 * v) + 1.7).sin() * 0.30
            + (TAU * (5.0 * u + 3.0 * v) + 2.9).sin() * 0.20
            + (tfbm(u, v, 8, 8, 2, 239) - 0.5) * 0.6;
        let crest = smoothstep(0.55, 0.85, waves);
        Vec3::splat(0.84 + waves * 0.06 + crest * 0.12)
    })
}

fn tex_wood(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        // 10 planks across v, running along u, each with its own butt joint.
        let row = (v * 10.0).floor();
        let fy = (v * 10.0).fract();
        let joint_u = h01(241, row as i64, 0);
        let du = (u - joint_u).rem_euclid(1.0);
        if fy < 0.06 || du < 0.008 {
            return Vec3::splat(0.50);
        }
        let plank = if du < 0.5 { 0 } else { 1 };
        let tone = h01(251, row as i64, plank);
        let warp = tnoise(u, v, 4, 40, 257 + row as u64) * 6.0;
        let grain = ((fy * 3.0 + warp) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        let fine = tnoise(u, v, 64, 160, 263);
        Vec3::splat(0.80 + (tone - 0.5) * 0.16 + (grain - 0.5) * 0.10 + (fine - 0.5) * 0.06)
    })
}

fn tex_bark(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        let ridge = tfbm(u, v, 12, 3, 3, 269);
        let crack = smoothstep(0.38, 0.30, ridge);
        Vec3::splat(0.62 + 0.36 * ridge - crack * 0.18)
    })
}

fn tex_foliage(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        // Cellular leaf clusters on a wrapping 8 x 8 grid: bright centres, shaded edges.
        let (gx, gy) = (u * 8.0, v * 8.0);
        let (cx, cy) = (gx.floor() as i64, gy.floor() as i64);
        let mut best = f32::MAX;
        let mut id = 0.0;
        for j in -1..=1 {
            for i in -1..=1 {
                let (nx, ny) = (cx + i, cy + j);
                let (wx, wy) = (nx.rem_euclid(8), ny.rem_euclid(8));
                let p = Vec2::new(nx as f32 + h01(271, wx, wy), ny as f32 + h01(277, wx, wy));
                let d = p.distance(Vec2::new(gx, gy));
                if d < best {
                    best = d;
                    id = h01(281, wx, wy);
                }
            }
        }
        let shade = 1.0 - smoothstep(0.2, 0.75, best) * 0.35;
        let speck = h01(283, (u * w as f32) as i64, (v * h as f32) as i64);
        let val = (0.80 + (id - 0.5) * 0.18) * shade + if speck > 0.93 { 0.10 } else { 0.0 };
        let tint = Vec3::new(0.92, 1.0, 0.86).lerp(Vec3::new(1.0, 0.96, 0.74), id * 0.6);
        tint * val
    })
}

fn tex_billboard(w: u32, h: u32, aniso: u16) -> (Image, f32) {
    bake_detail(w, h, aniso, |u, v| {
        // Steel frame with light bulbs, then an abstract poster: big disc, headline bars, stripe.
        let (x, y) = (u * 2.4, v); // poster aspect ~2.4 : 1
        let border = u.min(1.0 - u) * 2.4 < 0.07 || v.min(1.0 - v) < 0.07;
        if border {
            let bulb = ((u * 40.0).fract() - 0.5).abs() < 0.18 && v.min(1.0 - v) < 0.07;
            return Vec3::splat(if bulb { 1.0 } else { 0.35 });
        }
        let disc = Vec2::new(x - 1.75, y - 0.5).length() < 0.32;
        let ring = (Vec2::new(x - 1.75, y - 0.5).length() - 0.36).abs() < 0.025;
        let bar1 = (0.18..1.35).contains(&x) && (0.22..0.36).contains(&y);
        let bar2 = (0.18..1.05).contains(&x) && (0.46..0.54).contains(&y);
        let bar3 = (0.18..0.85).contains(&x) && (0.62..0.68).contains(&y);
        let stripe = ((x + y) * 4.0).fract() < 0.5 && y > 0.78;
        let val = if bar1 || ring {
            0.28
        } else if bar2 || bar3 {
            0.45
        } else if disc {
            0.62
        } else if stripe {
            0.80
        } else {
            1.0
        };
        Vec3::splat(val * (0.96 + 0.04 * tnoise(u, v, 16, 8, 293)))
    })
}

/// Bakes every city detail map once and registers it under its fixed handle. `?gfx=low` bakes
/// at half resolution without anisotropic filtering.
fn setup_city_textures(mut images: ResMut<Assets<Image>>) {
    let low = low_quality_requested();
    let (r, aniso) = if low { (2, 1) } else { (1, 8) };
    let mut gains = [1.0_f32; CITY_TEX_COUNT];
    for tex in CityTex::ALL {
        let (img, gain) = match tex {
            CityTex::FacadeBrick => tex_facade_brick(128 / r, 256 / r, aniso),
            CityTex::FacadeConcrete => tex_facade_concrete(128 / r, 256 / r, aniso),
            CityTex::FacadeGlass => tex_facade_glass(128 / r, 256 / r, aniso),
            CityTex::Roof => tex_roof(128 / r, 128 / r, aniso),
            CityTex::Asphalt => tex_asphalt(256 / r, 256 / r, aniso),
            CityTex::Paint => tex_paint(64 / r, 64 / r, aniso),
            CityTex::Pavers => tex_pavers(128 / r, 128 / r, aniso),
            CityTex::Brick => tex_brick(128 / r, 128 / r, aniso),
            CityTex::Grass => tex_grass(128 / r, 128 / r, aniso),
            CityTex::Water => tex_water(128 / r, 128 / r, aniso),
            CityTex::Wood => tex_wood(128 / r, 128 / r, aniso),
            CityTex::Bark => tex_bark(64 / r, 64 / r, aniso),
            CityTex::Foliage => tex_foliage(128 / r, 128 / r, aniso),
            CityTex::Billboard => tex_billboard(128 / r, 64 / r, aniso),
        };
        gains[tex as usize] = gain;
        images.insert(tex.handle().id(), img);
    }
    let _ = CITY_TEX_GAIN.set(gains);
    // Bash Lyte tall-boy labels (always full size: the lettering must stay readable).
    for way in 0..BEER_CAN_COLOURWAYS {
        images.insert(beer_can_tex_handle(way).id(), tex_beer_can(way));
    }
    images.insert(BEER_GLOW_TEX.id(), tex_soft_glow(64));
}

// ---------------------------------------------------------------------------------------------
// Pub 217 beer cans: "Bash Lyte" tall-boy label and the soft glow sprite
// ---------------------------------------------------------------------------------------------

/// Label colourways baked at startup (same layout, different inks).
pub const BEER_CAN_COLOURWAYS: usize = 3;
/// Can texture layout (v = 0 at the top): the label art fills v 0..`BEER_CAN_LABEL_V`, brushed
/// aluminium sits around `BEER_CAN_METAL_V` and the darker lid grey around `BEER_CAN_LID_V`.
/// The art repeats every u = 1 (the can mesh wraps it twice around the can).
pub const BEER_CAN_LABEL_V: f32 = 0.8;
pub const BEER_CAN_METAL_V: f32 = 0.85;
pub const BEER_CAN_LID_V: f32 = 0.95;
/// Radial falloff sprite (rgb and alpha) for additive glows.
pub const BEER_GLOW_TEX: Handle<Image> = Handle::weak_from_u128(0x6c1f_7e47_a2d9_4b3e_9e15_c17e_beef_0100);

pub const fn beer_can_tex_handle(way: usize) -> Handle<Image> {
    Handle::weak_from_u128(0x6c1f_7e47_a2d9_4b3e_9e15_c17e_beef_0000 + way as u128)
}

/// 5x7 block glyphs (one row per byte, bit 4 = leftmost column) for the label lettering.
fn label_glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        '$' => [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
        '>' => [0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000],
        _ => [0; 7],
    }
}

/// True when label pixel (x, y) is lit by `text` drawn centred at `cx` with its top at `top`,
/// `s` pixels per glyph cell.
fn label_text_hit(text: &str, cx: i32, top: i32, s: i32, x: i32, y: i32) -> bool {
    let n = text.chars().count() as i32;
    let left = cx - (n * 6 - 1) * s / 2;
    if y < top || y >= top + 7 * s || x < left {
        return false;
    }
    let (col, row) = ((x - left) / s, (y - top) / s);
    let (ci, gx) = (col / 6, col % 6);
    if ci >= n || gx == 5 {
        return false;
    }
    let g = label_glyph(text.chars().nth(ci as usize).unwrap_or(' '));
    (g[row as usize] >> (4 - gx)) & 1 == 1
}

/// "Bash Lyte" tall-boy label (original house brand): CRT scanline ground, chunky pixel
/// "BASH / LYTE" lettering, a "LIGHT LAGER" line and a terminal-window crest with a `$` prompt
/// and blinking cursor, flanked by a fading cursor-block stripe. Below the art: brushed
/// aluminium and lid grey (see `BEER_CAN_LABEL_V`). sRGB with a mip chain.
pub fn tex_beer_can(way: usize) -> Image {
    const W: i32 = 128;
    const H: i32 = 160;
    // (ground, lettering, accent) per colourway, sRGB.
    let (bg, fg, acc) = match way % BEER_CAN_COLOURWAYS {
        0 => (Vec3::new(0.07, 0.10, 0.08), Vec3::new(0.42, 1.0, 0.52), Vec3::new(1.0, 0.72, 0.20)),
        1 => (Vec3::new(0.98, 0.60, 0.12), Vec3::new(0.13, 0.07, 0.04), Vec3::new(1.0, 0.95, 0.84)),
        _ => (Vec3::new(0.36, 0.13, 0.44), Vec3::new(1.0, 0.93, 0.80), Vec3::new(0.35, 0.95, 0.95)),
    };
    let ink = |x: i32, y: i32| {
        label_text_hit("LIGHT LAGER", 64, 10, 1, x, y)
            || label_text_hit("BASH", 64, 21, 5, x, y)
            || label_text_hit("LYTE", 64, 60, 5, x, y)
    };
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let c = if y >= 128 {
                // Brushed aluminium (horizontal grain), then the darker lid grey.
                let grain = h01(311, y as i64, x as i64 / 3) * 0.6 + h01(313, y as i64, 0) * 0.4;
                let base = if y < 144 { 0.80 } else { 0.60 };
                Vec3::splat(base + (grain - 0.5) * 0.10)
            } else if y < 4 || y >= 124 {
                bg * 0.45
            } else if (4..7).contains(&y) || (121..124).contains(&y) {
                acc
            } else if (34..94).contains(&x) && (100..120).contains(&y) {
                // Terminal-window crest: accent frame, title bar, `$` prompt and a cursor block.
                let frame = x == 34 || x == 93 || y == 100 || y == 119 || y == 103;
                if frame {
                    acc
                } else if label_text_hit("$", 52, 105, 2, x, y) || label_text_hit(">", 40, 108, 1, x, y) {
                    fg
                } else if (62..70).contains(&x) && (105..118).contains(&y) {
                    acc
                } else if y < 103 {
                    acc * 0.55
                } else {
                    Vec3::splat(0.03)
                }
            } else if (106..114).contains(&y) && (x < 30 || x >= 98) && x % 8 < 4 {
                // Cursor-block stripe fading away from the crest.
                let d = if x < 30 { 30 - x } else { x - 97 } as f32;
                bg.lerp(acc, 1.0 - d / 34.0)
            } else if ink(x, y) {
                fg
            } else if ink(x - 1, y - 1) || ink(x - 2, y - 2) {
                // Hard drop shadow so the lettering reads at a distance.
                bg * 0.35
            } else {
                // Ground with CRT scanlines and a soft vertical sheen.
                let scan = if y % 3 == 0 { 0.90 } else { 1.0 };
                let sheen = 0.94 + 0.06 * (1.0 - ((y as f32 - 64.0) / 64.0).abs());
                bg * scan * sheen
            };
            let c = c.clamp(Vec3::ZERO, Vec3::ONE);
            data.extend_from_slice(&[
                (c.x * 255.0 + 0.5) as u8,
                (c.y * 255.0 + 0.5) as u8,
                (c.z * 255.0 + 0.5) as u8,
                255,
            ]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: W as u32,
            height: H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    if build_mip_chain(&mut img) {
        let mut sampler = trilinear_sampler(ImageAddressMode::Repeat);
        if let ImageSampler::Descriptor(d) = &mut sampler {
            d.address_mode_v = ImageAddressMode::ClampToEdge;
            d.anisotropy_clamp = 4;
        }
        img.sampler = sampler;
    }
    img
}

/// Soft round falloff (rgb = alpha) for additive halo sprites.
pub fn tex_soft_glow(n: u32) -> Image {
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let d = Vec2::new(
                (x as f32 + 0.5) / n as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / n as f32 * 2.0 - 1.0,
            )
            .length();
            let f = (1.0 - d).clamp(0.0, 1.0);
            let v = (f * f * 255.0 + 0.5) as u8;
            data.extend_from_slice(&[v, v, v, v]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    if build_mip_chain(&mut img) {
        img.sampler = trilinear_sampler(ImageAddressMode::ClampToEdge);
    }
    img
}
