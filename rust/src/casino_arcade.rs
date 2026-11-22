//! *The Coin Game*-style 3D physics arcade wing inside Tendy Palace Casino (`GameState::CasinoPlay`):
//! - **Tendy Coin Pusher** (`x = -9.2, z = 5.5`): motorized shelf pushing 70+ physical 3D coins & bonus bars.
//! - **Tendy Claw Machine** (`x = +9.2, z = 5.5`): 3D gantry + descending 3-prong claw grabbing plushies & tendies.
//! - **Plinko Tower** (`x = -9.2, z = 11.2`): 7-row staggered pegboard with bouncing 3D Plinko discs.
//! - **Skeeball Alley** (`x = +9.2, z = 11.2`): wooden lane, launch ramp, and concentric glowing scoring rings.
//!
//! All four machines run in pure third-person with zero modal popup windows in front, use persistent
//! Tendy Wendies (`TW`), and coexist alongside Chillary Clinton Blackjack and Buffalo/Lucky Ladies Slots.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

use crate::audio::play_sfx;
use crate::casino::{CASINO_LEVEL_ORIGIN, CasinoLevelEntity};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::ui::HelpPause;

pub struct CasinoArcadePlugin;

impl Plugin for CasinoArcadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArcadeState>()
            .add_systems(OnEnter(GameState::CasinoPlay), spawn_casino_arcade_machines)
            .add_systems(OnExit(GameState::CasinoPlay), exit_casino_arcade)
            .add_systems(
                Update,
                (
                    update_arcade_proximity.run_if(sim_running),
                    handle_arcade_input.run_if(sim_running),
                    step_coin_pusher_physics.run_if(sim_running),
                    step_claw_machine.run_if(sim_running),
                    step_plinko_physics.run_if(sim_running),
                    step_skeeball_physics.run_if(sim_running),
                    update_arcade_camera.run_if(sim_running),
                    refresh_arcade_hud,
                )
                    .chain()
                    .run_if(in_state(GameState::CasinoPlay)),
            );
    }
}

