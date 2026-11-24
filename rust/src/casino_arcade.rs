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
                    animate_jazz_band.run_if(sim_running),
                    step_coaster_and_stargate.run_if(sim_running),
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

pub const COASTER_RIDE_COST: u32 = 100_000;

pub fn coaster_ride_cost(online_players: u32) -> u32 {
    COASTER_RIDE_COST.saturating_mul(online_players.max(1))
}

fn format_tw_commas(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArcadeKind {
    CoinPusher,
    ClawMachine,
    Plinko,
    Skeeball,
    JazzBand,
    RollerCoaster,
}

impl ArcadeKind {
    fn prompt(self) -> &'static str {
        match self {
            Self::CoinPusher => "TENDY COIN PUSHER (1 TW - HOLD/SPAM FOR TSUNAMI!) [F/E]",
            Self::ClawMachine => "TENDY CLAW MACHINE (5 TW) [F/E]",
            Self::Plinko => "PLINKO TOWER (5 TW) [F/E]",
            Self::Skeeball => "SKEEBALL ALLEY (5 TW / 3 BALLS) [F/E]",
            Self::JazzBand => "TIP JAZZ BAND (5 TW) - TRUMPET SOLO! [F/E]",
            Self::RollerCoaster => "RIDE STARGATE COASTER (100,000 TW!) [F/E]",
        }
    }

    fn pose(self) -> (Vec3, f32) {
        match self {
            Self::CoinPusher => (Vec3::new(-9.2, 0.0, 5.5), FRAC_PI_2),
            Self::ClawMachine => (Vec3::new(9.2, 0.0, 5.5), -FRAC_PI_2),
            Self::Plinko => (Vec3::new(-9.2, 0.0, 11.2), FRAC_PI_2),
            Self::Skeeball => (Vec3::new(9.2, 0.0, 11.2), -FRAC_PI_2),
            Self::JazzBand => (Vec3::new(-14.6, 0.0, -14.8), 0.72),
            Self::RollerCoaster => (Vec3::new(13.8, 0.0, 14.2), -FRAC_PI_2),
        }
    }

    fn ring_offset(self) -> Vec3 {
        match self {
            Self::Skeeball => Vec3::new(0.0, 0.16, 3.6),
            Self::JazzBand => Vec3::new(0.0, 0.16, 2.7),
            Self::RollerCoaster => Vec3::ZERO,
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
    pusher_heat: f32,
    pusher_park_cursor: usize,
    pusher_session_coins: u32,
    pusher_session_tw: u32,
    pusher_last_chute_tw: u32,
    pusher_sfx_cd: f32,
    pusher_jackpot_sfx_cd: f32,
    pusher_neon_phase: f32,
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
    // Jazz Band
    jazz_solo_timer: f32,
    // Stargate Coaster
    coaster_u: f32,
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
            pusher_heat: 0.0,
            pusher_park_cursor: 0,
            pusher_session_coins: 0,
            pusher_session_tw: 0,
            pusher_last_chute_tw: 0,
            pusher_sfx_cd: 0.0,
            pusher_jackpot_sfx_cd: 0.0,
            pusher_neon_phase: 0.0,
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
            jazz_solo_timer: 0.0,
            coaster_u: 0.0,
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
struct PusherNeonMat;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PusherCoinPhase {
    Upper,
    Lower,
    Dropping,
    ChuteTumble,
    Parked,
}

#[derive(Component)]
struct PusherCoin {
    local: Vec3,
    vel: Vec3,
    radius: f32,
    value: u32,
    phase: PusherCoinPhase,
    spin: f32,
    is_bar: bool,
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

#[derive(Component)]
struct JazzMusician {
    role: u8, // 0 = Trumpet, 1 = Bassist, 2 = Drummer
    base_pos: Vec3,
}

#[derive(Component)]
struct JazzTrumpetPivot;

#[derive(Component)]
struct JazzDrumArm(bool);

#[derive(Component)]
struct JazzBassArm;

#[derive(Component)]
struct JazzNoteOrb {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct CoasterTrainCar(usize);

#[derive(Component)]
struct CoasterStargateRib {
    idx: usize,
    base_pos: Vec3,
    rot: Quat,
}

#[derive(Component)]
struct CoasterStrobeLight(usize);

#[derive(Component)]
struct CoasterTrippyOrb {
    base: Vec3,
    phase: f32,
    radius: f32,
}

/// Closed 22-waypoint Catmull-Rom coaster spline bursting out of the casino skylight
/// over nighttime Cedar Rapids (Downtown towers, Empire Citadel, Cedar River bridges),
/// with a Giant 360° Vertical Loop, 720° Double Corkscrew Barrel Roll, and 2001 Stargate Tunnel!
const COASTER_WAYPOINTS: [(Vec3, f32); 22] = [
    // Launch inside top of Casino -> burst up through the skylight!
    (Vec3::new(12.0, 9.5, 11.0), 0.10),
    (Vec3::new(2.0, 12.0, 2.0), 0.25),
    (Vec3::new(-10.0, 22.0, -18.0), 0.35),
    // Swoop past Empire Citadel Tower & Downtown Cedar Rapids
    (Vec3::new(16.0, 34.0, -48.0), -0.30),
    (Vec3::new(0.0, 26.0, -82.0), 0.45),
    (Vec3::new(-44.0, 22.0, -76.0), 0.35),
    // Dive low over the Cedar River, Mays Island & 1st/2nd/3rd Ave Bridges!
    (Vec3::new(-80.0, 11.0, -34.0), -0.25),
    (Vec3::new(-80.0, 10.5, 0.0), 0.15),
    (Vec3::new(-76.0, 15.0, 36.0), 0.30),
    // Giant Vertical 360° Loop over South Downtown Cedar Rapids!
    (Vec3::new(-32.0, 18.0, 58.0), 0.0),
    (Vec3::new(-32.0, 35.0, 44.0), 0.0),
    (Vec3::new(-32.0, 52.0, 58.0), 0.0),
    (Vec3::new(-32.0, 35.0, 72.0), 0.0),
    (Vec3::new(-32.0, 18.0, 58.0), 0.0),
    // 2001 Stargate Tunnel + 720° Double Corkscrew Barrel Roll high over the skyline!
    (Vec3::new(12.0, 28.0, 68.0), std::f32::consts::PI * 0.7),
    (Vec3::new(56.0, 38.0, 44.0), std::f32::consts::PI * 1.5),
    (Vec3::new(68.0, 34.0, -6.0), std::f32::consts::PI * 2.4),
    (Vec3::new(46.0, 26.0, -46.0), std::f32::consts::PI * 3.2),
    (Vec3::new(26.0, 20.0, -24.0), std::f32::consts::PI * 4.0),
    // High-speed dive back into the Casino skylight
    (Vec3::new(22.0, 15.0, 4.0), -0.35),
    (Vec3::new(17.0, 11.2, 13.5), -0.15),
    (Vec3::new(12.0, 9.5, 11.0), 0.10),
];

fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Returns `(local_pos_relative_to_casino_origin, rotation)` along the closed coaster spline.
fn coaster_sample(u: f32) -> (Vec3, Quat) {
    let n = COASTER_WAYPOINTS.len();
    let u = u.rem_euclid(1.0);
    let scaled = u * n as f32;
    let seg = (scaled.floor() as usize) % n;
    let t = scaled - scaled.floor();

    let (p0, _) = COASTER_WAYPOINTS[(seg + n - 1) % n];
    let (p1, r1) = COASTER_WAYPOINTS[seg];
    let (p2, r2) = COASTER_WAYPOINTS[(seg + 1) % n];
    let (p3, _) = COASTER_WAYPOINTS[(seg + 2) % n];

    let pos = catmull_rom(p0, p1, p2, p3, t);
    let dt = 0.005;
    let u2 = (u + dt).rem_euclid(1.0);
    let s2 = u2 * n as f32;
    let seg2 = (s2.floor() as usize) % n;
    let t2 = s2 - s2.floor();
    let next_pos = catmull_rom(
        COASTER_WAYPOINTS[(seg2 + n - 1) % n].0,
        COASTER_WAYPOINTS[seg2].0,
        COASTER_WAYPOINTS[(seg2 + 1) % n].0,
        COASTER_WAYPOINTS[(seg2 + 2) % n].0,
        t2,
    );
    let fwd = (next_pos - pos).normalize_or_zero();
    let bank = r1 + (r2 - r1) * t;
    // Handle vertical loop upside-down orientation cleanly near waypoints 9..13
    let base_up = if (9..=13).contains(&seg) && pos.y > 34.0 {
        -Vec3::Y
    } else {
        Vec3::Y
    };
    let right = fwd.cross(base_up).normalize_or_zero();
    let up = right.cross(fwd).normalize_or_zero();
    let rot = Quat::from_mat3(&Mat3::from_cols(right, up, -fwd)) * Quat::from_rotation_z(bank);
    (pos, rot)
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
    let coin_mesh = meshes.add(Cylinder::new(0.068, 0.030));
    let bar_mesh = meshes.add(Cuboid::new(0.15, 0.044, 0.095));
    let ruby_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.18, 0.34),
        emissive: LinearRgba::rgb(1.8, 0.2, 0.4),
        metallic: 0.65,
        perceptual_roughness: 0.2,
        ..default()
    });
    let diamond_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.96, 1.0),
        emissive: LinearRgba::rgb(0.5, 2.4, 3.2),
        metallic: 0.9,
        perceptual_roughness: 0.1,
        ..default()
    });
    let pusher_neon_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.84, 0.22),
        emissive: LinearRgba::rgb(2.4, 1.8, 0.3),
        unlit: true,
        ..default()
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
            // Base cabinet set slightly back so coins cascade cleanly in front of deck z=0.74
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_blue.clone()),
                Transform::from_xyz(0.0, 0.50, -0.06).with_scale(Vec3::new(1.8, 1.0, 1.56)),
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
            // Header crown + neon rails pulsing during Fever / Coin Tsunami
            p.spawn((
                PusherNeonMat,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(pusher_neon_mat.clone()),
                Transform::from_xyz(0.0, 2.98, 0.0).with_scale(Vec3::new(1.84, 0.24, 1.65)),
            ));
            for sx in [-0.89_f32, 0.89] {
                p.spawn((
                    PusherNeonMat,
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(pusher_neon_mat.clone()),
                    Transform::from_xyz(sx, 1.95, 0.76).with_scale(Vec3::new(0.06, 1.85, 0.06)),
                ));
            }
            // Visible front waterfall prize chute / payout hopper
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 0.30, 0.88)
                    .with_rotation(Quat::from_rotation_x(0.24))
                    .with_scale(Vec3::new(1.74, 0.06, 0.42)),
            ));
            p.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(glass_mat.clone()),
                Transform::from_xyz(0.0, 0.40, 1.08).with_scale(Vec3::new(1.74, 0.24, 0.04)),
            ));
            p.spawn((
                PusherNeonMat,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(pusher_neon_mat.clone()),
                Transform::from_xyz(0.0, 0.52, 1.08).with_scale(Vec3::new(1.76, 0.04, 0.05)),
            ));
            for sx in [-0.86_f32, 0.86] {
                p.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(cab_blue.clone()),
                    Transform::from_xyz(sx, 0.56, 0.90).with_scale(Vec3::new(0.06, 0.62, 0.36)),
                ));
            }
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
                Transform::from_xyz(0.0, 1.16, -0.54).with_scale(Vec3::new(1.56, 0.22, 0.72)),
            ));
            // Primary movable dropper nozzle + flanking fever ceiling chutes
            p.spawn((
                PusherDropper,
                Mesh3d(cyl_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_xyz(0.0, 2.68, -0.22).with_scale(Vec3::new(0.14, 0.24, 0.14)),
            ));
            for fx in [-0.46_f32, 0.46] {
                p.spawn((
                    PusherNeonMat,
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(pusher_neon_mat.clone()),
                    Transform::from_xyz(fx, 2.70, -0.20).with_scale(Vec3::new(0.11, 0.20, 0.11)),
                ));
            }

            // Pre-pooled 320 physical 3D coins, ruby tokens, gold bullion bars & diamond mega bars!
            let mut pool_specs: Vec<(Vec3, PusherCoinPhase, u32, f32, bool)> = Vec::with_capacity(320);
            let mut idx: usize = 0;
            // Lower deck layer 0 (48)
            for r in 0..5_usize {
                for c in 0..10_usize {
                    let x = -0.68 + c as f32 * 0.15 + if r % 2 == 1 { 0.04 } else { 0.0 };
                    let z = 0.08 + r as f32 * 0.145;
                    if x.abs() <= 0.72 && z <= 0.68 {
                        let val = if idx == 12 || idx == 33 {
                            35
                        } else if idx % 6 == 2 {
                            15
                        } else {
                            1
                        };
                        pool_specs.push((Vec3::new(x, 1.07, z), PusherCoinPhase::Lower, val, if val >= 35 { 0.072 } else { 0.068 }, val >= 35));
                        idx += 1;
                    }
                }
            }
            // Lower deck layer 1 (36)
            for r in 0..4_usize {
                for c in 0..9_usize {
                    let x = -0.62 + c as f32 * 0.155;
                    let z = 0.12 + r as f32 * 0.145;
                    let val = if idx == 58 || idx == 74 {
                        35
                    } else if idx % 6 == 1 {
                        15
                    } else {
                        1
                    };
                    pool_specs.push((Vec3::new(x, 1.11, z), PusherCoinPhase::Lower, val, if val >= 35 { 0.072 } else { 0.068 }, val >= 35));
                    idx += 1;
                }
            }
            // Upper shelf layer 0 (30)
            for r in 0..3_usize {
                for c in 0..10_usize {
                    let x = -0.68 + c as f32 * 0.15;
                    let z = -0.22 + r as f32 * 0.10;
                    let val = if idx % 5 == 0 { 15 } else { 1 };
                    pool_specs.push((Vec3::new(x, 1.29, z), PusherCoinPhase::Upper, val, 0.068, false));
                    idx += 1;
                }
            }
            // Upper shelf layer 1 (16)
            for r in 0..2_usize {
                for c in 0..8_usize {
                    let x = -0.56 + c as f32 * 0.16;
                    let z = -0.18 + r as f32 * 0.12;
                    let val = if c == 3 && r == 0 {
                        35
                    } else if c % 4 == 1 {
                        15
                    } else {
                        1
                    };
                    pool_specs.push((Vec3::new(x, 1.325, z), PusherCoinPhase::Upper, val, if val >= 35 { 0.072 } else { 0.068 }, val >= 35));
                }
            }
            // Parked reserve coins/bars up to 320 total
            let rem = 320_usize.saturating_sub(pool_specs.len());
            let gold_reserve = rem.saturating_sub(110);
            for _ in 0..gold_reserve {
                pool_specs.push((Vec3::new(0.0, -10.0, 0.0), PusherCoinPhase::Parked, 2, 0.068, false));
            }
            for _ in 0..50 {
                pool_specs.push((Vec3::new(0.0, -10.0, 0.0), PusherCoinPhase::Parked, 15, 0.068, false));
            }
            for _ in 0..40 {
                pool_specs.push((Vec3::new(0.0, -10.0, 0.0), PusherCoinPhase::Parked, 35, 0.072, true));
            }
            while pool_specs.len() < 320 {
                pool_specs.push((Vec3::new(0.0, -10.0, 0.0), PusherCoinPhase::Parked, 125, 0.072, true));
            }

            for (pos, phase, val, rad, is_bar) in pool_specs {
                let (mesh, mat) = match val {
                    125 => (bar_mesh.clone(), diamond_mat.clone()),
                    35 => (bar_mesh.clone(), neon_gold.clone()),
                    15 => (coin_mesh.clone(), ruby_mat.clone()),
                    _ => (coin_mesh.clone(), trim_gold.clone()),
                };
                let vis = if phase == PusherCoinPhase::Parked {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
                p.spawn((
                    PusherCoin {
                        local: pos,
                        vel: Vec3::ZERO,
                        radius: rad,
                        value: val,
                        phase,
                        spin: 0.0,
                        is_bar,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_translation(pos),
                    vis,
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

    // ---- 5. ART BLAKEY & THE JAZZ MESSENGERS STAGE (North-West Corner, larger 1.4x scale!) ----
    let (jz_pos, jz_yaw) = ArcadeKind::JazzBand.pose();
    let jz_world = CASINO_LEVEL_ORIGIN + jz_pos;
    let suit_charcoal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.11),
        perceptual_roughness: 0.35,
        ..default()
    });
    let suit_navy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.12, 0.28),
        perceptual_roughness: 0.32,
        ..default()
    });
    let suit_burgundy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.07, 0.14),
        perceptual_roughness: 0.35,
        ..default()
    });
    let shirt_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.94, 0.92),
        perceptual_roughness: 0.55,
        ..default()
    });
    let skin_deep = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.19, 0.12),
        perceptual_roughness: 0.52,
        ..default()
    });
    let skin_warm = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.25, 0.16),
        perceptual_roughness: 0.52,
        ..default()
    });
    let chrome_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.88, 0.92),
        metallic: 0.95,
        perceptual_roughness: 0.12,
        ..default()
    });
    let bass_wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.22, 0.08),
        perceptual_roughness: 0.22,
        clearcoat: 0.85,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Transform::from_translation(jz_world + Vec3::Y * 0.30),
        PlatformBox {
            half_extents: Vec3::new(2.7, 0.30, 1.95),
        },
    ));
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(jz_world)
                .with_rotation(Quat::from_rotation_y(jz_yaw))
                .with_scale(Vec3::splat(1.40)),
            Visibility::default(),
        ))
        .with_children(|jz| {
            // Raised walnut stage + gold trim + drum riser + velvet curtain backdrop + spotlights
            jz.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.16, 0.0).with_scale(Vec3::new(3.9, 0.32, 2.9)),
            ));
            jz.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 0.33, 0.0).with_scale(Vec3::new(4.0, 0.04, 3.0)),
            ));
            // Drum riser for Art Blakey in back center
            jz.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.43, -0.58).with_scale(Vec3::new(1.75, 0.16, 1.35)),
            ));
            jz.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cab_red.clone()),
                Transform::from_xyz(0.0, 1.65, -1.35).with_scale(Vec3::new(3.8, 2.4, 0.12)),
            ));
            jz.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_gold.clone()),
                Transform::from_xyz(0.0, 2.68, -1.26).with_scale(Vec3::new(2.6, 0.24, 0.08)),
            ));
            jz.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.78, 0.35),
                    intensity: 540_000.0,
                    range: 16.0,
                    ..default()
                },
                Transform::from_xyz(0.0, 3.4, 0.6),
            ));

            // Glowing golden Tip Jar + 2 Brass Music Stands in front
            jz.spawn((
                Mesh3d(cyl_mesh.clone()),
                MeshMaterial3d(glass_mat.clone()),
                Transform::from_xyz(0.0, 0.56, 1.24).with_scale(Vec3::new(0.22, 0.36, 0.22)),
            ));
            jz.spawn((
                Mesh3d(cyl_mesh.clone()),
                MeshMaterial3d(neon_gold.clone()),
                Transform::from_xyz(0.0, 0.45, 1.24).with_scale(Vec3::new(0.18, 0.14, 0.18)),
            ));
            for sx in [-0.95_f32, 0.95] {
                jz.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(chrome_mat.clone()), Transform::from_xyz(sx, 0.72, 0.96).with_scale(Vec3::new(0.025, 0.78, 0.025))));
                jz.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(sx, 1.12, 0.96).with_rotation(Quat::from_rotation_x(-0.55)).with_scale(Vec3::new(0.36, 0.26, 0.03))));
            }

            // Musician 0: Lee Morgan-style Trumpeter (front-left, midnight navy suit + 6-part brass trumpet)
            let t_base = Vec3::new(-0.92, 0.35, 0.48);
            jz.spawn((
                JazzMusician { role: 0, base_pos: t_base },
                Transform::from_translation(t_base),
                Visibility::default(),
            ))
            .with_children(|m| {
                // Legs, tailored navy suit jacket, white collar/shirt V, tie, pocket square, head, shades
                for lx in [-0.10_f32, 0.10] {
                    m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_navy.clone()), Transform::from_xyz(lx, 0.26, 0.0).with_scale(Vec3::new(0.09, 0.52, 0.09))));
                }
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_navy.clone()), Transform::from_xyz(0.0, 0.72, 0.0).with_scale(Vec3::new(0.42, 0.48, 0.24))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(0.0, 0.82, 0.115).with_scale(Vec3::new(0.12, 0.24, 0.03))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(-0.13, 0.84, 0.125).with_scale(Vec3::new(0.06, 0.03, 0.02))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(skin_warm.clone()), Transform::from_xyz(0.0, 1.12, 0.0).with_scale(Vec3::splat(0.18))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 1.24, -0.01).with_scale(Vec3::new(0.17, 0.09, 0.17))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 1.14, 0.15).with_scale(Vec3::new(0.24, 0.055, 0.07))));
                for sx in [-0.25_f32, 0.25] {
                    m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_navy.clone()), Transform::from_xyz(sx, 0.84, 0.14).with_rotation(Quat::from_rotation_x(-1.0)).with_scale(Vec3::new(0.065, 0.34, 0.065))));
                }
                // Detailed 6-part Brass Trumpet + Harmon Mute
                m.spawn((
                    JazzTrumpetPivot,
                    Transform::from_xyz(0.0, 1.03, 0.18),
                    Visibility::default(),
                ))
                .with_children(|tp| {
                    tp.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(trim_gold.clone()), Transform::from_xyz(0.0, 0.0, 0.24).with_rotation(Quat::from_rotation_x(FRAC_PI_2)).with_scale(Vec3::new(0.024, 0.46, 0.024))));
                    tp.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(trim_gold.clone()), Transform::from_xyz(0.0, -0.04, 0.26).with_rotation(Quat::from_rotation_x(FRAC_PI_2)).with_scale(Vec3::new(0.018, 0.24, 0.018))));
                    for vz in [0.22_f32, 0.26, 0.30] {
                        tp.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(chrome_mat.clone()), Transform::from_xyz(0.0, 0.01, vz).with_scale(Vec3::new(0.018, 0.13, 0.018))));
                    }
                    tp.spawn((Mesh3d(meshes.add(Cone::new(0.11, 0.20))), MeshMaterial3d(neon_gold.clone()), Transform::from_xyz(0.0, 0.0, 0.52).with_rotation(Quat::from_rotation_x(-FRAC_PI_2))));
                    tp.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(chrome_mat.clone()), Transform::from_xyz(0.0, 0.0, 0.59).with_scale(Vec3::new(0.055, 0.055, 0.07))));
                });
            });

            // Musician 1: Jymie Merritt-style Upright Bassist (front-right, burgundy suit, pork-pie hat, carved double bass)
            let b_base = Vec3::new(0.95, 0.35, 0.40);
            jz.spawn((
                JazzMusician { role: 1, base_pos: b_base },
                Transform::from_translation(b_base),
                Visibility::default(),
            ))
            .with_children(|m| {
                for lx in [-0.10_f32, 0.10] {
                    m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_burgundy.clone()), Transform::from_xyz(lx, 0.26, 0.0).with_scale(Vec3::new(0.09, 0.52, 0.09))));
                }
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_burgundy.clone()), Transform::from_xyz(0.0, 0.72, 0.0).with_scale(Vec3::new(0.42, 0.48, 0.24))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(0.0, 0.82, 0.115).with_scale(Vec3::new(0.11, 0.22, 0.03))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(skin_deep.clone()), Transform::from_xyz(0.0, 1.12, 0.0).with_scale(Vec3::splat(0.18))));
                // Pork-pie hat & glasses
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 1.26, 0.0).with_scale(Vec3::new(0.26, 0.035, 0.26))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 1.32, 0.0).with_scale(Vec3::new(0.17, 0.10, 0.17))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 1.14, 0.15).with_scale(Vec3::new(0.24, 0.05, 0.06))));
                // Carved hourglass Upright Double Bass + 4 gold strings + endpin
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(chrome_mat.clone()), Transform::from_xyz(0.24, 0.16, 0.30).with_scale(Vec3::new(0.02, 0.32, 0.02))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(bass_wood.clone()), Transform::from_xyz(0.24, 0.52, 0.30).with_scale(Vec3::new(0.30, 0.30, 0.16))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(bass_wood.clone()), Transform::from_xyz(0.24, 0.82, 0.30).with_scale(Vec3::new(0.23, 0.25, 0.15))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.24, 1.28, 0.30).with_scale(Vec3::new(0.06, 0.68, 0.05))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(bass_wood.clone()), Transform::from_xyz(0.24, 1.66, 0.30).with_scale(Vec3::splat(0.065))));
                for sx in [-0.02_f32, -0.007, 0.007, 0.02] {
                    m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(trim_gold.clone()), Transform::from_xyz(0.24 + sx, 0.96, 0.39).with_scale(Vec3::new(0.006, 1.18, 0.006))));
                }
                m.spawn((
                    JazzBassArm,
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(suit_burgundy.clone()),
                    Transform::from_xyz(0.28, 0.78, 0.18).with_scale(Vec3::new(0.08, 0.32, 0.08)),
                ));
            });

            // Musician 2: Art Blakey (center-back on riser, horn-rimmed glasses, vintage 5-piece Gretsch-style drum kit!)
            let d_base = Vec3::new(0.0, 0.51, -0.58);
            jz.spawn((
                JazzMusician { role: 2, base_pos: d_base },
                Transform::from_translation(d_base),
                Visibility::default(),
            ))
            .with_children(|m| {
                // Seated charcoal suit, white shirt + thin tie, horn-rimmed glasses, warm smile
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 0.54, 0.0).with_scale(Vec3::new(0.46, 0.54, 0.26))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(0.0, 0.62, 0.125).with_scale(Vec3::new(0.13, 0.28, 0.03))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 0.60, 0.145).with_scale(Vec3::new(0.035, 0.24, 0.02))));
                m.spawn((Mesh3d(sphere_mesh.clone()), MeshMaterial3d(skin_deep.clone()), Transform::from_xyz(0.0, 0.96, 0.0).with_scale(Vec3::splat(0.19))));
                m.spawn((Mesh3d(box_mesh.clone()), MeshMaterial3d(suit_charcoal.clone()), Transform::from_xyz(0.0, 0.98, 0.16).with_scale(Vec3::new(0.25, 0.05, 0.06))));
                for (side, is_l) in [(-0.28_f32, true), (0.28, false)] {
                    m.spawn((
                        JazzDrumArm(is_l),
                        Mesh3d(cyl_mesh.clone()),
                        MeshMaterial3d(suit_charcoal.clone()),
                        Transform::from_xyz(side, 0.68, 0.22).with_scale(Vec3::new(0.07, 0.30, 0.07)),
                    ))
                    .with_children(|arm| {
                        // Wooden drumstick
                        arm.spawn((
                            Mesh3d(cyl_mesh.clone()),
                            MeshMaterial3d(shirt_white.clone()),
                            Transform::from_xyz(0.0, -0.18, 0.18)
                                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                                .with_scale(Vec3::new(0.015, 0.34, 0.015)),
                        ));
                    });
                }
                // Vintage 5-piece Drum Kit: Bass drum + gold rims, Snare, Rack Tom, Floor Tom, Ride/Crash/Hi-Hat cymbals
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(cab_red.clone()), Transform::from_xyz(0.0, 0.34, 0.52).with_rotation(Quat::from_rotation_x(FRAC_PI_2)).with_scale(Vec3::new(0.36, 0.36, 0.36))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(0.0, 0.34, 0.71).with_rotation(Quat::from_rotation_x(FRAC_PI_2)).with_scale(Vec3::new(0.33, 0.02, 0.33))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(shirt_white.clone()), Transform::from_xyz(-0.26, 0.56, 0.36).with_scale(Vec3::new(0.16, 0.12, 0.16))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(cab_red.clone()), Transform::from_xyz(0.12, 0.72, 0.46).with_rotation(Quat::from_rotation_x(0.35)).with_scale(Vec3::new(0.15, 0.14, 0.15))));
                m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(cab_red.clone()), Transform::from_xyz(0.38, 0.42, 0.38).with_scale(Vec3::new(0.20, 0.30, 0.20))));
                for (cx, cy, cz, cr) in [(-0.46_f32, 0.88_f32, 0.36_f32, 0.19_f32), (-0.36, 1.05, 0.54, 0.25), (0.48, 0.96, 0.48, 0.28)] {
                    m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(chrome_mat.clone()), Transform::from_xyz(cx, cy * 0.5, cz).with_scale(Vec3::new(0.02, cy, 0.02))));
                    m.spawn((Mesh3d(cyl_mesh.clone()), MeshMaterial3d(trim_gold.clone()), Transform::from_xyz(cx, cy, cz).with_scale(Vec3::new(cr, 0.02, cr))));
                }
            });

            // Floating glowing musical note orbs rising above the stage
            for ni in 0..8_usize {
                let np = Vec3::new(-1.4 + ni as f32 * 0.40, 2.0 + (ni % 2) as f32 * 0.35, 0.1);
                jz.spawn((
                    JazzNoteOrb { base: np, phase: ni as f32 * 0.9 },
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(if ni % 2 == 0 { neon_gold.clone() } else { neon_pink.clone() }),
                    Transform::from_translation(np).with_scale(Vec3::splat(0.09)),
                ));
            }
        });

    // Overhead Stargate Roller Coaster (360 vertical loop, 720 corkscrews, 2001 slit-scan light tunnel & strobes)
    let coaster_rail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.90, 1.0),
        emissive: LinearRgba::new(0.25, 1.65, 2.8, 1.0),
        metallic: 0.90,
        perceptual_roughness: 0.14,
        ..default()
    });
    let coaster_tie_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.16, 0.78),
        emissive: LinearRgba::new(2.2, 0.25, 1.6, 1.0),
        metallic: 0.75,
        perceptual_roughness: 0.22,
        ..default()
    });
    let coaster_car_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.84, 0.18),
        emissive: LinearRgba::new(0.7, 0.5, 0.05, 1.0),
        metallic: 0.88,
        perceptual_roughness: 0.12,
        ..default()
    });

    const TRACK_SEGS: usize = 240;
    for i in 0..TRACK_SEGS {
        let u0 = i as f32 / TRACK_SEGS as f32;
        let u1 = (i + 1) as f32 / TRACK_SEGS as f32;
        let (p0, r0) = coaster_sample(u0);
        let (p1, _) = coaster_sample(u1);
        let mid = CASINO_LEVEL_ORIGIN + (p0 + p1) * 0.5;
        let seg_len = (p1 - p0).length().max(0.15) + 0.08;
        // Twin glowing neon rails
        for side in [-0.42_f32, 0.42] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(coaster_rail_mat.clone()),
                Transform::from_translation(mid + r0 * Vec3::new(side, 0.0, 0.0))
                    .with_rotation(r0)
                    .with_scale(Vec3::new(0.10, 0.10, seg_len)),
            ));
        }
        // Cross-tie spine & glowing neon tie every 2nd segment
        if i % 2 == 0 {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(coaster_tie_mat.clone()),
                Transform::from_translation(mid + r0 * Vec3::new(0.0, -0.14, 0.0))
                    .with_rotation(r0)
                    .with_scale(Vec3::new(0.98, 0.08, 0.16)),
            ));
        }
    }

    // 2001: A Space Odyssey Stargate Slit-Scan Dimensional Tunnel along the high corkscrew (u = 0.62..0.88)
    for ri in 0..36_usize {
        let u = 0.62 + (ri as f32 / 35.0) * 0.26;
        let (p, rot) = coaster_sample(u);
        let base_pos = CASINO_LEVEL_ORIGIN + p;
        let hue = (ri as f32 / 36.0) * 360.0;
        let col = Color::hsl(hue, 1.0, 0.58);
        let l = col.to_linear();
        let rib_mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 4.2, l.green * 4.2, l.blue * 4.2, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CoasterStargateRib {
                idx: ri,
                base_pos,
                rot,
            },
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(rib_mat),
            Transform::from_translation(base_pos)
                .with_rotation(rot * Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::splat(4.8)),
        ));
    }

    // Overhead & Skyline flashing strobe PointLights illuminating Cedar Rapids at night
    for (li, sp) in [
        Vec3::new(0.0, 28.0, -40.0),
        Vec3::new(-75.0, 26.0, 0.0),
        Vec3::new(52.0, 34.0, 24.0),
        Vec3::new(-28.0, 44.0, 56.0),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CoasterStrobeLight(li),
            PointLight {
                color: Color::srgb(0.2, 0.85, 1.0),
                intensity: 450_000.0,
                range: 110.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(CASINO_LEVEL_ORIGIN + sp),
        ));
    }

    // Nighttime Trippy Cedar Rapids Skyline surrounding the Casino (at y = -2.2 so it sits cleanly below casino floor)
    let cr_ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.025, 0.02, 0.06),
        perceptual_roughness: 0.78,
        ..default()
    });
    let cr_river_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.12, 0.28),
        emissive: LinearRgba::new(0.03, 0.16, 0.42, 1.0),
        metallic: 0.92,
        perceptual_roughness: 0.08,
        ..default()
    });
    let cr_bldg_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.08, 0.14),
        metallic: 0.45,
        perceptual_roughness: 0.35,
        ..default()
    });
    let cr_bldg_limestone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.12, 0.20),
        perceptual_roughness: 0.55,
        ..default()
    });
    let cr_violet_neon = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.24, 1.0),
        emissive: LinearRgba::new(1.8, 0.4, 2.8, 1.0),
        unlit: true,
        ..default()
    });
    let cr_origin = CASINO_LEVEL_ORIGIN + Vec3::new(0.0, -2.2, 0.0);

    // Midnight ground plane + 4x4 glowing Cedar Rapids neon avenue grid
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(box_mesh.clone()),
        MeshMaterial3d(cr_ground_mat),
        Transform::from_translation(cr_origin + Vec3::new(0.0, -0.5, 0.0)).with_scale(Vec3::new(280.0, 1.0, 280.0)),
    ));
    for road in [-69.0_f32, -23.0, 23.0, 69.0] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(neon_cyan.clone()),
            Transform::from_translation(cr_origin + Vec3::new(road, 0.05, 0.0)).with_scale(Vec3::new(0.55, 0.06, 260.0)),
        ));
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(neon_pink.clone()),
            Transform::from_translation(cr_origin + Vec3::new(0.0, 0.05, road)).with_scale(Vec3::new(260.0, 0.06, 0.55)),
        ));
    }

    // Cedar River (West Downtown, x = -84) + Mays Island Courthouse + 1st/2nd/3rd Ave Neon Bridges
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(box_mesh.clone()),
        MeshMaterial3d(cr_river_mat),
        Transform::from_translation(cr_origin + Vec3::new(-84.0, 0.08, 0.0)).with_scale(Vec3::new(28.0, 0.08, 270.0)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(box_mesh.clone()),
        MeshMaterial3d(cr_bldg_limestone.clone()),
        Transform::from_translation(cr_origin + Vec3::new(-84.0, 5.5, 0.0)).with_scale(Vec3::new(11.0, 11.0, 16.0)),
    ));
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(neon_gold.clone()),
        Transform::from_translation(cr_origin + Vec3::new(-84.0, 12.2, 0.0)).with_scale(Vec3::new(4.2, 3.6, 4.2)),
    ));
    for bz in [-38.0_f32, 0.0, 38.0] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(cr_bldg_dark.clone()),
            Transform::from_translation(cr_origin + Vec3::new(-84.0, 1.4, bz)).with_scale(Vec3::new(34.0, 0.6, 6.5)),
        ));
        for side in [-3.1_f32, 3.1] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_translation(cr_origin + Vec3::new(-84.0, 2.2, bz + side)).with_scale(Vec3::new(34.0, 0.25, 0.25)),
            ));
        }
    }

    // Downtown Cedar Rapids Skyscrapers (outside the 46x46 casino box) with lit nighttime window bands & rooftop beacons
    let towers: &[(f32, f32, f32, f32, f32)] = &[
        (-44.0, -44.0, 16.0, 36.0, 16.0),
        (44.0, -44.0, 18.0, 44.0, 18.0),
        (46.0, 44.0, 16.0, 38.0, 16.0),
        (-46.0, 44.0, 18.0, 34.0, 18.0),
        (-12.0, -46.0, 15.0, 30.0, 15.0),
        (16.0, -46.0, 14.0, 40.0, 14.0),
        (48.0, -10.0, 18.0, 32.0, 20.0),
        (48.0, 18.0, 15.0, 46.0, 15.0),
        (-48.0, -14.0, 14.0, 28.0, 16.0),
        (-48.0, 16.0, 16.0, 42.0, 16.0),
        (-14.0, 48.0, 16.0, 32.0, 15.0),
        (18.0, 48.0, 15.0, 48.0, 15.0),
        (82.0, -42.0, 18.0, 40.0, 18.0),
        (82.0, 0.0, 20.0, 52.0, 20.0),
        (82.0, 44.0, 18.0, 36.0, 18.0),
        (44.0, 84.0, 18.0, 42.0, 18.0),
        (0.0, 86.0, 20.0, 34.0, 18.0),
        (-44.0, 84.0, 18.0, 38.0, 18.0),
        (46.0, -84.0, 18.0, 44.0, 18.0),
        (-112.0, -30.0, 18.0, 30.0, 18.0),
        (-112.0, 26.0, 18.0, 38.0, 18.0),
    ];
    for (ti, &(bx, bz, bw, bh, bd)) in towers.iter().enumerate() {
        let body_mat = if ti % 2 == 0 { cr_bldg_dark.clone() } else { cr_bldg_limestone.clone() };
        let win_mat = match ti % 4 {
            0 => neon_gold.clone(),
            1 => neon_cyan.clone(),
            2 => neon_pink.clone(),
            _ => cr_violet_neon.clone(),
        };
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(body_mat),
            Transform::from_translation(cr_origin + Vec3::new(bx, bh * 0.5, bz)).with_scale(Vec3::new(bw, bh, bd)),
        ));
        // Glowing nighttime window bands & rooftop neon crown
        for wy in [bh * 0.28, bh * 0.52, bh * 0.76, bh - 0.8] {
            commands.spawn((
                LevelEntity,
                CasinoLevelEntity,
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(win_mat.clone()),
                Transform::from_translation(cr_origin + Vec3::new(bx, wy, bz)).with_scale(Vec3::new(bw + 0.35, 0.55, bd + 0.35)),
            ));
        }
    }

    // Iconic Tiered Empire Citadel Tower (0, -82) & Quaker Oats Twin Silos (-46, -74)
    for (ty, tw, th) in [(16.0_f32, 20.0_f32, 32.0_f32), (39.0, 14.0, 16.0), (52.0, 8.0, 12.0)] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(cr_bldg_dark.clone()),
            Transform::from_translation(cr_origin + Vec3::new(0.0, ty, -82.0)).with_scale(Vec3::new(tw, th, tw)),
        ));
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(box_mesh.clone()),
            MeshMaterial3d(neon_gold.clone()),
            Transform::from_translation(cr_origin + Vec3::new(0.0, ty + th * 0.5 - 0.5, -82.0)).with_scale(Vec3::new(tw + 0.5, 0.6, tw + 0.5)),
        ));
    }
    commands.spawn((
        LevelEntity,
        CasinoLevelEntity,
        Mesh3d(cyl_mesh.clone()),
        MeshMaterial3d(neon_pink.clone()),
        Transform::from_translation(cr_origin + Vec3::new(0.0, 65.0, -82.0)).with_scale(Vec3::new(0.45, 16.0, 0.45)),
    ));
    for sx in [-51.0_f32, -41.0] {
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(cyl_mesh.clone()),
            MeshMaterial3d(cr_bldg_limestone.clone()),
            Transform::from_translation(cr_origin + Vec3::new(sx, 18.0, -74.0)).with_scale(Vec3::new(4.6, 36.0, 4.6)),
        ));
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            Mesh3d(cyl_mesh.clone()),
            MeshMaterial3d(neon_cyan.clone()),
            Transform::from_translation(cr_origin + Vec3::new(sx, 34.5, -74.0)).with_scale(Vec3::new(4.9, 1.0, 4.9)),
        ));
    }

    // 72 Trippy Nighttime Sky Stars & Floating Psychedelic Orbs
    for si in 0..72_usize {
        let a = si as f32 * 2.39996;
        let dist = 38.0 + (si % 9) as f32 * 13.5;
        let base = CASINO_LEVEL_ORIGIN + Vec3::new(a.cos() * dist, 28.0 + (si % 7) as f32 * 9.5, a.sin() * dist);
        let col = Color::hsl((si as f32 * 37.0) % 360.0, 1.0, 0.62);
        let l = col.to_linear();
        let orb_mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: LinearRgba::new(l.red * 3.5, l.green * 3.5, l.blue * 3.5, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            LevelEntity,
            CasinoLevelEntity,
            CoasterTrippyOrb {
                base,
                phase: si as f32 * 0.45,
                radius: 0.75 + (si % 4) as f32 * 0.55,
            },
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(orb_mat),
            Transform::from_translation(base).with_scale(Vec3::splat(1.1)),
        ));
    }

    // Stargate Roller Coaster Boarding Arch & Glowing 3D "COASTER" Marquee Sign
    let (rc_pos, rc_yaw) = ArcadeKind::RollerCoaster.pose();
    let rc_world = CASINO_LEVEL_ORIGIN + rc_pos;
    commands
        .spawn((
            LevelEntity,
            CasinoLevelEntity,
            Transform::from_translation(rc_world).with_rotation(Quat::from_rotation_y(rc_yaw)),
            Visibility::default(),
        ))
        .with_children(|arch| {
            // Twin neon boarding pillars flanking the boarding zone
            for px in [-1.95_f32, 1.95] {
                arch.spawn((
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(cr_bldg_dark.clone()),
                    Transform::from_xyz(px, 1.8, -1.15).with_scale(Vec3::new(0.24, 3.6, 0.24)),
                ));
                arch.spawn((
                    Mesh3d(cyl_mesh.clone()),
                    MeshMaterial3d(neon_cyan.clone()),
                    Transform::from_xyz(px, 1.8, -1.03).with_scale(Vec3::new(0.08, 3.4, 0.08)),
                ));
                arch.spawn((
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(neon_gold.clone()),
                    Transform::from_xyz(px, 3.7, -1.15).with_scale(Vec3::splat(0.22)),
                ));
            }
            // Dark obsidian marquee backing + gold frame + glowing pink trim
            arch.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(cr_bldg_dark.clone()),
                Transform::from_xyz(0.0, 3.35, -1.15).with_scale(Vec3::new(4.4, 1.35, 0.24)),
            ));
            arch.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(trim_gold.clone()),
                Transform::from_xyz(0.0, 4.06, -1.15).with_scale(Vec3::new(4.55, 0.10, 0.30)),
            ));
            arch.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_pink.clone()),
                Transform::from_xyz(0.0, 2.64, -1.15).with_scale(Vec3::new(4.55, 0.10, 0.30)),
            ));
            // Halo Stargate ring + display mini-coaster car perched on top of the sign
            arch.spawn((
                Mesh3d(ring_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_xyz(0.0, 4.55, -1.15).with_scale(Vec3::new(0.95, 0.95, 0.35)),
            ));
            arch.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(coaster_car_mat.clone()),
                Transform::from_xyz(0.0, 4.35, -1.15)
                    .with_rotation(Quat::from_rotation_z(0.18))
                    .with_scale(Vec3::new(0.95, 0.34, 0.50)),
            ));
            // Downward neon pointer arrow above the boarding ring
            arch.spawn((
                Mesh3d(box_mesh.clone()),
                MeshMaterial3d(neon_cyan.clone()),
                Transform::from_xyz(0.0, 2.28, -0.55).with_scale(Vec3::new(0.14, 0.42, 0.14)),
            ));
            arch.spawn((
                Mesh3d(meshes.add(Cone::new(0.24, 0.32))),
                MeshMaterial3d(neon_gold.clone()),
                Transform::from_xyz(0.0, 1.98, -0.55).with_rotation(Quat::from_rotation_x(PI)),
            ));

            // Crisp 3D voxel-block letters spelling "COASTER" on both front & back of the marquee
            const GLYPHS: [&[&str]; 7] = [
                // C
                &["1111", "1000", "1000", "1000", "1111"],
                // O
                &["1111", "1001", "1001", "1001", "1111"],
                // A
                &["0110", "1001", "1111", "1001", "1001"],
                // S
                &["1111", "1000", "1111", "0001", "1111"],
                // T
                &["1111", "0110", "0110", "0110", "0110"],
                // E
                &["1111", "1000", "1110", "1000", "1111"],
                // R
                &["1110", "1001", "1110", "1010", "1001"],
            ];
            let cell = 0.125_f32;
            let total_w = 7.0 * 4.0 * cell + 6.0 * (cell * 0.9);
            let start_x = -total_w * 0.5 + cell * 0.5;
            for (li, rows) in GLYPHS.iter().enumerate() {
                let lx_base = start_x + li as f32 * (4.0 * cell + cell * 0.9);
                for (ry, row) in rows.iter().enumerate() {
                    let py = 3.35 + (2.0 - ry as f32) * (cell * 1.15);
                    for (cx, ch) in row.bytes().enumerate() {
                        if ch == b'1' {
                            let px = lx_base + cx as f32 * cell;
                            for z_face in [-0.14_f32, 0.14] {
                                arch.spawn((
                                    Mesh3d(box_mesh.clone()),
                                    MeshMaterial3d(neon_gold.clone()),
                                    Transform::from_xyz(px, py, -1.15 + z_face)
                                        .with_scale(Vec3::new(cell * 0.92, cell * 1.05, 0.06)),
                                ));
                            }
                        }
                    }
                }
            }
        });

    // 4-car neon coaster train
    for ci in 0..4_usize {
        let (p, rot) = coaster_sample(0.0);
        commands
            .spawn((
                LevelEntity,
                CasinoLevelEntity,
                CoasterTrainCar(ci),
                Transform::from_translation(CASINO_LEVEL_ORIGIN + p).with_rotation(rot),
                Visibility::default(),
            ))
            .with_children(|car| {
                car.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(if ci == 0 { coaster_car_mat.clone() } else { neon_pink.clone() }),
                    Transform::from_xyz(0.0, 0.32, 0.0).with_scale(Vec3::new(1.05, 0.44, 1.48)),
                ));
                car.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(neon_cyan.clone()),
                    Transform::from_xyz(0.0, 0.58, -0.35).with_scale(Vec3::new(0.92, 0.24, 0.22)),
                ));
                car.spawn((
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(neon_gold.clone()),
                    Transform::from_xyz(0.0, 0.45, 0.72).with_scale(Vec3::new(0.38, 0.16, 0.16)),
                ));
            });
    }

    // Floor interaction rings for all 4 arcade machines + Jazz Band + Stargate Roller Coaster
    for (kind, color) in [
        (ArcadeKind::CoinPusher, Color::srgb(1.0, 0.84, 0.18)),
        (ArcadeKind::ClawMachine, Color::srgb(1.0, 0.32, 0.80)),
        (ArcadeKind::Plinko, Color::srgb(0.22, 0.92, 1.0)),
        (ArcadeKind::Skeeball, Color::srgb(0.45, 1.0, 0.45)),
        (ArcadeKind::JazzBand, Color::srgb(1.0, 0.72, 0.16)),
        (ArcadeKind::RollerCoaster, Color::srgb(0.45, 0.85, 1.0)),
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
    net: Res<crate::net::NetState>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    mut arcade: ResMut<ArcadeState>,
    mut next_state: crate::transition::SceneTransition,
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

    let sq = crate::audio::net_squad_info();
    let in_club_peers = sq[5];
    let online_players = net.online_player_count().saturating_sub(in_club_peers).max(1);
    let coaster_cost = coaster_ride_cost(online_players);
    let label: String = if kind == ArcadeKind::RollerCoaster {
        if online_players > 1 {
            format!(
                "RIDE STARGATE COASTER ({} TW - {} ONLINE PLAYERS!) [F/E]",
                format_tw_commas(coaster_cost),
                online_players
            )
        } else {
            format!(
                "RIDE STARGATE COASTER ({} TW!) [F/E]",
                format_tw_commas(coaster_cost)
            )
        }
    } else {
        kind.prompt().to_string()
    };
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
                        Text::new(label.clone()),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    } else if let Ok(mut t) = text_q.get_single_mut() {
        if t.0 != label {
            t.0 = label.clone();
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
    if kind == ArcadeKind::JazzBand {
        if stats.spend_tendy_wendies(5) {
            arcade.jazz_solo_timer = 8.5;
            play_sfx("jazz_solo");
            stats.hint_text = "JAZZ SOLO! You tipped the band 5 TW - the trumpet player rips a brass solo!".to_string();
        } else {
            play_sfx("hit");
            stats.hint_text = "Need 5 TW to tip the Jazz Band! (Cashier gives +25 TW below 25 TW)".to_string();
        }
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        return;
    }
    if kind == ArcadeKind::RollerCoaster {
        if stats.spend_tendy_wendies(coaster_cost) {
            play_sfx("portal_suck");
            crate::audio::net_event(crate::net::EVENT_COASTER_WARP_ALL, 1);
            next_state.set(crate::state::GameState::CoasterCutscene);
        } else {
            play_sfx("hit");
            stats.hint_text = if online_players > 1 {
                format!(
                    "Need {} TW ({} online players x 100,000 TW) to ride the Stargate Coaster! (You have {} TW)",
                    format_tw_commas(coaster_cost),
                    online_players,
                    format_tw_commas(stats.tendy_wendies)
                )
            } else {
                format!(
                    "Need {} TW to ride the Stargate Coaster! (You have {} TW)",
                    format_tw_commas(coaster_cost),
                    format_tw_commas(stats.tendy_wendies)
                )
            };
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        return;
    }

    stats.arcade_open = true;
    arcade.active = Some(kind);
    if kind == ArcadeKind::CoinPusher {
        arcade.pusher_session_coins = 0;
        arcade.pusher_session_tw = 0;
        arcade.pusher_last_chute_tw = 0;
    }
    arcade.status_msg = match kind {
        ArcadeKind::CoinPusher => "Aim [A/D] | MASH/HOLD [SPACE] (1 TW) | FEVER RAIN [B] (3 TW)".to_string(),
        ArcadeKind::ClawMachine => "Move Claw [WASD] | Drop Claw (5 TW) [SPACE]".to_string(),
        ArcadeKind::Plinko => "Slide Puck [A/D] | Drop Plinko (5 TW) [SPACE]".to_string(),
        ArcadeKind::Skeeball => "Aim [A/D] | Launch Ball (5 TW / 3 Balls) [SPACE]".to_string(),
        ArcadeKind::JazzBand => String::new(),
        ArcadeKind::RollerCoaster => "2001 STARGATE TRIP! Enjoy the loops & corkscrews! [ESC to Exit]".to_string(),
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
                    (ArcadeHudBtn::Extra, "FEVER RAIN [B]", Color::srgb(0.56, 0.34, 0.12)),
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
    plinko_assets: Option<Res<PlinkoAssets>>,
    skee_assets: Option<Res<SkeeballAssets>>,
    cab_q: Query<(Entity, &Transform), Without<PusherCoin>>,
    hud_q: Query<Entity, With<ArcadeHudRoot>>,
    btn_q: Query<(&ArcadeHudBtn, &Interaction)>,
    mut prev_hud: Local<(bool, bool, bool)>,
    mut pusher_coin_q: Query<(Entity, &mut PusherCoin, &mut Transform, &mut Visibility), Without<ArcadeHudBtn>>,
) {
    let Some(kind) = arcade.active else {
        *prev_hud = (false, false, false);
        return;
    };
    let dt = time.delta_secs().min(0.05);

    let mut cur_hud = (false, false, false);
    for (bk, inter) in &btn_q {
        if *inter != Interaction::Pressed {
            continue;
        }
        match bk {
            ArcadeHudBtn::Action => cur_hud.0 = true,
            ArcadeHudBtn::Extra => cur_hud.1 = true,
            ArcadeHudBtn::Leave => cur_hud.2 = true,
        }
    }
    let btn_tap_act = cur_hud.0 && !prev_hud.0;
    let btn_tap_extra = cur_hud.1 && !prev_hud.1;
    let btn_tap_leave = cur_hud.2 && !prev_hud.2;
    let btn_hold_act = cur_hud.0;
    let btn_hold_extra = cur_hud.1;
    *prev_hud = cur_hud;

    let leave = keys.just_pressed(KeyCode::Escape) || btn_tap_leave;
    let act = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || btn_tap_act;
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
            arcade.pusher_aim_x = (arcade.pusher_aim_x + ax * 1.25 * dt).clamp(-0.62, 0.62);
            let tap_act = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || btn_tap_act;
            let tap_extra = keys.just_pressed(KeyCode::KeyB) || btn_tap_extra;
            let hold_act = keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::Enter) || btn_hold_act;
            let hold_extra = keys.pressed(KeyCode::KeyB) || btn_hold_extra;
            let want_extra = tap_extra || hold_extra;
            let want_drop = want_extra || tap_act || hold_act;
            let is_fresh_tap = tap_act || tap_extra;
            if want_drop && (arcade.pusher_cooldown <= 0.0 || (is_fresh_tap && arcade.pusher_cooldown <= 0.045)) {
                let mut cost = if want_extra { 3 } else { 1 };
                if !stats.spend_tendy_wendies(cost) {
                    if cost > 1 && stats.spend_tendy_wendies(1) {
                        cost = 1;
                    } else {
                        cost = 0;
                    }
                }
                if cost > 0 {
                    arcade.pusher_cooldown = 0.075;
                    let rapid_b = want_extra;
                    arcade.pusher_heat = (arcade.pusher_heat + if rapid_b { 0.068 } else { 0.050 }).min(1.0);
                    let spawn_n: usize = (if rapid_b { 5 } else { 3 }) + (arcade.pusher_heat * 5.0) as usize;
                    if arcade.pusher_sfx_cd <= 0.0 {
                        play_sfx("chip_bet");
                        arcade.pusher_sfx_cd = 0.09;
                    }

                    let want_high = arcade.pusher_heat >= 0.30;
                    let mut pool: Vec<(Entity, u32)> = Vec::new();
                    for (e, c, _, _) in &pusher_coin_q {
                        if c.phase == PusherCoinPhase::Parked {
                            pool.push((e, c.value));
                        }
                    }
                    let total_parked = pool.len();
                    let mut dropped_now = 0_usize;
                    if total_parked > 0 {
                        let start = arcade.pusher_park_cursor % total_parked;
                        arcade.pusher_park_cursor = arcade.pusher_park_cursor.wrapping_add(spawn_n);
                        let mut chosen: Vec<Entity> = Vec::with_capacity(spawn_n);
                        for pass in 0..2 {
                            for k in 0..total_parked {
                                let (e, val) = pool[(start + k) % total_parked];
                                if chosen.contains(&e) {
                                    continue;
                                }
                                if pass == 0 {
                                    if !want_high && val > 15 {
                                        continue;
                                    }
                                    if arcade.pusher_heat >= 0.65 && arcade.next_f32() < 0.45 && val < 15 {
                                        continue;
                                    }
                                }
                                chosen.push(e);
                                if chosen.len() >= spawn_n {
                                    break;
                                }
                            }
                            if chosen.len() >= spawn_n {
                                break;
                            }
                        }
                        let fever_spread = 0.35 + arcade.pusher_heat * 1.05;
                        for (ci, target_e) in chosen.into_iter().enumerate() {
                            let jx = (arcade.next_f32() - 0.5) * fever_spread;
                            let ly = 2.45 + arcade.next_f32() * 0.25 + (ci as f32) * 0.03;
                            let lz = -0.26 + arcade.next_f32() * 0.20;
                            let vx = (arcade.next_f32() - 0.5) * 0.40;
                            let vy = -1.6 - arcade.next_f32() * 0.80;
                            let vz = 0.40 + arcade.next_f32() * 0.30;
                            let spin = (arcade.next_f32() - 0.5) * 10.0;
                            let yaw = (arcade.next_f32() - 0.5) * 0.6;
                            if let Ok((_, mut c, mut tf, mut vis)) = pusher_coin_q.get_mut(target_e) {
                                let chute_x = if arcade.pusher_heat > 0.35 && ci % 3 == 1 {
                                    -0.46
                                } else if arcade.pusher_heat > 0.35 && ci % 3 == 2 {
                                    0.46
                                } else {
                                    arcade.pusher_aim_x
                                };
                                let lx = (chute_x + jx).clamp(-0.70, 0.70);
                                c.local = Vec3::new(lx, ly, lz);
                                c.vel = Vec3::new(vx, vy, vz);
                                c.phase = PusherCoinPhase::Dropping;
                                c.spin = spin;
                                tf.translation = c.local;
                                tf.rotation = if c.is_bar {
                                    Quat::from_rotation_y(yaw)
                                } else {
                                    Quat::IDENTITY
                                };
                                *vis = Visibility::Inherited;
                                dropped_now += 1;
                            }
                        }
                    }
                    let tier = if arcade.pusher_heat >= 0.75 {
                        "TSUNAMI 4X!"
                    } else if arcade.pusher_heat >= 0.35 {
                        "FEVER 2X!"
                    } else {
                        "PUSHING!"
                    };
                    arcade.status_msg = format!(
                        "{} Dropped {} | Session: +{} TW ({} pushed)",
                        tier, dropped_now, arcade.pusher_session_tw, arcade.pusher_session_coins
                    );
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
        ArcadeKind::JazzBand | ArcadeKind::RollerCoaster => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn step_coin_pusher_physics(
    time: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut arcade: ResMut<ArcadeState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shelf_q: Query<&mut Transform, (With<PusherShelf>, Without<PusherDropper>, Without<PusherCoin>)>,
    mut drop_q: Query<&mut Transform, (With<PusherDropper>, Without<PusherShelf>, Without<PusherCoin>)>,
    neon_q: Query<&MeshMaterial3d<StandardMaterial>, With<PusherNeonMat>>,
    mut coin_q: Query<(Entity, &mut PusherCoin, &mut Transform, &mut Visibility), (Without<PusherShelf>, Without<PusherDropper>)>,
) {
    let dt = time.delta_secs().min(0.05);
    arcade.pusher_heat = (arcade.pusher_heat - 0.18 * dt).max(0.0);
    arcade.pusher_cooldown = (arcade.pusher_cooldown - dt).max(0.0);
    arcade.pusher_sfx_cd = (arcade.pusher_sfx_cd - dt).max(0.0);
    arcade.pusher_jackpot_sfx_cd = (arcade.pusher_jackpot_sfx_cd - dt).max(0.0);

    let heat = arcade.pusher_heat;
    let omega = 2.4 + heat * 5.4;
    let prev_phase = arcade.pusher_phase;
    arcade.pusher_phase += dt * omega;
    arcade.pusher_neon_phase += dt * (2.5 + heat * 18.0);

    let prev_z = -0.54 + prev_phase.sin() * 0.18;
    let cur_z = -0.54 + arcade.pusher_phase.sin() * 0.18;
    let dz = (cur_z - prev_z).max(0.0);
    let pusher_front_z = cur_z + 0.36;

    for mut tf in &mut shelf_q {
        tf.translation.z = cur_z;
    }
    for mut tf in &mut drop_q {
        tf.translation.x = arcade.pusher_aim_x;
    }
    if let Some(mat_h) = neon_q.iter().next() {
        if let Some(mat) = materials.get_mut(&mat_h.0) {
            if heat > 0.35 {
                let hue = ((arcade.pusher_neon_phase * 55.0) % 360.0 + 360.0) % 360.0;
                let col = Color::hsl(hue, 1.0, 0.58);
                let l = col.to_linear();
                let boost = 2.8 + heat * 3.0 * (arcade.pusher_neon_phase.sin() * 0.5 + 0.5);
                mat.base_color = col;
                mat.emissive = LinearRgba::new(l.red * boost, l.green * boost, l.blue * boost, 1.0);
            } else {
                mat.base_color = Color::srgb(1.0, 0.84, 0.22);
                mat.emissive = LinearRgba::rgb(2.4, 1.8, 0.3);
            }
        }
    }

    #[derive(Clone, Copy)]
    struct SimCoin {
        entity: Entity,
        local: Vec3,
        vel: Vec3,
        radius: f32,
        phase: PusherCoinPhase,
        spin: f32,
    }

    let mut coins: Vec<SimCoin> = Vec::with_capacity(320);
    for (e, c, _, _) in &coin_q {
        if c.phase == PusherCoinPhase::Parked {
            continue;
        }
        coins.push(SimCoin {
            entity: e,
            local: c.local,
            vel: c.vel,
            radius: c.radius,
            phase: c.phase,
            spin: c.spin,
        });
    }

    // 1. Gravity, upper shelf sweep, step cascade, and front chute waterfall
    for c in &mut coins {
        match c.phase {
            PusherCoinPhase::Parked => {}
            PusherCoinPhase::Dropping => {
                c.vel.y -= 11.5 * dt;
                c.local += c.vel * dt;
                c.local.x = c.local.x.clamp(-0.72, 0.72);
                c.local.z = c.local.z.max(-0.52);
                let layer_off = if (c.entity.index() % 5) < 2 { 0.035 } else { 0.0 };
                if c.local.z < 0.02 && c.local.y <= 1.29 + layer_off && c.local.y >= 1.10 {
                    c.local.y = 1.29 + layer_off;
                    c.vel = Vec3::ZERO;
                    c.phase = PusherCoinPhase::Upper;
                } else if c.local.z >= 0.02 && c.local.y <= 1.07 + layer_off {
                    c.local.y = 1.07 + layer_off;
                    c.vel = Vec3::ZERO;
                    c.phase = PusherCoinPhase::Lower;
                }
            }
            PusherCoinPhase::ChuteTumble => {
                c.vel.y -= 9.5 * dt;
                c.local += c.vel * dt;
                c.local.x = c.local.x.clamp(-0.76, 0.76);
                c.local.z = c.local.z.clamp(0.75, 1.00);
            }
            PusherCoinPhase::Upper => {
                if c.local.z < pusher_front_z + c.radius {
                    c.local.z = pusher_front_z + c.radius;
                }
                if c.local.z >= 0.02 {
                    let jitter = ((c.entity.index() % 7) as f32 - 3.0) * 0.04;
                    c.phase = PusherCoinPhase::Dropping;
                    c.vel = Vec3::new(jitter, -0.25, 0.55 + heat * 0.50);
                }
                c.local.x = c.local.x.clamp(-0.72, 0.72);
            }
            PusherCoinPhase::Lower => {
                if dz > 0.0 && c.local.z < 0.02 + c.radius * 2.0 + dz {
                    c.local.z += dz * (0.95 + heat * 0.45);
                }
                if c.local.z > 0.74 {
                    let jx = ((c.entity.index() % 9) as f32 - 4.0) * 0.05;
                    let vz = 0.65 + ((c.entity.index() % 5) as f32) * 0.08;
                    c.phase = PusherCoinPhase::ChuteTumble;
                    c.vel = Vec3::new(jx, -0.30, vz);
                    c.spin = (((c.entity.index() % 7) as f32) - 3.0) * 4.5;
                }
                c.local.x = c.local.x.clamp(-0.72, 0.72);
            }
        }
    }

    // 2. Multi-pass planar separation with fast AABB skip for dense non-stalling cascades
    let n = coins.len();
    for _ in 0..3 {
        for i in 0..n {
            if !matches!(coins[i].phase, PusherCoinPhase::Upper | PusherCoinPhase::Lower) {
                continue;
            }
            for j in (i + 1)..n {
                if coins[j].phase != coins[i].phase || (coins[i].local.y - coins[j].local.y).abs() > 0.025 {
                    continue;
                }
                let dx = coins[j].local.x - coins[i].local.x;
                let ddz = coins[j].local.z - coins[i].local.z;
                let md = (coins[i].radius + coins[j].radius) * 1.02;
                if dx.abs() >= md || ddz.abs() >= md {
                    continue;
                }
                let d2 = dx * dx + ddz * ddz;
                if d2 > 1e-8 && d2 < md * md {
                    let len = d2.sqrt();
                    let push = (md - len) * 0.60 / len;
                    coins[i].local.x -= dx * push;
                    coins[i].local.z -= ddz * push;
                    coins[j].local.x += dx * push;
                    coins[j].local.z += ddz * push;
                    if coins[i].phase == PusherCoinPhase::Lower && coins[i].local.z > 0.74 {
                        coins[i].phase = PusherCoinPhase::ChuteTumble;
                        coins[i].vel = Vec3::new(0.0, -0.30, 0.65);
                    }
                    if coins[j].phase == PusherCoinPhase::Lower && coins[j].local.z > 0.74 {
                        coins[j].phase = PusherCoinPhase::ChuteTumble;
                        coins[j].vel = Vec3::new(0.0, -0.30, 0.65);
                    }
                }
            }
        }
    }

    // 3. Apply transforms, recycle tumbled coins & pay Fever/Tsunami batch rewards
    let mut batch_coins = 0_u32;
    let mut batch_tw = 0_u32;
    let mut max_single_payout = 0_u32;
    let fever_mult = if heat >= 0.75 {
        4
    } else if heat >= 0.35 {
        2
    } else {
        1
    };

    for sim in coins {
        if let Ok((_, mut c, mut tf, mut vis)) = coin_q.get_mut(sim.entity) {
            c.local = sim.local;
            c.vel = sim.vel;
            c.phase = sim.phase;
            c.spin = sim.spin;

            if c.phase == PusherCoinPhase::ChuteTumble && c.local.y <= 0.32 {
                let payout = c.value * fever_mult;
                batch_coins += 1;
                batch_tw += payout;
                max_single_payout = max_single_payout.max(payout);

                c.phase = PusherCoinPhase::Parked;
                c.local = Vec3::new(0.0, -10.0, 0.0);
                c.vel = Vec3::ZERO;
                tf.translation = c.local;
                tf.rotation = Quat::IDENTITY;
                *vis = Visibility::Hidden;
            } else {
                tf.translation = c.local;
                if matches!(c.phase, PusherCoinPhase::Dropping | PusherCoinPhase::ChuteTumble) {
                    let t = arcade.pusher_neon_phase + c.spin;
                    tf.rotation = Quat::from_rotation_x(t * 1.3) * Quat::from_rotation_z(t * 0.7);
                } else if !c.is_bar {
                    tf.rotation = Quat::IDENTITY;
                }
            }
        }
    }

    if batch_coins > 0 && arcade.active == Some(ArcadeKind::CoinPusher) {
        stats.add_tendy_wendies(batch_tw);
        arcade.pusher_session_coins += batch_coins;
        arcade.pusher_session_tw += batch_tw;
        arcade.pusher_last_chute_tw = batch_tw;
        let mode_tag = if fever_mult >= 4 {
            "COIN TSUNAMI 4X!"
        } else if fever_mult >= 2 {
            "FEVER CASCADE 2X!"
        } else {
            "COIN CASCADE!"
        };
        arcade.status_msg = format!(
            "{} +{} TW ({} coins) | Total: +{} TW ({} pushed)",
            mode_tag, batch_tw, batch_coins, arcade.pusher_session_tw, arcade.pusher_session_coins
        );
        if max_single_payout >= 30 && arcade.pusher_jackpot_sfx_cd <= 0.0 {
            play_sfx("slot_jackpot");
            arcade.pusher_jackpot_sfx_cd = 0.22;
        } else if arcade.pusher_sfx_cd <= 0.0 {
            play_sfx("tendy");
            arcade.pusher_sfx_cd = 0.06;
        }
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

fn animate_jazz_band(
    time: Res<Time<Real>>,
    mut arcade: ResMut<ArcadeState>,
    mut mus_q: Query<(&JazzMusician, &mut Transform)>,
    mut trp_q: Query<&mut Transform, (With<JazzTrumpetPivot>, Without<JazzMusician>, Without<JazzDrumArm>, Without<JazzBassArm>, Without<JazzNoteOrb>)>,
    mut drum_q: Query<(&JazzDrumArm, &mut Transform), (Without<JazzMusician>, Without<JazzTrumpetPivot>, Without<JazzBassArm>, Without<JazzNoteOrb>)>,
    mut bass_q: Query<&mut Transform, (With<JazzBassArm>, Without<JazzMusician>, Without<JazzTrumpetPivot>, Without<JazzDrumArm>, Without<JazzNoteOrb>)>,
    mut note_q: Query<(&JazzNoteOrb, &mut Transform), (Without<JazzMusician>, Without<JazzTrumpetPivot>, Without<JazzDrumArm>, Without<JazzBassArm>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    if arcade.jazz_solo_timer > 0.0 {
        arcade.jazz_solo_timer = (arcade.jazz_solo_timer - dt).max(0.0);
    }
    let solo = arcade.jazz_solo_timer > 0.0;

    for (m, mut tf) in &mut mus_q {
        let sway = match m.role {
            0 if solo => {
                // Trumpet player steps forward and grooves hard during his solo!
                tf.translation = m.base_pos + Vec3::new((t * 3.8).sin() * 0.14, (t * 7.6).sin().abs() * 0.08, 0.28);
                (t * 3.8).sin() * 0.24
            }
            0 => {
                tf.translation = m.base_pos + Vec3::new(0.0, (t * 3.6).sin().abs() * 0.03, 0.0);
                (t * 1.9).sin() * 0.10
            }
            1 => {
                tf.translation = m.base_pos + Vec3::new(0.0, (t * 3.8).sin().abs() * 0.03, 0.0);
                (t * 1.9 + 0.8).sin() * 0.12
            }
            _ => {
                tf.translation = m.base_pos + Vec3::new(0.0, (t * 7.6).sin().abs() * 0.025, 0.0);
                (t * 3.8).sin() * 0.08
            }
        };
        tf.rotation = Quat::from_rotation_y(sway);
    }

    for mut tf in &mut trp_q {
        let tilt = if solo {
            -0.38 - (t * 5.2).sin().abs() * 0.28
        } else {
            -0.06 + (t * 2.2).sin() * 0.08
        };
        tf.rotation = Quat::from_rotation_x(tilt) * Quat::from_rotation_z(if solo { (t * 4.4).cos() * 0.16 } else { 0.0 });
    }
    for (da, mut tf) in &mut drum_q {
        let ph = if da.0 { 0.0 } else { PI };
        let spd = if solo { 14.0 } else { 9.5 };
        tf.rotation = Quat::from_rotation_x(-0.35 + (t * spd + ph).sin() * 0.42);
    }
    for mut tf in &mut bass_q {
        tf.rotation = Quat::from_rotation_z((t * 5.4).sin() * 0.28);
    }
    for (no, mut tf) in &mut note_q {
        let spd = if solo { 2.4 } else { 0.9 };
        let cycle = (t * spd + no.phase) % 1.6;
        tf.translation = no.base + Vec3::new((t * 3.0 + no.phase).sin() * 0.14, cycle * 0.85, 0.0);
        let sc = if solo { 0.13 } else { 0.08 };
        tf.scale = Vec3::splat(sc * (1.0 - (cycle / 1.6) * 0.5));
    }
}

fn update_arcade_camera(
    time: Res<Time<Real>>,
    stats: Res<PlayerStats>,
    arcade: Res<ArcadeState>,
    mut player_q: Query<(&mut Transform, &mut Visibility), With<Player>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    mut orbit: ResMut<CameraOrbit>,
) {
    let Some(kind) = arcade.active else {
        if arcade.was_open {
            orbit.blend_request = true;
            for (_, mut vis) in &mut player_q {
                *vis = Visibility::Visible;
            }
        }
        return;
    };
    if !stats.arcade_open {
        for (_, mut vis) in &mut player_q {
            *vis = Visibility::Visible;
        }
        return;
    }

    let dt = time.delta_secs().min(0.05);
    if kind == ArcadeKind::RollerCoaster {
        let (car_p, car_rot) = coaster_sample(arcade.coaster_u);
        let (ahead_p, _) = coaster_sample((arcade.coaster_u + 0.028) % 1.0);
        let world_car = CASINO_LEVEL_ORIGIN + car_p;
        let world_ahead = CASINO_LEVEL_ORIGIN + ahead_p;
        let cam_pos = world_car + car_rot * Vec3::new(0.0, 1.15, -0.35);
        let up = (car_rot * Vec3::Y).normalize_or_zero();
        for (mut p_tf, mut vis) in &mut player_q {
            *vis = Visibility::Hidden;
            p_tf.translation = CASINO_LEVEL_ORIGIN + Vec3::new(13.8, 0.0, 14.2);
        }
        let k = 1.0 - (-12.0 * dt).exp();
        let dyn_fov = 0.98 + 0.14 * (time.elapsed_secs() * 2.4).sin().abs();
        if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
            c_tf.translation = c_tf.translation.lerp(cam_pos, k);
            let goal = Transform::from_translation(cam_pos)
                .looking_at(world_ahead + up * 0.35, up)
                .rotation;
            c_tf.rotation = c_tf.rotation.slerp(goal, k);
            if let Projection::Perspective(ref mut p) = *proj {
                p.fov += (dyn_fov - p.fov) * k;
            }
        }
        return;
    }

    let (mp, myaw) = kind.pose();
    let w = CASINO_LEVEL_ORIGIN + mp;
    let r = Quat::from_rotation_y(myaw);

    // Keep player visible standing cleanly beside the machine in third-person!
    for (mut p_tf, mut vis) in &mut player_q {
        *vis = Visibility::Visible;
        let p_off = match kind {
            ArcadeKind::Skeeball => Vec3::new(-0.95, 0.0, 2.85),
            _ => Vec3::new(-0.92, 0.0, 1.55),
        };
        p_tf.translation = w + r * p_off;
        p_tf.rotation = r;
    }

    let (cam_off, look_off) = match kind {
        ArcadeKind::CoinPusher => (Vec3::new(0.92, 2.22, 2.90), Vec3::new(0.0, 1.36, 0.12)),
        ArcadeKind::ClawMachine => (Vec3::new(1.10, 2.38, 2.95), Vec3::new(0.0, 1.55, 0.0)),
        ArcadeKind::Plinko => (Vec3::new(0.95, 2.25, 3.35), Vec3::new(0.0, 1.68, 0.0)),
        ArcadeKind::Skeeball | ArcadeKind::JazzBand | ArcadeKind::RollerCoaster => {
            (Vec3::new(1.15, 2.20, 4.30), Vec3::new(0.0, 1.35, -1.2))
        }
    };
    let pos = w + r * cam_off;
    let look = w + r * look_off;
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
            ArcadeKind::CoinPusher => "AIM: A/D  |  MASH/HOLD: SPACE  |  RAIN: B".to_string(),
            ArcadeKind::ClawMachine => "MOVE: WASD".to_string(),
            ArcadeKind::Plinko => "AIM: A/D".to_string(),
            ArcadeKind::RollerCoaster => "STARGATE TRIP: ESC TO EXIT".to_string(),
            ArcadeKind::Skeeball | ArcadeKind::JazzBand => {
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

fn step_coaster_and_stargate(
    time: Res<Time<Real>>,
    mut arcade: ResMut<ArcadeState>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    mut car_q: Query<(&CoasterTrainCar, &mut Transform)>,
    mut rib_q: Query<
        (&CoasterStargateRib, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        Without<CoasterTrainCar>,
    >,
    mut orb_q: Query<
        (&CoasterTrippyOrb, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        (Without<CoasterTrainCar>, Without<CoasterStargateRib>),
    >,
    mut strobe_q: Query<(&CoasterStrobeLight, &mut PointLight)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    // Trippy nighttime sky & ambient illumination while riding over Cedar Rapids
    if arcade.active == Some(ArcadeKind::RollerCoaster) {
        clear.0 = Color::srgb(
            0.03 + 0.04 * (t * 0.9).sin().abs(),
            0.012 + 0.02 * (t * 1.3).cos().abs(),
            0.10 + 0.08 * (t * 0.7).sin().abs(),
        );
        ambient.color = Color::srgb(0.55, 0.50, 0.95);
        ambient.brightness = 640.0;
    } else {
        clear.0 = Color::srgb(0.015, 0.008, 0.028);
        ambient.color = Color::srgb(0.45, 0.34, 0.72);
        ambient.brightness = 220.0;
    }

    // Dynamic speed: slower on climb, blazing fast through the vertical loop & 2001 Stargate corkscrew!
    let (cur_p, _) = coaster_sample(arcade.coaster_u);
    let speed = if cur_p.y > 32.0 {
        0.068
    } else if arcade.coaster_u > 0.60 && arcade.coaster_u < 0.90 {
        0.075
    } else {
        0.052
    };
    arcade.coaster_u = (arcade.coaster_u + dt * speed) % 1.0;

    for (car, mut tf) in &mut car_q {
        let u = (arcade.coaster_u - car.0 as f32 * 0.008).rem_euclid(1.0);
        let (p, rot) = coaster_sample(u);
        tf.translation = CASINO_LEVEL_ORIGIN + p;
        tf.rotation = rot;
    }

    // 2001: A Space Odyssey Slit-Scan hyper-spectral dimensional tunnel pulsing & color-cycling
    for (rib, mut tf, mat_h) in &mut rib_q {
        let wave = (t * 6.5 - rib.idx as f32 * 0.45).sin();
        let spin = t * 1.4 + rib.idx as f32 * 0.25;
        let pulse = 4.4 + 1.1 * wave;
        tf.translation = rib.base_pos;
        tf.rotation = rib.rot * Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_y(spin);
        tf.scale = Vec3::new(pulse, 1.0, pulse);

        if let Some(mat) = materials.get_mut(&mat_h.0) {
            let hue = ((t * 190.0 + rib.idx as f32 * 24.0) % 360.0 + 360.0) % 360.0;
            let col = Color::hsl(hue, 1.0, 0.56);
            let l = col.to_linear();
            let boost = 3.6 + 2.6 * ((t * 11.0 + rib.idx as f32 * 0.7).sin() * 0.5 + 0.5);
            mat.base_color = col;
            mat.emissive = LinearRgba::new(l.red * boost, l.green * boost, l.blue * boost, 1.0);
        }
    }

    // Trippy nighttime sky orbs & stars pulsing and color-cycling over Cedar Rapids
    for (orb, mut tf, mat_h) in &mut orb_q {
        tf.translation = orb.base
            + Vec3::new(
                (t * 0.9 + orb.phase).sin() * 3.5,
                (t * 1.4 + orb.phase * 1.3).cos() * 2.5,
                (t * 0.8 + orb.phase * 0.7).cos() * 3.5,
            );
        let sc = orb.radius * (0.75 + 0.45 * (t * 3.8 + orb.phase).sin().abs());
        tf.scale = Vec3::splat(sc);
        if let Some(mat) = materials.get_mut(&mat_h.0) {
            let hue = ((t * 120.0 + orb.phase * 55.0) % 360.0 + 360.0) % 360.0;
            let col = Color::hsl(hue, 1.0, 0.60);
            let l = col.to_linear();
            mat.base_color = col;
            mat.emissive = LinearRgba::new(l.red * 3.5, l.green * 3.5, l.blue * 3.5, 1.0);
        }
    }

    // Rapid strobe & rave colour-cycling PointLights over Cedar Rapids
    for (strobe, mut light) in &mut strobe_q {
        let ph = t * 14.0 + strobe.0 as f32 * 1.57;
        let flash = (ph.sin() * 0.5 + 0.5).powi(3);
        let hue = ((t * 240.0 + strobe.0 as f32 * 90.0) % 360.0 + 360.0) % 360.0;
        light.color = Color::hsl(hue, 1.0, 0.60);
        light.intensity = 120_000.0 + 750_000.0 * flash;
    }
}
