//! Hero ragdoll: Stan goes limp and tumbles when hit by a car, after a failed trick landing or
//! when the scooter hits a wall head on, then gets back up.
//!
//! The sim is a hand-rolled Verlet point/stick body ([`Body`]) with no Bevy dependencies beyond
//! the math types, so it is unit tested natively. The Bevy side ([`update_ragdoll`]) maps the
//! points onto the six rigid parts of `plaid_hero.glb` (`HeroTorso`, `HeroHead`, arms, legs),
//! moves the `Player` entity with the pelvis (camera, radar, pickups and multiplayer follow it)
//! and suspends `player_movement` / `animate_player_limbs` until Stan is standing again.
//!
//! Triggers call [`Ragdoll::request`]; the request is picked up the same frame by
//! [`update_ragdoll`], which also takes Stan off the scooter through the normal dismount path.

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

use crate::player::Player;
use crate::scooter::{CityScooter, ScootPromptUi};
use crate::state::{GameState, PlatformBox, PlayerStats};
use crate::stunts::{ramp_exit, ramp_probe, StuntRamp, StuntState};

// =================================================================================================
// Tuning
// =================================================================================================

/// No ragdoll at or above this gameplay scale (kaiju / titan sizes just keep the old reactions).
pub const RAGDOLL_MAX_SCALE: f32 = 4.0;
/// A street / park landing more than this far (radians) off a whole flip rotation wipes out.
pub const FLIP_FAIL_RAD: f32 = 60.0 * PI / 180.0;
/// An air spin only counts as a sideways landing once it turned at least this much (radians).
pub const SPIN_FAIL_MIN_RAD: f32 = FRAC_PI_2;
/// A spun landing within this many radians of perpendicular to the travel (mod 180) wipes out.
pub const SPIN_FAIL_SIDEWAYS_RAD: f32 = 30.0 * PI / 180.0;
/// Head-on scooter wall hits at or above this speed (m/s) throw the rider over the bars.
pub const WALL_RAGDOLL_SPEED: f32 = 9.0;

/// Fixed Verlet substep (s) and the most substeps run in one frame.
const H: f32 = 1.0 / 120.0;
const MAX_SUBSTEPS: u32 = 8;
const CONSTRAINT_ITERS: usize = 6;
const GRAVITY: f32 = 24.0;
/// Per-substep velocity retention in the air.
const AIR_DAMP: f32 = 0.998;
const RESTITUTION: f32 = 0.15;
const FRICTION: f32 = 0.6;
/// Per-point speed cap (m/s), so a silly impulse can never blow the body apart.
const MAX_POINT_SPEED: f32 = 45.0;
/// "Slow" body: every point under this speed (m/s per unit of body scale).
const SETTLE_SPEED: f32 = 0.7;
/// Slow for this long ends the tumble (Settling -> GetUp).
const SETTLE_SECS: f32 = 0.6;
/// Always lie at least this long, and never longer than the hard cap.
const MIN_SECS: f32 = 1.1;
const MAX_SECS: f32 = 4.5;
/// Blend from the limp pose back to the standing rest pose.
const GETUP_SECS: f32 = 0.5;
/// Car-hit invulnerability after standing up.
const POST_INVULN: f32 = 1.0;
/// Hard landings of a point faster than this (m/s) thump the camera and play a sound.
const IMPACT_SFX_SPEED: f32 = 7.0;

// =================================================================================================
// Body layout (model units of plaid_hero.glb: feet at y = 0, facing -Z)
// =================================================================================================

pub const HEAD: usize = 0;
pub const LS: usize = 1;
pub const RS: usize = 2;
pub const LH: usize = 3;
pub const RH: usize = 4;
pub const SPINE: usize = 5;
pub const LHAND: usize = 6;
pub const RHAND: usize = 7;
pub const LFOOT: usize = 8;
pub const RFOOT: usize = 9;
pub const N: usize = 10;

/// Rest positions of the points in model space.
const REST: [Vec3; N] = [
    Vec3::new(0.0, 1.84, -0.02),   // head centre
    Vec3::new(-0.35, 1.36, -0.02), // left shoulder (HeroLeftArm origin)
    Vec3::new(0.35, 1.36, -0.02),  // right shoulder
    Vec3::new(-0.19, 0.66, 0.0),   // left hip (HeroLeftLeg origin)
    Vec3::new(0.19, 0.66, 0.0),    // right hip
    Vec3::new(0.0, 1.05, 0.2),     // spine (behind the torso plane: makes the torso rigid in 3D)
    Vec3::new(-0.47, 0.86, -0.02), // left hand
    Vec3::new(0.47, 0.86, -0.02),  // right hand
    Vec3::new(-0.19, 0.12, 0.0),   // left foot
    Vec3::new(0.19, 0.12, 0.0),    // right foot
];
/// Collision sphere radius of each point (model units).
const RADIUS: [f32; N] = [0.30, 0.22, 0.22, 0.24, 0.24, 0.22, 0.11, 0.11, 0.12, 0.12];
/// Inverse masses: torso heavy, head medium, hands and feet light.
const INV_MASS: [f32; N] = [0.5, 0.34, 0.34, 0.34, 0.34, 0.34, 1.0, 1.0, 1.0, 1.0];