fn sim_running(help: Res<HelpPause>) -> bool {
    !help.open
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArcadeKind {
    CoinPusher,
    ClawMachine,
    Plinko,
    Skeeball,
}

impl ArcadeKind {
    fn prompt(self) -> &'static str {
        match self {
            Self::CoinPusher => "TENDY COIN PUSHER (1 TW) [F/E]",
            Self::ClawMachine => "TENDY CLAW MACHINE (5 TW) [F/E]",
            Self::Plinko => "PLINKO TOWER (5 TW) [F/E]",
            Self::Skeeball => "SKEEBALL ALLEY (5 TW / 3 BALLS) [F/E]",
        }
    }

    fn pose(self) -> (Vec3, f32) {
        match self {
            Self::CoinPusher => (Vec3::new(-9.2, 0.0, 5.5), FRAC_PI_2),
            Self::ClawMachine => (Vec3::new(9.2, 0.0, 5.5), -FRAC_PI_2),
            Self::Plinko => (Vec3::new(-9.2, 0.0, 11.2), FRAC_PI_2),
            Self::Skeeball => (Vec3::new(9.2, 0.0, 11.2), -FRAC_PI_2),
        }
    }

    fn ring_offset(self) -> Vec3 {
        match self {
            Self::Skeeball => Vec3::new(0.0, 0.16, 3.6),
            _ => Vec3::new(0.0, 0.16, 2.1),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClawPhase {
    Aiming,
    Descending,
    Grabbing,
    Ascending,
    Returning,
    Releasing,
}

#[derive(Resource)]
struct ArcadeState {
    active: Option<ArcadeKind>,
    was_open: bool,
    status_msg: String,
    rng: u64,
    // Coin Pusher
    pusher_phase: f32,
    pusher_aim_x: f32,
    pusher_cooldown: f32,
    // Claw Machine
    claw_phase: ClawPhase,
    claw_timer: f32,
    claw_pos: Vec2,
    claw_y: f32,
    claw_grabbed: Option<Entity>,
    // Plinko
    plinko_aim_x: f32,
    plinko_cooldown: f32,
    // Skeeball
    skee_aim_x: f32,
    skee_power_phase: f32,
    skee_balls_left: u8,
    skee_cooldown: f32,
}

impl Default for ArcadeState {
    fn default() -> Self {
        Self {
            active: None,
            was_open: false,
            status_msg: String::new(),
            rng: 0x5EED_CAFE_BABE_1234,
            pusher_phase: 0.0,
            pusher_aim_x: 0.0,
            pusher_cooldown: 0.0,
            claw_phase: ClawPhase::Aiming,
            claw_timer: 0.0,
            claw_pos: Vec2::ZERO,
            claw_y: 2.35,
            claw_grabbed: None,
            plinko_aim_x: 0.0,
            plinko_cooldown: 0.0,
            skee_aim_x: 0.0,
            skee_power_phase: 0.0,
            skee_balls_left: 0,
            skee_cooldown: 0.0,
        }
    }
}

impl ArcadeState {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x | 1;
        ((x >> 40) as f32) / ((1u64 << 24) as f32)
    }
}

#[derive(Component)]
struct ArcadeStationRing(ArcadeKind);

#[derive(Component)]
struct ArcadePromptUi;

#[derive(Component)]
struct ArcadePromptButton;

#[derive(Component)]
struct ArcadePromptText;

#[derive(Component)]
struct ArcadeHudRoot;

#[derive(Component)]
struct ArcadeHudStatus;

#[derive(Component)]
struct ArcadeHudControls;

#[derive(Component)]
enum ArcadeHudBtn {
    Action,
    Extra,
    Leave,
}

// ---- Coin Pusher Entities ----
#[derive(Component)]
struct PusherShelf;

#[derive(Component)]
struct PusherDropper;

#[derive(Component)]
struct PusherCoin {
    local: Vec3,
    vel: Vec3,
    radius: f32,
    value: u32,
    falling: bool,
}

#[derive(Resource)]
struct PusherAssets {
    coin_mesh: Handle<Mesh>,
    bar_mesh: Handle<Mesh>,
    gold_mat: Handle<StandardMaterial>,
    ruby_mat: Handle<StandardMaterial>,
    bar_mat: Handle<StandardMaterial>,
}

// ---- Claw Machine Entities ----
#[derive(Component)]
struct ClawGantry;

#[derive(Component)]
struct ClawProng(usize);

#[derive(Component)]
struct ClawPrize {
    local: Vec3,
    value: u32,
    name: &'static str,
    grabbed: bool,
    falling: bool,
    vel_y: f32,
}

// ---- Plinko Entities ----
#[derive(Component)]
struct PlinkoDropper;

#[derive(Component)]
struct PlinkoDisc {
    local: Vec2, // x (-0.75..0.75), y (0.65..2.95)
    vel: Vec2,
    spin: f32,
}

#[derive(Resource)]
struct PlinkoAssets {
    disc_mesh: Handle<Mesh>,
    disc_mat: Handle<StandardMaterial>,
}

// ---- Skeeball Entities ----
#[derive(Component)]
struct SkeeballAimArrow;

#[derive(Component)]
struct SkeeballBall {
    local: Vec3, // x (-0.55..0.55), y (0.55..2.5), z (2.8..-2.2)
    vel: Vec3,
    scored: bool,
}

#[derive(Resource)]
struct SkeeballAssets {
    ball_mesh: Handle<Mesh>,
    ball_mat: Handle<StandardMaterial>,
}

fn exit_casino_arcade(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    prompt_q: Query<Entity, With<ArcadePromptUi>>,
    hud_q: Query<Entity, With<ArcadeHudRoot>>,
) {
    stats.arcade_open = false;
    arcade.active = None;
    arcade.was_open = false;
    arcade.claw_grabbed = None;
    for e in prompt_q.iter().chain(hud_q.iter()) {
        commands.entity(e).despawn_recursive();
    }
}

fn spawn_casino_arcade_machines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut arcade: ResMut<ArcadeState>,
) {
    *arcade = ArcadeState::default();

    let box_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cyl_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    let sphere_mesh = meshes.add(Sphere::new(1.0));
    let ring_mesh = meshes.add(Torus::new(1.08, 1.25));

    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.13, 0.08),
        perceptual_roughness: 0.55,
        ..default()
    });
    let cab_blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.22, 0.54),
        perceptual_roughness: 0.35,
        metallic: 0.3,
        ..default()
    });
    let cab_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.10, 0.22),
        perceptual_roughness: 0.35,
        metallic: 0.25,
        ..default()
    });
    let cab_teal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.42, 0.48),
        perceptual_roughness: 0.35,
        ..default()
    });
    let trim_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.76, 0.20),
        metallic: 0.85,
        perceptual_roughness: 0.2,
        ..default()
    });
    let neon_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.95, 1.0),
        emissive: LinearRgba::rgb(0.4, 2.2, 2.8),
        unlit: true,
        ..default()
    });
    let neon_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.84, 0.22),
        emissive: LinearRgba::rgb(2.4, 1.8, 0.3),
        unlit: true,
        ..default()
    });
    let neon_pink = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.82),
        emissive: LinearRgba::rgb(2.4, 0.5, 1.8),
        unlit: true,
        ..default()
    });
    let glass_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.75, 0.92, 1.0, 0.18),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.08,
        ..default()
    });
    let dark_felt = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.06, 0.14),
        perceptual_roughness: 0.85,
        ..default()
    });

    // ---- 1. TENDY COIN PUSHER ----
    let coin_mesh = meshes.add(Cylinder::new(0.11, 0.036));
    let bar_mesh = meshes.add(Cuboid::new(0.26, 0.07, 0.14));
    let ruby_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.18, 0.34),
        emissive: LinearRgba::rgb(1.8, 0.2, 0.4),
        metallic: 0.6,
        ..default()
    });
    commands.insert_resource(PusherAssets {
        coin_mesh: coin_mesh.clone(),
        bar_mesh: bar_mesh.clone(),
        gold_mat: trim_gold.clone(),
        ruby_mat: ruby_mat.clone(),
        bar_mat: neon_gold.clone(),
    });

    let (cp_pos, cp_yaw) = ArcadeKind::CoinPusher.pose();
    let cp_world = CASINO_LEVEL_ORIGIN + cp_pos;
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(cp_world + Vec3::Y * 1.2),
        PlatformBox {
            half_extents: Vec3::new(0.95, 1.2, 1.1),
        },
    ));
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(cp_world).with_rotation(Quat::from_rotation_y(cp_yaw)),
            Visibility::default(),
        ))
        .with_children(|p| {
            // Base cabinet + deck + backboard
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_blue.clone()),
                Transform::from_xyz(0.0, 0.50, 0.0).with_scale(Vec3::new(1.8, 1.0, 1.7)),
            ));
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(dark_felt.clone()),
                Transform::from_xyz(0.0, 1.02, 0.05).with_scale(Vec3::new(1.66, 0.06, 1.45)),
            ));
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_blue.clone()),
                Transform::from_xyz(0.0, 1.95, -0.74).with_scale(Vec3::new(1.8, 2.0, 0.16)),
            ));
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_gold.clone()),
                Transform::from_xyz(0.0, 2.98, 0.0).with_scale(Vec3::new(1.84, 0.24, 1.65)),
            ));
            // Side glass panels
            for sx in [-0.88_f32, 0.88] {
                p.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(glass_mat.clone()),
                    Transform::from_xyz(sx, 1.95, 0.0).with_scale(Vec3::new(0.04, 1.85, 1.55)),
                ));
            }
            // Motorized oscillating pusher shelf
            p.spawn((
                PusherShelf,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 1.16, -0.35).with_scale(Vec3::new(1.56, 0.22, 0.72)),
            ));
            // Dropper nozzle at top
            p.spawn((
                PusherDropper,
                Mesh3d(cyl_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_xyz(0.0, 2.68, -0.58).with_scale(Vec3::new(0.12, 0.22, 0.12)),
            ));

            // Initial 72 physical 3D coins + 4 ruby tokens + 2 gold bars stacked on the shelf & deck
            for i in 0..78_u32 {
                let row = i / 13;
                let col = i % 13;
                let lx = -0.66 + col as f32 * 0.11 + ((i % 2) as f32 - 0.5) * 0.03;
                let (ly, lz) = if row < 2 {
                    (1.29, -0.45 + row as f32 * 0.22)
                } else {
                    (1.07, 0.08 + (row - 2) as f32 * 0.17)
                };
                let (mesh, mat, val, rad) = if i == 70 || i == 75 {
                    (bar_mesh.clone(), neon_gold.clone(), 15, 0.13)
                } else if i % 18 == 7 {
                    (coin_mesh.clone(), ruby_mat.clone(), 5, 0.11)
                } else {
                    (coin_mesh.clone(), trim_gold.clone(), 1, 0.11)
                };
                p.spawn((
                    PusherCoin {
                        local: Vec3::new(lx, ly, lz),
                        vel: Vec3::ZERO,
                        radius: rad,
                        value: val,
                        falling: false,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(lx, ly, lz),
                ));
            }
        });

    // ---- 2. TENDY CLAW MACHINE ----
    let (cm_pos, cm_yaw) = ArcadeKind::ClawMachine.pose();
    let cm_world = CASINO_LEVEL_ORIGIN + cm_pos;
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(cm_world + Vec3::Y * 1.3),
        PlatformBox {
            half_extents: Vec3::new(0.95, 1.3, 0.95),
        },
    ));
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(cm_world).with_rotation(Quat::from_rotation_y(cm_yaw)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_red.clone()),
                Transform::from_xyz(0.0, 0.45, 0.0).with_scale(Vec3::new(1.75, 0.90, 1.75)),
            ));
            c.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(dark_felt.clone()),
                Transform::from_xyz(0.0, 0.92, 0.0).with_scale(Vec3::new(1.58, 0.05, 1.58)),
            ));
            // 4 corner neon pillars + marquee roof
            for sx in [-0.82_f32, 0.82] {
                for sz in [-0.82_f32, 0.82] {
                    c.spawn((
                        Mesh3d(box_mesh.clone()),
                        MeshMaterial3d(neon_pink.clone()),
                        Transform::from_xyz(sx, 1.80, sz).with_scale(Vec3::new(0.09, 1.85, 0.09)),
                    ));
                }
            }
            c.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_red.clone()),
                Transform::from_xyz(0.0, 2.80, 0.0).with_scale(Vec3::new(1.80, 0.24, 1.80)),
            ));
            // Prize drop chute in front-left corner
            c.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_xyz(-0.56, 1.02, 0.56).with_scale(Vec3::new(0.42, 0.22, 0.42)),
            ));
            // Gantry & 3-prong mechanical claw
            c.spawn((
                ClawGantry,
                Transform::from_xyz(0.0, 2.35, 0.0),
                Visibility::default(),
            ))
            .with_children(|g| {
                g.spawn((
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(trim_gold.clone()),
                    Transform::from_xyz(0.0, 0.35, 0.0).with_scale(Vec3::new(0.03, 0.70, 0.03)),
                ));
                g.spawn((
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(trim_gold.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(0.11)),
                ));
                for pi in 0..3 {
                    let ang = pi as f32 * (2.0 * PI / 3.0);
                    g.spawn((
                        ClawProng(pi),
                        Mesh3d(box_mesh.clone()),
                        MeshMaterial3d(neon_cyan.clone()),
                        Transform::from_xyz(ang.cos() * 0.14, -0.16, ang.sin() * 0.14)
                            .with_rotation(Quat::from_rotation_y(-ang) * Quat::from_rotation_z(0.35))
                            .with_scale(Vec3::new(0.04, 0.26, 0.04)),
                    ));
                }
            });
            // 14 physical prizes inside the bin
            let prize_specs = [
                (10_u32, "GOLDEN TENDY", neon_gold.clone(), Vec3::new(0.24, 0.14, 0.14)),
                (25, "PLUSH BUFFALO", cab_red.clone(), Vec3::splat(0.22)),
                (15, "VIP CHIP STACK", neon_pink.clone(), Vec3::new(0.18, 0.22, 0.18)),
                (8, "RANCH BOTTLE", neon_cyan.clone(), Vec3::new(0.14, 0.26, 0.14)),
            ];
            for i in 0..14_usize {
                let (val, name, mat, sc) = &prize_specs[i % prize_specs.len()];
                let px = -0.35 + ((i * 5) % 4) as f32 * 0.30;
                let pz = -0.52 + (i / 4) as f32 * 0.32;
                let pos = Vec3::new(px, 1.06, pz);
                c.spawn((
                    ClawPrize {
                        local: pos,
                        value: *val,
                        name,
                        grabbed: false,
                        falling: false,
                        vel_y: 0.0,
                    },
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation(pos).with_scale(*sc),
                ));
            }
        });

    // ---- 3. PLINKO TOWER ----
    let disc_mesh = meshes.add(Cylinder::new(0.08, 0.05));
    commands.insert_resource(PlinkoAssets {
        disc_mesh,
        disc_mat: neon_pink.clone(),
    });
    let (pl_pos, pl_yaw) = ArcadeKind::Plinko.pose();
    let pl_world = CASINO_LEVEL_ORIGIN + pl_pos;
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(pl_world + Vec3::Y * 1.4),
        PlatformBox {
            half_extents: Vec3::new(0.95, 1.4, 0.45),
        },
    ));
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(pl_world).with_rotation(Quat::from_rotation_y(pl_yaw)),
            Visibility::default(),
        ))
        .with_children(|pl| {
            pl.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_teal.clone()),
                Transform::from_xyz(0.0, 1.65, -0.15).with_scale(Vec3::new(1.85, 3.1, 0.22)),
            ));
            // 7 staggered rows of brass pegs
            for row in 0..7_usize {
                let y = 2.65 - row as f32 * 0.30;
                let count = if row % 2 == 0 { 7 } else { 6 };
                let x0 = -(count as f32 - 1.0) * 0.11;
                for col in 0..count {
                    pl.spawn((
                        Mesh3d(cyl_mesh.clone()),
                        MeshMaterial3d(trim_gold.clone()),
                        Transform::from_xyz(x0 + col as f32 * 0.22, y, 0.02)
                            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                            .with_scale(Vec3::new(0.028, 0.14, 0.028)),
                    ));
                }
            }
            // 5 glowing bucket dividers at bottom
            for b in 0..6 {
                let bx = -0.80 + b as f32 * 0.32;
                pl.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(if b == 2 || b == 3 { neon_gold.clone() } else { neon_cyan.clone() }),
                    Transform::from_xyz(bx, 0.56, 0.02).with_scale(Vec3::new(0.04, 0.42, 0.18)),
                ));
            }
            pl.spawn((
                PlinkoDropper,
                Mesh3d(cyl_mesh.clone()),
                MeshMaterial3d(neon_pink.clone()),
                Transform::from_xyz(0.0, 2.95, 0.04)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                    .with_scale(Vec3::new(0.09, 0.06, 0.09)),
            ));
        });

    // ---- 4. SKEEBALL ALLEY ----
    let ball_mesh = meshes.add(Sphere::new(0.09));
    commands.insert_resource(SkeeballAssets {
        ball_mesh,
        ball_mat: neon_cyan.clone(),
    });
    let (sk_pos, sk_yaw) = ArcadeKind::Skeeball.pose();
    let sk_world = CASINO_LEVEL_ORIGIN + sk_pos;
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(sk_world + Vec3::Y * 0.45),
        PlatformBox {
            half_extents: Vec3::new(0.75, 0.45, 2.6),
        },
    ));
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(sk_world).with_rotation(Quat::from_rotation_y(sk_yaw)),
            Visibility::default(),
        ))
        .with_children(|sk| {
            // Wooden bowling lane + curved ramp + backboard + 4 glowing target rings
            sk.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.35, 0.3).with_scale(Vec3::new(1.25, 0.48, 5.2)),
            ));
            sk.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 0.72, -1.65)
                    .with_rotation(Quat::from_rotation_x(-0.45))
                    .with_scale(Vec3::new(1.18, 0.12, 0.95)),
            ));
            sk.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_blue.clone()),
                Transform::from_xyz(0.0, 1.55, -2.35).with_scale(Vec3::new(1.30, 1.9, 0.16)),
            ));
            for (ry, rrad, mat) in [
                (1.15_f32, 0.38_f32, neon_cyan.clone()),
                (1.52, 0.28, neon_pink.clone()),
                (1.88, 0.20, neon_gold.clone()),
                (2.22, 0.13, neon_gold.clone()),
            ] {
                sk.spawn((
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, ry, -2.24)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                        .with_scale(Vec3::new(rrad, 0.06, rrad)),
                ));
            }
            sk.spawn((
                SkeeballAimArrow,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_gold.clone()),
                Transform::from_xyz(0.0, 0.65, 2.4).with_scale(Vec3::new(0.08, 0.05, 0.45)),
            ));
        });

    // Floor interaction rings for all 4 arcade machines
    for (kind, color) in [
        (ArcadeKind::CoinPusher, Color::srgb(1.0, 0.84, 0.18)),
        (ArcadeKind::ClawMachine, Color::srgb(1.0, 0.32, 0.80)),
        (ArcadeKind::Plinko, Color::srgb(0.22, 0.92, 1.0)),
        (ArcadeKind::Skeeball, Color::srgb(0.45, 1.0, 0.45)),
    ] {
        let (p, yaw) = kind.pose();
        let l = color.to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::new(l.red * 1.8, l.green * 1.8, l.blue * 1.8, 1.0),
            unlit: true,
            ..default()
        });
        let world_p = CASINO_LEVEL_ORIGIN + p + Quat::from_rotation_y(yaw) * kind.ring_offset();
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            ArcadeStationRing(kind),
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(world_p),
        ));
    }
}

