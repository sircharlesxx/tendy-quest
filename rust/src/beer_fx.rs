//! Pub 217 beer: the "Bash Lyte" tall-boy can (one shared revolved mesh, label colourways baked
//! in `gfx.rs`), the stacked can pyramid on the corner table, and the hero can in front of it
//! with its "you can drink this" glow and sparkles.
//!
//! The glow/sparkles never decide availability themselves: they mirror the visibility of the
//! amber floor ring (`city::BeerTowerGlow`), which `city::update_city_props` drives from the
//! real gating condition, so a change to that condition is picked up here for free.

use std::f32::consts::{FRAC_1_SQRT_2, PI, TAU};

use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::city::BeerTowerGlow;
use crate::gfx::{
    beer_can_tex_handle, BEER_CAN_COLOURWAYS, BEER_CAN_LABEL_V, BEER_CAN_LID_V, BEER_CAN_METAL_V,
    BEER_GLOW_TEX,
};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, PlayerStats};

/// Tall boy: 0.64 tall, 0.26 wide (about 2.46 : 1), mesh origin at its centre.
pub const CAN_HALF_H: f32 = 0.32;
pub const CAN_R: f32 = 0.13;
/// Tier pitch of the pyramid (can height plus a sliver so lids and bases don't z-fight).
const TIER_PITCH: f32 = CAN_HALF_H * 2.0 + 0.006;
/// Centre spacing of cans within a tier (cans just touch).
const CAN_SPACING: f32 = CAN_R * 2.0 + 0.01;
const PYRAMID_TIERS: usize = 7;
const HERO_SCALE: f32 = 1.15;
const COASTER_H: f32 = 0.012;

pub struct BeerFxPlugin;

impl Plugin for BeerFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                animate_beer_hero.after(crate::city::update_city_props),
                debug_start_at_beer,
            )
                .run_if(in_state(GameState::CityPlay)),
        );
    }
}

/// The hero can (the one you drink). Holds its own material so its label can shimmer.
#[derive(Component)]
pub struct BeerHeroCan {
    mat: Handle<StandardMaterial>,
}

/// Root of the hero's glow + sparkles; its visibility mirrors the floor ring's.
#[derive(Component)]
pub struct BeerHeroFx;

#[derive(Component)]
pub struct BeerHeroHalo;

#[derive(Component)]
pub struct BeerSparkle {
    i: usize,
    n: usize,
}

// ---------------------------------------------------------------------------------------------
// Shared can mesh & materials
// ---------------------------------------------------------------------------------------------