/// Part order used everywhere below.
const PART_NAMES: [&str; 6] = ["HeroTorso", "HeroHead", "HeroLeftArm", "HeroRightArm", "HeroLeftLeg", "HeroRightLeg"];
const P_TORSO: usize = 0;
const P_HEAD: usize = 1;
const P_LARM: usize = 2;
const P_RARM: usize = 3;
const P_LLEG: usize = 4;
const P_RLEG: usize = 5;
/// Authored node translations (scene root space); rotations are identity.
const PART_REST: [Vec3; 6] = [
    Vec3::new(0.0, 0.72, 0.0),
    Vec3::new(0.0, 1.52, -0.02),
    Vec3::new(-0.35, 1.36, -0.02),
    Vec3::new(0.35, 1.36, -0.02),
    Vec3::new(-0.19, 0.66, 0.0),
    Vec3::new(0.19, 0.66, 0.0),
];
/// Hand and foot points in their limb node's local frame (also the limb axis at rest).
const HAND_LOCAL_L: Vec3 = Vec3::new(-0.12, -0.50, 0.0);
const HAND_LOCAL_R: Vec3 = Vec3::new(0.12, -0.50, 0.0);
const FOOT_LOCAL: Vec3 = Vec3::new(0.0, -0.54, 0.0);
/// Head centre in the head node frame, spine point in the torso node frame.
const HEAD_LOCAL: Vec3 = Vec3::new(0.0, 0.32, 0.0);
const SPINE_LOCAL: Vec3 = Vec3::new(0.0, 0.33, 0.2);

#[derive(Clone, Copy, Debug)]
struct Stick {
    a: usize,
    b: usize,
    min: f32,
    max: f32,
}

/// Fixed-length (`rigid`) or ranged sticks, lengths in model units (scaled per body).
fn stick_layout() -> Vec<Stick> {
    let rest = |a: usize, b: usize| REST[a].distance(REST[b]);
    let rigid = |a: usize, b: usize| Stick { a, b, min: rest(a, b), max: rest(a, b) };
    let at_least = |a: usize, b: usize, d: f32| Stick { a, b, min: d, max: f32::INFINITY };
    let mut v = Vec::new();
    // Torso: shoulders, hips and the spine point, all pairs (a rigid tetrahedral block).
    let torso = [LS, RS, LH, RH, SPINE];
    for i in 0..torso.len() {
        for j in (i + 1)..torso.len() {
            v.push(rigid(torso[i], torso[j]));
        }
    }
    // Neck: hinged on the shoulder line, nodding limited by the distance to the spine.
    v.push(rigid(HEAD, LS));
    v.push(rigid(HEAD, RS));
    v.push(Stick { a: HEAD, b: SPINE, min: 0.72, max: 0.90 });
    // Limbs are single rigid pieces.
    v.push(rigid(LS, LHAND));
    v.push(rigid(RS, RHAND));
    v.push(rigid(LH, LFOOT));
    v.push(rigid(RH, RFOOT));
    // Anti-fold: keep hands and feet out of the torso and head.
    for (hand, other_s, other_h) in [(LHAND, RS, RH), (RHAND, LS, LH)] {
        v.push(at_least(hand, other_s, 0.45));
        v.push(at_least(hand, other_h, 0.35));
        v.push(at_least(hand, SPINE, 0.32));
        v.push(at_least(hand, HEAD, 0.35));
    }
    v.push(at_least(LFOOT, RFOOT, 0.18));
    for (foot, other_h) in [(LFOOT, RH), (RFOOT, LH)] {
        v.push(at_least(foot, LS, 0.7));
        v.push(at_least(foot, RS, 0.7));
        v.push(at_least(foot, SPINE, 0.5));
        v.push(at_least(foot, HEAD, 0.8));
        v.push(at_least(foot, other_h, 0.3));
    }
    v
}

// =================================================================================================
// Pure sim core
// =================================================================================================

/// Collision world for the points.
pub trait RagWorld {
    /// A sphere of radius `r` that moved from `prev` to `p` overlaps the world: returns the
    /// pushed-out centre and the contact normal.
    fn contact(&self, prev: Vec3, p: Vec3, r: f32) -> Option<(Vec3, Vec3)>;
}

/// Flat ground at `y` (unit tests).
#[cfg_attr(not(test), allow(dead_code))]
pub struct FlatGround(pub f32);