fn update_arcade_proximity(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    mut arcade: ResMut<ArcadeState>,
    player_q: Query<&Transform, With<Player>>,
    ring_q: Query<(&ArcadeStationRing, &Transform)>,
    prompt_q: Query<Entity, With<ArcadePromptUi>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<ArcadePromptButton>)>,
    mut text_q: Query<&mut Text, With<ArcadePromptText>>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if !stats.in_casino_level || stats.casino_open || stats.arcade_open || help.open {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let mut best: Option<(f32, ArcadeKind)> = None;
    for (ring, r_tf) in &ring_q {
        let d = Vec2::new(p_tf.translation.x - r_tf.translation.x, p_tf.translation.z - r_tf.translation.z).length();
        if d < 3.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ring.0));
        }
    }
    let Some((_, kind)) = best else {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let label = kind.prompt();
    if prompt_q.is_empty() {
        commands
            .spawn((
                ArcadePromptUi,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(74.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|root| {
                root.spawn((
                    ArcadePromptButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.24, 0.50)),
                    BorderColor(Color::srgb(0.35, 0.92, 1.0)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ArcadePromptText,
                        Text::new(label),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    } else if let Ok(mut t) = text_q.get_single_mut() {
        if t.0 != label {
            t.0 = label.to_string();
        }
    }

    let pressed = btn_q.iter().any(|i| *i == Interaction::Pressed)
        || keys.just_pressed(KeyCode::KeyF)
        || keys.just_pressed(KeyCode::KeyE);
    if !pressed {
        return;
    }

    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }
    stats.arcade_open = true;
    arcade.active = Some(kind);
    arcade.status_msg = match kind {
        ArcadeKind::CoinPusher => "Aim [A/D] | Drop 1 TW [SPACE] | Rapid 3 TW [B]".to_string(),
        ArcadeKind::ClawMachine => "Move Claw [WASD] | Drop Claw (5 TW) [SPACE]".to_string(),
        ArcadeKind::Plinko => "Slide Puck [A/D] | Drop Plinko (5 TW) [SPACE]".to_string(),
        ArcadeKind::Skeeball => "Aim [A/D] | Launch Ball (5 TW / 3 Balls) [SPACE]".to_string(),
    };
    if let Ok(mut win) = window_q.get_single_mut() {
        win.cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
        win.cursor_options.visible = true;
    }
    spawn_arcade_hud(&mut commands);
}

fn spawn_arcade_hud(commands: &mut Commands) {
    commands
        .spawn((
            ArcadeHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            GlobalZIndex(895),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.04, 0.12, 0.82)),
                BorderColor(Color::srgba(0.35, 0.92, 1.0, 0.78)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|p| {
                p.spawn((
                    ArcadeHudStatus,
                    Text::new(""),
                    TextFont { font_size: 13.5, ..default() },
                    TextColor(Color::srgb(1.0, 0.94, 0.68)),
                ));
            });

            root.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.04, 0.12, 0.78)),
                BorderColor(Color::srgba(0.98, 0.78, 0.24, 0.78)),
                BorderRadius::all(Val::Px(999.0)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    ArcadeHudControls,
                    Text::new(""),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.84, 0.94, 1.0)),
                ));
                for (btn_kind, label, bg) in [
                    (ArcadeHudBtn::Action, "PLAY [SPACE]", Color::srgb(0.16, 0.56, 0.28)),
                    (ArcadeHudBtn::Extra, "3x DROP [B]", Color::srgb(0.56, 0.34, 0.12)),
                    (ArcadeHudBtn::Leave, "LEAVE [ESC]", Color::srgb(0.52, 0.14, 0.18)),
                ] {
                    bar.spawn((
                        btn_kind,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(11.0), Val::Px(5.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
                        BorderRadius::all(Val::Px(999.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(label),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn handle_arcade_input(
    mut commands: Commands,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    pusher_assets: Option<Res<PusherAssets>>,
    plinko_assets: Option<Res<PlinkoAssets>>,
    skee_assets: Option<Res<SkeeballAssets>>,
    cab_q: Query<(Entity, &Transform), Without<PusherCoin>>,
    hud_q: Query<Entity, With<ArcadeHudRoot>>,
    btn_q: Query<(&ArcadeHudBtn, &Interaction), Changed<Interaction>>,
) {
    let Some(kind) = arcade.active else {
        return;
    };
    let dt = time.delta_secs().min(0.05);

    let mut leave = keys.just_pressed(KeyCode::Escape);
    let mut act = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter);
    let mut extra = keys.just_pressed(KeyCode::KeyB);
    for (bk, inter) in &btn_q {
        if *inter != Interaction::Pressed {
            continue;
        }
        match bk {
            ArcadeHudBtn::Action => act = true,
            ArcadeHudBtn::Extra => extra = true,
            ArcadeHudBtn::Leave => leave = true,
        }
    }
    if leave {
        stats.arcade_open = false;
        stats.casino_esc_grace = true;
        arcade.active = None;
        for e in &hud_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    let left = keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft);
    let right = keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight);
    let up = keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp);
    let down = keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown);
    let ax = (right as i8 - left as i8) as f32;
    let az = (down as i8 - up as i8) as f32;

    match kind {
        ArcadeKind::CoinPusher => {
            arcade.pusher_aim_x = (arcade.pusher_aim_x + ax * 1.15 * dt).clamp(-0.62, 0.62);
            let count = if extra { 3 } else if act { 1 } else { 0 };
            if count > 0 && arcade.pusher_cooldown <= 0.0 {
                if stats.spend_tendy_wendies(count) {
                    arcade.pusher_cooldown = 0.25;
                    play_sfx("chip_bet");
                    if let Some(pa) = pusher_assets {
                        let (cp, _) = ArcadeKind::CoinPusher.pose();
                        let target_w = CASINO_LEVEL_ORIGIN + cp;
                        if let Some((parent, _)) = cab_q.iter().find(|(_, tf)| (tf.translation - target_w).length() < 0.2) {
                            for k in 0..count {
                                let ox = arcade.pusher_aim_x + (k as f32 - 1.0) * 0.08;
                                let local = Vec3::new(ox.clamp(-0.66, 0.66), 2.55 + k as f32 * 0.12, -0.58);
                                commands.entity(parent).with_children(|p| {
                                    p.spawn((
                                        PusherCoin {
                                            local,
                                            vel: Vec3::new(0.0, -0.6, 0.25),
                                            radius: 0.11,
                                            value: 1,
                                            falling: true,
                                        },
                                        Mesh3d(pa.coin_mesh.clone()),
                                        MeshMaterial3d(pa.gold_mat.clone()),
                                        Transform::from_translation(local),
                                    ));
                                });
                            }
                        }
                    }
                    arcade.status_msg = format!("Dropped {} TW coin(s)! Watch the edge!", count);
                } else {
                    arcade.status_msg = "Need at least 1 TW! (Cashier gives +25 TW below 25 TW)".to_string();
                }
            }
        }
        ArcadeKind::ClawMachine => {
            if arcade.claw_phase == ClawPhase::Aiming {
                arcade.claw_pos.x = (arcade.claw_pos.x + ax * 1.05 * dt).clamp(-0.62, 0.62);
                arcade.claw_pos.y = (arcade.claw_pos.y + az * 1.05 * dt).clamp(-0.62, 0.62);
                if act {
                    if stats.spend_tendy_wendies(5) {
                        arcade.claw_phase = ClawPhase::Descending;
                        arcade.claw_timer = 0.0;
                        arcade.status_msg = "Dropping claw...".to_string();
                        play_sfx("chip_bet");
                    } else {
                        arcade.status_msg = "Claw Machine costs 5 TW!".to_string();
                    }
                }
            }
        }
        ArcadeKind::Plinko => {
            arcade.plinko_aim_x = (arcade.plinko_aim_x + ax * 1.15 * dt).clamp(-0.68, 0.68);
            if act && arcade.plinko_cooldown <= 0.0 {
                if stats.spend_tendy_wendies(5) {
                    arcade.plinko_cooldown = 0.45;
                    play_sfx("chip_bet");
                    let (pp, _) = ArcadeKind::Plinko.pose();
                    let target_w = CASINO_LEVEL_ORIGIN + pp;
                    if let (Some(pa), Some((parent, _))) = (
                        plinko_assets,
                        cab_q.iter().find(|(_, tf)| (tf.translation - target_w).length() < 0.2),
                    ) {
                        let jitter = (arcade.next_f32() - 0.5) * 0.05;
                        let start = Vec2::new(arcade.plinko_aim_x + jitter, 2.86);
                        commands.entity(parent).with_children(|pl| {
                            pl.spawn((
                                PlinkoDisc {
                                    local: start,
                                    vel: Vec2::new(jitter * 2.0, -0.4),
                                    spin: 0.0,
                                },
                                Mesh3d(pa.disc_mesh.clone()),
                                MeshMaterial3d(pa.disc_mat.clone()),
                                Transform::from_xyz(start.x, start.y, 0.04)
                                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                            ));
                        });
                    }
                    arcade.status_msg = "Plinko disc away! Aiming for 25 TW center!".to_string();
                } else {
                    arcade.status_msg = "Plinko costs 5 TW!".to_string();
                }
            }
        }
        ArcadeKind::Skeeball => {
            arcade.skee_aim_x = (arcade.skee_aim_x + ax * 0.9 * dt).clamp(-0.42, 0.42);
            if act && arcade.skee_cooldown <= 0.0 {
                let can_throw = if arcade.skee_balls_left > 0 {
                    true
                } else if stats.spend_tendy_wendies(5) {
                    arcade.skee_balls_left = 3;
                    true
                } else {
                    arcade.status_msg = "Skeeball costs 5 TW for 3 balls!".to_string();
                    false
                };
                if can_throw {
                    arcade.skee_balls_left -= 1;
                    arcade.skee_cooldown = 0.55;
                    play_sfx("chip_bet");
                    let pwr = 0.55 + 0.45 * (0.5 + 0.5 * arcade.skee_power_phase.sin());
                    let (sp, _) = ArcadeKind::Skeeball.pose();
                    let target_w = CASINO_LEVEL_ORIGIN + sp;
                    if let (Some(sa), Some((parent, _))) = (
                        skee_assets,
                        cab_q.iter().find(|(_, tf)| (tf.translation - target_w).length() < 0.2),
                    ) {
                        let start = Vec3::new(arcade.skee_aim_x * 0.4, 0.68, 2.35);
                        let speed = 5.4 + pwr * 2.6;
                        commands.entity(parent).with_children(|sk| {
                            sk.spawn((
                                SkeeballBall {
                                    local: start,
                                    vel: Vec3::new(arcade.skee_aim_x * 1.15, 0.0, -speed),
                                    scored: false,
                                },
                                Mesh3d(sa.ball_mesh.clone()),
                                MeshMaterial3d(sa.ball_mat.clone()),
                                Transform::from_translation(start),
                            ));
                        });
                    }
                    arcade.status_msg = format!("Rolled Skeeball! ({} balls left)", arcade.skee_balls_left);
                }
            }
        }
    }
}

fn step_coin_pusher_physics(
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut shelf_q: Query<&mut Transform, (With<PusherShelf>, Without<PusherDropper>, Without<PusherCoin>)>,
    mut drop_q: Query<&mut Transform, (With<PusherDropper>, Without<PusherShelf>, Without<PusherCoin>)>,
    mut coin_q: Query<(Entity, &mut PusherCoin, &mut Transform), (Without<PusherShelf>, Without<PusherDropper>)>,
) {
    let dt = time.delta_secs().min(0.05);
    arcade.pusher_phase += dt * 1.45;
    if arcade.pusher_cooldown > 0.0 {
        arcade.pusher_cooldown = (arcade.pusher_cooldown - dt).max(0.0);
    }

    let prev_z = -0.35 + ((arcade.pusher_phase - dt * 1.45).sin() * 0.24);
    let cur_z = -0.35 + (arcade.pusher_phase.sin() * 0.24);
    let dz = (cur_z - prev_z).max(0.0);
    let pusher_front_z = cur_z + 0.36;

    for mut tf in &mut shelf_q {
        tf.translation.z = cur_z;
    }
    for mut tf in &mut drop_q {
        tf.translation.x = arcade.pusher_aim_x;
    }

    let mut coins: Vec<(Entity, PusherCoin)> = Vec::new();
    for (e, c, _) in &coin_q {
        coins.push((e, PusherCoin {
            local: c.local,
            vel: c.vel,
            radius: c.radius,
            value: c.value,
            falling: c.falling,
        }));
    }

    // 1. Gravity & pusher shelf advancement
    for (_, c) in &mut coins {
        if c.falling {
            c.vel.y -= 9.5 * dt;
            c.local += c.vel * dt;
            let floor_y = if c.local.z < 0.02 { 1.29 } else { 1.07 };
            if c.local.y <= floor_y {
                c.local.y = floor_y;
                c.vel = Vec3::ZERO;
                c.falling = false;
            }
        } else if c.local.y > 1.18 && c.local.z < pusher_front_z + c.radius {
            c.local.z = (pusher_front_z + c.radius).min(0.05);
        } else if c.local.y > 1.18 && c.local.z >= 0.02 {
            // Drop from upper tier to lower deck
            c.falling = true;
            c.vel = Vec3::new(0.0, -0.4, 0.35);
        } else if c.local.y <= 1.18 && c.local.z < 0.05 + dz + 0.02 && dz > 0.0 {
            c.local.z += dz * 0.65;
        }
        c.local.x = c.local.x.clamp(-0.70, 0.70);
    }

    // 2. Pairwise coin collision pushing on each tier
    let n = coins.len();
    for i in 0..n {
        for j in (i + 1)..n {
            if coins[i].1.falling || coins[j].1.falling {
                continue;
            }
            if (coins[i].1.local.y - coins[j].1.local.y).abs() > 0.08 {
                continue;
            }
            let diff = Vec2::new(
                coins[j].1.local.x - coins[i].1.local.x,
                coins[j].1.local.z - coins[i].1.local.z,
            );
            let min_d = coins[i].1.radius + coins[j].1.radius;
            let len = diff.length();
            if len > 0.0001 && len < min_d {
                let push = diff / len * (min_d - len) * 0.5;
                coins[i].1.local.x -= push.x;
                coins[i].1.local.z -= push.y;
                coins[j].1.local.x += push.x;
                coins[j].1.local.z += push.y;
            }
        }
    }

    // 3. Apply transforms & award coins pushed over the front lip!
    for (e, mut c, mut tf) in &mut coin_q {
        if let Some((_, sim)) = coins.iter().find(|(ce, _)| *ce == e) {
            c.local = sim.local;
            c.vel = sim.vel;
            c.falling = sim.falling;
        }
        if c.local.z > 0.74 {
            let val = c.value;
            if arcade.active == Some(ArcadeKind::CoinPusher) {
                stats.add_tendy_wendies(val);
                arcade.status_msg = format!("COIN CASCADE! +{} TW pushed off the ledge!", val);
                play_sfx(if val >= 5 { "slot_jackpot" } else { "tendy" });
            }
            // Recycle coin onto the back of the upper pusher shelf so the machine stays stocked!
            let rx = (arcade.next_f32() - 0.5) * 1.24;
            c.local = Vec3::new(rx, 1.32, -0.52);
            c.vel = Vec3::ZERO;
            c.falling = false;
        }
        tf.translation = c.local;
    }
}

fn step_claw_machine(
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut gantry_q: Query<&mut Transform, (With<ClawGantry>, Without<ClawProng>, Without<ClawPrize>)>,
    mut prong_q: Query<(&ClawProng, &mut Transform), (Without<ClawGantry>, Without<ClawPrize>)>,
    mut prize_q: Query<(Entity, &mut ClawPrize, &mut Transform), (Without<ClawGantry>, Without<ClawProng>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let open_ang = match arcade.claw_phase {
        ClawPhase::Aiming | ClawPhase::Descending | ClawPhase::Releasing => 0.45_f32,
        _ => 0.12_f32,
    };

    match arcade.claw_phase {
        ClawPhase::Aiming => {
            arcade.claw_y = 2.35;
        }
        ClawPhase::Descending => {
            arcade.claw_y -= 1.35 * dt;
            if arcade.claw_y <= 1.26 {
                arcade.claw_y = 1.26;
                arcade.claw_phase = ClawPhase::Grabbing;
                arcade.claw_timer = 0.0;
                // Find nearest prize within grab radius
                let mut best: Option<(f32, Entity)> = None;
                for (e, pr, _) in &prize_q {
                    if pr.grabbed || pr.falling {
                        continue;
                    }
                    let d = Vec2::new(pr.local.x - arcade.claw_pos.x, pr.local.z - arcade.claw_pos.y).length();
                    if d < 0.34 && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, e));
                    }
                }
                if let Some((_, pe)) = best {
                    if let Ok((_, mut pr, _)) = prize_q.get_mut(pe) {
                        pr.grabbed = true;
                        arcade.claw_grabbed = Some(pe);
                        arcade.status_msg = format!("Grabbed {} (+{} TW)! Lifting...", pr.name, pr.value);
                        play_sfx("powerup");
                    }
                } else {
                    arcade.status_msg = "Missed! Aim closer over a prize!".to_string();
                }
            }
        }
        ClawPhase::Grabbing => {
            arcade.claw_timer += dt;
            if arcade.claw_timer >= 0.45 {
                arcade.claw_phase = ClawPhase::Ascending;
            }
        }
        ClawPhase::Ascending => {
            arcade.claw_y += 1.25 * dt;
            if arcade.claw_y >= 2.35 {
                arcade.claw_y = 2.35;
                arcade.claw_phase = ClawPhase::Returning;
            }
        }
        ClawPhase::Returning => {
            let target = Vec2::new(-0.56, 0.56);
            let dir = target - arcade.claw_pos;
            let dist = dir.length();
            if dist < 0.04 {
                arcade.claw_pos = target;
                arcade.claw_phase = ClawPhase::Releasing;
                arcade.claw_timer = 0.0;
                if let Some(pe) = arcade.claw_grabbed.take() {
                    if let Ok((_, mut pr, _)) = prize_q.get_mut(pe) {
                        pr.grabbed = false;
                        pr.falling = true;
                        pr.vel_y = 0.0;
                    }
                }
            } else {
                arcade.claw_pos += dir / dist * (1.25 * dt).min(dist);
            }
        }
        ClawPhase::Releasing => {
            arcade.claw_timer += dt;
            if arcade.claw_timer >= 0.55 {
                arcade.claw_phase = ClawPhase::Aiming;
            }
        }
    }

    for mut tf in &mut gantry_q {
        tf.translation = Vec3::new(arcade.claw_pos.x, arcade.claw_y, arcade.claw_pos.y);
    }
    for (pr, mut tf) in &mut prong_q {
        let ang = pr.0 as f32 * (2.0 * PI / 3.0);
        tf.rotation = Quat::from_rotation_y(-ang) * Quat::from_rotation_z(open_ang);
    }
    for (_, mut pr, mut tf) in &mut prize_q {
        if pr.grabbed {
            pr.local = Vec3::new(arcade.claw_pos.x, arcade.claw_y - 0.24, arcade.claw_pos.y);
        } else if pr.falling {
            pr.vel_y -= 8.0 * dt;
            pr.local.y += pr.vel_y * dt;
            if pr.local.y <= 0.98 {
                let val = pr.value;
                stats.add_tendy_wendies(val);
                arcade.status_msg = format!("PRIZE WON! {} (+{} TW)!", pr.name, val);
                play_sfx("slot_jackpot");
                // Respawn a fresh prize in the bin
                pr.falling = false;
                pr.vel_y = 0.0;
                pr.local = Vec3::new((arcade.next_f32() - 0.5) * 0.9, 1.06, (arcade.next_f32() - 0.5) * 0.9);
            }
        }
        tf.translation = pr.local;
    }
}