/// One revolved tall-boy can: domed base on a standing ring, narrowed neck, straight label
/// body, tapered shoulder, rolled rim and a recessed lid with a pull tab. The label art is
/// wrapped twice around the body so the brand faces every way.
pub fn can_mesh() -> Mesh {
    const N: usize = 28;
    let (lab, met, lid) = (BEER_CAN_LABEL_V, BEER_CAN_METAL_V, BEER_CAN_LID_V);
    // (r0, y0, r1, y1, v0, v1) per profile segment, bottom centre to top centre.
    let segs: [(f32, f32, f32, f32, f32, f32); 13] = [
        (0.0, -0.300, 0.080, -0.318, lid, lid),
        (0.080, -0.318, 0.092, -0.320, met, met),
        (0.092, -0.320, 0.108, -0.312, met, met),
        (0.108, -0.312, 0.124, -0.285, met, met),
        (0.124, -0.285, CAN_R, -0.255, met, lab),
        (CAN_R, -0.255, CAN_R, 0.205, lab, 0.0),
        (CAN_R, 0.205, 0.126, 0.240, met, met),
        (0.126, 0.240, 0.112, 0.282, met, met),
        (0.112, 0.282, 0.104, 0.306, met, met),
        (0.104, 0.306, 0.105, CAN_HALF_H, met, met),
        (0.105, CAN_HALF_H, 0.098, CAN_HALF_H, met, met),
        (0.098, CAN_HALF_H, 0.094, 0.304, lid, lid),
        (0.094, 0.304, 0.0, 0.304, lid, lid),
    ];
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (r0, y0, r1, y1, v0, v1) in segs {
        let t = Vec2::new(r1 - r0, y1 - y0).normalize();
        let n2 = Vec2::new(t.y, -t.x);
        let base = pos.len() as u32;
        for k in 0..=N {
            let a = k as f32 / N as f32 * TAU;
            let (s, c) = a.sin_cos();
            // Decreasing with the angle so the art reads left to right from outside.
            let u = (N - k) as f32 / N as f32 * 2.0;
            for (r, y, v) in [(r0, y0, v0), (r1, y1, v1)] {
                pos.push([r * c, y, r * s]);
                nrm.push([n2.x * c, n2.y, n2.x * s]);
                uv.push([u, v]);
            }
        }
        for k in 0..N as u32 {
            let (a0, b0, a1, b1) = (base + k * 2, base + k * 2 + 1, base + k * 2 + 2, base + k * 2 + 3);
            idx.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
        }
    }
    // Pull tab: a small flat box lying on the recessed lid.
    let center = Vec3::new(0.034, 0.308, 0.0);
    let size = Vec3::new(0.078, 0.006, 0.036);
    for (n, t, b) in [
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
    ] {
        let base = pos.len() as u32;
        for (st, sb) in [(-0.5_f32, -0.5_f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            pos.push((center + (n * 0.5 + t * st + b * sb) * size).to_array());
            nrm.push(n.to_array());
            uv.push([st + 0.5, met]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

/// Printed aluminium: glossy, a little metallic (no environment map here, so full metal would
/// read black), label from the baked colourway texture.
fn can_material(way: usize) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(beer_can_tex_handle(way)),
        metallic: 0.3,
        perceptual_roughness: 0.34,
        reflectance: 0.6,
        ..default()
    }
}

/// One material per label colourway, shared by every can of a stack.
pub fn can_materials(materials: &mut Assets<StandardMaterial>) -> Vec<Handle<StandardMaterial>> {
    (0..BEER_CAN_COLOURWAYS).map(|w| materials.add(can_material(w))).collect()
}

// ---------------------------------------------------------------------------------------------
// Pub 217 can pyramid + hero can
// ---------------------------------------------------------------------------------------------

/// Spawns the can pyramid and the hero can as children of the Pub 217 root. `table_top` is the
/// centre of the tower table's top in the pub root's local space. The pyramid runs along the
/// table diagonal (as the old glass tower did) and faces the room; the hero can stands on the
/// room-side corner of the table, on the side Stan walks up from.
pub fn spawn_pub_beer_tower(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    table_top: Vec3,
) {
    let can = meshes.add(can_mesh());
    let mats = can_materials(materials);
    let along = Vec3::new(FRAC_1_SQRT_2, 0.0, FRAC_1_SQRT_2);
    let front = Vec3::new(FRAC_1_SQRT_2, 0.0, -FRAC_1_SQRT_2);
    for tier in 0..PYRAMID_TIERS {
        let count = PYRAMID_TIERS - tier;
        let y = table_top.y + CAN_HALF_H + tier as f32 * TIER_PITCH;
        for j in 0..count {
            let off = (j as f32 - (count as f32 - 1.0) * 0.5) * CAN_SPACING;
            // Labels face the room (-PI/4), with a deterministic jumble so they don't line up.
            let yaw = -PI * 0.25 + (((tier * 7 + j * 13) % 10) as f32 - 4.5) * 0.12;
            parent.spawn((
                Mesh3d(can.clone()),
                MeshMaterial3d(mats[(tier * 2 + j) % mats.len()].clone()),
                Transform::from_translation(Vec3::new(table_top.x, y, table_top.z) + along * off)
                    .with_rotation(Quat::from_rotation_y(yaw)),
            ));
        }
    }

    // Hero can on a coaster at the room-side corner.
    let hero_mat = materials.add(can_material(0));
    let coaster_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.10, 0.07),
        perceptual_roughness: 0.9,
        ..default()
    });
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(2.4, 2.0, 1.2)),
        unlit: true,
        ..default()
    });
    let halo_mat = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(1.1, 0.68, 0.2)),
        base_color_texture: Some(BEER_GLOW_TEX),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    let spark_mesh = meshes.add(sparkle_mesh());
    let halo_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let coaster_mesh = meshes.add(Cylinder::new(0.19, COASTER_H));
    let n_sparks = if crate::gfx::low_quality_requested() { 4 } else { 10 };

    let hero_base = table_top + front * 0.81 + along * 0.32;
    parent
        .spawn((Transform::from_translation(hero_base), Visibility::default()))
        .with_children(|h| {
            h.spawn((
                Mesh3d(coaster_mesh),
                MeshMaterial3d(coaster_mat),
                Transform::from_xyz(0.0, COASTER_H * 0.5, 0.0),
            ));
            h.spawn((
                BeerHeroCan { mat: hero_mat.clone() },
                Mesh3d(can),
                MeshMaterial3d(hero_mat),
                Transform::from_xyz(0.0, hero_can_y(), 0.0).with_scale(Vec3::splat(HERO_SCALE)),
                Visibility::default(),
            ));
            h.spawn((BeerHeroFx, Transform::default(), Visibility::Hidden))
                .with_children(|fx| {
                    fx.spawn((
                        BeerHeroHalo,
                        Mesh3d(halo_mesh),
                        MeshMaterial3d(halo_mat),
                        Transform::from_xyz(0.0, hero_can_y(), 0.0),
                        NotShadowCaster,
                        NotShadowReceiver,
                    ));
                    for i in 0..n_sparks {
                        fx.spawn((
                            BeerSparkle { i, n: n_sparks },
                            Mesh3d(spark_mesh.clone()),
                            MeshMaterial3d(spark_mat.clone()),
                            Transform::from_scale(Vec3::ZERO),
                            NotShadowCaster,
                            NotShadowReceiver,
                        ));
                    }
                });
        });
}

