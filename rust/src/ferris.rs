//! The RIVER STAR: a rideable observation wheel on a boardwalk pier in the Cedar River corner of
//! the city (the river block at x = -114, z = +114, which held only water, so nothing the game
//! uses was moved).
//!
//! * Site: the pier deck fills the river corner south of the z = 92 street and west of the
//!   x = -92 avenue. A round stone plaza (one 0.35 m step) carries the wheel, its boarding dais
//!   (a second step), an entrance arch with a bulb marquee, a ticket booth, lamp posts and
//!   festoon lights; two snack kiosks stand on the boardwalk. Everything sits inside the existing
//!   map bounds, so no clamp, camera bound, traffic wrap or radar scale changed.
//! * The wheel faces the city centre (it stands on the diagonal), so from the downtown streets
//!   it reads as a near circle over the rooftops. Rim radius 19.5 m, hub 23.5 m up, top about
//!   44 m; 14 gondolas hang from pins between the two rim planes and are re-levelled every frame.
//!   Meshes are merged per material (one entity per material per part) and shared between
//!   gondolas; bulbs are emissive spheres in merged meshes (fewer with `?gfx=low`).
//! * Riding: on foot at normal size on the dais, the shared city prompt (`city::update_city_props`
//!   through [`WheelPrompt`]) offers "RIDE THE WHEEL". The next gondola is called down (the wheel
//!   slows while it passes the dais), the hero steps in and rides two laps (48 s each) while the
//!   camera sits inside the gondola looking out over the city (free look; zoom out for a chase
//!   view of the gondola). "HOP OFF" lets him out the next time his gondola reaches the dais (the
//!   wheel speeds up to bring it down). He steps back onto the dais facing out and play resumes.
//! * While riding (`PlayerStats::riding_wheel`): movement, attacks, phone, vehicles and the other
//!   interactions are off, the hero is untouchable (invulnerability kept topped up, Lexie holds
//!   her garlic and stops walking), recruited crew wait by the plaza ([`crew_wait_anchor`]), and
//!   leaving CityPlay for any reason (pause Restart, retry, Clear save) clears the ride.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::sync::Mutex;

use bevy::image::ImageAddressMode;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::audio::{play_sfx, speak_voice};
use crate::city::{CityMinimapPlayerDot, CityWorldMarker};
use crate::player::{CameraSet, Player};
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::transition::{FadePhase, SceneFade};

// ---------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------

/// Plaza centre on the ground (the wheel's axle stands above it).
pub const WHEEL_CENTER: Vec3 = Vec3::new(-114.0, 0.0, 114.0);
/// Yaw of the wheel's local frame: local +Z (the gondola doors, the dais, the arch) faces the
/// city centre, local X runs along the wheel's plane.
const WHEEL_YAW: f32 = 3.0 * FRAC_PI_4;
const PLAZA_R: f32 = 13.5;
const PLAZA_TOP: f32 = 0.35;
const DAIS_R: f32 = 3.0;
const DAIS_TOP: f32 = 0.70;
/// Local z of the dais centre (its near edge clears the gondolas' swing by 0.15 m).
const DAIS_Z: f32 = 4.2;
/// Pin radius: the outer rim ring passes through the gondola pins.
const RIM_R: f32 = 19.5;
const RIM_IN: f32 = 17.0;
/// The two rim planes sit at local z = +-this.
const RIM_HALF_GAP: f32 = 1.5;
/// Pin to gondola floor.
const PIN_DROP: f32 = 3.2;
/// Hub height: the lowest gondola floor passes 5 cm above the dais.
const HUB_Y: f32 = DAIS_TOP + 0.05 + PIN_DROP + RIM_R;
const GONDOLAS: usize = 14;
/// Top of the A-frame leg plinths.
const PLINTH_TOP: f32 = PLAZA_TOP + 0.9;
/// A-frame feet (local, +-x, +-z) and apex z.
const FOOT_X: f32 = 10.5;
const FOOT_Z: f32 = 5.2;
const APEX_Z: f32 = 3.9;

/// Seconds per revolution at normal speed.
const REV_SECS: f32 = 48.0;
/// Laps per ride before the gondola lets the hero out on its own.
const RIDE_LAPS: f32 = 2.0;
/// Where the hero stands to board and where he is let out (local, on the dais by the door).
const BOARD_SPOT: Vec3 = Vec3::new(0.0, DAIS_TOP, 1.75);
/// Prompt reach around the boarding spot (the dais is 3 m in radius).
const BOARD_REACH: f32 = 2.6;
/// Biggest hero that fits a gondola (normal city size is 1.8).
const RIDER_MAX_SCALE: f32 = 2.2;
/// Step in / step out time.
const STEP_SECS: f32 = 0.8;
/// Remaining wheel angle (radians) to the bottom when the waiting hero starts his step in.
const STEP_LEAD: f32 = 0.035;
/// Eye height above the gondola floor for the in-gondola camera.
const EYE_H: f32 = 1.62;
/// First-visit hint range from the plaza centre.
const WELCOME_RANGE: f32 = 46.0;
/// Snack kiosks on the boardwalk: (x, z, half x, half z), world axis-aligned.
const KIOSKS: [(f32, f32, f32, f32); 2] = [(-100.6, 123.0, 1.0, 0.8), (-125.0, 102.4, 1.25, 0.85)];
/// Where recruited crew wait during a ride: the boardwalk east of the plaza (off the plaza, as
/// crew walk at street level), formation spread along z.
const CREW_WAIT: (Vec3, f32) = (Vec3::new(-98.6, 0.0, 112.5), FRAC_PI_2);

fn wheel_rot() -> Quat {
    Quat::from_rotation_y(WHEEL_YAW)
}

fn to_world(local: Vec3) -> Vec3 {
    WHEEL_CENTER + wheel_rot() * local
}

/// World direction the gondola doors and the boarding dais face (toward the city).
fn front_dir() -> Vec3 {
    wheel_rot() * Vec3::Z
}

/// Hero / camera yaw that faces (or looks along) `dir` (forward is -Z at yaw 0).
fn yaw_facing(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

/// Gondola pin in the wheel's local frame for wheel angle `theta` (0 = bottom).
fn pin_local(theta: f32) -> Vec3 {
    Vec3::new(RIM_R * theta.sin(), HUB_Y - RIM_R * theta.cos(), 0.0)
}

fn gondola_theta(angle: f32, k: usize) -> f32 {
    angle + k as f32 * TAU / GONDOLAS as f32
}

/// Radians the wheel still has to turn before gondola `k` reaches the bottom (0..TAU).
fn to_bottom(angle: f32, k: usize) -> f32 {
    (-gondola_theta(angle, k)).rem_euclid(TAU)
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn flat_dist(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let d = (b - a + PI).rem_euclid(TAU) - PI;
    a + d * t
}

/// True when (x, z) is on the wheel's plaza or a kiosk (with `margin`): random-spot pickers
/// (tendies, crew, parked car, license card) and Lexie's walk test go through
/// `city::is_clear_of_skyscrapers`, which asks this.
pub fn blocks_spot(x: f32, z: f32, margin: f32) -> bool {
    if Vec2::new(x - WHEEL_CENTER.x, z - WHEEL_CENTER.z).length() < PLAZA_R + margin {
        return true;
    }
    KIOSKS
        .iter()
        .any(|&(kx, kz, hx, hz)| (x - kx).abs() < hx + margin && (z - kz).abs() < hz + margin)
}

// ---------------------------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------------------------

/// Crew waiting spot while the hero rides (position, formation yaw), read by
/// `player::crew_follow_target`. A plain global so the four crew modules need no new parameters.
static CREW_WAIT_ANCHOR: Mutex<Option<(Vec3, f32)>> = Mutex::new(None);

fn set_crew_wait(v: Option<(Vec3, f32)>) {
    if let Ok(mut g) = CREW_WAIT_ANCHOR.lock() {
        *g = v;
    }
}

/// While the hero rides the wheel, recruited crew gather here instead of following him.
pub fn crew_wait_anchor() -> Option<(Vec3, f32)> {
    CREW_WAIT_ANCHOR.lock().ok().and_then(|g| *g)
}

/// Run condition: false while the hero is on the wheel (gates the scooter / phone system).
pub fn not_riding_wheel(stats: Res<PlayerStats>) -> bool {
    !stats.riding_wheel
}

/// The wheel's offer for the shared city prompt (`city::update_city_props` draws it and reports
/// presses back here, so the wheel uses the same F / E, gamepad X and touch button as the beer
/// tower and the parked car).
#[derive(Resource, Default)]
pub struct WheelPrompt {
    /// Prompt label this frame (None: the wheel offers nothing).
    pub label: Option<String>,
    /// Set by `update_city_props` when the prompt is pressed or tapped.
    pub pressed: bool,
    /// A press does something (board / hop off); status-only labels ignore presses.
    actionable: bool,
}

/// Wheel motion, shared by the animation, the ride and the ride camera.
#[derive(Resource)]
pub struct FerrisWheel {
    /// Wheel angle (radians, wrapped).
    angle: f32,
    /// Current speed factor (1 = one turn per `REV_SECS`).
    speed: f32,
    /// Wheel angle advanced this frame.
    step: f32,
    last_omega: f32,
    /// Common gondola swing (radians) from speed changes, a damped spring.
    sway: f32,
    sway_vel: f32,
    clock: f32,
    /// Debug speed multiplier (`wheel_demo=fast` / `exit` / `spin`).
    debug_speed: f32,
}

impl Default for FerrisWheel {
    fn default() -> Self {
        Self {
            angle: 0.37,
            speed: 1.0,
            step: 0.0,
            last_omega: 0.0,
            sway: 0.0,
            sway_vel: 0.0,
            clock: 0.0,
            debug_speed: 1.0,
        }
    }
}

impl FerrisWheel {
    /// Gondola `k` pin (world) and its level-but-swaying rotation.
    fn gondola_pose(&self, k: usize) -> (Vec3, Quat) {
        let th = gondola_theta(self.angle, k);
        let rot = wheel_rot() * Quat::from_rotation_z(self.gondola_swing(k));
        (to_world(pin_local(th)), rot)
    }

    fn gondola_swing(&self, k: usize) -> f32 {
        self.sway + 0.005 * (self.clock * 0.9 + k as f32 * 1.7).sin()
    }

    /// Where the rider stands in gondola `k` (world).
    fn cabin_floor(&self, k: usize) -> Vec3 {
        let (pin, rot) = self.gondola_pose(k);
        pin + rot * Vec3::new(0.0, -PIN_DROP + 0.02, 0.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum RidePhase {
    #[default]
    Idle,
    /// Boarding pressed; the hero waits on the dais for his gondola.
    Waiting,
    StepIn,
    Riding,
    StepOut,
}

#[derive(Resource, Default)]
pub struct FerrisRide {
    phase: RidePhase,
    gondola: usize,
    /// Seconds in the current phase.
    t: f32,
    /// Wheel angle turned since the hero sat down.
    travelled: f32,
    /// `travelled` at which he steps out (a whole number of laps: his gondola is at the bottom).
    exit_at: f32,
    exit_requested: bool,
    from: Vec3,
    from_yaw: f32,
    saved_yaw: f32,
    saved_pitch: f32,
    saved_dist: f32,
    saved_lock_on: bool,
    /// Camera pose when the step in began (the ride camera eases from it).
    cam_from: (Vec3, Quat, f32),
    /// Orbit distance when seated: zooming out past it moves the view outside the gondola.
    zoom_base: f32,
    /// 0 = in-gondola view, 1 = chase view of the gondola.
    view_out: f32,
    welcomed: bool,
    offer_hinted: bool,
    blocked_hinted: bool,
    completed_full_ride: bool,
}

impl FerrisRide {
    /// Back to idle (keeps the once-per-session welcome).
    fn clear(&mut self) {
        let welcomed = self.welcomed;
        *self = Self::default();
        self.welcomed = welcomed;
    }

    fn active(&self) -> bool {
        self.phase != RidePhase::Idle
    }
}

/// Debug driver (`?debug=1&wheel_demo=...`): `ride` boards automatically once on the dais,
/// `fast` does the same with the wheel at 10x speed (not while stepping in or out), `exit`
/// also asks to hop off early, `spin` only speeds the wheel, `titan` parks a kaiju-size hero
/// beside the wheel. Ride state goes to `window.tendyTelemetry.wheel` four times a second.
#[derive(Resource, Default)]
struct FerrisDebug {
    read: bool,
    mode: String,
    city_t: f32,
    boarded: bool,
    exit_sent: bool,
    placed: bool,
    tele_t: f32,
}

// ---------------------------------------------------------------------------------------------
// Components & handles
// ---------------------------------------------------------------------------------------------

#[derive(Component)]
pub struct WheelRoot;

#[derive(Component)]
struct WheelRotor;

#[derive(Component)]
struct WheelGondola(usize);

#[derive(Component)]
struct WheelRadarIcon;

#[derive(Component)]
struct WheelRideHud;

#[derive(Component)]
struct WheelRideHudText;

#[derive(Component)]
struct WheelRideHudFill;

/// Animated bulb materials (rim chase A / B, spoke breathing).
#[derive(Resource)]
struct WheelLights {
    chase_a: Handle<StandardMaterial>,
    chase_b: Handle<StandardMaterial>,
    spokes: Handle<StandardMaterial>,
}

const SIGN_TEX: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0001);
const SIGN_EMIS: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0002);
const TICKETS_TEX: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0003);
const TICKETS_EMIS: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0004);
const SNACK_TEX: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0005);
const SNACK_EMIS: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0006);
const STRIPE_TEX: Handle<Image> = Handle::weak_from_u128(0x2b7e_51d0_9c4a_4f6e_8d21_f3a0_57e1_0007);