fn step_plinko_physics(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut drop_q: Query<&mut Transform, (With<PlinkoDropper>, Without<PlinkoDisc>)>,
    mut disc_q: Query<(Entity, &mut PlinkoDisc, &mut Transform), Without<PlinkoDropper>>,
) {
    let dt = time.delta_secs().min(0.05);
    if arcade.plinko_cooldown > 0.0 {
        arcade.plinko_cooldown = (arcade.plinko_cooldown - dt).max(0.0);
    }
    for mut tf in &mut drop_q {
        tf.translation.x = arcade.plinko_aim_x;
    }

    for (e, mut disc, mut tf) in &mut disc_q {
        disc.vel.y -= 5.8 * dt;
        let dv = disc.vel * dt;
        disc.local += dv;
        disc.spin += disc.vel.x * 5.0 * dt;

        // Side wall bounces
        if disc.local.x < -0.78 {
            disc.local.x = -0.78;
            disc.vel.x = disc.vel.x.abs() * 0.65;
        } else if disc.local.x > 0.78 {
            disc.local.x = 0.78;
            disc.vel.x = -disc.vel.x.abs() * 0.65;
        }

        // Peg circle collisions (matches 7 staggered rows)
        for row in 0..7_usize {
            let py = 2.65 - row as f32 * 0.30;
            if (disc.local.y - py).abs() > 0.14 {
                continue;
            }
            let count = if row % 2 == 0 { 7 } else { 6 };
            let x0 = -(count as f32 - 1.0) * 0.11;
            for col in 0..count {
                let px = x0 + col as f32 * 0.22;
                let diff = disc.local - Vec2::new(px, py);
                let dist = diff.length();
                let min_d = 0.095;
                if dist > 0.0001 && dist < min_d {
                    let n = diff / dist;
                    disc.local = Vec2::new(px, py) + n * min_d;
                    let vn = disc.vel.dot(n);
                    if vn < 0.0 {
                        disc.vel -= 1.55 * vn * n;
                        disc.vel.x += (arcade.next_f32() - 0.5) * 0.35;
                        play_sfx("slot_tick");
                    }
                }
            }
        }

        if disc.local.y <= 0.58 {
            let bucket = (((disc.local.x + 0.80) / 0.32).floor() as i32).clamp(0, 4);
            let payout = match bucket {
                2 => 25,
                1 | 3 => 5,
                _ => 1,
            };
            stats.add_tendy_wendies(payout);
            arcade.status_msg = format!("PLINKO LANDED IN {} TW BUCKET!", payout);
            play_sfx(if payout >= 25 { "slot_jackpot" } else { "tendy" });
            commands.entity(e).despawn_recursive();
            continue;
        }
        tf.translation = Vec3::new(disc.local.x, disc.local.y, 0.04);
        tf.rotation = Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_y(disc.spin);
    }
}