impl RagWorld for FlatGround {
    fn contact(&self, _prev: Vec3, p: Vec3, r: f32) -> Option<(Vec3, Vec3)> {
        (p.y < self.0 + r).then(|| (Vec3::new(p.x, self.0 + r, p.z), Vec3::Y))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StepInfo {
    /// Fastest point (m/s) after the step.
    pub max_speed: f32,
    /// Hardest normal impact (m/s) of a point against the world this step.
    pub impact: f32,
    /// Substeps run.
    pub substeps: u32,
}

/// Verlet point/stick ragdoll body in world units (`scale` x model units).
#[derive(Clone, Debug)]
pub struct Body {
    pub pos: [Vec3; N],
    pub prev: [Vec3; N],
    radius: [f32; N],
    sticks: Vec<Stick>,
    pub scale: f32,
    accum: f32,
}

impl Body {
    /// Rest pose placed in the world by `origin` (feet), `rot` and `scale`.
    pub fn rest_points(origin: Vec3, rot: Quat, scale: f32) -> [Vec3; N] {
        REST.map(|p| origin + rot * (p * scale))
    }

    /// Body at `points` (at rest: zero velocity). Stick lengths always come from the rest
    /// layout, never from `points`, so a twisted start pose can't become the body's shape.
    pub fn new(points: [Vec3; N], scale: f32) -> Body {
        let scale = scale.max(0.05);
        let sticks = stick_layout()
            .into_iter()
            .map(|s| Stick { min: s.min * scale, max: s.max * scale, ..s })
            .collect();
        Body { pos: points, prev: points, radius: RADIUS.map(|r| r * scale), sticks, scale, accum: 0.0 }
    }

    pub fn pelvis(&self) -> Vec3 {
        (self.pos[LH] + self.pos[RH]) * 0.5
    }

    fn com(&self) -> Vec3 {
        let mut sum = Vec3::ZERO;
        let mut w = 0.0;
        for i in 0..N {
            let m = 1.0 / INV_MASS[i];
            sum += self.pos[i] * m;
            w += m;
        }
        sum / w
    }

    /// Sets every point's velocity to `vel` plus a tumble of `spin` rad/s about the horizontal
    /// axis perpendicular to `axis_dir` (positive spin: the head goes along `axis_dir`).
    pub fn kick(&mut self, vel: Vec3, spin: f32, axis_dir: Vec3) {
        let flat = Vec3::new(axis_dir.x, 0.0, axis_dir.z).normalize_or(Vec3::NEG_Z);
        let omega = Vec3::Y.cross(flat) * spin;
        let c = self.com();
        for i in 0..N {
            let v = (vel + omega.cross(self.pos[i] - c)).clamp_length_max(MAX_POINT_SPEED);
            self.prev[i] = self.pos[i] - v * H;
        }
    }

    pub fn is_finite(&self) -> bool {
        self.pos.iter().chain(self.prev.iter()).all(|p| p.is_finite())
    }

    /// Advances by `dt` in fixed substeps.
    pub fn step(&mut self, dt: f32, world: &impl RagWorld) -> StepInfo {
        let mut info = StepInfo::default();
        self.accum += dt.max(0.0);
        while self.accum >= H && info.substeps < MAX_SUBSTEPS {
            self.accum -= H;
            info.substeps += 1;
            info.impact = info.impact.max(self.substep(world));
        }
        if info.substeps >= MAX_SUBSTEPS {
            self.accum = 0.0;
        }
        info.max_speed = (0..N).map(|i| (self.pos[i] - self.prev[i]).length() / H).fold(0.0, f32::max);
        info
    }

    fn substep(&mut self, world: &impl RagWorld) -> f32 {
        let start = self.pos;
        let mut v_in = [Vec3::ZERO; N];
        for i in 0..N {
            let v = ((self.pos[i] - self.prev[i]) * AIR_DAMP).clamp_length_max(MAX_POINT_SPEED * H);
            self.prev[i] = self.pos[i];
            self.pos[i] += v + Vec3::NEG_Y * GRAVITY * H * H;
            v_in[i] = self.pos[i] - self.prev[i];
        }
        let mut normal: [Option<Vec3>; N] = [None; N];
        for _ in 0..CONSTRAINT_ITERS {
            for s in &self.sticks {
                let d = self.pos[s.b] - self.pos[s.a];
                let len = d.length();
                if len < 1e-6 {
                    continue;
                }
                let target = len.clamp(s.min, s.max);
                if (len - target).abs() < 1e-7 {
                    continue;
                }
                let (wa, wb) = (INV_MASS[s.a], INV_MASS[s.b]);
                let corr = d * ((len - target) / len / (wa + wb));
                self.pos[s.a] += corr * wa;
                self.pos[s.b] -= corr * wb;
            }
            for i in 0..N {
                if let Some((q, n)) = world.contact(start[i], self.pos[i], self.radius[i]) {
                    self.pos[i] = q;
                    normal[i] = Some(n);
                }
            }
        }
        // Contact response: bounce a little, Coulomb friction on the tangential motion.
        let mut impact = 0.0_f32;
        for i in 0..N {
            let Some(n) = normal[i] else {
                continue;
            };
            let n = n.normalize_or(Vec3::Y);
            let into = (-v_in[i].dot(n)).max(0.0);
            impact = impact.max(into / H);
            let v = self.pos[i] - self.prev[i];
            let vn = v.dot(n);
            let vt = v - n * vn;
            let vn2 = vn.max(0.0).max(into * RESTITUTION);
            let vt_len = vt.length();
            let vt2 = if vt_len > 1e-9 {
                vt * ((vt_len - FRICTION * into).max(0.0) / vt_len) * 0.995
            } else {
                Vec3::ZERO
            };
            self.prev[i] = self.pos[i] - (n * vn2 + vt2);
        }
        impact
    }

    /// World pose (node origin, rotation) of each of the six hero parts.
    pub fn part_poses(&self) -> [(Vec3, Quat); 6] {
        let s = self.scale;
        let p = &self.pos;
        let (pel, rt) = torso_frame(p);
        let rt = rt * rest_torso_rot().inverse();
        let pel_rest = (REST[LH] + REST[RH]) * 0.5;
        let model = |m: Vec3| pel + rt * ((m - pel_rest) * s);
        let swing = |rest_dir: Vec3, from: Vec3, to: Vec3| {
            let cur = (to - from).normalize_or(rt * rest_dir);
            Quat::from_rotation_arc(rt * rest_dir, cur) * rt
        };
        let neck = model(PART_REST[P_HEAD]);
        let mut out = [(Vec3::ZERO, Quat::IDENTITY); 6];
        out[P_TORSO] = (model(PART_REST[P_TORSO]), rt);
        out[P_HEAD] = (neck, swing(HEAD_LOCAL.normalize(), neck, p[HEAD]));
        out[P_LARM] = (p[LS], swing(HAND_LOCAL_L.normalize(), p[LS], p[LHAND]));
        out[P_RARM] = (p[RS], swing(HAND_LOCAL_R.normalize(), p[RS], p[RHAND]));
        out[P_LLEG] = (p[LH], swing(Vec3::NEG_Y, p[LH], p[LFOOT]));
        out[P_RLEG] = (p[RH], swing(Vec3::NEG_Y, p[RH], p[RFOOT]));
        for o in &mut out {
            o.1 = o.1.normalize();
        }
        out
    }
}

/// Pelvis centre and orthonormal torso frame (right, up, back) of a point set.
fn torso_frame(p: &[Vec3; N]) -> (Vec3, Quat) {
    let pel = (p[LH] + p[RH]) * 0.5;
    let chest = (p[LS] + p[RS]) * 0.5;
    let up = (chest - pel).normalize_or(Vec3::Y);
    let r_raw = (p[RS] - p[LS]) + (p[RH] - p[LH]);
    let right = (r_raw - up * r_raw.dot(up)).normalize_or(up.any_orthonormal_vector());
    let back = right.cross(up);
    (pel, Quat::from_mat3(&Mat3::from_cols(right, up, back)).normalize())
}

fn rest_torso_rot() -> Quat {
    torso_frame(&REST).1
}

// =================================================================================================
// Bevy side
// =================================================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Tumbling.
    Active,
    /// Slow enough: lying there; back to Active if something knocks it fast again.
    Settling,
    /// Blending the parts back to the standing rest pose.
    GetUp,
}

#[derive(Clone, Copy, Debug)]
struct Request {
    base_vel: Option<Vec3>,
    impulse: Vec3,
    spin: f32,
}

#[derive(Clone, Copy)]
struct Parts {
    top: Entity,
    ents: [Entity; 6],
}

#[derive(Resource, Default)]
pub struct Ragdoll {
    pub phase: Phase,
    pending: Option<Request>,
    body: Option<Body>,
    parts: Option<Parts>,
    /// Seconds since the ragdoll started / spent slow / into the get-up blend.
    t: f32,
    slow_t: f32,
    getup_t: f32,
    /// Hero facing before the fall (restored when standing).
    facing: f32,
    /// Player entity yaw while limp (pelvis -> head, flattened).
    body_yaw: f32,
    part_world: [(Vec3, Quat); 6],
    getup_from: [(Vec3, Quat); 6],
    /// Player translation written last frame; a jump away from it means something teleported Stan.
    last_written: Vec3,
    sfx_cd: f32,
}

impl Ragdoll {
    /// Owns the hero (limp or getting up): input, movement and the limb animator stand down.
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Limp right now (not yet getting up).
    pub fn limp(&self) -> bool {
        matches!(self.phase, Phase::Active | Phase::Settling)
    }

