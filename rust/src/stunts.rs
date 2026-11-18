use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::audio::play_sfx;
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::scooter::CityScooter;
use crate::state::{LevelEntity, PlayerStats};

// ---------------------------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------------------------

/// Wedge launch ramp. Local +Z is the "uphill" direction, origin is the ground centre of the wedge.
#[derive(Component)]
pub struct StuntRamp {
    pub half_w: f32,
    pub half_l: f32,
    pub height: f32,
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
}

#[derive(Component)]
pub struct ScooterBoostFlame;

// Each of the 3 Cedar Rapids stunt circuits has 6 sequential boost rings (`bit` 0..5).
// `StuntState.collected` stores a bitmask per circuit so hitting all 6 awards a circuit bonus.
pub const CIRCUIT_RING_COUNTS: [u32; 3] = [6, 6, 6];
pub const CIRCUIT_NAMES: [&str; 3] = [
    "3RD ST SE GAUNTLET",
    "CROSS-STREET SEND",
    "GREENE SQUARE PARK",
];

#[derive(Resource, Default)]
pub struct StuntState {
    pub combo: u32,
    pub combo_t: f32,
    pub rings_total: u32,
    pub rings_since_heart: u32,
    pub collected: [u32; 3],
    pub circuits_done: u32,
}

// ---------------------------------------------------------------------------------------------
// Course layout (shared by the spawner and the tendy-trail helper)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct RunSpec {
    start: Vec3,
    yaw: f32,
    k: f32,
    circuit: usize,
    gate: bool,
}

const RUNS: [RunSpec; 2] = [
    // Circuit 0: 3rd St SE Gauntlet, northbound from the Pub 217 block toward Empire Citadel
    RunSpec {
        start: Vec3::new(0.0, 0.0, 24.0),
        yaw: PI,
        k: 0.85,
        circuit: 0,
        gate: true,
    },
    // Circuit 1: Cross-Street Send, eastbound along the wide street south of the pub
    RunSpec {
        start: Vec3::new(12.0, 0.0, 46.0),
        yaw: FRAC_PI_2,
        k: 1.0,
        circuit: 1,
        gate: true,
    },
];

const PARK_CENTER: Vec3 = Vec3::new(69.0, 0.0, 69.0);
const PARK_RADIUS: f32 = 9.0;

fn run_pos(spec: &RunSpec, s: f32, y: f32) -> Vec3 {
    let d = s * spec.k;
    Vec3::new(
        spec.start.x + spec.yaw.sin() * d,
        y,
        spec.start.z + spec.yaw.cos() * d,
    )
}

fn park_point(deg: f32) -> (Vec3, f32) {
    let th = deg.to_radians();
    let pos = Vec3::new(
        PARK_CENTER.x + th.cos() * PARK_RADIUS,
        0.0,
        PARK_CENTER.z + th.sin() * PARK_RADIUS,
    );
    // Travel direction = increasing angle
    let yaw = (-th.sin()).atan2(th.cos());
    (pos, yaw)
}