fn step_skeeball_physics(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut arrow_q: Query<&mut Transform, (With<SkeeballAimArrow>, Without<SkeeballBall>)>,
    mut ball_q: Query<(Entity, &mut SkeeballBall, &mut Transform), Without<SkeeballAimArrow>>,
) {
    let dt = time.delta_secs().min(0.05);
    arcade.skee_power_phase += dt * 4.2;
    if arcade.skee_cooldown > 0.0 {
        arcade.skee_cooldown = (arcade.skee_cooldown - dt).max(0.0);
    }
    for mut tf in &mut arrow_q {
        tf.translation.x = arcade.skee_aim_x * 0.4;
        tf.rotation = Quat::from_rotation_y(-arcade.skee_aim_x * 0.45);
    }

    for (e, mut b, mut tf) in &mut ball_q {
        let dv = b.vel * dt;
        b.local += dv;
        // Lane rails
        if b.local.x < -0.52 {
            b.local.x = -0.52;
            b.vel.x = b.vel.x.abs() * 0.7;
        } else if b.local.x > 0.52 {
            b.local.x = 0.52;
            b.vel.x = -b.vel.x.abs() * 0.7;
        }
        // Curved launch ramp near z = -1.2..-2.1
        if b.local.z < -1.2 && b.vel.y == 0.0 {
            b.vel.y = (-b.vel.z) * 0.62;
        }
        if b.vel.y != 0.0 || b.local.y > 0.69 {
            b.vel.y -= 7.2 * dt;
        }
        if b.local.z <= -2.18 && !b.scored {
            b.scored = true;
            // Check 4 target rings on backboard
            let rings = [(1.15_f32, 0.40_f32, 5_u32), (1.52, 0.30, 10), (1.88, 0.22, 20), (2.22, 0.16, 40)];
            let mut best = 1_u32;
            for (ry, rad, val) in rings {
                let d = Vec2::new(b.local.x, b.local.y - ry).length();
                if d <= rad {
                    best = best.max(val);
                }
            }
            stats.add_tendy_wendies(best);
            arcade.status_msg = format!("SKEEBALL HIT! +{} TW! ({} balls left)", best, arcade.skee_balls_left);
            play_sfx(if best >= 20 { "slot_jackpot" } else { "tendy" });
            commands.entity(e).despawn_recursive();
            continue;
        }
        if b.local.y < 0.2 || b.local.z < -3.0 {
            commands.entity(e).despawn_recursive();
            continue;
        }
        tf.translation = b.local;
    }
}