    /// Asks for a ragdoll this frame. `base_vel`: the body's velocity before the hit (`None`:
    /// the ridden scooter's, else the on-foot velocity). `impulse` adds to it; `spin` (rad/s)
    /// tumbles the body about the horizontal axis across `impulse` (positive: head goes along
    /// the impulse, as over the handlebars; negative: legs swept out). Ignored while active.
    pub fn request(&mut self, base_vel: Option<Vec3>, impulse: Vec3, spin: f32) {
        if !self.active() {
            self.pending = Some(Request { base_vel, impulse, spin });
        }
    }
}

/// Run condition: the hero is not ragdolled (or getting up).
pub fn hero_free(rag: Res<Ragdoll>) -> bool {
    !rag.active()
}

/// Whether a ragdoll may start right now (city gameplay, human sized, on foot or on a scooter).
pub fn can_ragdoll(stats: &PlayerStats) -> bool {
    stats.in_city
        && !stats.in_casino_level
        && !stats.in_club_level
        && stats.player_scale <= RAGDOLL_MAX_SCALE
        && stats.driving_car.is_none()
        && !stats.riding_wheel
        && !stats.phone_open
        && !stats.bartender_open
        && !stats.char_select_open
        && !stats.casino_open
        && !stats.arcade_open
}

/// Sideways landing out of an air spin (radians spun, mod 180 within the tolerance of 90).
pub fn spin_landing_failed(spin: f32) -> bool {
    let a = spin.abs();
    a >= SPIN_FAIL_MIN_RAD && ((a % PI) - FRAC_PI_2).abs() < SPIN_FAIL_SIDEWAYS_RAD
}

/// Collision world for the city: `PlatformBox` boxes (street slab, buildings, walls), stunt
/// wedge ramps and the skate park / Five Seasons Park surfaces.
struct CityWorld {
    boxes: Vec<(Vec3, Vec3)>,
    ramps: Vec<(Transform, StuntRamp)>,
}

impl CityWorld {
    fn gather(
        near: Vec3,
        reach: f32,
        boxes: impl Iterator<Item = (Vec3, Vec3)>,
        ramps: impl Iterator<Item = (Transform, StuntRamp)>,
    ) -> CityWorld {
        let boxes = boxes
            .filter(|(mn, mx)| near.clamp(*mn, *mx).distance(near) < reach)
            .collect();
        let ramps = ramps
            .filter(|(t, r)| Vec2::new(t.translation.x - near.x, t.translation.z - near.z).length() < r.half_l + r.half_w + reach)
            .collect();
        CityWorld { boxes, ramps }
    }