/// Floating Tendy trails that follow the ground path and the jump arcs of every circuit.
pub fn stunt_tendy_spots() -> Vec<Vec3> {
    let mut out = Vec::new();
    for spec in RUNS.iter() {
        for (s, y) in [
            (5.0_f32, 1.0_f32),
            (9.0, 1.0),
            (18.0, 1.0),
            (27.0, 1.0),
            (45.0, 3.6),
            (51.0, 4.4),
            (62.0, 4.6),
            (74.0, 3.4),
            (84.0, 1.0),
            (94.0, 1.0),
        ] {
            out.push(run_pos(spec, s, y));
        }
    }
    for deg in [5.0_f32, 45.0, 95.0, 105.0, 165.0, 225.0, 275.0, 285.0, 345.0] {
        let (p, _) = park_point(deg);
        let y = if (40.0..60.0).contains(&deg) || (220.0..235.0).contains(&deg) {
            3.2
        } else {
            1.0
        };
        out.push(Vec3::new(p.x, y, p.z));
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
    half_w: f32,
    half_l: f32,
    height: f32,
) {
    let angle = (height / (2.0 * half_l)).atan();
    let slope_len = ((2.0 * half_l).powi(2) + height.powi(2)).sqrt();
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let wedge = meshes.add(wedge_mesh(half_w, half_l, height));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            StuntRamp {
                half_w,
                half_l,
                height,
            },
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
                let mat = if i % 2 == 0 {
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
            for sx in [-half_w, half_w] {
                r.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mats.stripe_cyan.clone()),
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
    half_w: f32,
    half_l: f32,
) {
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            BoostPad { half_w, half_l },
            Transform::from_translation(Vec3::new(pos.x, 0.03, pos.z))
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(mats.pad_base.clone()),
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
                        MeshMaterial3d(mats.pad_chevron.clone()),
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
    if spec.gate {
        // Gate is rotated so its +Z lettering faces the approaching rider
        spawn_gate(commands, meshes, mats, run_pos(spec, -6.0, 0.0), yaw + PI);
    }
    spawn_pad(commands, meshes, mats, run_pos(spec, 0.0, 0.0), yaw, 2.0, 3.0);
    spawn_ring(commands, meshes, mats, run_pos(spec, 14.0, 2.3), yaw, 2.6, spec.circuit, 0, false);
    spawn_ring(commands, meshes, mats, run_pos(spec, 22.0, 2.3), yaw, 2.6, spec.circuit, 1, false);
    spawn_ramp(commands, meshes, mats, run_pos(spec, 34.0, 0.0), yaw, 2.4, 4.5, 2.4);
    spawn_ring(commands, meshes, mats, run_pos(spec, 56.0, 4.2), yaw, 3.6, spec.circuit, 2, true);
    spawn_ring(commands, meshes, mats, run_pos(spec, 68.0, 4.4), yaw, 3.6, spec.circuit, 3, true);
    spawn_pad(commands, meshes, mats, run_pos(spec, 80.0, 0.0), yaw, 2.0, 3.0);
    spawn_ring(commands, meshes, mats, run_pos(spec, 88.0, 2.3), yaw, 2.6, spec.circuit, 4, false);
    spawn_ring(commands, meshes, mats, run_pos(spec, 100.0, 2.6), yaw, 3.0, spec.circuit, 5, true);
}

fn spawn_park_circuit(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &StuntMats) {
    // Kickers + pads feeding the circular loop
    for deg in [20.0_f32, 200.0] {
        let (p, yaw) = park_point(deg);
        spawn_ramp(commands, meshes, mats, p, yaw, 2.4, 4.0, 2.0);
    }
    for deg in [352.0_f32, 172.0] {
        let (p, yaw) = park_point(deg);
        spawn_pad(commands, meshes, mats, p, yaw, 1.8, 2.5);
    }
    // (degrees, height, radius, bit, gold)
    for (deg, y, radius, bit, gold) in [
        (120.0_f32, 2.3_f32, 2.6_f32, 0_u32, false),
        (150.0, 2.3, 2.6, 1, false),
        (70.0, 4.0, 3.3, 2, true),
        (300.0, 2.3, 2.6, 3, false),
        (330.0, 2.3, 2.6, 4, false),
        (250.0, 4.0, 3.3, 5, true),
    ] {
        let (p, yaw) = park_point(deg);
        spawn_ring(commands, meshes, mats, Vec3::new(p.x, y, p.z), yaw, radius, 2, bit, gold);
    }
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
    spawn_park_circuit(commands, meshes, &mats);
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

/// Hop take-off speed (m/s) under the stunt gravity of 28 m/s^2. Grows gently with Stan like the
/// on-foot jump: 7.8 at 0.55 (about 1.1 m), 8.4 at the city's 1.8 (about 1.3 m, 0.6 s airtime),
/// capped at 10.5 (about 2 m) near the 6.0 riding limit.
fn scooter_hop_speed(player_scale: f32) -> f32 {
    (7.8 + (player_scale - 0.55).max(0.0) * 0.5).min(10.5)
}

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
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    // --- Boost flames on every scooter -------------------------------------------------------
    let boost_levels: Vec<(Entity, f32)> = scooter_q
        .iter()
        .map(|(e, _, sc)| (e, sc.boost_t))
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
            ftf.scale = Vec3::new(1.0, flick, 1.0);
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

    // --- Terrain height beneath the scooter (ramps) ------------------------------------------
    let mut ground_h = 0.0_f32;
    let mut slope = 0.0_f32;
    for (rtf, ramp) in &ramp_q {
        let local = rtf.rotation.inverse() * (stf.translation - rtf.translation);
        if local.x.abs() <= ramp.half_w && local.z.abs() <= ramp.half_l {
            let h = ramp.height * (local.z + ramp.half_l) / (2.0 * ramp.half_l);
            if h > sc.y + 0.9 {
                // Ran into the back wall or side of a ramp: bump back out
                let to_back = ramp.half_l - local.z;
                let to_side = ramp.half_w - local.x.abs();
                let mut out = local;
                if to_back < to_side {
                    out.z = ramp.half_l + 0.25;
                } else {
                    out.x = local.x.signum() * (ramp.half_w + 0.25);
                }
                stf.translation = rtf.translation + rtf.rotation * out;
                sc.speed *= 0.4;
                continue;
            }
            if h > ground_h {
                ground_h = h;
                slope = ramp.height / (2.0 * ramp.half_l);
            }
        }
    }

    let prev_ground = sc.ground_h;
    sc.ground_h = ground_h;

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
            let on_ramp = ground_h > 0.1 && sc.speed > 5.0;
            sc.vy = if on_ramp {
                (sc.speed * slope * 1.1).clamp(3.5, 14.5) + hop_v * LIP_HOP_SHARE
            } else {
                hop_v
            };
            sc.grounded = false;
            sc.y = sc.y.max(ground_h);
            sc.air_t = 0.0;
            sc.flip = 0.0;
            sc.hopped = true;
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
        if prev_ground - ground_h > 0.35 && sc.speed > 5.0 {
            // Rolled off the lip of a ramp: LAUNCH!
            sc.grounded = false;
            sc.vy = (sc.speed * sc.last_slope * 1.1).clamp(3.5, 14.5);
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
        sc.vy -= 28.0 * dt;
        sc.y += sc.vy * dt;
        sc.air_t += dt;

        let flipping = space_down && sc.air_t > 0.12 && !sc.hop_hold;
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
            let trick = flips > 0 || !sc.hopped || sc.air_t > 0.9;
            sc.hopped = false;
            sc.hop_hold = false;
            let judged = sc.air_t > 0.35 && !(off < 1.0 && !trick);
            if !judged && sc.air_t > 0.15 {
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
                    play_sfx(if flips > 0 { "powerup" } else { "tendy" });
                    let flip_txt = if flips > 0 {
                        format!("{}x BACKFLIP! ", flips)
                    } else {
                        String::new()
                    };
                    stats.hint_text = format!(
                        "{}AIR {:.1}s  +{}  COMBO x{}",
                        flip_txt, sc.air_t, pts, st.combo
                    );
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
                if sc.boost_t < 0.35 {
                    play_sfx("dash");
                    stats.hint_text = "BOOST PAD! HOLD ON!".to_string();
                }
                sc.boost_t = sc.boost_t.max(1.6);
                sc.speed = sc.speed.max(34.0);
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
            sc.boost_t = (sc.boost_t + 1.8).min(3.2);
            sc.speed = (sc.speed.max(30.0) + 8.0).min(56.0);
            st.combo += 1;
            st.combo_t = 4.0;
            st.rings_total += 1;
            st.rings_since_heart += 1;
            let high = rtf.translation.y > 3.0;
            let pts = 250 * st.combo * if high { 2 } else { 1 };
            stats.score += pts;
            stats.tendy_credits += if high { 2 } else { 1 };
            stats.dash_energy = (stats.dash_energy + 25.0).min(100.0);
            play_sfx("powerup");
            stats.hint_text = format!(
                "{} +{}  COMBO x{}",
                if high { "AIR RING!" } else { "BOOST RING!" },
                pts,
                st.combo
            );
            if st.rings_since_heart >= 5 {
                st.rings_since_heart = 0;
                if stats.hearts < stats.max_hearts {
                    stats.hearts += 1;
                    stats.hint_text.push_str("  |  +1 HEART!");
                }
            }
            let c = ring.circuit.min(2);
            st.collected[c] |= 1 << ring.bit;
            if st.collected[c].count_ones() >= CIRCUIT_RING_COUNTS[c] {
                st.collected[c] = 0;
                st.circuits_done += 1;
                stats.score += 5000;
                stats.tendy_credits += 10;
                stats.hearts = (stats.hearts + 3).min(stats.max_hearts);
                stats.dash_energy = 100.0;
                play_sfx("win");
                stats.hint_text = format!(
                    "CIRCUIT COMPLETE: {}!  +5000  +3 HEARTS!  ({} done)",
                    CIRCUIT_NAMES[c], st.circuits_done
                );
            }
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
