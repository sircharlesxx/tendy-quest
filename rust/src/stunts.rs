use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::audio::play_sfx;
use crate::city::{city_car_contact, CarContact, CarRide, CityCar, CityWorldMarker};
use crate::state::CameraOrbit;
use crate::player::Player;
use crate::scooter::CityScooter;
use crate::state::{LevelEntity, PlayerStats};

// ---------------------------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------------------------

/// Wedge launch ramp. Local +Z is the "uphill" direction, origin is the ground centre of the wedge.
#[derive(Component, Clone, Copy)]
pub struct StuntRamp {
    pub half_w: f32,
    pub half_l: f32,
    pub height: f32,
}

// ---------------------------------------------------------------------------------------------
// Shared wedge ramp query (scooter, on-foot hero and cars all use this, so they agree)
// ---------------------------------------------------------------------------------------------

/// A world point relative to one wedge ramp. Ramps stand on the street (y = 0).
#[derive(Clone, Copy, Debug)]
pub struct RampProbe {
    /// Point in the ramp's local frame (+Z uphill, origin at the ground centre).
    pub local: Vec3,
    /// Deck height at the point's position along the ramp (local z clamped onto the deck).
    pub h: f32,
    /// Rise per metre along the uphill direction (world uphill is `ramp_tf.rotation * Vec3::Z`).
    pub slope: f32,
    /// Inside the unpadded footprint (standing on the deck is possible here).
    pub on_deck: bool,
}

impl StuntRamp {
    pub fn slope(&self) -> f32 {
        self.height / (2.0 * self.half_l)
    }
}

/// Probe `p` against a ramp; `None` when outside the footprint grown by `pad` on every side.
pub fn ramp_probe(rtf: &Transform, ramp: &StuntRamp, p: Vec3, pad: f32) -> Option<RampProbe> {
    let local = rtf.rotation.inverse() * (p - rtf.translation);
    if local.x.abs() > ramp.half_w + pad || local.z.abs() > ramp.half_l + pad {
        return None;
    }
    let z = local.z.clamp(-ramp.half_l, ramp.half_l);
    Some(RampProbe {
        local,
        h: ramp.height * (z + ramp.half_l) / (2.0 * ramp.half_l),
        slope: ramp.slope(),
        on_deck: local.x.abs() <= ramp.half_w && local.z.abs() <= ramp.half_l,
    })
}

/// Which vertical face a point inside (or against) the footprint leaves through: the back wall
/// (+Z end) or a side. `prev_local` (where the mover came from, in the ramp frame) decides when it
/// is known; otherwise the shallower face wins (the scooter's original rule).
/// Returns (new local point placed `margin` outside that face, world outward normal).
pub fn ramp_exit(
    rtf: &Transform,
    ramp: &StuntRamp,
    local: Vec3,
    prev_local: Option<Vec3>,
    margin: f32,
) -> (Vec3, Vec3) {
    let to_back = ramp.half_l - local.z;
    let to_side = ramp.half_w - local.x.abs();
    let back = match prev_local {
        Some(pl) => {
            let from_back = pl.z >= ramp.half_l - 0.05;
            let from_side = pl.x.abs() >= ramp.half_w - 0.05;
            if from_back != from_side {
                from_back
            } else {
                to_back < to_side
            }
        }
        None => to_back < to_side,
    };
    let mut out = local;
    let n_local = if back {
        out.z = ramp.half_l + margin;
        Vec3::Z
    } else {
        let sx = if local.x == 0.0 { 1.0 } else { local.x.signum() };
        out.x = sx * (ramp.half_w + margin);
        Vec3::X * sx
    };
    (out, rtf.rotation * n_local)
}

/// Vertical boost ring. The ring plane's normal is the entity's local Z axis.
#[derive(Component)]
pub struct BoostRing {
    pub radius: f32,
    pub cooldown: f32,
    pub circuit: usize,
    pub bit: u32,
}

/// Flat glowing speed strip painted on the asphalt.
#[derive(Component)]
pub struct BoostPad {
    pub half_w: f32,
    pub half_l: f32,
    /// Speed the pad kicks the scooter up to straight away (m/s).
    pub kick: f32,
    /// Boost top speed while this pad's boost burns (`CityScooter::boost_top`).
    pub top: f32,
    /// Boost seconds, counted from leaving the pad.
    pub secs: f32,
    /// The one full-speed pad in front of the big ramp: forces full throttle (brake still wins).
    pub mega: bool,
}

/// Ordinary pad: a short, controllable push a little above turbo (23 m/s).
const PAD_KICK: f32 = 25.0;
const PAD_SECS: f32 = 1.2;
/// Ground boost rings: a small bump that tops the boost up, never past these caps.
const RING_SPEED_BUMP: f32 = 2.0;
const RING_BOOST_SECS: f32 = 0.6;
const RING_BOOST_CAP: f32 = 1.9;
/// The mega pad (Mays Island Mega Send): the only place the scooter reaches 50 m/s. 2.3 s runs
/// out about 0.1 s after a no-hop landing off the mega ramp (see `MEGA` layout below).
const MEGA_SPEED: f32 = 50.0;
const MEGA_SECS: f32 = 2.3;

#[derive(Component)]
pub struct ScooterBoostFlame;

// Cedar Rapids stunt circuits: three street runs with 6 rings each (2 ground, 3 air, 1 landing)
// and the Mays Island Mega Send with 3 big air rings. `StuntState.collected` stores a bitmask per
// circuit so hitting all of a circuit's rings awards the circuit bonus.
pub const CIRCUIT_RING_COUNTS: [u32; 4] = [1, 1, 1, 1];
pub const CIRCUIT_NAMES: [&str; 4] = [
    "3RD ST SE GAUNTLET",
    "CROSS-STREET SEND",
    "GREENE SQUARE FLYBY",
    "MAYS ISLAND MEGA SEND",
];

#[derive(Resource, Default)]
pub struct StuntState {
    pub combo: u32,
    pub combo_t: f32,
    pub rings_total: u32,
    pub rings_since_heart: u32,
    pub collected: [u32; 4],
    pub circuits_done: u32,
}

// ---------------------------------------------------------------------------------------------
// Course layout (shared by the spawner and the tendy-trail helper)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct RunSpec {
    /// Centre of the run's boost pad (run distance s = 0).
    pad: Vec3,
    /// Travel direction: (sin yaw, cos yaw) on the ground.
    yaw: f32,
    circuit: usize,
    /// The Mays Island Mega Send: mega pad, the big ramp, 3 big air rings.
    mega: bool,
}

// Layout along each run (s in metres from the pad centre, along `yaw`). Ordinary run: gate -6,
// pad 0, ground rings 10 and 18, ramp centre 30 (lip 34.5), 3 air rings 8/16/24 m past the lip on
// the boosted no-hop arc, landing ring 12 m past that arc's touchdown. The roads are 20 m wide
// with lanes 3.6 m off the centre line (cars reach 2.525 m in), so the 2.4 m half-wide ramps and
// pads on the centre line leave every lane clear, and each ramp sits mid-block.
//
// All runs share no ramp, pad or ring with another run's line; they only cross in intersections
// that one of them flies over. Flight numbers (`STUNT_GRAVITY` 28, gain 2.28, cap 22):
//   ordinary ramp at 27 m/s (boosted): 1.30 s, 35 m; with a hop 1.83 s, 49 m
//                 at 23 m/s (turbo):   1.15 s, 26 m; with a hop 1.66 s, 38 m
//                 at 17 m/s:           0.92 s, 16 m; with a hop 1.42 s, 24 m
//   mega ramp at 50 m/s:               1.74 s, 87 m; with a hop 2.26 s, ~110 m
const RUNS: [RunSpec; 4] = [
    // Circuit 0: 3rd St SE Gauntlet, southbound on x = 0 from the 6th St crossing (z = -46),
    // over 7th St, past Pub 217. Gate z = -20, pad -14, ramp 16 (toe 11.5, lip 20.5, clear of the
    // z = 0 lanes), boosted landing z = 55.7, hop landing 69.9; 56 m+ of road to the south edge.
    // No longer northbound: the old line flew into Empire Citadel (z = -71).
    RunSpec {
        pad: Vec3::new(0.0, 0.0, -14.0),
        yaw: 0.0,
        circuit: 0,
        mega: false,
    },
    // Circuit 1: Cross-Street Send, westbound on z = -46 (moved from z = 46 eastbound, which ran
    // out at the east edge). Gate x = 62, pad 56, ramp 26 (lip 21.5), boosted landing x = -13.7,
    // hop landing -27.9; 98 m+ of road to the river side (x = -126).
    RunSpec {
        pad: Vec3::new(56.0, 0.0, -46.0),
        yaw: -FRAC_PI_2,
        circuit: 1,
        mega: false,
    },
    // Circuit 2: Greene Square Flyby, northbound on x = 92 from beside Greene Square (replaces
    // the plaza loop, whose 9 m circle of kickers launched riders into the skate park pieces and
    // could not be followed at boost speed). Gate z = 58, pad 52, ramp 22 (lip 17.5), boosted
    // landing z = -17.7, hop landing -31.9; 94 m+ of road to the north edge.
    RunSpec {
        pad: Vec3::new(92.0, 0.0, 52.0),
        yaw: PI,
        circuit: 2,
        mega: false,
    },
    // Circuit 3: Mays Island Mega Send, eastbound on 7th St (z = 0), the longest clear straight
    // in the city (edge to edge, 252 m, no building, the river is not solid). Gate x = -99 off
    // the Mays Island bridge, mega pad -87, mega ramp -67 (toe -74, lip -60), no-hop landing
    // x = 26.8, hop landing ~48; the boost runs out at landing and the road runs on to x = 126.
    RunSpec {
        pad: Vec3::new(-87.0, 0.0, 0.0),
        yaw: FRAC_PI_2,
        circuit: 3,
        mega: true,
    },
];