// ---------------------------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------------------------

pub struct FerrisPlugin;

impl Plugin for FerrisPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FerrisWheel>()
            .init_resource::<FerrisRide>()
            .init_resource::<WheelPrompt>()
            .init_resource::<FerrisDebug>()
            .add_systems(Startup, setup_wheel_textures)
            // The wheel turns in every state that shows the city (cutscenes included).
            .add_systems(Update, animate_wheel.before(ferris_ride_update))
            .add_systems(
                Update,
                (
                    ferris_debug.before(ferris_ride_update),
                    // After everything that moves the hero (his own movement stands down while
                    // riding) and after the shared prompt reported this frame's press; before the
                    // follow camera. (`player_movement` / `update_camera` are registered in
                    // several states, so the order is set against their `CameraSet`s.)
                    ferris_ride_update
                        .after(CameraSet::Motion)
                        .after(crate::city::update_city_props)
                        .before(CameraSet::Follow),
                    // Overrides the follow camera while seated.
                    ferris_ride_camera.after(CameraSet::Follow),
                    update_wheel_ui.after(ferris_ride_update),
                )
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(crate::ui::gameplay_running),
            )
            .add_systems(OnExit(GameState::CityPlay), reset_ride_on_exit);
    }
}

// ---------------------------------------------------------------------------------------------
// Mesh building
// ---------------------------------------------------------------------------------------------

/// Accumulates transformed copies of primitive meshes into one mesh (one draw per material).
#[derive(Default)]
struct Geo {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl Geo {
    fn add(&mut self, mesh: &Mesh, tf: Transform) {
        let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            return;
        };
        let n: &[[f32; 3]] = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(n)) => n,
            _ => &[],
        };
        let uv: &[[f32; 2]] = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(u)) => u,
            _ => &[],
        };
        let base = self.pos.len() as u32;
        let inv = tf.scale.recip();
        for (i, v) in p.iter().enumerate() {
            self.pos.push(tf.transform_point(Vec3::from_array(*v)).to_array());
            let nn = n.get(i).map_or(Vec3::Y, |v| Vec3::from_array(*v));
            self.nrm.push((tf.rotation * (nn * inv)).normalize_or_zero().to_array());
            self.uv.push(uv.get(i).copied().unwrap_or([0.0, 0.0]));
        }
        match mesh.indices() {
            Some(ix) => self.idx.extend(ix.iter().map(|i| base + i as u32)),
            None => self.idx.extend(base..base + p.len() as u32),
        }
    }

    fn cuboid(&mut self, center: Vec3, size: Vec3, rot: Quat) {
        self.add(
            &Mesh::from(Cuboid::from_size(size)),
            Transform::from_translation(center).with_rotation(rot),
        );
    }

    fn rod(&mut self, a: Vec3, b: Vec3, r: f32, res: u32) {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return;
        }
        let m = Cylinder::new(r, len).mesh().resolution(res).build();
        self.add(
            &m,
            Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len)),
        );
    }

    fn ball(&mut self, tpl: &Mesh, c: Vec3, r: f32) {
        self.add(tpl, Transform::from_translation(c).with_scale(Vec3::splat(r)));
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Flat five-point star facing +Z.
fn star_mesh(r_out: f32, r_in: f32) -> Mesh {
    let mut pos = vec![[0.0, 0.0, 0.0]];
    let mut uv = vec![[0.5, 0.5]];
    for i in 0..10 {
        let a = i as f32 * PI / 5.0;
        let r = if i % 2 == 0 { r_out } else { r_in };
        pos.push([a.sin() * r, a.cos() * r, 0.0]);
        uv.push([0.5 + a.sin() * 0.5, 0.5 - a.cos() * 0.5]);
    }
    let mut idx = Vec::new();
    for i in 0..10u32 {
        idx.extend_from_slice(&[0, 1 + (i + 1) % 10, 1 + i]);
    }
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

/// Pennant: a flat triangle in the XY plane (drawn double-sided), hoist on the Y axis.
fn pennant_mesh(len: f32, h: f32) -> Mesh {
    let pos = vec![[0.0, h * 0.5, 0.0], [0.0, -h * 0.5, 0.0], [len, 0.0, 0.0]];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.5]])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

/// Ring in the XY plane (around local Z) at depth `z`.
fn ring_tf(z: f32) -> Transform {
    Transform::from_xyz(0.0, 0.0, z).with_rotation(Quat::from_rotation_x(FRAC_PI_2))
}

/// Unit direction in the wheel plane for angle `a` (0 = straight down, matches `pin_local`).
fn wdir(a: f32) -> Vec3 {
    Vec3::new(a.sin(), -a.cos(), 0.0)
}

