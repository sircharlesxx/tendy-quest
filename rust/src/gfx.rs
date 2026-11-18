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
            .add_systems(Startup, setup_gfx)
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
fn low_quality_requested() -> bool {
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
fn low_quality_requested() -> bool {
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

fn trilinear_sampler(address: ImageAddressMode) -> ImageSampler {
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
fn build_mip_chain(image: &mut Image) -> bool {
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
        GameState::Tutorial | GameState::Playing | GameState::BossFight => SceneKind::Apartment,
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
        | GameState::GlennCutscene => SceneKind::City,
        GameState::Cutscene
        | GameState::HeadphoneCutscene
        | GameState::MikeCutscene
        | GameState::TjCutscene
        | GameState::LexieVictoryCutscene => SceneKind::Stylized,
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
        SceneKind::Apartment | SceneKind::Stylized => None,
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
    let city_sf = (scale / 1.8).max(1.0);
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
        };
        if let Some(mut bloom) = bloom {
            if (bloom.intensity - want_bloom).abs() > 1e-4 {
                bloom.intensity = want_bloom;
            }
        }
        // The city palette is already saturated (sidewalk orange, grass green): grade it neutral.
        let (post_sat, mid_sat) = if kind == SceneKind::City {
            (0.97, 1.0)
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
    // Inside Pub 217 the camera is deliberately boxed in by its walls: leave the pub alone.
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