const RUN_RAMP: StuntRamp = StuntRamp {
    half_w: 2.4,
    half_l: 4.5,
    height: 2.4,
};
/// The one big ramp: same width (stays out of the lanes), longer and 4 m tall.
const MEGA_RAMP: StuntRamp = StuntRamp {
    half_w: 2.4,
    half_l: 7.0,
    height: 4.0,
};

fn run_pos(spec: &RunSpec, s: f32, y: f32) -> Vec3 {
    Vec3::new(spec.pad.x + spec.yaw.sin() * s, y, spec.pad.z + spec.yaw.cos() * s)
}

fn run_ramp(spec: &RunSpec) -> StuntRamp {
    if spec.mega {
        MEGA_RAMP
    } else {
        RUN_RAMP
    }
}

/// Ramp centre along the run.
fn ramp_s(spec: &RunSpec) -> f32 {
    if spec.mega {
        20.0
    } else {
        30.0
    }
}

fn lip_s(spec: &RunSpec) -> f32 {
    ramp_s(spec) + run_ramp(spec).half_l
}

/// Speed the run's air rings and arc tendies are laid out for: its pad's boost top speed.
fn design_speed(spec: &RunSpec) -> f32 {
    if spec.mega {
        MEGA_SPEED
    } else {
        crate::scooter::BOOST_TOP_SPEED
    }
}

/// Lip launch speed (m/s up) for a ground speed and ramp slope. Shared with the physics below.
fn ramp_launch_vy(speed: f32, slope: f32) -> f32 {
    (speed * slope * RAMP_LAUNCH_GAIN).clamp(3.5, RAMP_LAUNCH_CAP)
}

/// Wheel height `d` metres past the lip on the run's boosted, no-hop arc.
fn arc_y(spec: &RunSpec, d: f32) -> f32 {
    let ramp = run_ramp(spec);
    let v = design_speed(spec);
    let vy = ramp_launch_vy(v, ramp.slope());
    let t = d / v;
    ramp.height + vy * t - 0.5 * STUNT_GRAVITY * t * t
}

/// Run distance of the boosted, no-hop touchdown.
fn land_s(spec: &RunSpec) -> f32 {
    let ramp = run_ramp(spec);
    let v = design_speed(spec);
    let vy = ramp_launch_vy(v, ramp.slope());
    let t = (vy + (vy * vy + 2.0 * STUNT_GRAVITY * ramp.height).sqrt()) / STUNT_GRAVITY;
    lip_s(spec) + v * t
}

/// Distance past the lip of the highest point of the run's boosted, no-hop arc: where its one
/// ring hangs.
fn apex_d(spec: &RunSpec) -> f32 {
    let v = design_speed(spec);
    v * ramp_launch_vy(v, run_ramp(spec).slope()) / STUNT_GRAVITY
}

/// Floating Tendy trails that follow the ground path and the jump arcs of every circuit. They
/// also keep the random parked car off each run (it stays 12 m from these).
pub fn stunt_tendy_spots() -> Vec<Vec3> {
    let mut out = Vec::new();
    for spec in RUNS.iter() {
        let lip = lip_s(spec);
        let land = land_s(spec);
        let (ground, air, roll): (&[f32], &[f32], &[f32]) = if spec.mega {
            (&[10.0], &[10.0, 37.0, 64.0, 82.0], &[15.0, 40.0])
        } else {
            (&[-14.0, 8.0, 20.0], &[6.0, 16.0, 26.0], &[8.0, 28.0, 48.0])
        };
        for &s in ground {
            out.push(run_pos(spec, s, 1.0));
        }
        for &d in air {
            out.push(run_pos(spec, lip + d, arc_y(spec, d) + 1.0));
        }
        for &r in roll {
            out.push(run_pos(spec, land + r, 1.0));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Meshes & materials
// ---------------------------------------------------------------------------------------------

fn wedge_mesh(hw: f32, hl: f32, h: f32) -> Mesh {
    let a = [-hw, 0.0, -hl];
    let b = [hw, 0.0, -hl];
    let c = [-hw, 0.0, hl];
    let d = [hw, 0.0, hl];
    let e = [-hw, h, hl];
    let f = [hw, h, hl];
    let slope_n = Vec3::new(0.0, 2.0 * hl, -h).normalize().to_array();
    let tris: [([[f32; 3]; 3], [f32; 3]); 6] = [
        ([a, e, f], slope_n),
        ([a, f, b], slope_n),
        ([c, d, f], [0.0, 0.0, 1.0]),
        ([c, f, e], [0.0, 0.0, 1.0]),
        ([a, c, e], [-1.0, 0.0, 0.0]),
        ([b, f, d], [1.0, 0.0, 0.0]),
    ];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    for (verts, n) in tris {
        for (i, v) in verts.iter().enumerate() {
            positions.push(*v);
            normals.push(n);
            uvs.push(match i {
                0 => [0.0, 0.0],
                1 => [1.0, 0.0],
                _ => [0.0, 1.0],
            });
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

struct StuntMats {
    ramp_body: Handle<StandardMaterial>,
    stripe_cyan: Handle<StandardMaterial>,
    stripe_pink: Handle<StandardMaterial>,
    ring_cyan: Handle<StandardMaterial>,
    ring_gold: Handle<StandardMaterial>,
    ring_glow_cyan: Handle<StandardMaterial>,
    ring_glow_gold: Handle<StandardMaterial>,
    pad_base: Handle<StandardMaterial>,
    pad_chevron: Handle<StandardMaterial>,
    pad_mega: Handle<StandardMaterial>,
    pad_mega_chevron: Handle<StandardMaterial>,
    gate: Handle<StandardMaterial>,
    gate_letter: Handle<StandardMaterial>,
}

fn make_stunt_mats(materials: &mut Assets<StandardMaterial>) -> StuntMats {
    StuntMats {
        ramp_body: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.12, 0.26),
            metallic: 0.5,
            perceptual_roughness: 0.4,
            ..default()
        }),
        stripe_cyan: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.95, 1.0),
            emissive: LinearRgba::new(0.4, 3.2, 3.8, 1.0),
            ..default()
        }),
        stripe_pink: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.3, 0.8),
            emissive: LinearRgba::new(3.6, 0.7, 2.6, 1.0),
            ..default()
        }),
        ring_cyan: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.95, 1.0),
            emissive: LinearRgba::new(0.5, 3.6, 4.4, 1.0),
            ..default()
        }),
        ring_gold: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.2),
            emissive: LinearRgba::new(4.4, 3.2, 0.4, 1.0),
            ..default()
        }),
        ring_glow_cyan: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.95, 1.0, 0.13),
            emissive: LinearRgba::new(0.1, 0.5, 0.6, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        ring_glow_gold: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.85, 0.2, 0.15),
            emissive: LinearRgba::new(0.6, 0.45, 0.05, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        pad_base: materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.2, 0.7),
            emissive: LinearRgba::new(1.6, 0.25, 1.2, 1.0),
            ..default()
        }),
        pad_chevron: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.95, 0.5),
            emissive: LinearRgba::new(4.0, 3.6, 1.0, 1.0),
            ..default()
        }),
        // Mega pad: hot orange-gold, much brighter than the pink pads, white-hot chevrons.
        pad_mega: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.55, 0.08),
            emissive: LinearRgba::new(6.0, 2.6, 0.2, 1.0),
            ..default()
        }),
        pad_mega_chevron: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 0.95),
            emissive: LinearRgba::new(7.0, 6.6, 5.0, 1.0),
            ..default()
        }),
        gate: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.10, 0.18),
            metallic: 0.6,
            perceptual_roughness: 0.35,
            emissive: LinearRgba::new(0.2, 0.1, 0.4, 1.0),
            ..default()
        }),
        gate_letter: materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 1.0, 0.45),
            emissive: LinearRgba::new(0.6, 4.2, 1.4, 1.0),
            ..default()
        }),
    }
}

// ---------------------------------------------------------------------------------------------
// Spawners
// ---------------------------------------------------------------------------------------------