/// Rotor meshes in rotor space (hub at the origin): [steel, hub paint, chase A bulbs,
/// chase B bulbs, spoke bulbs, gold emblems].
fn rotor_meshes(low: bool, bulb: &Mesh) -> [Mesh; 6] {
    let n = GONDOLAS;
    let d = TAU / n as f32;
    let mut steel = Geo::default();
    let mut hub = Geo::default();
    let mut bulbs_a = Geo::default();
    let mut bulbs_b = Geo::default();
    let mut spoke_bulbs = Geo::default();
    let mut emblem = Geo::default();
    let ring = |r: f32, minor: f32| {
        Torus::new(r - minor, r + minor)
            .mesh()
            .minor_resolution(8)
            .major_resolution(if low { 112 } else { 192 })
            .build()
    };
    let outer = ring(RIM_R, 0.17);
    let inner = ring(RIM_IN, 0.12);
    for zs in [-1.0_f32, 1.0] {
        let z = Vec3::Z * zs * RIM_HALF_GAP;
        steel.add(&outer, ring_tf(z.z));
        steel.add(&inner, ring_tf(z.z));
        // Truss between the rings: zig-zag diagonals plus a radial tie at every half step.
        for j in 0..(2 * n) {
            let a0 = j as f32 * d * 0.5;
            let a1 = a0 + d * 0.5;
            steel.rod(wdir(a0) * RIM_R + z, wdir(a1) * RIM_IN + z, 0.07, 6);
            steel.rod(wdir(a0) * RIM_IN + z, wdir(a0) * RIM_R + z, 0.07, 6);
        }
        for k in 0..n {
            let a = k as f32 * d;
            // Radial spoke from the hub flange to the inner ring.
            let hub_end = wdir(a) * 1.8 + Vec3::Z * zs * 1.55;
            steel.rod(hub_end, wdir(a) * RIM_IN + z, 0.1, 8);
            // Thin tension rods leaning one way on the front face and the other on the back.
            let lean = a + zs * 0.55;
            steel.rod(wdir(lean) * 1.6 + Vec3::Z * zs * 1.55, wdir(a + d * 0.5) * RIM_IN + z, 0.045, 5);
            if !low {
                for t in [0.2_f32, 0.35, 0.5, 0.65, 0.8, 0.93] {
                    let p = hub_end.lerp(wdir(a) * RIM_IN + z, t) + Vec3::Z * zs * 0.14;
                    spoke_bulbs.ball(bulb, p, 0.085);
                }
            }
        }
        // Rim bulbs on the outside face of each ring, alternating between the two chase sets.
        let per = if low { 2 } else { 4 };
        for i in 0..(n * per) {
            let a = (i as f32 + 0.5) * d / per as f32;
            let p = wdir(a) * RIM_R + Vec3::Z * zs * (RIM_HALF_GAP + 0.2);
            if i % 2 == 0 {
                bulbs_a.ball(bulb, p, 0.11);
            } else {
                bulbs_b.ball(bulb, p, 0.11);
            }
        }
    }
    for k in 0..n {
        let a = k as f32 * d;
        // Gondola pin axle and a cross strut between the rim planes half a step further on.
        steel.rod(wdir(a) * RIM_R - Vec3::Z * 1.62, wdir(a) * RIM_R + Vec3::Z * 1.62, 0.08, 6);
        let h = a + d * 0.5;
        steel.rod(wdir(h) * RIM_IN - Vec3::Z * RIM_HALF_GAP, wdir(h) * RIM_IN + Vec3::Z * RIM_HALF_GAP, 0.07, 6);
    }
    // Hub drum, flanges and the gold stars on its faces.
    hub.add(
        &Cylinder::new(1.5, 3.1).mesh().resolution(24).build(),
        Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    for zs in [-1.0_f32, 1.0] {
        steel.add(
            &Cylinder::new(1.85, 0.2).mesh().resolution(28).build(),
            Transform::from_xyz(0.0, 0.0, zs * 1.55).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        );
        let face = if zs > 0.0 { Quat::IDENTITY } else { Quat::from_rotation_y(PI) };
        emblem.add(&star_mesh(1.25, 0.52), Transform::from_xyz(0.0, 0.0, zs * 1.67).with_rotation(face));
    }
    [
        steel.into_mesh(),
        hub.into_mesh(),
        bulbs_a.into_mesh(),
        bulbs_b.into_mesh(),
        spoke_bulbs.into_mesh(),
        emblem.into_mesh(),
    ]
}

/// A-frame support in the wheel's local frame: [navy steel, dark steel, concrete, bulbs].
fn frame_meshes(low: bool, bulb: &Mesh) -> [Mesh; 4] {
    let mut navy = Geo::default();
    let mut dark = Geo::default();
    let mut concrete = Geo::default();
    let mut bulbs = Geo::default();
    let apex = |zs: f32| Vec3::new(0.0, HUB_Y, zs * APEX_Z);
    let foot = |xs: f32, zs: f32| Vec3::new(xs * FOOT_X, PLINTH_TOP, zs * FOOT_Z);
    let on_leg = |xs: f32, zs: f32, y: f32| {
        let t = (y - PLINTH_TOP) / (HUB_Y - PLINTH_TOP);
        foot(xs, zs).lerp(apex(zs), t)
    };
    for zs in [-1.0_f32, 1.0] {
        for xs in [-1.0_f32, 1.0] {
            navy.rod(foot(xs, zs), apex(zs), 0.36, 10);
            concrete.cuboid(
                Vec3::new(xs * FOOT_X, PLAZA_TOP + 0.45, zs * FOOT_Z),
                Vec3::new(1.7, 0.9, 1.7),
                Quat::IDENTITY,
            );
            // Base shoe on the plinth.
            dark.cuboid(foot(xs, zs) + Vec3::Y * 0.12, Vec3::new(1.0, 0.24, 1.0), Quat::IDENTITY);
            if !low {
                let mut y = 2.6;
                while y < HUB_Y - 2.0 {
                    bulbs.ball(bulb, on_leg(xs, zs, y) + Vec3::Z * zs * 0.42, 0.1);
                    y += 1.7;
                }
            }
        }
        // Lattice from 7 m up (the hero walks under the front frame to the dais).
        let levels = [7.0_f32, 11.0, 15.0, 19.0];
        for (i, &y) in levels.iter().enumerate() {
            navy.rod(on_leg(-1.0, zs, y), on_leg(1.0, zs, y), 0.16, 8);
            if let Some(&y2) = levels.get(i + 1) {
                let (a, b) = if i % 2 == 0 { (-1.0, 1.0) } else { (1.0, -1.0) };
                navy.rod(on_leg(a, zs, y), on_leg(b, zs, y2), 0.12, 6);
            }
        }
        // Bearing block at the apex, flag pole and finial above it.
        navy.cuboid(apex(zs), Vec3::new(1.4, 1.7, 1.0), Quat::IDENTITY);
        dark.rod(apex(zs) + Vec3::Y * 0.8, apex(zs) + Vec3::Y * 4.2, 0.06, 6);
    }
    // Ground ties along z between the front and back feet.
    for xs in [-1.0_f32, 1.0] {
        dark.rod(
            Vec3::new(xs * FOOT_X, PLAZA_TOP + 0.55, -FOOT_Z),
            Vec3::new(xs * FOOT_X, PLAZA_TOP + 0.55, FOOT_Z),
            0.2,
            8,
        );
    }
    // Static axle through both bearings.
    dark.rod(Vec3::new(0.0, HUB_Y, -4.7), Vec3::new(0.0, HUB_Y, 4.7), 0.45, 16);
    for zs in [-1.0_f32, 1.0] {
        dark.add(
            &Cylinder::new(0.62, 0.3).mesh().resolution(16).build(),
            Transform::from_xyz(0.0, HUB_Y, zs * 4.75).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        );
    }
    [navy.into_mesh(), dark.into_mesh(), concrete.into_mesh(), bulbs.into_mesh()]
}

/// One gondola in pin space (pin at the origin, floor `PIN_DROP` below): [trim, paint, roof,
/// glass, wood, bulbs]. Shared by every gondola.
fn gondola_meshes(bulb: &Mesh) -> [Mesh; 6] {
    let yf = -PIN_DROP;
    let (hx, hz) = (1.25_f32, 1.05_f32);
    let mut trim = Geo::default();
    let mut paint = Geo::default();
    let mut roof = Geo::default();
    let mut glass = Geo::default();
    let mut wood = Geo::default();
    let mut bulbs = Geo::default();
    let q = Quat::IDENTITY;
    // Floor slab and plank deck.
    paint.cuboid(Vec3::new(0.0, yf - 0.08, 0.0), Vec3::new(2.6, 0.16, 2.24), q);
    wood.cuboid(Vec3::new(0.0, yf + 0.01, 0.0), Vec3::new(2.4, 0.02, 2.0), q);
    // Waist panels; the front (door side, +Z) leaves a doorway in the middle.
    let wh = 1.0;
    paint.cuboid(Vec3::new(0.0, yf + wh * 0.5, -hz + 0.03), Vec3::new(2.0 * hx, wh, 0.06), q);
    for xs in [-1.0_f32, 1.0] {
        paint.cuboid(Vec3::new(xs * (hx - 0.03), yf + wh * 0.5, 0.0), Vec3::new(0.06, wh, 2.0 * hz), q);
        let x0 = 0.42;
        let w = hx - x0;
        paint.cuboid(Vec3::new(xs * (x0 + w * 0.5), yf + wh * 0.5, hz - 0.03), Vec3::new(w, wh, 0.06), q);
        trim.cuboid(Vec3::new(xs * x0, yf + wh * 0.5, hz - 0.03), Vec3::new(0.07, wh, 0.09), q);
    }
    // Corner posts and the waist / top rails.
    for xs in [-1.0_f32, 1.0] {
        for zs in [-1.0_f32, 1.0] {
            trim.cuboid(Vec3::new(xs * (hx - 0.04), yf + 1.15, zs * (hz - 0.04)), Vec3::new(0.09, 2.3, 0.09), q);
        }
    }
    for y in [yf + wh, yf + 2.3] {
        for zs in [-1.0_f32, 1.0] {
            trim.cuboid(Vec3::new(0.0, y, zs * (hz - 0.04)), Vec3::new(2.0 * hx, 0.07, 0.08), q);
        }
        for xs in [-1.0_f32, 1.0] {
            trim.cuboid(Vec3::new(xs * (hx - 0.04), y, 0.0), Vec3::new(0.08, 0.07, 2.0 * hz), q);
        }
    }
    // Windows between the rails, a mullion on the back one.
    let gy = yf + wh + 0.65;
    let gh = 1.26;
    for zs in [-1.0_f32, 1.0] {
        glass.cuboid(Vec3::new(0.0, gy, zs * (hz - 0.04)), Vec3::new(2.0 * hx - 0.12, gh, 0.02), q);
    }
    for xs in [-1.0_f32, 1.0] {
        glass.cuboid(Vec3::new(xs * (hx - 0.04), gy, 0.0), Vec3::new(0.02, gh, 2.0 * hz - 0.12), q);
    }
    trim.cuboid(Vec3::new(0.0, gy, -(hz - 0.04)), Vec3::new(0.06, gh, 0.06), q);
    // Roof: painted eave slab, white pyramid, finial.
    paint.cuboid(Vec3::new(0.0, yf + 2.36, 0.0), Vec3::new(2.0 * hx + 0.3, 0.12, 2.0 * hz + 0.3), q);
    roof.add(
        &Cone::new(1.95, 0.62).mesh().resolution(4).build(),
        Transform::from_xyz(0.0, yf + 2.42 + 0.31, 0.0)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_4))
            .with_scale(Vec3::new(1.0, 1.0, 0.88)),
    );
    trim.add(
        &Sphere::new(0.12).mesh().uv(10, 8),
        Transform::from_xyz(0.0, yf + 3.1, 0.0),
    );
    // Hanger from the roof to the pin, with a yoke on the pin axle.
    trim.rod(Vec3::new(0.0, yf + 3.0, 0.0), Vec3::ZERO, 0.07, 8);
    trim.rod(Vec3::new(0.0, 0.0, -0.55), Vec3::new(0.0, 0.0, 0.55), 0.1, 8);
    // Benches at both ends.
    for xs in [-1.0_f32, 1.0] {
        wood.cuboid(Vec3::new(xs * 0.92, yf + 0.44, 0.0), Vec3::new(0.42, 0.06, 1.7), q);
        wood.cuboid(Vec3::new(xs * 1.13, yf + 0.78, 0.0), Vec3::new(0.06, 0.55, 1.7), q);
        trim.cuboid(Vec3::new(xs * 0.92, yf + 0.2, 0.0), Vec3::new(0.34, 0.4, 1.5), q);
    }
    // Bulbs round the eaves and a cabin lamp.
    let (ex, ez) = (hx + 0.13, hz + 0.13);
    for (x, z) in [
        (-ex, -ez),
        (0.0, -ez),
        (ex, -ez),
        (ex, 0.0),
        (ex, ez),
        (0.0, ez),
        (-ex, ez),
        (-ex, 0.0),
    ] {
        bulbs.ball(bulb, Vec3::new(x, yf + 2.36, z), 0.085);
    }
    bulbs.ball(bulb, Vec3::new(0.0, yf + 2.2, 0.0), 0.1);
    [
        trim.into_mesh(),
        paint.into_mesh(),
        roof.into_mesh(),
        glass.into_mesh(),
        wood.into_mesh(),
        bulbs.into_mesh(),
    ]
}

/// Approximates a disc (world axis-aligned) with `n` overlapping boxes whose corners sit just
/// outside the circle, so the union hugs it within a few centimetres per metre of radius.
fn disc_boxes(center: Vec3, r: f32, bottom: f32, top: f32, n: usize) -> Vec<(Vec3, Vec3)> {
    let step = FRAC_PI_2 / n as f32;
    let notch = 2.0_f32.sqrt() * (FRAC_PI_4 + step * 0.5).cos();
    let f = 1.0 + 0.5 * (1.0 - notch);
    (0..n)
        .map(|i| {
            let a = (i as f32 + 0.5) * step;
            (
                Vec3::new(center.x, (bottom + top) * 0.5, center.z),
                Vec3::new(r * f * a.cos(), (top - bottom) * 0.5, r * f * a.sin()),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Textures (procedural, built once at startup under fixed handles)
// ---------------------------------------------------------------------------------------------

fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        '*' => [0b00100, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00100],
        _ => [0; 7],
    }
}

/// Carnival marquee: `text` in 5x7 bulb letters inside a frame of alternating bulbs.
/// Returns (albedo, emissive), both sRGB with mip chains.
fn marquee_images(text: &str, cell: u32, bg: [u8; 3], letter: [u8; 3], frame: [[u8; 3]; 2]) -> (Image, Image) {
    let chars: Vec<char> = text.chars().collect();
    let cols = chars.len() as u32 * 6 - 1 + 4;
    let rows = 7 + 4;
    let (w, h) = (cols * cell, rows * cell);
    let mut base = vec![0u8; (w * h * 4) as usize];
    let mut emis = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            // Thin gold line one cell in from the edge.
            let edge = x.min(w - 1 - x).min(y).min(h - 1 - y);
            let gold = edge >= cell && edge < cell + 2;
            let c = if gold { [176u8, 132, 52] } else { bg };
            base[i..i + 3].copy_from_slice(&c);
            base[i + 3] = 255;
            emis[i + 3] = 255;
        }
    }
    let mut dot = |cx: u32, cy: u32, col: [u8; 3]| {
        let r = cell as f32 * 0.5;
        for py in 0..cell {
            for px in 0..cell {
                let dx = px as f32 + 0.5 - r;
                let dy = py as f32 + 0.5 - r;
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d > 0.86 {
                    continue;
                }
                let k = (1.0 - (d / 0.86).powi(2)).clamp(0.0, 1.0);
                let x = cx * cell + px;
                let y = cy * cell + py;
                let i = ((y * w + x) * 4) as usize;
                for ch in 0..3 {
                    let lit = col[ch] as f32 * (0.55 + 0.45 * k);
                    base[i + ch] = (base[i + ch] as f32 * (1.0 - k) + (200.0 + 55.0 * k) * k).min(255.0) as u8;
                    emis[i + ch] = emis[i + ch].max(lit as u8);
                }
            }
        }
    };
    for (n, ch) in chars.iter().enumerate() {
        let g = glyph(*ch);
        for (ry, bits) in g.iter().enumerate() {
            for cx in 0..5u32 {
                if bits >> (4 - cx) & 1 == 1 {
                    dot(2 + n as u32 * 6 + cx, 2 + ry as u32, letter);
                }
            }
        }
    }
    // Frame bulbs on every other cell of the outer ring.
    let mut flip = false;
    for cx in (0..cols).step_by(2) {
        for cy in [0, rows - 1] {
            dot(cx, cy, frame[flip as usize]);
            flip = !flip;
        }
    }
    for cy in (2..rows - 1).step_by(2) {
        for cx in [0, cols - 1] {
            dot(cx, cy, frame[flip as usize]);
            flip = !flip;
        }
    }
    let mk = |data: Vec<u8>| {
        let mut img = Image::new(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        if crate::gfx::build_mip_chain(&mut img) {
            img.sampler = crate::gfx::trilinear_sampler(ImageAddressMode::ClampToEdge);
        }
        img
    };
    (mk(base), mk(emis))
}

/// Red and cream awning stripes (repeating).
fn stripe_image() -> Image {
    let (w, h) = (64u32, 8u32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let c = if (x / 8) % 2 == 0 { [196u8, 32, 44] } else { [244, 234, 214] };
            data[i..i + 3].copy_from_slice(&c);
            data[i + 3] = 255;
        }
    }
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    if crate::gfx::build_mip_chain(&mut img) {
        img.sampler = crate::gfx::trilinear_sampler(ImageAddressMode::Repeat);
    }
    img
}