    /// Ground height under (x, z) for standing up: highest surface at most `y_hint + 0.6`.
    fn ground_at(&self, x: f32, z: f32, y_hint: f32) -> f32 {
        let mut best: Option<f32> = None;
        for (mn, mx) in &self.boxes {
            if x >= mn.x && x <= mx.x && z >= mn.z && z <= mx.z && mx.y <= y_hint + 0.6 && best.is_none_or(|b| mx.y > b) {
                best = Some(mx.y);
            }
        }
        let p = Vec3::new(x, 0.0, z);
        for (rtf, ramp) in &self.ramps {
            if let Some(pr) = ramp_probe(rtf, ramp, p, 0.0) {
                if pr.on_deck && pr.h <= y_hint + 0.6 && best.is_none_or(|b| pr.h > b) {
                    best = Some(pr.h);
                }
            }
        }
        if let Some(h) = crate::skatepark::ragdoll_ground(Vec2::new(x, z)) {
            if h <= y_hint + 0.6 && best.is_none_or(|b| h > b) {
                best = Some(h);
            }
        }
        best.unwrap_or(0.0)
    }
}

fn box_contact(mn: Vec3, mx: Vec3, prev: Vec3, p: Vec3, r: f32) -> Option<(Vec3, Vec3)> {
    let emn = mn - Vec3::splat(r);
    let emx = mx + Vec3::splat(r);
    if !(p.x > emn.x && p.x < emx.x && p.z > emn.z && p.z < emx.z) {
        return None;
    }
    // Came down onto (or through) the top this step: land on it.
    if prev.y >= emx.y - 1e-3 && p.y < emx.y {
        return Some((Vec3::new(p.x, emx.y, p.z), Vec3::Y));
    }
    if p.y <= emn.y || p.y >= emx.y {
        return None;
    }
    let mut best = (emx.y - p.y, Vec3::Y);
    let mut cands = vec![
        (p.x - emn.x, Vec3::NEG_X),
        (emx.x - p.x, Vec3::X),
        (p.z - emn.z, Vec3::NEG_Z),
        (emx.z - p.z, Vec3::Z),
    ];
    if prev.y <= emn.y + 1e-3 {
        cands.push((p.y - emn.y, Vec3::NEG_Y));
    }
    for c in cands {
        if c.0 < best.0 {
            best = c;
        }
    }
    Some((p + best.1 * best.0, best.1))
}

impl RagWorld for CityWorld {
    fn contact(&self, prev: Vec3, p: Vec3, r: f32) -> Option<(Vec3, Vec3)> {
        let mut q = p;
        let mut hit: Option<Vec3> = None;
        for (mn, mx) in &self.boxes {
            if let Some((q2, n)) = box_contact(*mn, *mx, prev, q, r) {
                q = q2;
                hit = Some(n);
            }
        }
        for (rtf, ramp) in &self.ramps {
            let Some(pr) = ramp_probe(rtf, ramp, q, 0.0) else {
                continue;
            };
            if !pr.on_deck || pr.local.y < -0.3 {
                continue;
            }
            let n_local = Vec3::new(0.0, 1.0, -pr.slope).normalize();
            let d = (pr.local.y - pr.h) * n_local.y;
            if d >= r {
                continue;
            }
            let prev_local = rtf.rotation.inverse() * (prev - rtf.translation);
            let from_above = prev_local.y >= pr.h - 0.3;
            if from_above || r - d < 0.5 {
                let n = rtf.rotation * n_local;
                q += n * (r - d);
                hit = Some(n);
            } else {
                let (out, n) = ramp_exit(rtf, ramp, pr.local, Some(prev_local), r);
                q = rtf.translation + rtf.rotation * out;
                hit = Some(n);
            }
        }
        if let Some((q2, n)) = crate::skatepark::ragdoll_contact(prev, q, r) {
            q = q2;
            hit = Some(n);
        }
        hit.map(|n| (q, n))
    }
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Finds the hero's glTF scene root and six named parts under the player.
fn find_parts(
    root_children: &Children,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
) -> Option<Parts> {
    let mut ents: [Option<Entity>; 6] = [None; 6];
    let mut top_found = None;
    for &top in root_children.iter() {
        let mut stack = vec![top];
        while let Some(e) = stack.pop() {
            if let Ok(kids) = children_q.get(e) {
                stack.extend(kids.iter().copied());
            }
            if let Ok(name) = name_q.get(e) {
                if let Some(i) = PART_NAMES.iter().position(|n| *n == name.as_str()) {
                    ents[i] = Some(e);
                    top_found = Some(top);
                }
            }
        }
    }
    let top = top_found?;
    let mut out = [Entity::PLACEHOLDER; 6];
    for i in 0..6 {
        out[i] = ents[i]?;
    }
    Some(Parts { top, ents: out })
}

type PartQuery<'w, 's> = Query<'w, 's, &'static mut Transform, (Without<Player>, Without<CityScooter>)>;

/// Puts the six parts and the scene root back to the authored rest pose.
fn restore_rest(parts: &Parts, tf_q: &mut PartQuery) {
    for i in 0..6 {
        if let Ok(mut tf) = tf_q.get_mut(parts.ents[i]) {
            tf.translation = PART_REST[i];
            tf.rotation = Quat::IDENTITY;
        }
    }
    if let Ok(mut tf) = tf_q.get_mut(parts.top) {
        *tf = Transform::IDENTITY;
    }
}

/// Ends the ragdoll at once (teleport, vehicle, state exit): rest pose, upright, control back.
fn finish_now(rag: &mut Ragdoll, p_tf: &mut Transform, player: &mut Player, tf_q: &mut PartQuery) {
    if let Some(parts) = rag.parts {
        restore_rest(&parts, tf_q);
    }
    p_tf.rotation = Quat::from_rotation_y(rag.facing);
    player.facing_yaw = rag.facing;
    player.velocity = Vec3::ZERO;
    player.knockback = Vec3::ZERO;
    player.invuln_timer = player.invuln_timer.max(POST_INVULN);
    rag.phase = Phase::Idle;
    rag.body = None;
    rag.parts = None;
    rag.pending = None;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_ragdoll(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rag: ResMut<Ragdoll>,
    mut stats: ResMut<PlayerStats>,
    mut st: ResMut<StuntState>,
    mut player_q: Query<(&mut Transform, &mut Player, &Children)>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
    mut tf_q: PartQuery,
    mut scooter_q: Query<(Entity, &mut Transform, &mut CityScooter), Without<Player>>,
    box_q: Query<(&GlobalTransform, &PlatformBox)>,
    ramp_q: Query<(&GlobalTransform, &StuntRamp)>,
    prompt_q: Query<Entity, With<ScootPromptUi>>,
) {
    let dt = time.delta_secs().min(0.05);
    let rag = &mut *rag;
    let Ok((mut p_tf, mut player, root_children)) = player_q.get_single_mut() else {
        rag.pending = None;
        return;
    };

    // Debug hook: `?debug=1`, K ragdolls the hero forward and up (on foot or on the scooter).
    if crate::audio::debug_keys_enabled() && keys.just_pressed(KeyCode::KeyK) && !rag.active() {
        let fwd = match stats.riding_scooter.and_then(|e| scooter_q.get(e).ok()) {
            Some((_, _, sc)) => Vec3::new(sc.yaw.sin(), 0.0, sc.yaw.cos()),
            None => Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos()),
        };
        rag.request(None, fwd * 6.0 + Vec3::Y * 6.0, 4.0);
    }

    // Something else took over the hero (vehicle, wheel, teleport): hand back at once.
    if rag.active() {
        let teleported = p_tf.translation.distance(rag.last_written) > 3.0;
        let bad_body = rag.body.as_ref().is_some_and(|b| !b.is_finite());
        if stats.driving_car.is_some() || stats.riding_scooter.is_some() || stats.riding_wheel || teleported || bad_body {
            finish_now(rag, &mut p_tf, &mut player, &mut tf_q);
            return;
        }
    }

    // ---- Start ---------------------------------------------------------------------------------
    if let Some(req) = rag.pending.take() {
        if !rag.active() && can_ragdoll(&stats) {
            if let Some(parts) = find_parts(root_children, &children_q, &name_q) {
                start(rag, req, parts, &mut commands, &mut stats, &mut st, &mut p_tf, &mut player, &mut tf_q, &mut scooter_q, &prompt_q);
            }
        }
    }
    if !rag.active() {
        return;
    }
    let Some(parts) = rag.parts else {
        rag.phase = Phase::Idle;
        return;
    };
    let s = stats.body_scale();
    player.velocity = Vec3::ZERO;
    player.knockback = Vec3::ZERO;
    player.squash = 0.0;
    player.squash_vel = 0.0;
    player.roll_timer = 0.0;
    player.attack_timer = 0.0;
    player.dash_timer = 0.0;
    player.jump_buffer = 0.0;
    p_tf.scale = Vec3::splat(s);
    rag.t += dt;
    rag.sfx_cd = (rag.sfx_cd - dt).max(0.0);

    let gather = |near: Vec3| {
        CityWorld::gather(
            near,
            10.0 * s.max(1.0),
            box_q.iter().map(|(g, b)| {
                let c = g.translation();
                (c - b.half_extents, c + b.half_extents)
            }),
            ramp_q.iter().map(|(g, r)| (g.compute_transform(), *r)),
        )
    };

    match rag.phase {
        Phase::Active | Phase::Settling => {
            player.invuln_timer = player.invuln_timer.max(0.5);
            let Some(mut body) = rag.body.take() else {
                finish_now(rag, &mut p_tf, &mut player, &mut tf_q);
                return;
            };
            let world = gather(body.pelvis());
            let info = body.step(dt, &world);
            if info.impact > IMPACT_SFX_SPEED && rag.sfx_cd <= 0.0 {
                rag.sfx_cd = 0.25;
                crate::audio::play_sfx("step");
                stats.shake_request = stats.shake_request.max((0.04 + info.impact * 0.008).min(0.2));
            }
            let slow = info.max_speed < SETTLE_SPEED * s.max(0.5);
            if slow {
                rag.slow_t += dt;
                rag.phase = Phase::Settling;
            } else {
                rag.slow_t = 0.0;
                rag.phase = Phase::Active;
            }
            let pel = body.pelvis();
            if !body.is_finite() || pel.y < -12.0 {
                // Fell out of the world: same spot `player_movement` respawns a fall at.
                finish_now(rag, &mut p_tf, &mut player, &mut tf_q);
                p_tf.translation = Vec3::new(0.0, 1.0, 82.0);
                return;
            }
            // Player entity rides the pelvis; yaw follows the body axis when it is lying down.
            let axis = body.pos[HEAD] - pel;
            let flat = Vec2::new(axis.x, axis.z);
            if flat.length() > 0.5 * axis.length().max(1e-4) {
                rag.body_yaw = (-flat.x).atan2(-flat.y);
            }
            let yaw_q = Quat::from_rotation_y(rag.body_yaw);
            p_tf.translation = pel;
            p_tf.rotation = yaw_q;
            rag.part_world = body.part_poses();
            write_parts(&parts, &rag.part_world, pel, yaw_q, s, &mut tf_q);

            let done = (rag.t >= MIN_SECS && rag.slow_t >= SETTLE_SECS) || rag.t >= MAX_SECS;
            if done {
                // Stand up at the pelvis on valid ground, outside any building.
                let world = gather(pel);
                let spot = stand_spot(&world, pel, stats.player_scale);
                p_tf.translation = spot;
                p_tf.rotation = Quat::from_rotation_y(rag.facing);
                player.facing_yaw = rag.facing;
                player.grounded = true;
                player.jumps_used = 0;
                let inv = p_tf.rotation.inverse();
                for i in 0..6 {
                    let (wp, wr) = rag.part_world[i];
                    rag.getup_from[i] = (inv * (wp - spot) / s, (inv * wr).normalize());
                }
                rag.getup_t = 0.0;
                rag.phase = Phase::GetUp;
            } else {
                rag.body = Some(body);
            }
        }
        Phase::GetUp => {
            player.invuln_timer = player.invuln_timer.max(POST_INVULN);
            rag.getup_t += dt;
            let k = smooth01(rag.getup_t / GETUP_SECS);
            for i in 0..6 {
                if let Ok(mut tf) = tf_q.get_mut(parts.ents[i]) {
                    let (ft, fr) = rag.getup_from[i];
                    tf.translation = ft.lerp(PART_REST[i], k);
                    tf.rotation = fr.slerp(Quat::IDENTITY, k);
                }
            }
            if rag.getup_t >= GETUP_SECS {
                restore_rest(&parts, &mut tf_q);
                rag.phase = Phase::Idle;
                rag.body = None;
                rag.parts = None;
                player.grounded = true;
                player.invuln_timer = player.invuln_timer.max(POST_INVULN);
            }
        }
        Phase::Idle => {}
    }
    rag.last_written = p_tf.translation;
}

/// Writes the parts' world poses relative to the player entity at `origin` / `yaw_q` / scale `s`.
fn write_parts(parts: &Parts, world: &[(Vec3, Quat); 6], origin: Vec3, yaw_q: Quat, s: f32, tf_q: &mut PartQuery) {
    let inv = yaw_q.inverse();
    for i in 0..6 {
        if let Ok(mut tf) = tf_q.get_mut(parts.ents[i]) {
            let (wp, wr) = world[i];
            tf.translation = inv * (wp - origin) / s;
            tf.rotation = (inv * wr).normalize();
        }
    }
}

/// Standing spot near `pel`: on the ground below it and pushed out of walls like the on-foot
/// hero (`player_movement`'s wall radius).
fn stand_spot(world: &CityWorld, pel: Vec3, player_scale: f32) -> Vec3 {
    let wall_r = (0.28 * player_scale).clamp(0.45, 2.8);
    let mut p = Vec3::new(pel.x, 0.0, pel.z);
    p.y = world.ground_at(p.x, p.z, pel.y);
    for _ in 0..3 {
        for (mn, mx) in &world.boxes {
            if mx.y <= p.y + 0.45 || mn.y >= p.y + 1.6 {
                continue;
            }
            let (min_x, max_x, min_z, max_z) = (mn.x - wall_r, mx.x + wall_r, mn.z - wall_r, mx.z + wall_r);
            if p.x > min_x && p.x < max_x && p.z > min_z && p.z < max_z {
                let c = [(p.x - min_x, 0), (max_x - p.x, 1), (p.z - min_z, 2), (max_z - p.z, 3)];
                let m = c.iter().copied().fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a });
                match m.1 {
                    0 => p.x = min_x,
                    1 => p.x = max_x,
                    2 => p.z = min_z,
                    _ => p.z = max_z,
                }
            }
        }
        p.y = world.ground_at(p.x, p.z, pel.y.max(p.y));
    }
    p.x = p.x.clamp(-126.0, 126.0);
    p.z = p.z.clamp(-126.0, 126.0);
    p
}