fn spawn_ramp(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &StuntMats,
    pos: Vec3,
    yaw: f32,
    ramp: StuntRamp,
    mega: bool,
) {
    let StuntRamp {
        half_w,
        half_l,
        height,
    } = ramp;
    let angle = (height / (2.0 * half_l)).atan();
    let slope_len = ((2.0 * half_l).powi(2) + height.powi(2)).sqrt();
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let wedge = meshes.add(wedge_mesh(half_w, half_l, height));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            ramp,
            Transform::from_translation(Vec3::new(pos.x, 0.0, pos.z))
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|r| {
            r.spawn((
                Mesh3d(wedge),
                MeshMaterial3d(mats.ramp_body.clone()),
                Transform::IDENTITY,
            ));
            // Glowing chevron stripes along the deck
            for i in 0..5 {
                let f = (i as f32 + 0.5) / 5.0;
                let z = -half_l + f * 2.0 * half_l;
                let y = height * f + 0.04;
                let mat = if mega {
                    mats.pad_mega_chevron.clone()
                } else if i % 2 == 0 {
                    mats.stripe_cyan.clone()
                } else {
                    mats.stripe_pink.clone()
                };
                r.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, y, z)
                        .with_rotation(Quat::from_rotation_x(-angle))
                        .with_scale(Vec3::new(half_w * 1.6, 0.05, 0.55)),
                ));
            }
            // Neon side rails
            let rail = if mega { mats.pad_mega.clone() } else { mats.stripe_cyan.clone() };
            for sx in [-half_w, half_w] {
                r.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(rail.clone()),
                    Transform::from_xyz(sx, height * 0.5 + 0.05, 0.0)
                        .with_rotation(Quat::from_rotation_x(-angle))
                        .with_scale(Vec3::new(0.14, 0.14, slope_len)),
                ));
            }
            // Bright lip at the launch edge
            r.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(mats.stripe_pink.clone()),
                Transform::from_xyz(0.0, height + 0.05, half_l - 0.05)
                    .with_scale(Vec3::new(half_w * 2.0, 0.12, 0.22)),
            ));
        });
}

fn spawn_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &StuntMats,
    center: Vec3,
    yaw: f32,
    radius: f32,
    circuit: usize,
    bit: u32,
    gold: bool,
) {
    let (ring_mat, glow_mat) = if gold {
        (mats.ring_gold.clone(), mats.ring_glow_gold.clone())
    } else {
        (mats.ring_cyan.clone(), mats.ring_glow_cyan.clone())
    };
    let torus = meshes.add(Torus::new(radius - 0.22, radius + 0.22));
    let disc = meshes.add(Cylinder::new(radius - 0.2, 0.03));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            BoostRing {
                radius,
                cooldown: 0.0,
                circuit,
                bit,
            },
            Transform::from_translation(center).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|r| {
            r.spawn((
                Mesh3d(torus),
                MeshMaterial3d(ring_mat),
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
            r.spawn((
                Mesh3d(disc),
                MeshMaterial3d(glow_mat),
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
        });
}

fn spawn_pad(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &StuntMats,
    pos: Vec3,
    yaw: f32,
    pad: BoostPad,
) {
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let (half_w, half_l) = (pad.half_w, pad.half_l);
    let (base_mat, chevron_mat) = if pad.mega {
        (mats.pad_mega.clone(), mats.pad_mega_chevron.clone())
    } else {
        (mats.pad_base.clone(), mats.pad_chevron.clone())
    };
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            pad,
            Transform::from_translation(Vec3::new(pos.x, 0.03, pos.z))
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(base_mat),
                Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(
                    half_w * 2.0,
                    0.06,
                    half_l * 2.0,
                )),
            ));
            // Three forward-pointing chevrons
            let arm_w = half_w * 0.75;
            let arm_d = half_l * 0.38;
            let arm_len = (arm_w * arm_w + arm_d * arm_d).sqrt();
            for i in 0..3 {
                let z0 = half_l * 0.62 - i as f32 * half_l * 0.62;
                for side in [-1.0_f32, 1.0] {
                    p.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(chevron_mat.clone()),
                        Transform::from_xyz(side * arm_w * 0.5, 0.05, z0 - arm_d * 0.5)
                            .with_rotation(Quat::from_rotation_y((side * arm_w).atan2(-arm_d)))
                            .with_scale(Vec3::new(0.28, 0.04, arm_len)),
                    ));
                }
            }
        });
}

fn spawn_gate(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &StuntMats, pos: Vec3, yaw: f32) {
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            Transform::from_translation(Vec3::new(pos.x, 0.0, pos.z))
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|g| {
            for sx in [-7.4_f32, 7.4] {
                g.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mats.gate.clone()),
                    Transform::from_xyz(sx, 3.9, 0.0).with_scale(Vec3::new(0.7, 7.8, 0.7)),
                ));
                g.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mats.stripe_pink.clone()),
                    Transform::from_xyz(sx, 3.9, 0.0).with_scale(Vec3::new(0.78, 7.6, 0.2)),
                ));
            }
            g.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(mats.gate.clone()),
                Transform::from_xyz(0.0, 7.9, 0.0).with_scale(Vec3::new(15.6, 1.9, 0.8)),
            ));
            // Block letters "V E O" on the header, readable from the approach side (+Z local)
            let bars: [(f32, f32, f32, f32, f32); 13] = [
                // V
                (-3.2, 8.15, 0.28, 1.5, 0.30),
                (-2.4, 8.15, 0.28, 1.5, -0.30),
                // E
                (-0.45, 7.9, 0.28, 1.5, 0.0),
                (0.0, 8.57, 0.9, 0.26, 0.0),
                (0.0, 7.9, 0.7, 0.26, 0.0),
                (0.0, 7.23, 0.9, 0.26, 0.0),
                // O
                (2.3, 8.57, 1.0, 0.26, 0.0),
                (2.3, 7.23, 1.0, 0.26, 0.0),
                (1.82, 7.9, 0.26, 1.5, 0.0),
                (2.78, 7.9, 0.26, 1.5, 0.0),
                // arrow tick marks
                (-5.8, 7.9, 0.5, 0.2, 0.0),
                (5.8, 7.9, 0.5, 0.2, 0.0),
                (0.0, 6.95, 14.0, 0.14, 0.0),
            ];
            for (x, y, sx, sy, rz) in bars {
                g.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mats.gate_letter.clone()),
                    Transform::from_xyz(x, y, 0.45)
                        .with_rotation(Quat::from_rotation_z(rz))
                        .with_scale(Vec3::new(sx, sy, 0.14)),
                ));
            }
        });
}

fn spawn_run(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &StuntMats,
    spec: &RunSpec,
) {
    let yaw = spec.yaw;
    let c = spec.circuit;
    let lip = lip_s(spec);
    // Gate is rotated so its +Z lettering faces the approaching rider
    let gate_s = if spec.mega { -12.0 } else { -6.0 };
    spawn_gate(commands, meshes, mats, run_pos(spec, gate_s, 0.0), yaw + PI);
    let pad = if spec.mega {
        BoostPad {
            half_w: 2.4,
            half_l: 4.0,
            kick: MEGA_SPEED,
            top: MEGA_SPEED,
            secs: MEGA_SECS,
            mega: true,
        }
    } else {
        BoostPad {
            half_w: 2.0,
            half_l: 3.0,
            kick: PAD_KICK,
            top: crate::scooter::BOOST_TOP_SPEED,
            secs: PAD_SECS,
            mega: false,
        }
    };
    spawn_pad(commands, meshes, mats, run_pos(spec, 0.0, 0.0), yaw, pad);
    spawn_ramp(commands, meshes, mats, run_pos(spec, ramp_s(spec), 0.0), yaw, run_ramp(spec), spec.mega);
    // Rider point is 0.9 m above the wheels (see the ring test), so ring centres sit 0.9 m above
    // the arc.
    // One ring per jump, at the apex of the boosted arc. It is wide enough to catch a slower
    // (unboosted) or faster (Shift on the pad, a hop off the lip is higher still) flight nearby.
    let d = apex_d(spec);
    let p = run_pos(spec, lip + d, arc_y(spec, d) + 0.9);
    spawn_ring(commands, meshes, mats, p, yaw, if spec.mega { 5.5 } else { 4.2 }, c, 0, true);
}

pub fn spawn_stunt_course(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mats = make_stunt_mats(materials);
    for spec in RUNS.iter() {
        spawn_run(commands, meshes, &mats, spec);
    }
}

// ---------------------------------------------------------------------------------------------
// Per-frame stunt physics
// ---------------------------------------------------------------------------------------------

/// Same press-before-touchdown window as the on-foot jump (`player.rs` JUMP_BUFFER_SECS).
const SCOOTER_JUMP_BUFFER_SECS: f32 = 0.12;
/// Pre-jump crouch before the hop leaves the ground (short enough not to feel laggy).
const SCOOTER_CROUCH_SECS: f32 = 0.05;
/// A press this soon after leaving a ramp lip still counts as jumping off it.
const LIP_GRACE_SECS: f32 = 0.15;
/// Share of the hop impulse stacked on top of a ramp launch.
const LIP_HOP_SHARE: f32 = 0.75;