fn setup_wheel_textures(mut images: ResMut<Assets<Image>>) {
    let warm = [255u8, 214, 140];
    let pink = [255u8, 120, 196];
    let (b, e) = marquee_images("*RIVER STAR*", 8, [74, 14, 30], warm, [warm, pink]);
    images.insert(SIGN_TEX.id(), b);
    images.insert(SIGN_EMIS.id(), e);
    let (b, e) = marquee_images("TICKETS", 8, [18, 34, 74], warm, [warm, [120, 220, 255]]);
    images.insert(TICKETS_TEX.id(), b);
    images.insert(TICKETS_EMIS.id(), e);
    let (b, e) = marquee_images("POPCORN", 8, [92, 20, 18], [255, 236, 120], [warm, pink]);
    images.insert(SNACK_TEX.id(), b);
    images.insert(SNACK_EMIS.id(), e);
    images.insert(STRIPE_TEX.id(), stripe_image());
}

// ---------------------------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------------------------

fn mat(materials: &mut Assets<StandardMaterial>, c: Color, metallic: f32, rough: f32) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        metallic,
        perceptual_roughness: rough,
        ..default()
    })
}

fn bulb_mat(materials: &mut Assets<StandardMaterial>, base: Color, e: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: base,
        emissive: e,
        perceptual_roughness: 0.3,
        ..default()
    })
}

fn sign_mat(materials: &mut Assets<StandardMaterial>, base: Handle<Image>, emis: Handle<Image>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(base),
        emissive: LinearRgba::rgb(2.2, 2.2, 2.2),
        emissive_texture: Some(emis),
        perceptual_roughness: 0.5,
        ..default()
    })
}

/// Spawns one merged mesh as a child.
fn part(p: &mut ChildBuilder, meshes: &mut Assets<Mesh>, mesh: Mesh, m: &Handle<StandardMaterial>, glow: bool) {
    let mut e = p.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(m.clone()), Transform::default()));
    if glow {
        e.insert(NotShadowCaster);
    }
}