#[allow(clippy::too_many_arguments)]
fn start(
    rag: &mut Ragdoll,
    req: Request,
    parts: Parts,
    commands: &mut Commands,
    stats: &mut PlayerStats,
    st: &mut StuntState,
    p_tf: &mut Transform,
    player: &mut Player,
    tf_q: &mut PartQuery,
    scooter_q: &mut Query<(Entity, &mut Transform, &mut CityScooter), Without<Player>>,
    prompt_q: &Query<Entity, With<ScootPromptUi>>,
) {
    let s = stats.body_scale();
    // Start pose: the parts' current world positions (player * scene root * part).
    let player_m = p_tf.compute_affine();
    let top_m = tf_q.get(parts.top).map(|t| t.compute_affine()).unwrap_or_default();
    let part_m = |i: usize| {
        let m = tf_q.get(parts.ents[i]).map(|t| t.compute_affine()).unwrap_or_default();
        player_m * top_m * m
    };
    let (torso, head, la, ra, ll, rl) = (part_m(P_TORSO), part_m(P_HEAD), part_m(P_LARM), part_m(P_RARM), part_m(P_LLEG), part_m(P_RLEG));
    let mut pts = [Vec3::ZERO; N];
    pts[HEAD] = head.transform_point3(HEAD_LOCAL);
    pts[LS] = la.transform_point3(Vec3::ZERO);
    pts[RS] = ra.transform_point3(Vec3::ZERO);
    pts[LH] = ll.transform_point3(Vec3::ZERO);
    pts[RH] = rl.transform_point3(Vec3::ZERO);
    pts[SPINE] = torso.transform_point3(SPINE_LOCAL);
    pts[LHAND] = la.transform_point3(HAND_LOCAL_L);
    pts[RHAND] = ra.transform_point3(HAND_LOCAL_R);
    pts[LFOOT] = ll.transform_point3(FOOT_LOCAL);
    pts[RFOOT] = rl.transform_point3(FOOT_LOCAL);
    if pts.iter().any(|p| !p.is_finite()) {
        pts = Body::rest_points(p_tf.translation, p_tf.rotation, s);
    }

    // Velocity before the hit: the scooter's when riding, else the hero's own.
    let mut base = player.velocity + player.knockback;
    if let Some(se) = stats.riding_scooter {
        if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
            base = Vec3::new(sc.yaw.sin(), 0.0, sc.yaw.cos()) * sc.speed + Vec3::Y * sc.vy;
            // Same reset as hopping off (`update_city_scooter_and_phone`): the scooter stays
            // where it stopped, tipped on its side, and Stan has to walk back and remount.
            sc.occupied = false;
            sc.speed = 0.0;
            sc.y = 0.0;
            sc.vy = 0.0;
            sc.grounded = true;
            sc.flip = 0.0;
            sc.air_t = 0.0;
            sc.boost_t = 0.0;
            sc.reset_hop();
            stf.translation.y = 0.0;
            stf.scale = Vec3::ONE;
            let mut hero_spot = stf.translation;
            crate::skatepark::settle_dismount(&mut stf, &mut sc, &mut hero_spot);
            stf.rotation = Quat::from_rotation_y(sc.yaw) * Quat::from_rotation_z(1.45);
            stf.translation.y += 0.1;
        }
        stats.riding_scooter = None;
        stats.scooter_gravity_scale = 1.0;
        st.combo = 0;
        st.combo_t = 0.0;
        crate::audio::play_viking_chant_music();
        for e in prompt_q {
            commands.entity(e).despawn_recursive();
        }
    }
    let vel = req.base_vel.unwrap_or(base) + req.impulse;

    let mut body = Body::new(pts, s);
    let axis = if Vec2::new(req.impulse.x, req.impulse.z).length() > 0.1 { req.impulse } else { vel };
    body.kick(vel, req.spin, axis);

    rag.facing = player.facing_yaw;
    rag.body_yaw = player.facing_yaw;
    rag.t = 0.0;
    rag.slow_t = 0.0;
    rag.getup_t = 0.0;
    rag.sfx_cd = 0.15;
    rag.parts = Some(parts);
    rag.body = Some(body);
    rag.phase = Phase::Active;
    rag.last_written = p_tf.translation;

    // Scene root back to identity (a dodge-roll tumble may have been on it).
    if let Ok(mut tf) = tf_q.get_mut(parts.top) {
        *tf = Transform::IDENTITY;
    }
    player.velocity = Vec3::ZERO;
    player.knockback = Vec3::ZERO;
    player.grounded = false;
    player.block_active = false;
    player.cancel_charge();
    player.invuln_timer = player.invuln_timer.max(0.5);
    stats.shake_request = stats.shake_request.max(0.35);
}