fn update_arcade_camera(
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    arcade: Res<ArcadeState>,
    mut player_q: Query<&mut Transform, With<Player>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    mut orbit: ResMut<CameraOrbit>,
) {
    let Some(kind) = arcade.active else {
        if arcade.was_open {
            orbit.blend_request = true;
        }
        return;
    };
    if !stats.arcade_open {
        return;
    }
    let (mp, myaw) = kind.pose();
    let w = CASINO_LEVEL_ORIGIN + mp;
    let r = Quat::from_rotation_y(myaw);

    // Keep player visible standing cleanly beside the machine in third-person!
    for mut p_tf in &mut player_q {
        let p_off = match kind {
            ArcadeKind::Skeeball => Vec3::new(-0.95, 0.0, 2.85),
            _ => Vec3::new(-0.92, 0.0, 1.55),
        };
        p_tf.translation = w + r * p_off;
        p_tf.rotation = r;
    }

    let (cam_off, look_off) = match kind {
        ArcadeKind::CoinPusher => (Vec3::new(0.85, 2.25, 2.75), Vec3::new(0.0, 1.48, 0.0)),
        ArcadeKind::ClawMachine => (Vec3::new(1.10, 2.38, 2.95), Vec3::new(0.0, 1.55, 0.0)),
        ArcadeKind::Plinko => (Vec3::new(0.95, 2.25, 3.35), Vec3::new(0.0, 1.68, 0.0)),
        ArcadeKind::Skeeball => (Vec3::new(1.15, 2.20, 4.30), Vec3::new(0.0, 1.35, -1.2)),
    };
    let pos = w + r * cam_off;
    let look = w + r * look_off;
    let dt = time.delta_secs().min(0.05);
    let k = 1.0 - (-6.5 * dt).exp();
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = c_tf.translation.lerp(pos, k);
        let goal = Transform::from_translation(c_tf.translation).looking_at(look, Vec3::Y).rotation;
        c_tf.rotation = c_tf.rotation.slerp(goal, k);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov += (0.76 - p.fov) * k;
        }
    }
}