/// Builds the wheel, its plaza and the boardwalk pier (called from `city::spawn_nyc_world_if_needed`).
pub fn spawn_ferris_wheel(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let low = crate::gfx::low_quality_requested();
    let bulb = Sphere::new(1.0).mesh().uv(8, 6);

    // Materials: painted steel with a few accent colours; emissive within the gfx caps.
    let steel_white = mat(materials, Color::srgb(0.90, 0.90, 0.86), 0.35, 0.38);
    let steel_navy = mat(materials, Color::srgb(0.09, 0.17, 0.36), 0.45, 0.42);
    let steel_dark = mat(materials, Color::srgb(0.16, 0.17, 0.19), 0.6, 0.4);
    let hub_red = mat(materials, Color::srgb(0.72, 0.10, 0.12), 0.35, 0.35);
    let brass = mat(materials, Color::srgb(0.86, 0.66, 0.30), 0.85, 0.28);
    let concrete = mat(materials, Color::srgb(0.64, 0.62, 0.58), 0.0, 0.9);
    let roof_white = mat(materials, Color::srgb(0.95, 0.94, 0.90), 0.1, 0.45);
    let rope = mat(materials, Color::srgb(0.55, 0.05, 0.08), 0.0, 0.8);
    let booth_red = mat(materials, Color::srgb(0.70, 0.12, 0.14), 0.2, 0.45);
    let kiosk_teal = mat(materials, Color::srgb(0.06, 0.48, 0.52), 0.2, 0.45);
    let paints = [
        Color::srgb(0.80, 0.10, 0.14),
        Color::srgb(0.96, 0.70, 0.10),
        Color::srgb(0.04, 0.55, 0.60),
        Color::srgb(0.45, 0.20, 0.66),
    ]
    .map(|c| mat(materials, c, 0.25, 0.35));
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.62, 0.80, 0.92, 0.10),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.08,
        metallic: 0.1,
        ..default()
    });
    let wood = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.46, 0.31, 0.19),
            perceptual_roughness: 0.75,
            ..default()
        },
        crate::gfx::CityTex::Wood,
    ));
    let deck_wood = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.40, 0.29, 0.20),
            perceptual_roughness: 0.82,
            ..default()
        },
        crate::gfx::CityTex::Wood,
    ));
    let dark_wood = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.20, 0.14, 0.10),
            perceptual_roughness: 0.9,
            ..default()
        },
        crate::gfx::CityTex::Wood,
    ));
    let pavers = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.70, 0.66, 0.60),
            perceptual_roughness: 0.86,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(PLAZA_R * 2.0 / 3.0)),
            ..default()
        },
        crate::gfx::CityTex::Pavers,
    ));
    let band = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.46, 0.22, 0.18),
            perceptual_roughness: 0.88,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::new(0.5, 66.0)),
            ..default()
        },
        crate::gfx::CityTex::Brick,
    ));
    let dais_top = materials.add(crate::gfx::city_textured(
        StandardMaterial {
            base_color: Color::srgb(0.50, 0.34, 0.20),
            perceptual_roughness: 0.7,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(DAIS_R)),
            ..default()
        },
        crate::gfx::CityTex::Wood,
    ));
    let warm_e = LinearRgba::rgb(2.8, 2.1, 1.1);
    let chase_a = bulb_mat(materials, Color::srgb(1.0, 0.88, 0.62), warm_e);
    let chase_b = bulb_mat(materials, Color::srgb(1.0, 0.88, 0.62), warm_e);
    let spoke_bulb = bulb_mat(materials, Color::srgb(1.0, 0.55, 0.82), LinearRgba::rgb(2.6, 0.9, 1.9));
    let warm_bulb = bulb_mat(materials, Color::srgb(1.0, 0.88, 0.62), LinearRgba::rgb(2.6, 1.9, 1.0));
    let pink_bulb = bulb_mat(materials, Color::srgb(1.0, 0.55, 0.82), LinearRgba::rgb(2.5, 0.85, 1.8));
    let teal_bulb = bulb_mat(materials, Color::srgb(0.55, 0.95, 1.0), LinearRgba::rgb(0.6, 2.1, 2.4));
    let globe = bulb_mat(materials, Color::srgb(1.0, 0.92, 0.75), LinearRgba::rgb(2.4, 1.9, 1.15));
    let gold_glow = bulb_mat(materials, Color::srgb(1.0, 0.80, 0.30), LinearRgba::rgb(2.6, 1.7, 0.35));
    let popcorn_glow = bulb_mat(materials, Color::srgb(1.0, 0.86, 0.45), LinearRgba::rgb(1.4, 1.1, 0.35));
    let flag_mats = [Color::srgb(0.85, 0.12, 0.18), Color::srgb(0.98, 0.78, 0.16)].map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.7,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });
    let stripes = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(STRIPE_TEX),
        perceptual_roughness: 0.7,
        ..default()
    });
    let sign = sign_mat(materials, SIGN_TEX, SIGN_EMIS);
    let tickets = sign_mat(materials, TICKETS_TEX, TICKETS_EMIS);
    let snack_sign = sign_mat(materials, SNACK_TEX, SNACK_EMIS);
    commands.insert_resource(WheelLights {
        chase_a: chase_a.clone(),
        chase_b: chase_b.clone(),
        spokes: spoke_bulb.clone(),
    });

    let root = commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            WheelRoot,
            Transform::from_translation(WHEEL_CENTER).with_rotation(wheel_rot()),
            Visibility::default(),
        ))
        .id();

    // ---- Plaza, dais, arch, booth, queue, lamps, festoons, benches (local frame) ----
    let base = {
        let mut stone = Geo::default();
        let mut brass_g = Geo::default();
        let mut navy_g = Geo::default();
        let mut iron = Geo::default();
        let mut wood_g = Geo::default();
        let mut rope_g = Geo::default();
        let mut booth_g = Geo::default();
        let mut stripe_g = Geo::default();
        let mut globes = Geo::default();
        let mut gold = Geo::default();
        let mut bulbs_w = Geo::default();
        let mut bulbs_p = Geo::default();
        let mut bulbs_t = Geo::default();
        let mut sign_g = Geo::default();
        let mut arch_stone = Geo::default();
        let mut arch_brass = Geo::default();
        let mut arch_board = Geo::default();
        let mut tickets_g = Geo::default();
        let mut paver_g = Geo::default();
        let mut band_g = Geo::default();
        let mut dais_g = Geo::default();
        let mut dark_glass = Geo::default();
        let q = Quat::IDENTITY;

        // Plaza drum, paver top, brick border band and a brick ring round the wheel's foot.
        stone.add(
            &Cylinder::new(PLAZA_R, PLAZA_TOP).mesh().resolution(72).build(),
            Transform::from_xyz(0.0, PLAZA_TOP * 0.5, 0.0),
        );
        let up = Quat::from_rotation_x(-FRAC_PI_2);
        paver_g.add(
            &Circle::new(PLAZA_R - 0.25).mesh().resolution(72).build(),
            Transform::from_xyz(0.0, PLAZA_TOP + 0.004, 0.0).with_rotation(up),
        );
        band_g.add(
            &Annulus::new(PLAZA_R - 0.8, PLAZA_R - 0.25).mesh().resolution(72).build(),
            Transform::from_xyz(0.0, PLAZA_TOP + 0.006, 0.0).with_rotation(up),
        );
        band_g.add(
            &Annulus::new(7.4, 7.9).mesh().resolution(56).build(),
            Transform::from_xyz(0.0, PLAZA_TOP + 0.006, 0.0).with_rotation(up),
        );
        // Boarding dais: wooden drum with a brass edge and a rail that leaves the gondola side
        // and the queue side open.
        let dc = Vec3::new(0.0, 0.0, DAIS_Z);
        wood_g.add(
            &Cylinder::new(DAIS_R, DAIS_TOP - PLAZA_TOP).mesh().resolution(40).build(),
            Transform::from_translation(dc + Vec3::Y * (PLAZA_TOP + DAIS_TOP) * 0.5),
        );
        dais_g.add(
            &Circle::new(DAIS_R - 0.05).mesh().resolution(40).build(),
            Transform::from_translation(dc + Vec3::Y * (DAIS_TOP + 0.004)).with_rotation(up),
        );
        brass_g.add(
            &Torus::new(DAIS_R - 0.05, DAIS_R + 0.03).mesh().minor_resolution(6).major_resolution(48).build(),
            Transform::from_translation(dc + Vec3::Y * DAIS_TOP),
        );
        let rail_pt = |a: f32, y: f32| dc + Vec3::new(a.sin() * (DAIS_R - 0.15), DAIS_TOP + y, a.cos() * (DAIS_R - 0.15));
        for side in [-1.0_f32, 1.0] {
            let angles: Vec<f32> = (0..6).map(|i| side * (0.6 + i as f32 * 0.36)).collect();
            for (i, &a) in angles.iter().enumerate() {
                brass_g.rod(rail_pt(a, 0.0), rail_pt(a, 1.0), 0.035, 6);
                brass_g.ball(&bulb, rail_pt(a, 1.02), 0.06);
                if let Some(&b) = angles.get(i + 1) {
                    brass_g.rod(rail_pt(a, 1.0), rail_pt(b, 1.0), 0.03, 6);
                    brass_g.rod(rail_pt(a, 0.55), rail_pt(b, 0.55), 0.025, 6);
                }
            }
        }
        // Queue line: brass stanchions with red ropes from the arch to the dais.
        let stanch_z = [7.6_f32, 8.9, 10.2, 11.2];
        for xs in [-1.0_f32, 1.0] {
            for (i, &z) in stanch_z.iter().enumerate() {
                let p = Vec3::new(xs * 1.5, PLAZA_TOP, z);
                brass_g.rod(p, p + Vec3::Y * 0.95, 0.04, 6);
                brass_g.ball(&bulb, p + Vec3::Y * 0.98, 0.07);
                brass_g.add(
                    &Cylinder::new(0.18, 0.05).mesh().resolution(12).build(),
                    Transform::from_translation(p + Vec3::Y * 0.025),
                );
                if let Some(&z2) = stanch_z.get(i + 1) {
                    let a = p + Vec3::Y * 0.85;
                    let b = Vec3::new(xs * 1.5, PLAZA_TOP + 0.85, z2);
                    let mid = (a + b) * 0.5 - Vec3::Y * 0.12;
                    rope_g.rod(a, mid, 0.03, 6);
                    rope_g.rod(mid, b, 0.03, 6);
                }
            }
        }
        // Entrance arch with the RIVER STAR marquee (both faces) and a gold star on top.
        let arch_z = 11.5;
        for xs in [-1.0_f32, 1.0] {
            navy_g.cuboid(Vec3::new(xs * 3.4, PLAZA_TOP + 3.0, arch_z), Vec3::new(0.5, 6.0, 0.5), q);
            arch_stone.cuboid(Vec3::new(xs * 3.4, PLAZA_TOP + 0.3, arch_z), Vec3::new(0.8, 0.6, 0.8), q);
            arch_brass.ball(&bulb, Vec3::new(xs * 3.4, PLAZA_TOP + 6.25, arch_z), 0.3);
        }
        arch_board.cuboid(Vec3::new(0.0, PLAZA_TOP + 5.45, arch_z), Vec3::new(7.6, 1.6, 0.28), q);
        let sign_quad = Mesh::from(Rectangle::new(7.2, 7.2 * 88.0 / 600.0));
        sign_g.add(&sign_quad, Transform::from_xyz(0.0, PLAZA_TOP + 5.45, arch_z + 0.145));
        sign_g.add(
            &sign_quad,
            Transform::from_xyz(0.0, PLAZA_TOP + 5.45, arch_z - 0.145).with_rotation(Quat::from_rotation_y(PI)),
        );
        for zs in [-1.0_f32, 1.0] {
            let face = if zs > 0.0 { q } else { Quat::from_rotation_y(PI) };
            gold.add(
                &star_mesh(0.62, 0.26),
                Transform::from_xyz(0.0, PLAZA_TOP + 6.75, arch_z + zs * 0.03).with_rotation(face),
            );
        }
        // Ticket booth beside the gate, window toward the street.
        let bc = Vec3::new(5.2, PLAZA_TOP, 8.4);
        booth_g.cuboid(bc + Vec3::Y * 1.05, Vec3::new(1.7, 2.1, 1.5), q);
        dark_glass.cuboid(bc + Vec3::new(0.0, 1.5, 0.76), Vec3::new(1.1, 0.75, 0.03), q);
        wood_g.cuboid(bc + Vec3::new(0.0, 1.12, 0.86), Vec3::new(1.3, 0.06, 0.3), q);
        stripe_g.cuboid(bc + Vec3::Y * 2.2, Vec3::new(2.1, 0.16, 1.9), q);
        stripe_g.add(
            &Cone::new(1.45, 0.55).mesh().resolution(4).build(),
            Transform::from_translation(bc + Vec3::Y * 2.55)
                .with_rotation(Quat::from_rotation_y(FRAC_PI_4))
                .with_scale(Vec3::new(1.0, 1.0, 0.9)),
        );
        booth_g.cuboid(bc + Vec3::new(0.0, 3.05, 0.0), Vec3::new(1.7, 0.42, 0.08), q);
        let tq = Mesh::from(Rectangle::new(1.6, 1.6 * 88.0 / 360.0));
        tickets_g.add(&tq, Transform::from_translation(bc + Vec3::new(0.0, 3.05, 0.045)));
        tickets_g.add(
            &tq,
            Transform::from_translation(bc + Vec3::new(0.0, 3.05, -0.045)).with_rotation(Quat::from_rotation_y(PI)),
        );
        // Lamp posts round the plaza edge, festoons from the A-frame legs to the posts.
        let post_deg = [28.0_f32, 72.0, 108.0, 150.0, -150.0, -108.0, -72.0, -28.0];
        let post_top = |deg: f32| {
            let a = deg.to_radians();
            Vec3::new(a.sin() * 12.7, PLAZA_TOP + 4.1, a.cos() * 12.7)
        };
        for &deg in &post_deg {
            let top = post_top(deg);
            let foot = Vec3::new(top.x, PLAZA_TOP, top.z);
            iron.rod(foot, top + Vec3::Y * 0.2, 0.08, 8);
            iron.add(
                &Cylinder::new(0.2, 0.5).mesh().resolution(10).build(),
                Transform::from_translation(foot + Vec3::Y * 0.25),
            );
            iron.rod(top + Vec3::Y * 0.2, top + Vec3::Y * 0.32, 0.14, 8);
            globes.ball(&bulb, top + Vec3::Y * 0.58, 0.27);
        }
        let leg_pt = |xs: f32, zs: f32, y: f32| {
            let t = (y - PLINTH_TOP) / (HUB_Y - PLINTH_TOP);
            Vec3::new(xs * FOOT_X, PLINTH_TOP, zs * FOOT_Z).lerp(Vec3::new(0.0, HUB_Y, zs * APEX_Z), t)
        };
        // (start, end, sag) for each festoon; each stays on one side of the wheel plane.
        let mut festoons: Vec<(Vec3, Vec3)> = Vec::new();
        for (xs, zs, d1, d2) in [
            (1.0_f32, 1.0_f32, 28.0_f32, 72.0_f32),
            (-1.0, 1.0, -28.0, -72.0),
            (1.0, -1.0, 108.0, 150.0),
            (-1.0, -1.0, -108.0, -150.0),
        ] {
            let leg = leg_pt(xs, zs, 7.0) + Vec3::Z * zs * 0.4;
            festoons.push((leg, post_top(d1)));
            festoons.push((leg, post_top(d2)));
        }
        for xs in [-1.0_f32, 1.0] {
            festoons.push((Vec3::new(xs * 3.4, PLAZA_TOP + 6.0, arch_z), post_top(xs * 28.0)));
        }
        if low {
            festoons.truncate(4);
        }
        let mut colour = 0usize;
        for (a, b) in festoons {
            let n = ((a.distance(b) / 0.9) as usize).max(4);
            let sag = 0.06 * a.distance(b);
            let pt = |t: f32| a.lerp(b, t) - Vec3::Y * sag * 4.0 * t * (1.0 - t);
            for i in 0..n {
                let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
                iron.rod(pt(t0), pt(t1), 0.012, 4);
                if i > 0 {
                    let g = match colour % 3 {
                        0 => &mut bulbs_w,
                        1 => &mut bulbs_p,
                        _ => &mut bulbs_t,
                    };
                    g.ball(&bulb, pt(t0) - Vec3::Y * 0.08, 0.075);
                    colour += 1;
                }
            }
        }
        // Benches facing the wheel, and flower planters by the arch.
        for deg in [130.0_f32, -130.0, -45.0] {
            let a = deg.to_radians();
            let c = Vec3::new(a.sin() * 9.6, PLAZA_TOP, a.cos() * 9.6);
            let rot = Quat::from_rotation_y(a);
            wood_g.cuboid(c + rot * Vec3::new(0.0, 0.46, 0.0), Vec3::new(1.9, 0.07, 0.5), rot);
            wood_g.cuboid(c + rot * Vec3::new(0.0, 0.8, 0.24), Vec3::new(1.9, 0.4, 0.06), rot);
            for xs in [-0.8_f32, 0.8] {
                iron.cuboid(c + rot * Vec3::new(xs, 0.23, 0.05), Vec3::new(0.08, 0.46, 0.5), rot);
            }
        }
        for xs in [-1.0_f32, 1.0] {
            let c = Vec3::new(xs * 5.7, PLAZA_TOP, 11.0);
            stone.add(
                &Cylinder::new(0.75, 0.6).mesh().resolution(16).build(),
                Transform::from_translation(c + Vec3::Y * 0.3),
            );
            bulbs_p.ball(&bulb, c + Vec3::new(0.25, 0.75, 0.1), 0.12);
            bulbs_w.ball(&bulb, c + Vec3::new(-0.2, 0.72, -0.2), 0.12);
        }

        let mut spawn_list: Vec<(Geo, Handle<StandardMaterial>, bool)> = vec![
            (stone, concrete.clone(), false),
            (paver_g, pavers.clone(), false),
            (band_g, band.clone(), false),
            (dais_g, dais_top.clone(), false),
            (wood_g, wood.clone(), false),
            (brass_g, brass.clone(), false),
            (rope_g, rope.clone(), false),
            (iron, steel_dark.clone(), false),
            (booth_g, booth_red.clone(), false),
            (stripe_g, stripes.clone(), false),
            (dark_glass, steel_dark.clone(), false),
            (tickets_g, tickets.clone(), true),
            (globes, globe.clone(), true),
            (bulbs_w, warm_bulb.clone(), true),
            (bulbs_p, pink_bulb.clone(), true),
            (bulbs_t, teal_bulb.clone(), true),
        ];
        // The arch stands where the follow camera often hangs while the hero faces the wheel
        // from the dais: it gets its own ghosting group (a separate root, so it never nests in
        // the wheel's kaiju-scale group) and turns see-through when it blocks the view.
        let mut arch_list: Vec<(Geo, Handle<StandardMaterial>, bool)> = vec![
            (navy_g, steel_navy.clone(), false),
            (arch_stone, concrete.clone(), false),
            (arch_brass, brass.clone(), false),
            (arch_board, booth_red.clone(), false),
            (sign_g, sign.clone(), true),
            (gold, gold_glow.clone(), true),
        ];
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                crate::gfx::CityOccluderGroup,
                Transform::from_translation(WHEEL_CENTER).with_rotation(wheel_rot()),
                Visibility::default(),
            ))
            .with_children(|a| {
                a.spawn((Transform::default(), Visibility::default())).with_children(|arch| {
                    for (g, m, glow) in arch_list.drain(..) {
                        part(arch, meshes, g.into_mesh(), &m, glow);
                    }
                });
            });
        commands
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|b| {
                for (g, m, glow) in spawn_list.drain(..) {
                    if !g.pos.is_empty() {
                        part(b, meshes, g.into_mesh(), &m, glow);
                    }
                }
            })
            .id()
    };

    // ---- A-frame support ----
    let [navy_m, dark_m, plinth_m, leg_bulbs] = frame_meshes(low, &bulb);
    let flag = meshes.add(pennant_mesh(1.6, 0.9));
    let frame = commands
        .spawn((Transform::default(), Visibility::default()))
        .with_children(|f| {
            part(f, meshes, navy_m, &steel_navy, false);
            part(f, meshes, dark_m, &steel_dark, false);
            part(f, meshes, plinth_m, &concrete, false);
            if !low {
                part(f, meshes, leg_bulbs, &warm_bulb, true);
            }
            for (i, zs) in [-1.0_f32, 1.0].into_iter().enumerate() {
                f.spawn((
                    Mesh3d(flag.clone()),
                    MeshMaterial3d(flag_mats[i].clone()),
                    Transform::from_xyz(0.0, HUB_Y + 3.7, zs * APEX_Z).with_rotation(Quat::from_rotation_y(-zs * 0.3)),
                ));
            }
        })
        .id();

    // ---- Rotor (turns about local Z at the hub) ----
    let [steel_m, hub_m, a_m, b_m, spoke_m, emblem_m] = rotor_meshes(low, &bulb);
    let rotor = commands
        .spawn((WheelRotor, Transform::from_xyz(0.0, HUB_Y, 0.0), Visibility::default()))
        .with_children(|r| {
            part(r, meshes, steel_m, &steel_white, false);
            part(r, meshes, hub_m, &hub_red, false);
            part(r, meshes, a_m, &chase_a, true);
            part(r, meshes, b_m, &chase_b, true);
            if !low {
                part(r, meshes, spoke_m, &spoke_bulb, true);
            }
            part(r, meshes, emblem_m, &gold_glow, true);
        })
        .id();

    // ---- Gondolas (children of the root, re-levelled every frame by `animate_wheel`) ----
    let g = gondola_meshes(&bulb).map(|m| meshes.add(m));
    let mut kids = vec![base, frame, rotor];
    for k in 0..GONDOLAS {
        let id = commands
            .spawn((
                WheelGondola(k),
                Transform::from_translation(pin_local(gondola_theta(0.37, k))),
                Visibility::default(),
            ))
            .with_children(|c| {
                let mats = [
                    steel_white.clone(),
                    paints[k % paints.len()].clone(),
                    roof_white.clone(),
                    glass.clone(),
                    wood.clone(),
                    warm_bulb.clone(),
                ];
                for (i, m) in mats.into_iter().enumerate() {
                    let mut e = c.spawn((Mesh3d(g[i].clone()), MeshMaterial3d(m), Transform::default()));
                    if i >= 3 {
                        e.insert(NotShadowCaster);
                    }
                }
            })
            .id();
        kids.push(id);
    }
    commands.entity(root).add_children(&kids);

    // ---- Boardwalk pier over the river corner (world axes) ----
    let (x0, x1, z0, z1) = (-135.0_f32, -97.0_f32, 100.75_f32, 135.0_f32);
    let deck = Vec3::new(x1 - x0, 0.12, z1 - z0);
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(crate::gfx::tiled_box(deck, deck, Vec2::splat(2.0), 0.0, None))),
        MeshMaterial3d(deck_wood),
        Transform::from_xyz((x0 + x1) * 0.5, 0.06, (z0 + z1) * 0.5),
    ));
    let mut piles = Geo::default();
    let mut rail = Geo::default();
    let mut lamp_iron = Geo::default();
    let mut lamp_globes = Geo::default();
    piles.cuboid(Vec3::new((x0 + x1) * 0.5, 0.02, z0 + 0.1), Vec3::new(x1 - x0, 0.26, 0.22), Quat::IDENTITY);
    let mut x = x0 + 1.0;
    while x < x1 {
        piles.rod(Vec3::new(x, -0.8, z0 - 0.12), Vec3::new(x, 0.3, z0 - 0.12), 0.17, 8);
        x += 2.4;
    }
    // Rails on the water side (west part of the north edge) and along the map edges.
    let rail_line = |g: &mut Geo, a: Vec3, b: Vec3| {
        let n = ((a.distance(b) / 2.0).ceil() as usize).max(1);
        for i in 0..=n {
            let p = a.lerp(b, i as f32 / n as f32);
            g.rod(p, p + Vec3::Y * 1.05, 0.05, 6);
        }
        g.rod(a + Vec3::Y * 1.05, b + Vec3::Y * 1.05, 0.045, 6);
        g.rod(a + Vec3::Y * 0.55, b + Vec3::Y * 0.55, 0.03, 6);
    };
    rail_line(&mut rail, Vec3::new(x0 + 0.3, 0.12, z0 + 0.3), Vec3::new(-108.0, 0.12, z0 + 0.3));
    rail_line(&mut rail, Vec3::new(x0 + 0.3, 0.12, z0 + 0.3), Vec3::new(x0 + 0.3, 0.12, z1 - 0.3));
    rail_line(&mut rail, Vec3::new(x0 + 0.3, 0.12, z1 - 0.3), Vec3::new(x1 - 0.5, 0.12, z1 - 0.3));
    for lx in [-131.0_f32, -121.0, -111.0] {
        let p = Vec3::new(lx, 0.12, z0 + 0.6);
        lamp_iron.rod(p, p + Vec3::Y * 4.0, 0.08, 8);
        lamp_iron.rod(p + Vec3::Y * 4.0, p + Vec3::new(0.0, 4.0, 0.6), 0.05, 6);
        lamp_globes.ball(&bulb, p + Vec3::new(0.0, 3.8, 0.6), 0.24);
    }
    for (g, m, glow) in [
        (piles, dark_wood, false),
        (rail, roof_white.clone(), false),
        (lamp_iron, steel_dark.clone(), false),
        (lamp_globes, globe.clone(), true),
    ] {
        let mut e = commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(meshes.add(g.into_mesh())),
            MeshMaterial3d(m),
            Transform::default(),
        ));
        if glow {
            e.insert(NotShadowCaster);
        }
    }

    // ---- Snack kiosks on the boardwalk (world axes) ----
    {
        let q = Quat::IDENTITY;
        let mut red = Geo::default();
        let mut teal = Geo::default();
        let mut stripe = Geo::default();
        let mut poles = Geo::default();
        let mut glass_g = Geo::default();
        let mut pop = Geo::default();
        let mut signs = Geo::default();
        let mut tyres = Geo::default();
        // Popcorn cart: counter facing the plaza (-x).
        let (cx, cz, _, _) = KIOSKS[0];
        let c = Vec3::new(cx, 0.12, cz);
        red.cuboid(c + Vec3::Y * 0.7, Vec3::new(1.3, 0.95, 1.6), q);
        glass_g.cuboid(c + Vec3::Y * 1.62, Vec3::new(1.0, 0.85, 1.3), q);
        pop.cuboid(c + Vec3::Y * 1.35, Vec3::new(0.9, 0.32, 1.2), q);
        for (dx, dz) in [(-0.6, -0.75), (0.6, -0.75), (-0.6, 0.75), (0.6, 0.75)] {
            poles.rod(c + Vec3::new(dx, 1.15, dz), c + Vec3::new(dx, 2.75, dz), 0.035, 6);
        }
        stripe.cuboid(c + Vec3::Y * 2.8, Vec3::new(1.7, 0.12, 2.0), q);
        for zs in [-1.0_f32, 1.0] {
            tyres.add(
                &Cylinder::new(0.38, 0.12).mesh().resolution(16).build(),
                Transform::from_translation(c + Vec3::new(0.25, 0.38, zs * 0.84))
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
        }
        let sq = Mesh::from(Rectangle::new(1.6, 1.6 * 88.0 / 360.0));
        red.cuboid(c + Vec3::new(0.0, 3.1, 0.0), Vec3::new(0.08, 0.44, 1.7), q);
        signs.add(&sq, Transform::from_translation(c + Vec3::new(-0.045, 3.1, 0.0)).with_rotation(Quat::from_rotation_y(-FRAC_PI_2)));
        signs.add(&sq, Transform::from_translation(c + Vec3::new(0.045, 3.1, 0.0)).with_rotation(Quat::from_rotation_y(FRAC_PI_2)));
        // Snack stand: counter facing the boardwalk (+z), back wall, striped awning.
        let (sx, sz, hx, hz) = KIOSKS[1];
        let s = Vec3::new(sx, 0.12, sz);
        teal.cuboid(s + Vec3::Y * 0.55, Vec3::new(hx * 2.0, 1.1, hz * 2.0 - 0.2), q);
        teal.cuboid(s + Vec3::new(0.0, 1.75, -hz + 0.1), Vec3::new(hx * 2.0, 1.3, 0.12), q);
        stripe.cuboid(
            s + Vec3::new(0.0, 2.55, 0.15),
            Vec3::new(hx * 2.0 + 0.3, 0.1, hz * 2.0 + 0.5),
            Quat::from_rotation_x(-0.18),
        );
        for xs in [-1.0_f32, 1.0] {
            poles.rod(s + Vec3::new(xs * hx, 1.1, hz - 0.1), s + Vec3::new(xs * hx, 2.6, hz - 0.1), 0.04, 6);
        }
        pop.cuboid(s + Vec3::new(-0.5, 1.2, 0.2), Vec3::new(0.5, 0.2, 0.4), q);
        pop.cuboid(s + Vec3::new(0.45, 1.2, 0.25), Vec3::new(0.4, 0.2, 0.35), q);
        for (g, m, glow) in [
            (red, booth_red.clone(), false),
            (teal, kiosk_teal, false),
            (stripe, stripes.clone(), false),
            (poles, brass.clone(), false),
            (glass_g, glass.clone(), true),
            (pop, popcorn_glow, true),
            (signs, snack_sign, true),
            (tyres, steel_dark.clone(), false),
        ] {
            let mut e = commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(meshes.add(g.into_mesh())),
                MeshMaterial3d(m),
                Transform::default(),
            ));
            if glow {
                e.insert(NotShadowCaster);
            }
        }
    }

    // ---- Warm light over the plaza (and one in the hub unless `?gfx=low`) ----
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        PointLight {
            color: Color::srgb(1.0, 0.82, 0.58),
            intensity: 70_000.0,
            range: 30.0,
            ..default()
        },
        Transform::from_translation(to_world(Vec3::new(0.0, 6.5, 7.5))),
    ));
    if !low {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            PointLight {
                color: Color::srgb(1.0, 0.70, 0.80),
                intensity: 55_000.0,
                range: 26.0,
                ..default()
            },
            Transform::from_translation(to_world(Vec3::new(0.0, HUB_Y, 4.5))),
        ));
    }

    // ---- Colliders: plaza and dais (stepped discs), plinths, booth, arch posts, kiosks.
    // The turning wheel and the light props are not solid.
    let mut boxes = disc_boxes(WHEEL_CENTER, PLAZA_R, 0.0, PLAZA_TOP, 16);
    boxes.extend(disc_boxes(to_world(Vec3::new(0.0, 0.0, DAIS_Z)), DAIS_R, 0.0, DAIS_TOP, 8));
    for xs in [-1.0_f32, 1.0] {
        for zs in [-1.0_f32, 1.0] {
            let p = to_world(Vec3::new(xs * FOOT_X, 0.0, zs * FOOT_Z));
            boxes.push((Vec3::new(p.x, PLINTH_TOP * 0.5, p.z), Vec3::new(1.0, PLINTH_TOP * 0.5, 1.0)));
        }
        let p = to_world(Vec3::new(xs * 3.4, 0.0, 11.5));
        boxes.push((Vec3::new(p.x, 3.2, p.z), Vec3::new(0.32, 3.2, 0.32)));
    }
    let bc = to_world(Vec3::new(5.2, 0.0, 8.4));
    boxes.push((Vec3::new(bc.x, 1.3, bc.z), Vec3::new(1.05, 1.3, 1.05)));
    for &(kx, kz, hx, hz) in &KIOSKS {
        boxes.push((Vec3::new(kx, 1.45, kz), Vec3::new(hx, 1.45, hz)));
    }
    for (c, half) in boxes {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Transform::from_translation(c),
            PlatformBox { half_extents: half },
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------------------------

/// Turns the wheel, levels the gondolas, runs the bulb chase, and marks the wheel as an occluder
/// once the hero is kaiju-sized (at normal size the camera rides through it, and a ghosted
/// wheel would flicker as the follow camera passes its legs).
#[allow(clippy::too_many_arguments)]
fn animate_wheel(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<PlayerStats>,
    ride: Res<FerrisRide>,
    mut wheel: ResMut<FerrisWheel>,
    lights: Option<Res<WheelLights>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    root_q: Query<(Entity, Has<crate::gfx::CityOccluderGroup>), With<WheelRoot>>,
    mut rotor_q: Query<&mut Transform, With<WheelRotor>>,
    mut gondola_q: Query<(&WheelGondola, &mut Transform), Without<WheelRotor>>,
) {
    let Ok((root, ghostable)) = root_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    wheel.clock += dt;

    // The operator eases the wheel down to a crawl as the rider's gondola nears the dais (to
    // board and to let him out), and runs it up to 3x to bring his gondola down after "hop
    // off". The speed follows the remaining angle, so it brakes smoothly into the platform.
    let approach = |rem: f32, top: f32| (0.35 + rem).clamp(0.35, top);
    let target = match ride.phase {
        RidePhase::Waiting | RidePhase::StepIn => {
            let r = to_bottom(wheel.angle, ride.gondola);
            if r > TAU - 0.15 { 0.35 } else { approach(r, 1.0) }
        }
        RidePhase::Riding => {
            let rem = (ride.exit_at - ride.travelled).max(0.0);
            approach(rem, if ride.exit_requested { 3.0 } else { 1.0 })
        }
        RidePhase::StepOut => 0.35,
        RidePhase::Idle => 1.0,
    };
    let k = 1.0 - (-1.8 * dt).exp();
    wheel.speed += (target - wheel.speed) * k;
    // Debug speed-up (headless demos) never applies while the hero steps in or out.
    let debug_mul = match ride.phase {
        RidePhase::StepIn | RidePhase::StepOut => 1.0,
        RidePhase::Waiting => wheel.debug_speed.min(3.0),
        _ => wheel.debug_speed,
    };
    let omega = TAU / REV_SECS * wheel.speed * debug_mul;
    wheel.step = omega * dt;
    wheel.angle = (wheel.angle + wheel.step).rem_euclid(TAU);
    // Gondolas lag a touch when the wheel changes speed: a damped spring toward the pendulum
    // angle of the rim's tangential acceleration, scaled down and capped at about one degree
    // so the in-gondola camera stays steady.
    if dt > 0.0 {
        let tangential = (omega - wheel.last_omega) / dt * RIM_R;
        let goal = (-tangential / 9.81 * 0.5).clamp(-0.02, 0.02);
        let acc = (goal - wheel.sway) * 10.0 - wheel.sway_vel * 5.0;
        wheel.sway_vel += acc * dt;
        wheel.sway = (wheel.sway + wheel.sway_vel * dt).clamp(-0.03, 0.03);
    }
    wheel.last_omega = omega;

    for mut tf in &mut rotor_q {
        tf.translation = Vec3::new(0.0, HUB_Y, 0.0);
        tf.rotation = Quat::from_rotation_z(wheel.angle);
    }
    for (g, mut tf) in &mut gondola_q {
        tf.translation = pin_local(gondola_theta(wheel.angle, g.0));
        tf.rotation = Quat::from_rotation_z(wheel.gondola_swing(g.0));
    }

    if let Some(lights) = lights {
        let t = wheel.clock;
        let s = 0.5 + 0.5 * (t * 2.4).sin();
        let warm = Vec3::new(2.8, 2.1, 1.1);
        for (h, w) in [(&lights.chase_a, 0.35 + 0.65 * s), (&lights.chase_b, 0.35 + 0.65 * (1.0 - s))] {
            if let Some(m) = materials.get_mut(h) {
                let e = warm * w;
                m.emissive = LinearRgba::rgb(e.x, e.y, e.z);
            }
        }
        if let Some(m) = materials.get_mut(&lights.spokes) {
            let w = 0.7 + 0.3 * (t * 1.1).sin();
            m.emissive = LinearRgba::rgb(2.6 * w, 0.9 * w, 1.9 * w);
        }
    }

    if !ghostable && stats.in_city && stats.player_scale > 5.0 {
        commands.entity(root).insert(crate::gfx::CityOccluderGroup);
    }
}

// ---------------------------------------------------------------------------------------------
// Riding
// ---------------------------------------------------------------------------------------------

fn next_gondola(angle: f32) -> usize {
    (0..GONDOLAS)
        .min_by(|&a, &b| {
            let lead = |k: usize| {
                let r = to_bottom(angle, k);
                if r < 0.12 { r + TAU } else { r }
            };
            lead(a).total_cmp(&lead(b))
        })
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn ferris_ride_update(
    time: Res<Time>,
    fade: Res<SceneFade>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    wheel: Res<FerrisWheel>,
    mut ride: ResMut<FerrisRide>,
    mut prompt: ResMut<WheelPrompt>,
    mut player_q: Query<(&mut Transform, &mut Player, &mut Visibility)>,
    root_q: Query<(), With<WheelRoot>>,
    cam_q: Query<(&Transform, &Projection), (With<Camera3d>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let pressed = std::mem::take(&mut prompt.pressed) && prompt.actionable;
    prompt.label = None;
    prompt.actionable = false;
    let Ok((mut tf, mut player, mut vis)) = player_q.get_single_mut() else {
        return;
    };
    // Something else ended the ride (a pause Restart or retry resets the stats): drop it here.
    if ride.active() && !stats.riding_wheel {
        ride.clear();
        set_crew_wait(None);
        *vis = Visibility::Visible;
    }
    if root_q.is_empty() {
        return;
    }
    let p = tf.translation;
    let spot = to_world(BOARD_SPOT);
    let fwd = front_dir();
    let out_yaw = yaw_facing(fwd);
    let in_yaw = yaw_facing(-fwd);

    if ride.active() {
        // Untouchable for the whole ride: every hit check in the city wants this at zero.
        player.invuln_timer = player.invuln_timer.max(0.5);
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        set_crew_wait(Some(CREW_WAIT));
    }

    match ride.phase {
        RidePhase::Idle => {
            let d_center = flat_dist(p, WHEEL_CENTER);
            if d_center > PLAZA_R + 8.0 {
                ride.offer_hinted = false;
                ride.blocked_hinted = false;
            }
            if !ride.welcomed && d_center < WELCOME_RANGE {
                ride.welcomed = true;
                stats.hint_text = "THE RIVER STAR! Ride the Ferris wheel from its platform on the pier.".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
            let in_vehicle = stats.driving_car.is_some() || stats.riding_scooter.is_some();
            let oversized = stats.player_scale > RIDER_MAX_SCALE;
            let on_dais = flat_dist(p, spot) < BOARD_REACH && (p.y - DAIS_TOP).abs() < 0.6;
            if on_dais && !in_vehicle && !oversized && !stats.phone_open {
                prompt.label = Some("RIDE THE WHEEL [F/E]".to_string());
                prompt.actionable = true;
                if !ride.offer_hinted {
                    ride.offer_hinted = true;
                    stats.hint_text = "[F / E] Ride the RIVER STAR: two laps, hop off at the bottom.".to_string();
                    stats.hint_bump = stats.hint_bump.wrapping_add(1);
                }
                if pressed && fade.phase == FadePhase::Idle {
                    ride.phase = RidePhase::Waiting;
                    ride.t = 0.0;
                    ride.from = p;
                    ride.from_yaw = player.facing_yaw;
                    ride.gondola = next_gondola(wheel.angle);
                    ride.saved_yaw = orbit.yaw;
                    ride.saved_pitch = orbit.pitch;
                    ride.saved_dist = orbit.distance;
                    ride.saved_lock_on = stats.lock_on_active;
                    ride.exit_requested = false;
                    ride.travelled = 0.0;
                    ride.view_out = 0.0;
                    stats.lock_on_active = false;
                    stats.riding_wheel = true;
                    player.velocity = Vec3::ZERO;
                    player.knockback = Vec3::ZERO;
                    player.invuln_timer = player.invuln_timer.max(0.5);
                    set_crew_wait(Some(CREW_WAIT));
                    play_sfx("hack_success");
                    stats.hint_text = "Your gondola is coming down to the platform...".to_string();
                    stats.hint_bump = stats.hint_bump.wrapping_add(1);
                }
            } else if !ride.blocked_hinted && d_center < PLAZA_R + 3.0 + stats.player_scale * 0.5 {
                let line = if in_vehicle {
                    Some("Hop off to ride the wheel: walk up onto its boarding platform.")
                } else if oversized {
                    Some("Too big for the gondolas! The wheel only fits a normal-sized hero.")
                } else {
                    None
                };
                if let Some(line) = line {
                    ride.blocked_hinted = true;
                    stats.hint_text = line.to_string();
                    stats.hint_bump = stats.hint_bump.wrapping_add(1);
                }
            }
        }
        RidePhase::Waiting => {
            ride.t += dt;
            let a = smooth01(ride.t / 0.45);
            tf.translation = ride.from.lerp(spot, a);
            player.facing_yaw = lerp_angle(ride.from_yaw, in_yaw, a);
            tf.rotation = Quat::from_rotation_y(player.facing_yaw);
            prompt.label = Some("GONDOLA ARRIVING...".to_string());
            let r = to_bottom(wheel.angle, ride.gondola);
            if ride.t >= 0.45 && (r < STEP_LEAD || r > TAU - 0.05) {
                ride.phase = RidePhase::StepIn;
                ride.t = 0.0;
                ride.from = tf.translation;
                if let Ok((c_tf, proj)) = cam_q.get_single() {
                    let fov = match proj {
                        Projection::Perspective(pp) => pp.fov,
                        _ => FRAC_PI_4,
                    };
                    ride.cam_from = (c_tf.translation, c_tf.rotation, fov);
                }
                // Seated view: looking out of the door window toward the city.
                orbit.yaw = out_yaw;
                orbit.pitch = 0.1;
                ride.zoom_base = orbit.distance;
            }
        }
        RidePhase::StepIn => {
            ride.t += dt;
            let s = smooth01(ride.t / STEP_SECS);
            let floor = wheel.cabin_floor(ride.gondola);
            tf.translation = ride.from.lerp(floor, s) + Vec3::Y * (s * PI).sin() * 0.12;
            player.facing_yaw = if s < 0.6 { in_yaw } else { lerp_angle(in_yaw, out_yaw, (s - 0.6) / 0.4) };
            tf.rotation = Quat::from_rotation_y(player.facing_yaw);
            prompt.label = Some("ALL ABOARD!".to_string());
            let want = if s > 0.7 { Visibility::Hidden } else { Visibility::Visible };
            if *vis != want {
                *vis = want;
            }
            if ride.t >= STEP_SECS {
                ride.phase = RidePhase::Riding;
                ride.t = 0.0;
                ride.travelled = 0.0;
                ride.exit_at = RIDE_LAPS * TAU;
                play_sfx("car_door");
                stats.hint_text = "Enjoy the view! Look around; scroll out for an outside view.".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
        }
        RidePhase::Riding => {
            ride.t += dt;
            ride.travelled += wheel.step;
            tf.translation = wheel.cabin_floor(ride.gondola);
            player.facing_yaw = out_yaw;
            tf.rotation = Quat::from_rotation_y(out_yaw);
            let want = if ride.view_out > 0.4 { Visibility::Visible } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
            if pressed && !ride.exit_requested {
                ride.exit_requested = true;
                ride.exit_at = ((ride.travelled / TAU).floor() + 1.0).min(RIDE_LAPS) * TAU;
                stats.hint_text = "Hopping off when your gondola is back at the platform.".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
            prompt.label = Some(if ride.exit_requested {
                "HOPPING OFF AT THE BOTTOM...".to_string()
            } else {
                "HOP OFF AT THE BOTTOM [F/E]".to_string()
            });
            prompt.actionable = !ride.exit_requested;
            if ride.travelled >= ride.exit_at - 0.02 {
                ride.completed_full_ride = !ride.exit_requested && ride.travelled >= RIDE_LAPS * TAU - 0.15;
                ride.phase = RidePhase::StepOut;
                ride.t = 0.0;
                // Hand the camera back: behind the hero as he steps out facing the city, at the
                // pitch and zoom he had before the ride; `update_camera` eases into it.
                orbit.yaw = out_yaw;
                orbit.pitch = ride.saved_pitch;
                orbit.distance = ride.saved_dist;
                orbit.blend_request = true;
                orbit.snap_follow = true;
                *vis = Visibility::Visible;
                play_sfx("car_door");
            }
        }
        RidePhase::StepOut => {
            ride.t += dt;
            let s = smooth01(ride.t / STEP_SECS);
            let floor = wheel.cabin_floor(ride.gondola);
            tf.translation = floor.lerp(spot, s) + Vec3::Y * (s * PI).sin() * 0.12;
            player.facing_yaw = out_yaw;
            tf.rotation = Quat::from_rotation_y(out_yaw);
            prompt.label = Some("THANKS FOR RIDING!".to_string());
            if ride.t >= STEP_SECS {
                let full_reward = ride.completed_full_ride;
                tf.translation = spot;
                player.velocity = Vec3::ZERO;
                player.grounded = true;
                player.jumps_used = 0;
                player.invuln_timer = player.invuln_timer.max(1.0);
                stats.riding_wheel = false;
                stats.lock_on_active = ride.saved_lock_on;
                ride.clear();
                ride.offer_hinted = true;
                ride.blocked_hinted = true;
                set_crew_wait(None);
                *vis = Visibility::Visible;
                if full_reward {
                    let first_time = !stats.has_ferris_hat;
                    stats.has_ferris_hat = true;
                    stats.score += 1000;
                    stats.add_tendy_wendies(if first_time { 25 } else { 10 });
                    play_sfx("win");
                    play_sfx("powerup");
                    stats.hint_text = if first_time {
                        "FULL RIVER STAR RIDE COMPLETE! Unlocked the COOL RIVER STAR PROPELLER HAT! (+25 TW)"
                            .to_string()
                    } else {
                        "FULL RIVER STAR RIDE COMPLETE! Propeller Hat Turbo Sparkle! (+10 TW)".to_string()
                    };
                    speak_voice("Full ride complete! You earned the cool River Star hat!", "narrator");
                } else {
                    stats.hint_text = "Back on the platform. Ride all the way to win the cool hat!".to_string();
                    speak_voice("Thanks for riding the River Star!", "narrator");
                }
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
        }
    }
}

/// Seated camera: inside the gondola at eye height looking out (free look from the orbit yaw /
/// pitch), or, zoomed out past the seated zoom, a chase view of the gondola. Analytic and
/// collision-free (the wheel is not solid), so the rim and spokes can't yank it around.
fn ferris_ride_camera(
    time: Res<Time>,
    mut ride: ResMut<FerrisRide>,
    wheel: Res<FerrisWheel>,
    orbit: Res<CameraOrbit>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    if !matches!(ride.phase, RidePhase::StepIn | RidePhase::Riding) {
        return;
    }
    let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let (pin, rot) = wheel.gondola_pose(ride.gondola);
    let eye = pin + rot * Vec3::new(0.0, -PIN_DROP + EYE_H, 0.0);
    let centre = pin + rot * Vec3::new(0.0, -PIN_DROP + 1.2, 0.0);
    let pitch = orbit.pitch.clamp(-0.3, 1.3);
    let look = Vec3::new(-orbit.yaw.sin() * pitch.cos(), -pitch.sin(), -orbit.yaw.cos() * pitch.cos());
    let look_h = Vec3::new(look.x, 0.0, look.z).normalize_or_zero();
    // A step back from the window being looked through keeps the frame in view.
    let inside = eye - look_h * 0.4;
    let zoom = (orbit.distance - ride.zoom_base).max(0.0);
    let want_out = smooth01((zoom - 0.5) / 3.0);
    ride.view_out += (want_out - ride.view_out) * (1.0 - (-5.0 * dt).exp());
    let v = ride.view_out;
    let chase_d = 7.5 + (zoom - 3.5).max(0.0) * 1.2;
    let mut chase = centre - look * chase_d;
    chase.y = chase.y.max(1.5);
    let pos = inside.lerp(chase, v);
    let focus = (inside + look).lerp(centre, v);
    let goal = Transform::from_translation(pos).looking_at(focus, Vec3::Y);
    let mut fov = 1.0 + (FRAC_PI_4 - 1.0) * v;
    if ride.phase == RidePhase::StepIn {
        let s = smooth01(ride.t / STEP_SECS);
        c_tf.translation = ride.cam_from.0.lerp(goal.translation, s);
        c_tf.rotation = ride.cam_from.1.slerp(goal.rotation, s);
        fov = ride.cam_from.2 + (fov - ride.cam_from.2) * s;
    } else {
        c_tf.translation = goal.translation;
        c_tf.rotation = goal.rotation;
    }
    if let Projection::Perspective(ref mut p) = *proj {
        p.fov = fov;
    }
}

/// Radar icon for the wheel and the ride panel (lap, height, progress).
#[allow(clippy::too_many_arguments)]
fn update_wheel_ui(
    mut commands: Commands,
    ride: Res<FerrisRide>,
    wheel: Res<FerrisWheel>,
    root_q: Query<(), With<WheelRoot>>,
    dot_q: Query<&Parent, With<CityMinimapPlayerDot>>,
    icon_q: Query<(), With<WheelRadarIcon>>,
    hud_q: Query<Entity, With<WheelRideHud>>,
    mut text_q: Query<&mut Text, With<WheelRideHudText>>,
    mut fill_q: Query<&mut Node, With<WheelRideHudFill>>,
) {
    if root_q.is_empty() {
        return;
    }
    if icon_q.is_empty() {
        if let Ok(radar) = dot_q.get_single() {
            let pct = |v: f32| {
                ((v + crate::city::CITY_HALF_EXTENT) / (crate::city::CITY_HALF_EXTENT * 2.0) * 100.0).clamp(3.0, 97.0)
            };
            let icon = commands
                .spawn((
                    WheelRadarIcon,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(pct(WHEEL_CENTER.x)),
                        top: Val::Percent(pct(WHEEL_CENTER.z)),
                        width: Val::Px(12.0),
                        height: Val::Px(12.0),
                        margin: UiRect::all(Val::Px(-6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.95, 0.55, 0.95, 0.22)),
                    BorderColor(Color::srgb(0.98, 0.74, 1.0)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|c| {
                    c.spawn((
                        Node {
                            width: Val::Px(3.0),
                            height: Val::Px(3.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        BorderRadius::all(Val::Px(1.5)),
                    ));
                })
                .id();
            // First child, so the hero's dot draws over it.
            commands.entity(radar.get()).insert_children(0, &[icon]);
        }
    }

    if !ride.active() {
        for e in &hud_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }
    let height = wheel.cabin_floor(ride.gondola).y;
    let (line, progress) = match ride.phase {
        RidePhase::Waiting => ("RIVER STAR  |  Your gondola is coming down...".to_string(), 0.0),
        RidePhase::StepIn => ("RIVER STAR  |  All aboard!".to_string(), 0.0),
        RidePhase::Riding => {
            let lap = ((ride.travelled / TAU).floor() as u32 + 1).min(RIDE_LAPS as u32);
            let tail = if ride.exit_requested {
                "hopping off at the bottom".to_string()
            } else {
                format!("lap {} of {}", lap, RIDE_LAPS as u32)
            };
            (
                format!("RIVER STAR  |  {}  |  {:.0} m up", tail, height.max(0.0)),
                (ride.travelled / ride.exit_at.max(0.01)).clamp(0.0, 1.0),
            )
        }
        _ => ("RIVER STAR  |  Thanks for riding!".to_string(), 1.0),
    };
    if hud_q.is_empty() {
        commands
            .spawn((
                WheelRideHud,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(62.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|root| {
                root.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                        row_gap: Val::Px(5.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.05, 0.14, 0.86)),
                    BorderColor(Color::srgb(0.98, 0.74, 1.0)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        WheelRideHudText,
                        Text::new(line.clone()),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.90, 0.62)),
                    ));
                    panel
                        .spawn((
                            Node {
                                width: Val::Px(220.0),
                                height: Val::Px(7.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                            BorderRadius::all(Val::Px(4.0)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                WheelRideHudFill,
                                Node {
                                    width: Val::Percent(progress * 100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.98, 0.62, 0.92)),
                                BorderRadius::all(Val::Px(4.0)),
                            ));
                        });
                });
            });
        return;
    }
    if let Ok(mut t) = text_q.get_single_mut() {
        if t.0 != line {
            t.0 = line;
        }
    }
    if let Ok(mut n) = fill_q.get_single_mut() {
        n.width = Val::Percent(progress * 100.0);
    }
}

/// OnExit(CityPlay): pause Restart, death retry, Clear save and cutscene triggers all leave
/// CityPlay, so this is the one place a ride is torn down when play stops mid-ride.
fn reset_ride_on_exit(
    mut commands: Commands,
    mut ride: ResMut<FerrisRide>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut prompt: ResMut<WheelPrompt>,
    hud_q: Query<Entity, With<WheelRideHud>>,
) {
    if ride.active() {
        orbit.pitch = ride.saved_pitch;
        orbit.distance = ride.saved_dist;
        orbit.current_dist = ride.saved_dist;
        stats.lock_on_active = ride.saved_lock_on;
    }
    ride.clear();
    stats.riding_wheel = false;
    *prompt = WheelPrompt::default();
    set_crew_wait(None);
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
}

// ---------------------------------------------------------------------------------------------
// Debug (`?debug=1&wheel_demo=...`, bridged through audio.rs / index.html)
// ---------------------------------------------------------------------------------------------

/// `city_start=wheel...` poses for `city::debug_city_start`: (position, facing) and an optional
/// (orbit pitch, orbit distance).
pub fn debug_start_pose(mode: &str) -> Option<(Vec3, Vec3, Option<(f32, f32)>)> {
    let fwd = front_dir();
    let towards = |from: Vec3| (Vec3::new(WHEEL_CENTER.x, 0.0, WHEEL_CENTER.z) - from).normalize();
    match mode {
        // On the dais by the door, facing the gondolas (inside the ride prompt's reach).
        "wheel" => Some((to_world(BOARD_SPOT) + fwd * 0.6, -fwd, Some((0.3, 9.0)))),
        // Street level at the avenue corner, looking up at the wheel.
        "wheelst" => {
            let p = Vec3::new(-80.5, 0.0, 103.0);
            Some((p, towards(p), Some((-0.28, 8.0))))
        }
        // Down the z = 92 street from the middle of town: the wheel over the rooftops.
        "wheelfar" => {
            let p = Vec3::new(-6.0, 0.0, 92.0);
            Some((p, towards(p), Some((0.05, 10.0))))
        }
        // From the bridge over the river, upstream.
        "wheelriver" => {
            let p = Vec3::new(-110.0, 0.0, 46.0);
            Some((p, towards(p), Some((0.02, 10.0))))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn ferris_debug(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    ride: Res<FerrisRide>,
    mut dbg: ResMut<FerrisDebug>,
    mut wheel: ResMut<FerrisWheel>,
    mut prompt: ResMut<WheelPrompt>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player, &Visibility)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<Player>)>,
) {
    if !dbg.read {
        dbg.read = true;
        dbg.mode = crate::audio::debug_wheel_demo();
        wheel.debug_speed = if matches!(dbg.mode.as_str(), "fast" | "exit" | "spin") { 10.0 } else { 1.0 };
    }
    let city_start = crate::audio::debug_city_start();
    if dbg.mode.is_empty() && !city_start.starts_with("wheel") {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    dbg.city_t += dt;
    let Ok((mut tf, mut player, vis)) = player_q.get_single_mut() else {
        return;
    };
    let rides = matches!(dbg.mode.as_str(), "ride" | "fast" | "exit");
    if rides && !dbg.boarded && dbg.city_t > 1.0 && ride.phase == RidePhase::Idle && prompt.actionable {
        dbg.boarded = true;
        prompt.pressed = true;
    }
    if dbg.mode == "exit" && !dbg.exit_sent && ride.phase == RidePhase::Riding && ride.travelled > 1.2 {
        dbg.exit_sent = true;
        prompt.pressed = true;
    }
    if dbg.mode == "titan" && !dbg.placed && dbg.city_t > 0.5 {
        dbg.placed = true;
        let p = Vec3::new(-70.0, 0.0, 70.0);
        let face = (Vec3::new(WHEEL_CENTER.x, 0.0, WHEEL_CENTER.z) - p).normalize();
        tf.translation = p;
        player.facing_yaw = yaw_facing(face);
        tf.rotation = Quat::from_rotation_y(player.facing_yaw);
        orbit.yaw = player.facing_yaw;
        orbit.snap_follow = true;
    }
    dbg.tele_t -= dt;
    if dbg.tele_t <= 0.0 {
        dbg.tele_t = 0.25;
        let c = cam_q.get_single().map(|c| c.translation).unwrap_or_default();
        let floor = wheel.cabin_floor(ride.gondola);
        let line = format!(
            "phase={:?} k={} travelled={:.2} exit_at={:.2} riding={} speed={:.2} angle={:.2} sway={:.4} hero=({:.1},{:.2},{:.1}) floor=({:.1},{:.2},{:.1}) vis={:?} cam=({:.1},{:.1},{:.1}) view_out={:.2} prompt={:?} scale={:.2} invuln={:.2}",
            ride.phase,
            ride.gondola,
            ride.travelled,
            ride.exit_at,
            stats.riding_wheel,
            wheel.speed,
            wheel.angle,
            wheel.gondola_swing(ride.gondola),
            tf.translation.x,
            tf.translation.y,
            tf.translation.z,
            floor.x,
            floor.y,
            floor.z,
            vis,
            c.x,
            c.y,
            c.z,
            ride.view_out,
            prompt.label,
            stats.player_scale,
            player.invuln_timer,
        );
        crate::audio::set_wheel_telemetry(&line);
    }
}