/// OnExit(CityPlay) (also run by the pause-menu Restart / Clear save): never leave Stan limp.
pub fn reset_ragdoll_on_exit(
    mut rag: ResMut<Ragdoll>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut tf_q: PartQuery,
) {
    rag.pending = None;
    if !rag.active() {
        return;
    }
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        finish_now(&mut rag, &mut p_tf, &mut player, &mut tf_q);
    } else {
        if let Some(parts) = rag.parts {
            restore_rest(&parts, &mut tf_q);
        }
        *rag = Ragdoll::default();
    }
}

/// While ragdolled, drop fresh presses of the action keys so no interact (mount, car, beer,
/// wheel, attacks, jump) fires from the floor. Held keys stay held for after the get-up.
fn ragdoll_input_guard(
    rag: Res<Ragdoll>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
) {
    if !rag.active() {
        return;
    }
    stats.car_prompt_tap = false;
    for k in [
        KeyCode::KeyF,
        KeyCode::KeyE,
        KeyCode::KeyP,
        KeyCode::Space,
        KeyCode::KeyQ,
        KeyCode::Digit1,
        KeyCode::Tab,
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::KeyJ,
        KeyCode::KeyX,
        KeyCode::KeyR,
        KeyCode::KeyV,
        KeyCode::KeyG,
    ] {
        keys.clear_just_pressed(k);
    }
    mouse.clear_just_pressed(MouseButton::Left);
    mouse.clear_just_pressed(MouseButton::Right);
}