fn hero_can_y() -> f32 {
    COASTER_H + CAN_HALF_H * HERO_SCALE
}

/// Six-point star: three thin diamonds (stretched octahedra) along X, Y and Z, unit length.
fn sparkle_mesh() -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let (p, q) = (axis.any_orthonormal_vector(), axis.cross(axis.any_orthonormal_vector()));
        let tips = [axis * 0.5, -axis * 0.5];
        let ring = [p * 0.09, q * 0.09, -p * 0.09, -q * 0.09];
        for tip in tips {
            for k in 0..4 {
                let (a, b) = (ring[k], ring[(k + 1) % 4]);
                let (a, b) = if tip.dot(axis) > 0.0 { (a, b) } else { (b, a) };
                let n = (a - tip).cross(b - tip).normalize();
                let n = if n.dot(a + b + tip) < 0.0 { -n } else { n };
                let base = pos.len() as u32;
                for v in [tip, a, b] {
                    pos.push(v.to_array());
                    nrm.push(n.to_array());
                }
                idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 1]);
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_indices(Indices::U32(idx))
}

/// Mirrors the floor ring's visibility onto the hero glow/sparkles, animates them (pulse,
/// twinkle, gentle bob and spin) and plays the drink reaction: when the pour cooldown starts
/// the can lifts, pops away in a sparkle burst and a fresh one pops back in as it ends.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate_beer_hero(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ring_q: Query<&Visibility, With<BeerTowerGlow>>,
    mut fx_q: Query<&mut Visibility, (With<BeerHeroFx>, Without<BeerTowerGlow>, Without<BeerHeroCan>)>,
    mut can_q: Query<(&mut Transform, &mut Visibility, &BeerHeroCan), (Without<BeerHeroFx>, Without<BeerTowerGlow>)>,
    mut halo_q: Query<(&mut Transform, &GlobalTransform), (With<BeerHeroHalo>, Without<BeerHeroCan>)>,
    mut spark_q: Query<
        (&mut Transform, &BeerSparkle),
        (Without<BeerHeroHalo>, Without<BeerHeroCan>),
    >,
    cam_q: Query<&GlobalTransform, (With<Camera3d>, Without<BeerHeroHalo>)>,
    mut react: Local<(f32, f32)>,
) {
    let Ok((mut c_tf, mut c_vis, hero)) = can_q.get_single_mut() else {
        return;
    };
    let available = ring_q.iter().any(|v| *v != Visibility::Hidden);
    let want = if available { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut fx_q {
        if *v != want {
            *v = want;
        }
    }
    let t = time.elapsed_secs();

    // Drink reaction, keyed off the pour cooldown jumping up.
    let cd = stats.beer_cooldown;
    let (prev, total) = &mut *react;
    if cd > *prev + 0.5 {
        *total = cd;
    }
    *prev = cd;
    let (mut s, mut lift, mut burst) = (1.0_f32, 0.0_f32, 0.0_f32);
    if cd > 0.0 && *total > 0.0 {
        let el = *total - cd;
        burst = (1.0 - el / 0.8).max(0.0);
        if el < 0.25 {
            let k = el / 0.25;
            lift = 0.35 * (k * PI * 0.5).sin();
            s = 1.0 + 0.15 * k;
        } else if el < 0.4 {
            lift = 0.35;
            s = 1.15 * (1.0 - (el - 0.25) / 0.15);
        } else if cd > 0.45 {
            s = 0.0;
        } else {
            let k = 1.0 - cd / 0.45;
            s = k * (1.0 + 0.35 * (k * PI).sin());
        }
    } else {
        *total = 0.0;
    }
    if available && s >= 1.0 {
        lift += 0.025 * (t * 2.2).sin();
    }
    let can_vis = if s < 0.02 { Visibility::Hidden } else { Visibility::Inherited };
    if *c_vis != can_vis {
        *c_vis = can_vis;
    }
    c_tf.translation.y = hero_can_y() + lift;
    c_tf.scale = Vec3::splat(HERO_SCALE * s.max(0.02));
    c_tf.rotation = if available { Quat::from_rotation_y(t * 0.7) } else { Quat::IDENTITY };

    // Label shimmer: a gentle warm emissive pulse only while the beer is on offer.
    let glow = if available { 0.16 + 0.14 * (0.5 + 0.5 * (t * 3.0).sin()) } else { 0.0 };
    if let Some(m) = materials.get(&hero.mat) {
        if (m.emissive.red - glow).abs() > 0.004 || (glow > 0.0) != m.emissive_texture.is_some() {
            if let Some(m) = materials.get_mut(&hero.mat) {
                m.emissive = LinearRgba::rgb(glow, glow * 0.82, glow * 0.6);
                m.emissive_texture = (glow > 0.0).then(|| beer_can_tex_handle(0));
            }
        }
    }
    if !available {
        return;
    }

    // Halo: camera-facing soft sprite behind/around the can, breathing with the pulse.
    if let (Ok((mut h_tf, h_gt)), Some(cam)) = (halo_q.get_single_mut(), cam_q.iter().next()) {
        let parent_rot = h_gt.compute_transform().rotation * h_tf.rotation.inverse();
        h_tf.rotation = parent_rot.inverse() * cam.compute_transform().rotation;
        h_tf.translation.y = hero_can_y() + lift;
        let k = (0.95 + 0.12 * (t * 3.0).sin()) * s.max(0.3);
        h_tf.scale = Vec3::new(k * 0.9, k * 1.25, 1.0);
    }

    // Sparkles: spiral up around the can, twinkling; burst outward on a drink.
    let spread = 1.0 + burst * 2.5;
    for (mut tf, sp) in &mut spark_q {
        let ph = sp.i as f32 / sp.n as f32;
        let cyc = (t * 0.32 + ph * 0.618 * 7.0).fract();
        let dir = if sp.i % 2 == 0 { 1.0 } else { -1.0 };
        let ang = ph * TAU + t * 0.9 * dir;
        let r = (0.24 + 0.07 * (sp.i as f32 * 1.7).sin()) * spread;
        tf.translation = Vec3::new(ang.cos() * r, 0.08 + cyc * 0.95 + burst * 0.3, ang.sin() * r);
        let tw = (cyc * PI).sin().powi(2) * (0.55 + 0.45 * (t * 9.0 + sp.i as f32 * 2.1).sin());
        tf.scale = Vec3::splat((0.075 * tw * (1.0 + burst)).max(0.0001));
        tf.rotation = Quat::from_rotation_y(t * 2.0 + sp.i as f32) * Quat::from_rotation_x(t * 1.3);
    }
}

/// `?debug=1&at_beer=1`: once the city is up, put Stan just in front of the hero can, facing it.
fn debug_start_at_beer(
    mut done: Local<u32>,
    hero_q: Query<&GlobalTransform, With<BeerHeroCan>>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut orbit: ResMut<CameraOrbit>,
) {
    if *done >= 100 {
        return;
    }
    if !crate::audio::debug_at_beer() {
        *done = 100;
        return;
    }
    let (Ok(hero), Ok((mut tf, mut player))) = (hero_q.get_single(), player_q.get_single_mut()) else {
        return;
    };
    // Give the city entry a few frames to place Stan first.
    *done += 1;
    if *done < 5 {
        return;
    }
    *done = 100;
    let h = hero.translation();
    let away = Vec3::new(1.0, 0.0, -0.25).normalize();
    // A step to the side so the camera (behind Stan) sees the hero can past his shoulder.
    let side = Vec3::new(0.25, 0.0, 1.0).normalize();
    tf.translation = Vec3::new(h.x, 0.0, h.z) + away * 1.9 + side * 1.2;
    player.velocity = Vec3::ZERO;
    let dir = -away;
    player.facing_yaw = (-dir.x).atan2(-dir.z);
    tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    // Over the shoulder so Stan doesn't hide the hero can.
    orbit.yaw = player.facing_yaw + 0.55;
    orbit.pitch = 0.4;
    orbit.distance = 6.5;
    orbit.snap_follow = true;
}