fn refresh_arcade_hud(
    stats: Res<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut status_q: Query<&mut Text, (With<ArcadeHudStatus>, Without<ArcadeHudControls>)>,
    mut ctrl_q: Query<&mut Text, (With<ArcadeHudControls>, Without<ArcadeHudStatus>)>,
    mut btn_q: Query<(&ArcadeHudBtn, &mut Node)>,
) {
    arcade.was_open = arcade.active.is_some();
    let Some(kind) = arcade.active else {
        return;
    };
    if let Ok(mut t) = status_q.get_single_mut() {
        t.0 = format!("{} TW  |  {}", stats.tendy_wendies, arcade.status_msg);
    }
    if let Ok(mut t) = ctrl_q.get_single_mut() {
        t.0 = match kind {
            ArcadeKind::CoinPusher => "AIM: A/D".to_string(),
            ArcadeKind::ClawMachine => "MOVE: WASD".to_string(),
            ArcadeKind::Plinko => "AIM: A/D".to_string(),
            ArcadeKind::Skeeball => {
                let pct = ((0.5 + 0.5 * arcade.skee_power_phase.sin()) * 100.0).round() as u32;
                format!("AIM: A/D  |  PWR: {}%", pct)
            }
        };
    }
    for (bk, mut node) in &mut btn_q {
        if matches!(bk, ArcadeHudBtn::Extra) {
            node.display = if kind == ArcadeKind::CoinPusher {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