/// Awards combo, score, TW, energy, hearts, and circuit completion when threading a boost ring.
fn score_boost_ring(stats: &mut PlayerStats, st: &mut StuntState, ring: &BoostRing, high: bool) {
    st.combo += 1;
    st.combo_t = 4.0;
    st.rings_total += 1;
    st.rings_since_heart += 1;
    let pts = 250 * st.combo * if high { 2 } else { 1 };
    stats.score += pts;
    stats.tendy_credits += if high { 2 } else { 1 };
    let ring_tw = if high { 3 } else { 1 };
    stats.add_tendy_wendies(ring_tw);
    stats.dash_energy = (stats.dash_energy + 25.0).min(100.0);
    play_sfx("powerup");
    stats.hint_text = format!(
        "{} +{}  +{} TW  COMBO x{}",
        if high { "AIR RING!" } else { "BOOST RING!" },
        pts,
        ring_tw,
        st.combo
    );
    if st.rings_since_heart >= 5 {
        st.rings_since_heart = 0;
        if stats.hearts < stats.max_hearts {
            stats.hearts += 1;
            stats.hint_text.push_str("  |  +1 HEART!");
        }
    }
    let c = ring.circuit.min(CIRCUIT_NAMES.len() - 1);
    st.collected[c] |= 1 << ring.bit;
    if st.collected[c].count_ones() >= CIRCUIT_RING_COUNTS[c] {
        st.collected[c] = 0;
        st.circuits_done += 1;
        stats.score += 2000;
        stats.tendy_credits += 4;
        stats.add_tendy_wendies(15);
        stats.hearts = (stats.hearts + 1).min(stats.max_hearts);
        stats.dash_energy = 100.0;
        play_sfx("win");
        stats.hint_text = format!(
            "JUMP NAILED: {}!  +2000  +15 TW  +1 HEART!  ({} done)",
            CIRCUIT_NAMES[c], st.circuits_done
        );
    }
}

/// Hop take-off speed (m/s). Sized to clear the city cars (cab sign tops 1.89 m, 4.66 m long):
/// at the city's 1.8 scale 10.5 m/s under `HOP_RISE_GRAVITY` peaks at 2.5 m, stays above a cab
/// sign for 0.45 s and above the hood for 0.67 s, and lands after 0.9 s (13.5 m at 15 m/s).
/// Grows gently with Stan; capped at 11.5 near the 6.0 riding limit.
fn scooter_hop_speed(player_scale: f32) -> f32 {
    (10.2 + (player_scale - 0.55).max(0.0) * 0.25).min(11.5)
}
/// Stunt gravity (ramp launches and every descent).
const STUNT_GRAVITY: f32 = 28.0;
/// Softer gravity while a flat hop is still rising: more height and hang without a floaty fall.
const HOP_RISE_GRAVITY: f32 = 22.0;
/// Ramp-lip launch: vy = speed * slope * gain, clamped 3.5..`RAMP_LAUNCH_CAP` (`ramp_launch_vy`).
/// Tuned for the controllable boosts (27 m/s): on a run ramp (slope 0.267) 17 m/s gives 10.3 m/s
/// up (0.9 s of air), 23 gives 14.0 (1.15 s), a boosted 27 gives 16.4 (1.3 s, 1.8 s with a hop).
/// Only the mega ramp at 50 m/s reaches the cap, so its arc (and air rings) do not depend on the
/// exact arrival speed.
const RAMP_LAUNCH_GAIN: f32 = 2.28;
const RAMP_LAUNCH_CAP: f32 = 22.0;