pub struct RagdollPlugin;

impl Plugin for RagdollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ragdoll>()
            .add_systems(PreUpdate, ragdoll_input_guard.after(bevy::input::InputSystem))
            .add_systems(OnExit(GameState::CityPlay), reset_ragdoll_on_exit)
            .add_systems(
                Update,
                update_ragdoll
                    .after(crate::skatepark::ParkWalkSet)
                    .after(crate::city::update_city_traffic)
                    .before(crate::player::animate_player_limbs)
                    .before(crate::player::CameraSet::Follow)
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(crate::ui::gameplay_running),
            );
    }
}

// =================================================================================================
// Tests (native: `cargo test ragdoll`)
// =================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn rigid_ok(b: &Body, tol: f32) -> bool {
        b.sticks.iter().filter(|s| s.min == s.max).all(|s| {
            let d = b.pos[s.a].distance(b.pos[s.b]);
            (d - s.min).abs() <= s.min * tol
        })
    }

    #[test]
    fn ragdoll_sticks_keep_length() {
        let pts = Body::rest_points(Vec3::new(0.0, 2.0, 0.0), Quat::from_rotation_x(0.6), 1.22);
        let mut b = Body::new(pts, 1.22);
        b.kick(Vec3::new(9.0, 5.0, -3.0), 6.0, Vec3::X);
        let ground = FlatGround(0.0);
        for _ in 0..180 {
            b.step(1.0 / 60.0, &ground);
            assert!(rigid_ok(&b, 0.06), "a rigid stick stretched more than 6%");
        }
    }

    #[test]
    fn ragdoll_dropped_body_rests_above_ground() {
        let pts = Body::rest_points(Vec3::new(0.0, 1.5, 0.0), Quat::from_rotation_z(0.3), 1.22);
        let mut b = Body::new(pts, 1.22);
        b.kick(Vec3::new(2.0, 0.0, 0.0), 2.0, Vec3::X);
        let ground = FlatGround(0.0);
        let mut info = StepInfo::default();
        for _ in 0..300 {
            info = b.step(1.0 / 60.0, &ground);
        }
        assert!(info.max_speed < SETTLE_SPEED * 1.22, "still moving at {}", info.max_speed);
        for i in 0..N {
            assert!(b.pos[i].y >= b.radius[i] - 0.03, "point {} sank to {}", i, b.pos[i].y);
        }
        assert!(b.pelvis().y > 0.0);
    }

    #[test]
    fn ragdoll_no_nan_after_huge_impulse() {
        let pts = Body::rest_points(Vec3::ZERO, Quat::IDENTITY, 1.22);
        let mut b = Body::new(pts, 1.22);
        b.kick(Vec3::new(500.0, 300.0, -800.0), 250.0, Vec3::new(1.0, 0.0, 1.0));
        let ground = FlatGround(0.0);
        for _ in 0..240 {
            b.step(0.05, &ground);
            assert!(b.is_finite());
        }
        for p in b.part_poses() {
            assert!(p.0.is_finite() && p.1.is_finite());
        }
    }

    #[test]
    fn ragdoll_rest_pose_maps_to_authored_parts() {
        let b = Body::new(Body::rest_points(Vec3::ZERO, Quat::IDENTITY, 1.0), 1.0);
        let poses = b.part_poses();
        for i in 0..6 {
            assert!(poses[i].0.distance(PART_REST[i]) < 0.02, "part {} at {:?}", i, poses[i].0);
            assert!(poses[i].1.angle_between(Quat::IDENTITY) < 0.05, "part {} rotated", i);
        }
    }

    #[test]
    fn ragdoll_spin_fail_window() {
        assert!(!spin_landing_failed(0.3));
        assert!(spin_landing_failed(FRAC_PI_2));
        assert!(!spin_landing_failed(PI));
        assert!(spin_landing_failed(-3.0 * FRAC_PI_2));
        assert!(!spin_landing_failed(std::f32::consts::TAU));
    }
}