#[allow(clippy::type_complexity)]
pub fn update_scooter_stunts(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut st: ResMut<StuntState>,
    mut player_q: Query<
        (&mut Transform, &mut Player),
        (
            Without<CityScooter>,
            Without<StuntRamp>,
            Without<BoostRing>,
            Without<BoostPad>,
            Without<ScooterBoostFlame>,
        ),
    >,
    mut scooter_q: Query<
        (Entity, &mut Transform, &mut CityScooter),
        (
            Without<Player>,
            Without<StuntRamp>,
            Without<BoostRing>,
            Without<BoostPad>,
            Without<ScooterBoostFlame>,
        ),
    >,
    ramp_q: Query<
        (&Transform, &StuntRamp),
        (
            Without<Player>,
            Without<CityScooter>,
            Without<BoostRing>,
            Without<BoostPad>,
            Without<ScooterBoostFlame>,
        ),
    >,
    mut ring_q: Query<
        (&mut Transform, &mut BoostRing),
        (
            Without<Player>,
            Without<CityScooter>,
            Without<StuntRamp>,
            Without<BoostPad>,
            Without<ScooterBoostFlame>,
        ),
    >,
    pad_q: Query<
        (&Transform, &BoostPad),
        (
            Without<Player>,
            Without<CityScooter>,
            Without<StuntRamp>,
            Without<BoostRing>,
            Without<ScooterBoostFlame>,
        ),
    >,
    mut flame_q: Query<
        (&Parent, &mut Visibility, &mut Transform),
        (
            With<ScooterBoostFlame>,
            Without<Player>,
            Without<CityScooter>,
            Without<StuntRamp>,
            Without<BoostRing>,
            Without<BoostPad>,
        ),
    >,
    car_q: Query<
        (&Transform, &CityCar),
        (
            Without<Player>,
            Without<CityScooter>,
            Without<StuntRamp>,
            Without<BoostRing>,
            Without<BoostPad>,
            Without<ScooterBoostFlame>,
        ),
    >,
    mut ragdoll: ResMut<crate::ragdoll::Ragdoll>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    // --- Boost flames on every scooter -------------------------------------------------------
    // Pads and rings burn a full flame; Shift turbo a shorter one.
    let boost_levels: Vec<(Entity, f32)> = scooter_q
        .iter()
        .map(|(e, _, sc)| {
            let lvl = if sc.boost_t > 0.0 && sc.boost_top > crate::scooter::BOOST_TOP_SPEED + 1.0 {
                1.6
            } else if sc.boost_t > 0.0 {
                1.0
            } else if sc.turbo_on && sc.occupied {
                0.6
            } else {
                0.0
            };
            (e, lvl)
        })
        .collect();
    for (parent, mut vis, mut ftf) in &mut flame_q {
        let boost = boost_levels
            .iter()
            .find(|(e, _)| *e == parent.get())
            .map(|(_, b)| *b)
            .unwrap_or(0.0);
        if boost > 0.0 {
            *vis = Visibility::Inherited;
            let flick = 1.0 + (t * 38.0).sin() * 0.28;
            ftf.scale = Vec3::new(boost.max(0.75), flick * boost, boost.max(0.75));
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // --- Rings pulse idly ---------------------------------------------------------------------
    for (mut rtf, mut ring) in &mut ring_q {
        if ring.cooldown > 0.0 {
            ring.cooldown = (ring.cooldown - dt).max(0.0);
            rtf.scale = Vec3::splat(1.0 + ring.cooldown * 0.45);
        } else {
            rtf.scale = Vec3::splat(1.0 + (t * 3.6 + ring.bit as f32).sin() * 0.035);
        }
    }

    st.combo_t -= dt;
    if st.combo_t <= 0.0 {
        st.combo_t = 0.0;
        st.combo = 0;
    }

    let Some(se) = stats.riding_scooter else {
        return;
    };
    let Ok((mut p_tf, mut rider)) = player_q.get_single_mut() else {
        return;
    };
    let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) else {
        return;
    };
    if sc.park {
        // On / over / grinding a skate park piece: `skatepark::update_park_ride` owns it.
        return;
    }

    // --- Terrain height beneath the scooter (ramps) ------------------------------------------
    let mut ground_h = 0.0_f32;
    let mut slope = 0.0_f32;
    for (rtf, ramp) in &ramp_q {
        // Shared ramp query (`ramp_probe` / `ramp_exit`), also used on foot and by cars.
        let Some(pr) = ramp_probe(rtf, ramp, stf.translation, 0.0) else {
            continue;
        };
        if pr.h > sc.y + 0.9 {
            // Ran into the back wall or side of a ramp: bump back out
            let (out, _) = ramp_exit(rtf, ramp, pr.local, None, 0.25);
            stf.translation = rtf.translation + rtf.rotation * out;
            sc.speed *= 0.4;
            continue;
        }
        if pr.h > ground_h {
            ground_h = pr.h;
            slope = pr.slope;
        }
    }

    // --- Car hoods and roofs: ride on them when coming down from above ----------------------
    // Same contact test the traffic hit uses (`city_car_contact`), so a scooter is either over a
    // car (surface below, no hit) or inside it (hit), never both.
    let mut car_ground = false;
    let wheels = Vec3::new(stf.translation.x, sc.y, stf.translation.z);
    for (ctf, car) in &car_q {
        if let CarContact::Over(h) = city_car_contact(ctf, car, wheels, 0.45) {
            if !sc.grounded && sc.flat_hop && sc.y > h {
                sc.car_hop = true;
            }
            if h > ground_h {
                ground_h = h;
                slope = 0.0;
                car_ground = true;
            }
        }
    }

    let prev_ground = sc.ground_h;
    sc.ground_h = ground_h;
    let was_on_car = sc.on_car;
    sc.on_car = car_ground;

    // --- Hop: Space / gamepad A (synthesised Space) / touch JUMP -------------------------------
    // Runs before the grounded/airborne split so a hop simply hands the scooter to the airborne
    // branch below (same state as a ramp launch: flips, scoring and touchdown all apply). The touch
    // queue is drained every riding frame so a tap here can't leak into an on-foot jump later.
    let touch_jump = crate::audio::take_touch_jump();
    let space_down = keys.pressed(KeyCode::Space);
    sc.jump_lock = (sc.jump_lock - dt).max(0.0);
    sc.jump_buf = (sc.jump_buf - dt).max(0.0);
    if (touch_jump || keys.just_pressed(KeyCode::Space)) && sc.jump_lock <= 0.0 {
        sc.jump_buf = SCOOTER_JUMP_BUFFER_SECS;
    }
    if !space_down {
        sc.hop_hold = false;
    }
    let hop_v = scooter_hop_speed(stats.player_scale);
    let mut hop_launched = false;
    if sc.crouch_t > 0.0 {
        sc.crouch_t -= dt;
        if !sc.grounded {
            // Rolled off a ramp lip mid-crouch: stack the hop onto the launch right away.
            sc.crouch_t = 0.0;
            sc.vy += hop_v * LIP_HOP_SHARE;
            sc.hopped = true;
            hop_launched = true;
        } else if sc.crouch_t <= 0.0 {
            sc.crouch_t = 0.0;
            // On a ramp face the hop adds to what the ramp would have launched at the lip.
            let on_ramp = ground_h > 0.1 && !car_ground && sc.speed > 5.0;
            sc.vy = if on_ramp {
                ramp_launch_vy(sc.speed, slope) + hop_v * LIP_HOP_SHARE
            } else {
                hop_v
            };
            sc.grounded = false;
            sc.y = sc.y.max(ground_h);
            sc.air_t = 0.0;
            sc.flip = 0.0;
            sc.hopped = true;
            sc.flat_hop = !on_ramp;
            sc.car_hop = false;
            // Flat hops keep Space-held from turning straight into a backflip; ramp hops keep the
            // classic "hold through the launch to flip" feel.
            sc.hop_hold = space_down && !on_ramp;
            hop_launched = true;
        }
    } else if sc.jump_buf > 0.0 {
        if sc.grounded {
            sc.jump_buf = 0.0;
            sc.crouch_t = SCOOTER_CROUCH_SECS;
            sc.squash_vel += 4.0;
            rider.squash_vel += 3.0;
        } else if !sc.hopped && sc.air_t < LIP_GRACE_SECS {
            // Pressed a beat after leaving a ramp lip: still counts as jumping off it.
            sc.jump_buf = 0.0;
            sc.vy += hop_v * LIP_HOP_SHARE;
            sc.hopped = true;
            hop_launched = true;
        }
    }
    if hop_launched {
        sc.squash_vel -= 5.0;
        rider.squash_vel -= 5.0;
        play_sfx("jump");
    }

    if sc.grounded {
        if was_on_car && ground_h < prev_ground - 0.05 {
            // The car drove out from under us (or we rode off it): just drop, no ramp launch
            // and no score for the fall.
            sc.grounded = false;
            sc.vy = 0.0;
            sc.y = prev_ground;
            sc.air_t = 0.0;
            sc.flip = 0.0;
            sc.hopped = true;
            sc.flat_hop = false;
            sc.car_hop = false;
        } else if prev_ground - ground_h > 0.35 && sc.speed > 5.0 {
            // Rolled off the lip of a ramp: LAUNCH!
            sc.grounded = false;
            sc.vy = ramp_launch_vy(sc.speed, sc.last_slope);
            sc.y = prev_ground;
            sc.air_t = 0.0;
            sc.flip = 0.0;
            play_sfx("dash");
            stats.hint_text = "AIRBORNE! Hold [SPACE] to BACKFLIP!".to_string();
        } else {
            sc.y = ground_h;
            if ground_h > 0.1 {
                sc.last_slope = slope;
            }
        }
    } else {
        let g = if sc.flat_hop && sc.vy > 0.0 { HOP_RISE_GRAVITY } else { STUNT_GRAVITY }
            * sc.gravity_scale;
        sc.vy -= g * dt;
        sc.y += sc.vy * dt;
        sc.air_t += dt;

        // Flat hops don't flip: their longer car-clearing airtime would otherwise turn hop-spam
        // into backflip points. Ramp launches and ramp hops flip as before.
        let flipping = space_down && sc.air_t > 0.12 && !sc.hop_hold && !sc.flat_hop;
        if flipping {
            sc.flip += 7.2 * dt;
        } else {
            let target = (sc.flip / TAU).round() * TAU;
            sc.flip += (target - sc.flip) * (dt * 9.0).min(1.0);
        }

        if sc.y <= ground_h && sc.vy <= 0.0 {
            // Touchdown
            let impact = -sc.vy;
            sc.y = ground_h;
            sc.vy = 0.0;
            sc.grounded = true;
            let nearest = (sc.flip / TAU).round();
            let off = (sc.flip - nearest * TAU).abs();
            let flips = nearest.max(0.0) as u32;
            // Landing bounce + thump, harder the faster it came down.
            if sc.air_t > 0.15 {
                let k = (impact / 14.0).clamp(0.3, 1.3);
                sc.squash_vel += 6.0 * k;
                rider.squash_vel += 5.0 * k;
                stats.shake_request = stats.shake_request.max(0.05 + 0.1 * k);
            }
            // A plain flat-ground hop is movement, not a trick: no points/credits/energy for it
            // (else hop-spamming would farm combos). Flips or big ramp-hop air still score.
            // Clearing a car is the one flat hop that scores (small: airtime points only).
            let car_hop = sc.car_hop;
            let trick = flips > 0 || !sc.hopped || (sc.air_t > 0.9 && !sc.flat_hop) || car_hop;
            sc.hopped = false;
            sc.hop_hold = false;
            sc.flat_hop = false;
            sc.car_hop = false;
            let judged = sc.air_t > 0.35 && !(off < 1.0 && !trick);
            // Failed landing: way off a whole flip, or an air spin put down sideways. No score,
            // combo broken, and the rider is thrown (`ragdoll.rs`). Too big to ragdoll: the old
            // sketchy landing below.
            let flip_fail = judged && off > crate::ragdoll::FLIP_FAIL_RAD;
            let spin_fail = sc.air_t > 0.15 && crate::ragdoll::spin_landing_failed(sc.air_spin);
            let wiped = (flip_fail || spin_fail) && crate::ragdoll::can_ragdoll(&stats);
            if wiped {
                st.combo = 0;
                st.combo_t = 0.0;
                let fwd = Vec3::new(sc.yaw.sin(), 0.0, sc.yaw.cos());
                // Flip short / over-rotated: thrown backwards; sideways spin: pitched forward.
                let spin = if flip_fail { -6.0 } else { 5.0 };
                ragdoll.request(Some(fwd * sc.speed - Vec3::Y * impact * 0.3), Vec3::Y * 2.5, spin);
                sc.air_spin = 0.0;
                play_sfx("hit");
                stats.hint_text = if flip_fail {
                    "BAILED! Let go of the flip before you land!".to_string()
                } else {
                    "BAILED! Landed sideways out of the spin!".to_string()
                };
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
            let judged = judged && !wiped;
            if !judged && !wiped && sc.air_t > 0.15 {
                play_sfx("step");
            }
            if judged {
                if off < 1.0 {
                    st.combo += 1;
                    st.combo_t = 4.0;
                    let base = (sc.air_t * 400.0) as u32 + flips * 1500;
                    let pts = base * st.combo.max(1);
                    stats.score += pts;
                    stats.dash_energy = (stats.dash_energy + 15.0).min(100.0);
                    stats.tendy_credits += 1 + flips;
                    let tw = (1 + flips * 3 + if car_hop { 2 } else { 0 } + (st.combo / 3)).min(15);
                    stats.add_tendy_wendies(tw);
                    let grav_dropped = sc.apply_landed_backflips(flips);
                    if grav_dropped {
                        stats.scooter_gravity_scale = sc.gravity_scale;
                    }
                    play_sfx(if flips > 0 { "powerup" } else { "tendy" });
                    let flip_txt = if flips > 0 {
                        format!("{}x BACKFLIP! ", flips)
                    } else if car_hop {
                        "CAR HOP! ".to_string()
                    } else {
                        String::new()
                    };
                    let grav_txt = if grav_dropped {
                        format!("  GRAV {}%", sc.gravity_pct())
                    } else {
                        String::new()
                    };
                    stats.hint_text = format!(
                        "{}AIR {:.1}s  +{}  +{} TW  COMBO x{}{}",
                        flip_txt, sc.air_t, pts, tw, st.combo, grav_txt
                    );
                    stats.hint_bump = stats.hint_bump.wrapping_add(1);
                } else {
                    st.combo = 0;
                    st.combo_t = 0.0;
                    sc.speed *= 0.45;
                    play_sfx("hit");
                    stats.hint_text = "SKETCHY LANDING! Level out before you touch down!".to_string();
                }
            }
            sc.air_t = 0.0;
            sc.flip = 0.0;
        }
    }

    stf.translation.y = sc.y;

    // --- Boost pads -----------------------------------------------------------------------------
    if sc.y < 1.0 {
        for (ptf, pad) in &pad_q {
            let local = ptf.rotation.inverse() * (stf.translation - ptf.translation);
            if local.x.abs() <= pad.half_w && local.z.abs() <= pad.half_l {
                let boosting = sc.boost_t > 0.0;
                if !boosting || sc.boost_t < 0.35 || (pad.mega && sc.boost_top < pad.top) {
                    play_sfx(if pad.mega { "powerup" } else { "dash" });
                    stats.hint_text = if pad.mega {
                        "MEGA BOOST! Hold it straight and SEND IT!".to_string()
                    } else {
                        "BOOST PAD!".to_string()
                    };
                }
                sc.boost_top = if boosting { sc.boost_top.max(pad.top) } else { pad.top };
                sc.boost_t = sc.boost_t.max(pad.secs);
                sc.speed = sc.speed.max(pad.kick);
            }
        }
    }

    // --- Boost rings ----------------------------------------------------------------------------
    let rider_pt = stf.translation + Vec3::Y * 0.9;
    for (rtf, mut ring) in &mut ring_q {
        if ring.cooldown > 0.0 {
            continue;
        }
        let local = rtf.rotation.inverse() * (rider_pt - rtf.translation);
        let radial = Vec2::new(local.x, local.y).length();
        let reach = 1.4 + sc.speed.abs() * dt * 0.5;
        if local.z.abs() < reach && radial < ring.radius * 0.95 {
            ring.cooldown = 1.2;
            if sc.grounded {
                // Small, capped top-up (never lowers a faster speed or a longer mega boost).
                let top = crate::scooter::BOOST_TOP_SPEED;
                sc.boost_top = if sc.boost_t > 0.0 { sc.boost_top.max(top) } else { top };
                sc.boost_t = (sc.boost_t + RING_BOOST_SECS).min(RING_BOOST_CAP.max(sc.boost_t));
                let bumped = (sc.speed.max(crate::scooter::STREET_TOP_SPEED) + RING_SPEED_BUMP).min(top);
                sc.speed = sc.speed.max(bumped);
            }
            // Air rings only score: the flight and the boost's end at landing stay as laid out.
            score_boost_ring(&mut stats, &mut st, &ring, rtf.translation.y > 3.0);
        }
    }

    // --- Hop squash & stretch (same damped spring as the on-foot hero; deck stays on the ground) -
    {
        let accel = -320.0 * sc.squash - 20.0 * sc.squash_vel;
        sc.squash_vel += accel * dt;
        sc.squash = (sc.squash + sc.squash_vel * dt).clamp(-0.3, 0.3);
        stf.scale = Vec3::new(1.0 + 0.3 * sc.squash, 1.0 - 0.5 * sc.squash, 1.0 + 0.3 * sc.squash);
    }

    // --- Follow transforms (rider rides the deck; flips/pitch apply to both) ------------------
    p_tf.translation = stf.translation + Vec3::Y * 0.25;
    if !sc.grounded || sc.flip.abs() > 0.01 {
        let pitch = (sc.vy * 0.035).clamp(-0.45, 0.45);
        let extra = Quat::from_rotation_x(-(sc.flip + pitch));
        stf.rotation = stf.rotation * extra;
        p_tf.rotation = stf.rotation * Quat::from_rotation_y(PI);
    }
}

// ---------------------------------------------------------------------------------------------
// Ramps are solid for the hero on foot and for cars (same `ramp_probe` the scooter uses)
// ---------------------------------------------------------------------------------------------

/// At or above this size the hero walks straight over the wedge ramps (2.4 m: ankle high on a
/// kaiju). Between 3 and this he steps onto them from any side (no walls), like the skate park.
const RAMP_IGNORE_SCALE: f32 = 8.0;

/// On foot: stand on and walk up the wedge ramps, stopped by the back and side faces below their
/// top. Runs with the skate park's on-foot pass (after `player_movement`, before the camera), so
/// `grounded` and the height it leaves are what the limb animation and camera see.
pub fn update_ramp_walking(
    stats: Res<PlayerStats>,
    ramp_q: Query<(&Transform, &StuntRamp), Without<Player>>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut prev: Local<Option<Vec3>>,
    mut on_ramp: Local<bool>,
) {
    let Ok((mut tf, mut pl)) = player_q.get_single_mut() else {
        return;
    };
    let scale = stats.player_scale.max(0.5);
    if !stats.in_city
        || stats.driving_car.is_some()
        || stats.riding_scooter.is_some()
        || stats.riding_wheel
        || scale >= RAMP_IGNORE_SCALE
    {
        *prev = None;
        *on_ramp = false;
        return;
    }
    let mut p = tf.translation;
    let flat = |a: Vec3, b: Vec3| Vec2::new(a.x - b.x, a.z - b.z).length();
    // A teleport (respawn, car exit, debug drop) is not a move through the ramp.
    let pv = prev.filter(|q| flat(*q, p) < 3.0);
    let step = if scale > 3.0 { 0.6 * scale } else { 0.45 };
    let wall_r = (0.28 * scale).clamp(0.45, 2.8);

    // Back and side faces: push out through the face the hero came through.
    for (rtf, ramp) in &ramp_q {
        let Some(pr) = ramp_probe(rtf, ramp, p, wall_r) else {
            continue;
        };
        if pr.h <= p.y + step || (pr.on_deck && pv.is_none()) {
            continue;
        }
        let prev_local = pv.map(|q| rtf.rotation.inverse() * (q - rtf.translation));
        let (out, n) = ramp_exit(rtf, ramp, pr.local, prev_local, wall_r);
        p = rtf.translation + rtf.rotation * out;
        let n2 = Vec2::new(n.x, n.z);
        let v = Vec2::new(pl.velocity.x, pl.velocity.z);
        let v = v - n2 * v.dot(n2).min(0.0);
        pl.velocity.x = v.x;
        pl.velocity.z = v.y;
        let k = Vec2::new(pl.knockback.x, pl.knockback.z);
        let k = k - n2 * k.dot(n2).min(0.0);
        pl.knockback.x = k.x;
        pl.knockback.z = k.y;
    }

    // Deck: the highest one reachable from here (or any, popping out after a teleport inside).
    let mut best: Option<f32> = None;
    for (rtf, ramp) in &ramp_q {
        if let Some(pr) = ramp_probe(rtf, ramp, p, 0.0) {
            if pr.on_deck && (pr.h <= p.y + step || pv.is_none()) && best.is_none_or(|b| pr.h > b) {
                best = Some(pr.h);
            }
        }
    }
    let mut now_on = false;
    if let Some(h) = best {
        let moved = pv.map_or(0.0, |q| flat(q, p));
        let stick = if *on_ramp { 0.25 + 1.2 * moved } else { 0.02 };
        if p.y < h - 0.001 || (pl.velocity.y <= 0.0 && p.y <= h + stick) {
            if !*on_ramp {
                // Same landing squash `player_movement` gives a platform landing.
                let impact = (-pl.velocity.y).max(0.0) / (9.0 + scale * 1.2);
                if impact > 0.3 {
                    pl.squash_vel += 6.5 * impact.min(1.3);
                }
            }
            p.y = h;
            pl.velocity.y = pl.velocity.y.max(0.0);
            pl.grounded = true;
            pl.jumps_used = 0;
            now_on = true;
        }
    }
    *on_ramp = now_on;
    tf.translation = p;
    *prev = Some(p);
}

/// Car wheel contact points (local x, z), as built in `city.rs`.
const CAR_WHEELS: [(f32, f32); 4] = [(-1.05, 1.32), (1.05, 1.32), (-1.05, -1.32), (1.05, -1.32)];
/// Body corners (bumpers) for the wall test.
const CAR_CORNERS: [(f32, f32); 4] = [(-1.075, 2.33), (1.075, 2.33), (-1.075, -2.33), (1.075, -2.33)];
/// A deck this far above a wheel/corner is a wall, not something to climb.
const CAR_RAMP_STEP: f32 = 0.95;
const CAR_RAMP_GRAVITY: f32 = 24.0;

/// Cars on the wedge ramps. Every car rides the same height field: wheels follow the deck (body
/// pitches and rolls to match), a car rolling off the lip launches with its climbing speed and
/// comes down under gravity. The driven car and parked/knocked cars are stopped by the back and
/// side faces (sliding along them); lane traffic never meets a ramp (ramps sit off the lanes) but
/// if shoved into one it rides over it rather than clipping through. Runs after every system that
/// moves cars and before the camera; cars far from any ramp with a flat ride state are untouched.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_ramp_cars(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut st: ResMut<StuntState>,
    ramp_q: Query<(&Transform, &StuntRamp), (Without<CityCar>, Without<Player>, Without<BoostPad>, Without<BoostRing>)>,
    pad_q: Query<(&Transform, &BoostPad), (Without<CityCar>, Without<StuntRamp>, Without<Player>, Without<BoostRing>)>,
    mut ring_q: Query<(&Transform, &mut BoostRing), (Without<CityCar>, Without<StuntRamp>, Without<Player>, Without<BoostPad>)>,
    mut car_q: Query<(Entity, &mut Transform, &mut CityCar), (Without<StuntRamp>, Without<Player>, Without<BoostPad>, Without<BoostRing>)>,
    mut player_q: Query<&mut Transform, (With<Player>, Without<CityCar>, Without<StuntRamp>, Without<BoostPad>, Without<BoostRing>)>,
) {
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    let driven_in_park = stats
        .driving_car
        .and_then(|ce| car_q.get(ce).ok())
        .is_some_and(|(_, _, c)| c.park);
    let touch_jump = if stats.driving_car.is_some() && !driven_in_park {
        crate::audio::take_touch_jump()
    } else {
        false
    };
    let jump_pressed = keys.just_pressed(KeyCode::Space) || touch_jump;
    let space_down = keys.pressed(KeyCode::Space) || crate::audio::touch_jump_held();
    let ramps: Vec<(Transform, StuntRamp)> = ramp_q.iter().map(|(t, r)| (*t, *r)).collect();
    for (e, mut tf, mut car) in &mut car_q {
        if car.park {
            continue;
        }
        let driven = stats.driving_car == Some(e);
        let traffic = !car.occupied && !car.parked && !driven;
        let mut pos = tf.translation;
        let near = ramps.iter().any(|(rtf, r)| {
            Vec2::new(pos.x - rtf.translation.x, pos.z - rtf.translation.z).length() < r.half_l + r.half_w + 3.0
        });
        let mut ride = car.ride;
        if near || ride.y != 0.0 || ride.air || ride.pitch != 0.0 || ride.roll != 0.0 || ride.flip != 0.0 || ride.air_spin != 0.0 {
            let yaw_q = Quat::from_rotation_y(car.yaw);
            let est_y = |r: &CarRide, lx: f32, lz: f32| r.y + lz * r.pitch.tan() + lx * r.roll.tan();

            // Back and side faces (not for lane traffic: pushing it off its lane would pin it there).
            if !traffic && near {
                for &(lx, lz) in CAR_CORNERS.iter().chain(CAR_WHEELS.iter()) {
                    let w = pos + yaw_q * Vec3::new(lx, 0.0, lz);
                    for (rtf, r) in &ramps {
                        let Some(pr) = ramp_probe(rtf, r, w, 0.0) else {
                            continue;
                        };
                        if !pr.on_deck || pr.h <= est_y(&ride, lx, lz) + CAR_RAMP_STEP || pr.h <= ride.y + 0.9 {
                            continue;
                        }
                        let centre = rtf.rotation.inverse() * (pos - rtf.translation);
                        let (out, n) = ramp_exit(rtf, r, pr.local, Some(centre), 0.02);
                        let push = rtf.rotation * (out - pr.local);
                        pos.x += push.x;
                        pos.z += push.z;
                        let n2 = Vec2::new(n.x, n.z);
                        let into = car.velocity.dot(n2).min(0.0);
                        if driven && into < -9.5 && car.horn_cooldown <= 0.0 {
                            play_sfx("car_crash");
                            car.horn_cooldown = 0.35;
                        }
                        car.velocity -= n2 * into * 1.25;
                        let kin = car.knock_vel.dot(n2).min(0.0);
                        car.knock_vel -= n2 * kin;
                    }
                }
            }

            // Height under each wheel.
            let mut hs = [0.0_f32; 4];
            let mut max_slope = 0.0_f32;
            for (i, &(lx, lz)) in CAR_WHEELS.iter().enumerate() {
                let w = pos + yaw_q * Vec3::new(lx, 0.0, lz);
                for (rtf, r) in &ramps {
                    if let Some(pr) = ramp_probe(rtf, r, w, 0.0) {
                        let reachable = traffic || ride.air || pr.h <= est_y(&ride, lx, lz) + CAR_RAMP_STEP + 0.05;
                        if pr.on_deck && reachable && pr.h > hs[i] {
                            hs[i] = pr.h;
                            max_slope = max_slope.max(pr.slope);
                        }
                    }
                }
            }
            let ground = (hs[0] + hs[1] + hs[2] + hs[3]) * 0.25;
            let g_pitch = (((hs[0] + hs[1]) - (hs[2] + hs[3])) * 0.5 / 2.64).atan();
            let g_roll = (((hs[1] + hs[3]) - (hs[0] + hs[2])) * 0.5 / 2.1).atan();

            if !ride.air {
                let speed_h = car.velocity.length();
                if driven && ground > 0.10 && speed_h > 5.0 && jump_pressed {
                    // Explicit ramp hop with [SPACE]
                    ride.air = true;
                    ride.air_t = 0.0;
                    ride.flip = 0.0;
                    ride.air_spin = 0.0;
                    ride.hopped = true;
                    ride.vy = ramp_launch_vy(speed_h, ride.last_slope.max(0.26)) + 4.5;
                    play_sfx("jump");
                    stats.hint_text = "CAR AIRBORNE! Hold [SPACE] to BACKFLIP, [A/D] to SPIN!".to_string();
                    stats.hint_bump = stats.hint_bump.wrapping_add(1);
                } else {
                    let allowance = 0.25 + (-ride.vy).max(0.0) * dt * 1.5;
                    if ride.y - ground > allowance {
                        // Rolled off the lip (or a side): launch with the speed it was climbing at.
                        ride.air = true;
                        ride.air_t = 0.0;
                        ride.flip = 0.0;
                        ride.air_spin = 0.0;
                        ride.hopped = false;
                        if driven {
                            ride.vy = ramp_launch_vy(speed_h, ride.last_slope.max(0.26))
                                .max(ride.vy.max(0.0) * 1.45)
                                .min(RAMP_LAUNCH_CAP);
                            play_sfx("dash");
                            stats.hint_text = "CAR AIRBORNE! Hold [SPACE] to BACKFLIP, [A/D] to SPIN!".to_string();
                            stats.hint_bump = stats.hint_bump.wrapping_add(1);
                        } else {
                            ride.vy = (ride.vy.max(0.0) * 1.1).min(14.0);
                        }
                    } else if traffic && ground - ride.y > CAR_RAMP_STEP {
                        // Shoved into a ramp: climb onto it quickly rather than pop.
                        ride.y += (ground - ride.y).min(10.0 * dt);
                        ride.vy = 0.0;
                    } else {
                        let meas = ((ground - ride.y) / dt).clamp(-15.0, 15.0);
                        ride.vy += (meas - ride.vy) * 0.5;
                        ride.y = ground;
                        if ground > 0.05 && max_slope > 0.0 {
                            ride.last_slope = max_slope;
                        }
                    }
                }
            }
            if ride.air {
                let g_scale = if driven { car.gravity_scale } else { 1.0 };
                ride.vy -= CAR_RAMP_GRAVITY * g_scale * dt;
                ride.y += ride.vy * dt;
                ride.air_t += dt;
                if driven {
                    if space_down && ride.air_t > 0.12 {
                        ride.flip += 7.2 * dt;
                    } else {
                        let target = (ride.flip / TAU).round() * TAU;
                        ride.flip += (target - ride.flip) * (dt * 9.0).min(1.0);
                    }
                }
                if ride.y <= ground && ride.vy <= 0.0 {
                    let impact = -ride.vy;
                    ride.y = ground;
                    ride.vy = 0.0;
                    ride.air = false;
                    if !traffic {
                        car.pitch_lean = (car.pitch_lean + 0.02 * impact.min(12.0)).min(0.2);
                    }
                    if driven {
                        let nearest = (ride.flip / TAU).round();
                        let off = (ride.flip - nearest * TAU).abs();
                        let flips = nearest.max(0.0) as u32;
                        let spins = ((ride.air_spin.abs() + 0.6) / TAU).floor() as u32;
                        let trick = flips > 0 || spins > 0 || !ride.hopped || ride.air_t > 0.9;
                        let judged = ride.air_t > 0.35 && (trick || off >= 1.0);
                        if judged && off < 1.0 {
                            let grav_dropped = car.apply_landed_backflips(flips);
                            if grav_dropped {
                                stats.car_gravity_scale = car.gravity_scale;
                            }
                            st.combo += 1;
                            st.combo_t = 4.0;
                            let base = (ride.air_t * 400.0) as u32 + flips * 1500 + spins * 800;
                            let pts = base * st.combo.max(1);
                            stats.score += pts;
                            stats.dash_energy = (stats.dash_energy + 15.0).min(100.0);
                            stats.tendy_credits += 1 + flips + spins;
                            let tw = (1 + flips * 3 + spins * 2 + (st.combo / 3)).min(15);
                            stats.add_tendy_wendies(tw);
                            play_sfx(if flips > 0 || spins > 0 { "powerup" } else { "tendy" });
                            let mut labels: Vec<String> = Vec::new();
                            if spins > 0 {
                                labels.push(format!("{} SPIN!", spins * 360));
                            }
                            if flips > 0 {
                                labels.push(format!("{}x CAR BACKFLIP!", flips));
                            }
                            let label_txt = if labels.is_empty() {
                                String::new()
                            } else {
                                format!("{} ", labels.join(" "))
                            };
                            let grav_txt = if grav_dropped {
                                format!("  GRAV {}%", car.gravity_pct())
                            } else {
                                String::new()
                            };
                            stats.hint_text = format!(
                                "{}AIR {:.1}s  +{}  +{} TW  COMBO x{}{}",
                                label_txt, ride.air_t, pts, tw, st.combo, grav_txt
                            );
                            stats.hint_bump = stats.hint_bump.wrapping_add(1);
                        } else if judged && off >= 1.0 {
                            st.combo = 0;
                            st.combo_t = 0.0;
                            car.velocity *= 0.45;
                            play_sfx("car_crash");
                            stats.hint_text =
                                "SKETCHY CAR LANDING! Level out before you touch down!".to_string();
                            stats.hint_bump = stats.hint_bump.wrapping_add(1);
                        } else if impact > 4.0 {
                            play_sfx(if impact > 10.0 { "car_crash" } else { "step" });
                        }
                        if impact > 4.0 {
                            stats.shake_request = stats.shake_request.max(0.05 + 0.012 * impact.min(16.0));
                        }
                    }
                    ride.air_t = 0.0;
                    ride.flip = 0.0;
                    ride.air_spin = 0.0;
                    ride.hopped = false;
                }
            }
            let (tp, tr, rate) = if ride.air {
                ((ride.vy * 0.025).clamp(-0.35, 0.35), 0.0, 5.0)
            } else {
                (g_pitch, g_roll, 14.0)
            };
            let k = (dt * rate).min(1.0);
            ride.pitch += (tp - ride.pitch) * k;
            ride.roll += (tr - ride.roll) * k;
            if !ride.air
                && ground == 0.0
                && ride.y.abs() < 1e-3
                && ride.pitch.abs() < 1e-3
                && ride.roll.abs() < 1e-3
                && ride.flip.abs() < 1e-3
                && ride.air_spin.abs() < 1e-3
            {
                let (bt, btop) = (ride.boost_t, ride.boost_top);
                ride = CarRide::default();
                ride.boost_t = bt;
                ride.boost_top = btop;
            }
            car.ride = ride;

            tf.translation = Vec3::new(pos.x, ride.y, pos.z);
            let (lean_x, lean_z) = if traffic { (0.0, 0.0) } else { (car.pitch_lean, car.roll_lean) };
            tf.rotation = Quat::from_rotation_y(car.yaw + ride.air_spin)
                * Quat::from_rotation_x(lean_x - ride.pitch - ride.flip)
                * Quat::from_rotation_z(lean_z + ride.roll);
            if driven {
                if let Ok(mut p_tf) = player_q.get_single_mut() {
                    p_tf.translation = tf.translation;
                    p_tf.rotation = Quat::from_rotation_y(car.yaw + ride.air_spin);
                }
            }
        }

        if !driven {
            continue;
        }

        // --- Street boost pads for driven cars ---
        if car.ride.y < 1.2 {
            let car_pos = Vec3::new(pos.x, car.ride.y, pos.z);
            for (ptf, pad) in &pad_q {
                let local = ptf.rotation.inverse() * (car_pos - ptf.translation);
                if local.x.abs() <= pad.half_w + 0.8 && local.z.abs() <= pad.half_l + 0.8 {
                    let boosting = car.ride.boost_t > 0.0;
                    if !boosting || car.ride.boost_t < 0.35 || (pad.mega && car.ride.boost_top < pad.top) {
                        play_sfx(if pad.mega { "powerup" } else { "dash" });
                        stats.hint_text = if pad.mega {
                            "MEGA BOOST! Hold it straight and SEND IT!".to_string()
                        } else {
                            "BOOST PAD!".to_string()
                        };
                        stats.hint_bump = stats.hint_bump.wrapping_add(1);
                    }
                    let top = if pad.mega { pad.top } else { pad.top.max(44.0) };
                    car.ride.boost_top = if boosting { car.ride.boost_top.max(top) } else { top };
                    car.ride.boost_t = car.ride.boost_t.max(pad.secs);
                    let fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
                    let kick_spd = if pad.mega { pad.kick } else { pad.kick.max(36.0) };
                    if car.velocity.length() < kick_spd {
                        car.velocity = fwd * kick_spd;
                    }
                }
            }
        }

        // --- Street & air boost rings for driven cars ---
        let car_center = Vec3::new(pos.x, car.ride.y + 0.9, pos.z);
        let spd_h = car.velocity.length();
        for (rtf, mut ring) in &mut ring_q {
            if ring.cooldown > 0.0 {
                continue;
            }
            let local = rtf.rotation.inverse() * (car_center - rtf.translation);
            let radial = Vec2::new(local.x, local.y).length();
            let reach = 1.8 + spd_h * dt * 0.5;
            if local.z.abs() < reach && radial < ring.radius * 1.05 {
                ring.cooldown = 1.2;
                if !car.ride.air {
                    let top = 44.0_f32;
                    car.ride.boost_top = if car.ride.boost_t > 0.0 { car.ride.boost_top.max(top) } else { top };
                    car.ride.boost_t = (car.ride.boost_t + RING_BOOST_SECS).min(RING_BOOST_CAP.max(car.ride.boost_t));
                    let bumped = (spd_h.max(36.0) + RING_SPEED_BUMP).min(top);
                    if spd_h < bumped {
                        let dir = car.velocity.normalize_or_zero();
                        let fwd = if dir.length_squared() > 0.1 { dir } else { Vec2::new(car.yaw.sin(), car.yaw.cos()) };
                        car.velocity = fwd * bumped;
                    }
                }
                score_boost_ring(&mut stats, &mut st, &ring, rtf.translation.y > 3.0);
            }
        }
    }
}

/// `?debug=1&city_start=ramp|rampback|rampside|rampcar`: on foot in front of the 3rd St ramp's
/// slope, behind its back wall, beside it, or (rampcar) in a car on 6th St (z = -46) facing the
/// Cross-Street Send ramp (`debug_ramp_car` seats Stan in it). (hero position, facing)
pub fn debug_start_pose(mode: &str) -> Option<(Vec3, Vec3)> {
    let fwd = |spec: &RunSpec| Vec3::new(spec.yaw.sin(), 0.0, spec.yaw.cos());
    let (r0, r1) = (&RUNS[0], &RUNS[1]);
    let ramp0 = run_pos(r0, ramp_s(r0), 0.0);
    let ramp1 = run_pos(r1, ramp_s(r1), 0.0);
    let f0 = fwd(r0);
    // Left of the travel direction (the old northbound layout used -X here).
    let left0 = Vec3::new(f0.z, 0.0, -f0.x);
    match mode {
        "ramp" => Some((ramp0 - f0 * 9.0, f0)),
        "rampback" => Some((ramp0 + f0 * 9.0, -f0)),
        "rampside" => Some((ramp0 + left0 * 6.0 + f0 * 2.0, -left0)),
        "rampcar" => Some((ramp1 - fwd(r1) * 20.0, fwd(r1))),
        _ => None,
    }
}

/// Debug only (`city_start=rampcar`): once, take a traffic car, put it at the start pose facing
/// the ramp and seat Stan in it (license granted so the drive is allowed).
#[allow(clippy::type_complexity)]
pub fn debug_ramp_car(
    mut done: Local<bool>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut car_q: Query<(Entity, &mut Transform, &mut CityCar), Without<Player>>,
    mut player_q: Query<(&mut Transform, &mut Visibility), With<Player>>,
) {
    if *done {
        return;
    }
    let mode = crate::audio::debug_city_start();
    if mode != "rampcar" {
        *done = true;
        return;
    }
    let Some((pos, face)) = debug_start_pose(&mode) else {
        return;
    };
    let Ok((mut p_tf, mut vis)) = player_q.get_single_mut() else {
        return;
    };
    let Some((e, mut tf, mut car)) = car_q.iter_mut().find(|(_, _, c)| !c.parked) else {
        return;
    };
    let yaw = face.x.atan2(face.z);
    car.yaw = yaw;
    car.occupied = true;
    car.parked = false;
    car.velocity = Vec2::ZERO;
    car.knock_vel = Vec2::ZERO;
    tf.translation = pos;
    tf.rotation = Quat::from_rotation_y(yaw);
    p_tf.translation = pos;
    *vis = Visibility::Hidden;
    stats.driving_car = Some(e);
    stats.has_license = true;
    orbit.yaw = yaw - PI;
    *done = true;
}
