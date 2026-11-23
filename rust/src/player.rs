use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::state::{
    CAMERA_BLEND_SECS, CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats, PowerupKind, PowerupPickup,
    TendyPickup,
};

#[derive(Component)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
    pub jumps_used: u8,
    pub dash_timer: f32,
    pub invuln_timer: f32,
    pub slow_timer: f32,
    pub facing_yaw: f32,
    pub walk_cycle: f32,
    pub last_step_half: i32,
    pub roll_timer: f32,
    pub dodge_counter_timer: f32,
    pub attack_timer: f32,
    pub attack_cooldown: f32,
    /// Horizontal knockback velocity layered on top of input velocity; decays exponentially
    /// (`KNOCKBACK_DECAY`) so a hit shoves the hero instead of teleporting them.
    pub knockback: Vec3,
    /// Seconds of damage blink left (set from the invulnerability window on a damaging hit).
    pub hurt_blink: f32,
    /// True while the blink has hidden the hero, so the blink only ever restores what it hid.
    pub blink_hidden: bool,
    /// Seconds a jump press stays queued, so a press just before landing still jumps.
    pub jump_buffer: f32,
    /// Squash (+) / stretch (-) spring state applied as a scale multiplier.
    pub squash: f32,
    pub squash_vel: f32,
    /// Saber guard is up this frame (boss mode, block input held, not swinging or rolling).
    pub block_active: bool,
    /// Seconds since the guard went up; a hit inside `PERFECT_BLOCK_SECS` is a perfect block.
    pub block_time: f32,
    /// Seconds the guard has been down; the perfect window only re-arms after `BLOCK_REARM_SECS`
    /// so mashing block can't keep it open.
    pub block_down_time: f32,
    /// Guard-break lockout: the guard can't be raised again until this runs out.
    pub block_lockout: f32,
    /// Pending block spark burst for `update_saber_fx` (0 none, 1 block, 2 perfect block).
    pub block_spark: u8,
    /// Short recoil of the guard pose after a blocked hit (1 -> 0).
    pub block_recoil: f32,
    /// Which way the current light swing travels (+1 forehand right-to-left, -1 backhand).
    pub swing_side: f32,
    /// Light swings started while this is running alternate sides (a combo).
    pub combo_timer: f32,
    /// Bumped at the start of every saber swing; targets remember the last swing that hit them so
    /// each swing lands at most once per target.
    pub swing_id: u32,
    /// Pending saber hit spark (contact point) for `update_saber_fx`.
    pub hit_spark: Option<Vec3>,
    /// Pending hit-stop request in real seconds, consumed by `update_hit_stop`.
    pub hit_stop: f32,
    /// Strafe mode this frame (boss mode: lock-on, block held or the strafe key).
    pub strafing: bool,
    /// Attack button hold (boss mode): a tap is a light swing, a held charge releases a heavy.
    pub attack_hold: AttackHold,
}

/// Attack button state for the tap / hold-to-charge saber trigger.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum AttackHold {
    /// No tracked press.
    #[default]
    Idle,
    /// Button down; seconds held (counted only while no swing is playing).
    Held(f32),
    /// The press was used up or cancelled; waits for the button to be released.
    Spent,
}

/// Released before this (seconds held) the press is a tap: a light swing fires on release.
pub const ATTACK_TAP_SECS: f32 = 0.18;
/// Held at least this long, a release unleashes the heavy swing (between the two: light).
pub const HEAVY_CHARGE_SECS: f32 = 0.45;
/// A full charge held this long releases on its own.
const CHARGE_AUTO_RELEASE_SECS: f32 = 2.5;
/// Walking speed while charging.
const CHARGE_MOVE_MUL: f32 = 0.5;

impl Player {
    /// Heavy charge level while Stan is visibly charging: 0 at the tap threshold, 1 once a release
    /// would unleash the heavy. None when not charging.
    pub fn charge_level(&self) -> Option<f32> {
        match self.attack_hold {
            AttackHold::Held(t)
                if t >= ATTACK_TAP_SECS
                    && self.attack_timer <= 0.0
                    && self.roll_timer <= 0.0
                    && !self.block_active =>
            {
                Some(((t - ATTACK_TAP_SECS) / (HEAVY_CHARGE_SECS - ATTACK_TAP_SECS)).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// Drop a press in progress (no swing); the button must be released before it counts again.
    pub fn cancel_charge(&mut self) {
        if matches!(self.attack_hold, AttackHold::Held(_)) {
            self.attack_hold = AttackHold::Spent;
        }
    }
}

/// Dodge roll length in seconds (shared by the roll input and the tumble animation).
pub const ROLL_SECS: f32 = 0.45;
/// Light / heavy saber swing lengths (`attack_timer` at the start of the swing).
pub const LIGHT_SWING_SECS: f32 = 0.28;
pub const HEAVY_SWING_SECS: f32 = 0.40;
/// Saber block tuning. The guard covers a 120 degree arc in front of Stan.
const BLOCK_ARC_COS: f32 = 0.5;
pub const PERFECT_BLOCK_SECS: f32 = 0.18;
const BLOCK_REARM_SECS: f32 = 0.30;
const BLOCK_STAMINA_COST: f32 = 14.0;
const BLOCK_KNOCKBACK: f32 = 0.7;
/// Brief invulnerability after a block so contact damage (veggies, cars) can't drain the guard
/// every frame while overlapping.
const BLOCK_INVULN_SECS: f32 = 0.35;
const GUARD_BREAK_LOCKOUT_SECS: f32 = 0.7;
const BLOCK_MOVE_MUL: f32 = 0.45;
/// Strafing (facing locked) walks a little slower than free movement.
const STRAFE_MOVE_MUL: f32 = 0.85;

/// Saber reach from Stan's centre, in hero model units (times `player_scale`): shoulder 0.35 +
/// arm 0.55 + blade tip 1.66 (`HeroSaberBlade`), rounded up a little so the glow counts.
const SABER_REACH: f32 = 2.4;
/// The heavy chop's deep lunge pose reaches a bit further.
const HEAVY_REACH_BONUS: f32 = 0.35;
/// Half-arc of the swing (cosine). The light slash sweeps from Stan's right across the front to
/// his left (about 110 degrees each side); the heavy chop comes down in front (about 70).
const LIGHT_ARC_COS: f32 = -0.34;
const HEAVY_ARC_COS: f32 = 0.34;
/// Swing progress (0..1) in which the blade can land: from just after the wind-up through the
/// visible strike (light crosses the front at 30 to 45 percent, heavy lands near 42 percent).
const LIGHT_ACTIVE: (f32, f32) = (0.10, 0.70);
const HEAVY_ACTIVE: (f32, f32) = (0.20, 0.78);
/// Attack assist looks this much (model units) past the reach for a target to turn to.
const ASSIST_EXTRA: f32 = 0.8;

/// A saber target as a vertical cylinder: feet position, radius and height (world units).
#[derive(Clone, Copy)]
pub struct SaberTarget {
    pub pos: Vec3,
    pub radius: f32,
    pub height: f32,
}

impl SaberTarget {
    /// Mom (`spawn_mom_mesh_hierarchy`, about 0.42 wide and 1.95 tall before scaling).
    pub fn mom(tf: &Transform) -> Self {
        Self { pos: tf.translation, radius: 0.42 * tf.scale.x, height: 1.95 * tf.scale.y }
    }

    /// Lexie (hips about 1.1 wide, about 4.4 tall before her 5.8 scale).
    pub fn lexie(tf: &Transform) -> Self {
        Self { pos: tf.translation, radius: 1.1 * tf.scale.x, height: 4.4 * tf.scale.y }
    }

    /// Veggie minions, sized from their glTF bounds.
    pub fn veggie(tf: &Transform, kind: crate::enemies::EnemyKind) -> Self {
        use crate::enemies::EnemyKind;
        let (r, h) = match kind {
            EnemyKind::BroccoliBrute => (0.9, 2.1),
            EnemyKind::CarrotLancer => (0.4, 1.8),
            EnemyKind::KaleSlime => (1.15, 0.8),
        };
        Self { pos: tf.translation, radius: r * tf.scale.x, height: h * tf.scale.y }
    }
}

fn saber_reach(stats: &PlayerStats, heavy: bool) -> f32 {
    (SABER_REACH + if heavy { HEAVY_REACH_BONUS } else { 0.0 }) * stats.body_scale().max(0.55)
}

/// Geometry part of the saber test: the target's surface is within `reach`, its body overlaps
/// the blade's vertical band (feet to well above the head), and some of it lies inside the arc.
/// Returns (gap to the surface, flat direction to the target).
fn saber_covers(
    fwd: Vec3,
    hero_pos: Vec3,
    scale: f32,
    target: &SaberTarget,
    reach: f32,
    arc_cos: f32,
) -> Option<(f32, Vec3)> {
    let lo = hero_pos.y - 0.3 * scale;
    let hi = hero_pos.y + 3.2 * scale;
    if target.pos.y > hi || target.pos.y + target.height < lo {
        return None;
    }
    let to = Vec3::new(target.pos.x - hero_pos.x, 0.0, target.pos.z - hero_pos.z);
    let d = to.length();
    let gap = d - target.radius;
    if gap > reach {
        return None;
    }
    let dir = if d > 1e-3 { to / d } else { fwd };
    if gap <= 0.35 * scale {
        // Touching (or inside): the swing can't miss.
        return Some((gap, dir));
    }
    // Widen the arc by the target's angular half-size, so the edge of a wide body counts.
    let ang = fwd.dot(dir).clamp(-1.0, 1.0).acos() - (target.radius / d).min(1.0).asin();
    if ang > arc_cos.clamp(-1.0, 1.0).acos() {
        return None;
    }
    Some((gap, dir))
}

/// Saber hit test shared by every target. Returns the contact point when the active part of the
/// current swing covers `target`. Callers still apply the hit only once per `player.swing_id`.
pub fn saber_contact(player: &Player, stats: &PlayerStats, hero_pos: Vec3, target: &SaberTarget) -> Option<Vec3> {
    if !stats.boss_mode || player.attack_timer <= 0.0 {
        return None;
    }
    let heavy = stats.heavy_attack;
    let total = if heavy { HEAVY_SWING_SECS } else { LIGHT_SWING_SECS };
    let p = 1.0 - player.attack_timer / total;
    let (a, b) = if heavy { HEAVY_ACTIVE } else { LIGHT_ACTIVE };
    if p < a || p > b {
        return None;
    }
    let scale = stats.body_scale().max(0.55);
    let arc = if heavy { HEAVY_ARC_COS } else { LIGHT_ARC_COS };
    let (_, dir) = saber_covers(hero_forward(player), hero_pos, scale, target, saber_reach(stats, heavy), arc)?;
    let y = (hero_pos.y + 1.3 * scale).clamp(target.pos.y, target.pos.y + target.height);
    let surf = target.pos - dir * target.radius;
    Some(Vec3::new(surf.x, y, surf.z))
}

/// Hit confirmation for a landed saber hit: spark at the contact point, a few frames of
/// hit-stop and a small shake (the caller plays the hit sound as before).
pub fn register_saber_hit(player: &mut Player, stats: &mut PlayerStats, contact: Vec3) {
    let heavy = stats.heavy_attack;
    player.hit_spark = Some(contact);
    player.hit_stop = player.hit_stop.max(if heavy { 0.09 } else { 0.055 });
    stats.shake_request = stats.shake_request.max(if heavy { 0.35 } else { 0.18 });
}

/// Nearest lock-on candidate (same rule the lock-on camera uses).
fn pick_lock_target(hero_pos: Vec3, candidates: impl Iterator<Item = Vec3>) -> Option<Vec3> {
    let mut best: Option<Vec3> = None;
    let mut best_dist = 70.0_f32;
    for pos in candidates {
        let d = hero_pos.distance(pos);
        if d > 0.5 && d < best_dist {
            best_dist = d;
            best = Some(pos);
        }
    }
    best
}

/// Attack assist at swing start: turn Stan to the best target in reach (roughly in front of him
/// or the camera, or the lock-on target) and step in a little. Returns the remaining gap.
fn attack_assist(
    player: &mut Player,
    tf: &mut Transform,
    stats: &PlayerStats,
    cam_fwd: Vec3,
    lock: Option<Vec3>,
    targets: &[SaberTarget],
    heavy: bool,
) -> Option<f32> {
    let scale = stats.body_scale().max(0.55);
    let reach = saber_reach(stats, heavy);
    let fwd = hero_forward(player);
    let mut best: Option<(f32, f32, Vec3)> = None;
    for t in targets {
        let is_lock = lock.is_some_and(|l| l.distance(t.pos) < 0.01);
        let Some((gap, dir)) = saber_covers(fwd, tf.translation, scale, t, reach + ASSIST_EXTRA * scale, -1.0) else {
            continue;
        };
        let facing_dot = dir.dot(fwd);
        if !(is_lock || facing_dot >= -0.05 || dir.dot(cam_fwd) >= 0.2) {
            continue;
        }
        let score = gap / reach + (1.0 - facing_dot) * 0.5 - if is_lock { 10.0 } else { 0.0 };
        if best.map_or(true, |(s, _, _)| score < s) {
            best = Some((score, gap, dir));
        }
    }
    let (_, gap, dir) = best?;
    player.facing_yaw = (-dir.x).atan2(-dir.z);
    tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    let step = (gap - 0.55 * reach).clamp(0.0, 0.35 * scale);
    tf.translation += dir * step;
    Some(gap - step)
}

/// Result of an attack meeting Stan's saber guard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockOutcome {
    /// Not blocked (no guard, hit from behind, or guard broken): apply the hit as normal.
    None,
    /// Blocked: no heart loss, small shove, stamina spent.
    Blocked,
    /// Perfect block (guard raised just in time): free, and staggers / deflects the attacker.
    Perfect,
}

impl BlockOutcome {
    pub fn blocked(self) -> bool {
        self != BlockOutcome::None
    }
}

fn hero_forward(player: &Player) -> Vec3 {
    Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos())
}

/// True when the raised guard faces `source_pos` (within the front arc). Costs nothing.
pub fn guard_covers(player: &Player, stats: &PlayerStats, hero_pos: Vec3, source_pos: Vec3) -> bool {
    if !stats.boss_mode || !player.block_active {
        return false;
    }
    let to_src = Vec3::new(source_pos.x - hero_pos.x, 0.0, source_pos.z - hero_pos.z);
    if to_src.length_squared() < 1e-4 {
        return true;
    }
    to_src.normalize().dot(hero_forward(player)) >= BLOCK_ARC_COS
}

/// Resolve an incoming hit against the saber guard. Call it where a hit would cost a heart; if it
/// returns `BlockOutcome::None` apply the hit exactly as before. On a block this already applies
/// the shove, stamina, invulnerability, sound, shake and spark request; the caller only adds
/// attacker-specific reactions (stagger, deflect).
pub fn try_block(
    player: &mut Player,
    stats: &mut PlayerStats,
    hero_pos: Vec3,
    source_pos: Vec3,
) -> BlockOutcome {
    if !guard_covers(player, stats, hero_pos, source_pos) {
        return BlockOutcome::None;
    }
    let perfect = player.block_time < PERFECT_BLOCK_SECS;
    if !perfect {
        if stats.dash_energy < BLOCK_STAMINA_COST {
            // Guard break: the hit goes through and the guard stays down for a moment.
            player.block_active = false;
            player.block_lockout = GUARD_BREAK_LOCKOUT_SECS;
            crate::audio::play_sfx("guard_break");
            stats.hint_text = "GUARD BROKEN! Out of stamina, ROLL instead!".to_string();
            return BlockOutcome::None;
        }
        stats.dash_energy -= BLOCK_STAMINA_COST;
        stats.stamina_regen_delay = stats.stamina_regen_delay.max(0.5);
    }
    let push = Vec3::new(hero_pos.x - source_pos.x, 0.0, hero_pos.z - source_pos.z);
    let push = if push.length_squared() > 1e-4 { push } else { -hero_forward(player) };
    hit_reaction(player, stats, push, if perfect { BLOCK_KNOCKBACK * 0.5 } else { BLOCK_KNOCKBACK }, false);
    player.invuln_timer = player.invuln_timer.max(BLOCK_INVULN_SECS);
    player.block_recoil = 1.0;
    if perfect {
        player.block_spark = 2;
        stats.shake_request = stats.shake_request.max(0.4);
        crate::audio::play_sfx("saber_parry");
        stats.hint_text = "PERFECT BLOCK!".to_string();
        BlockOutcome::Perfect
    } else {
        player.block_spark = player.block_spark.max(1);
        crate::audio::play_sfx("saber_block");
        stats.hint_text = "BLOCKED! (stamina drained)".to_string();
        BlockOutcome::Blocked
    }
}

/// Crew follow slots in Level 2: each companion flanks Stan instead of trailing him, so at
/// kaiju scale nobody stands between Stan and the camera.
#[derive(Clone, Copy)]
pub enum CrewSlot {
    RightInner,
    LeftInner,
    RightOuter,
    LeftOuter,
}

/// World target for a crew follower. The formation is laid out in the camera's view frame
/// (camera right / camera forward), so followers stay to Stan's screen sides and slightly ahead
/// of him whichever way he faces, and never in the camera line behind him.
pub fn crew_follow_target(
    slot: CrewSlot,
    stan_pos: Vec3,
    stan_scale: f32,
    follower_scale: f32,
    orbit_yaw: f32,
) -> Vec3 {
    // While the hero rides the Ferris wheel the crew wait in formation by its plaza instead.
    let (stan_pos, stan_scale, orbit_yaw) = match crate::ferris::crew_wait_anchor() {
        Some((pos, yaw)) => (pos, 1.8, yaw),
        None => (stan_pos, stan_scale, orbit_yaw),
    };
    let cam_fwd = Vec3::new(-orbit_yaw.sin(), 0.0, -orbit_yaw.cos());
    let cam_right = Vec3::new(orbit_yaw.cos(), 0.0, -orbit_yaw.sin());
    // Clear Stan's half width plus the follower's own, with a little air between them.
    let inner = 0.75 * stan_scale + 1.0 * follower_scale + 1.0;
    let outer = inner + 1.9 * follower_scale + 0.5;
    let (side, lateral, ahead) = match slot {
        CrewSlot::RightInner => (1.0, inner, 0.35 * stan_scale),
        CrewSlot::LeftInner => (-1.0, inner, 0.35 * stan_scale),
        CrewSlot::RightOuter => (1.0, outer, 0.10 * stan_scale),
        CrewSlot::LeftOuter => (-1.0, outer, 0.10 * stan_scale),
    };
    let p = stan_pos + cam_right * (side * lateral) + cam_fwd * ahead;
    Vec3::new(p.x, 0.0, p.z)
}

/// How long a jump press is remembered while it can't be used yet.
const JUMP_BUFFER_SECS: f32 = 0.12;
/// Exponential approach rates (per second) toward the input's target horizontal velocity.
const GROUND_ACCEL_RATE: f32 = 26.0;
const GROUND_DECEL_RATE: f32 = 30.0;
const AIR_ACCEL_RATE: f32 = 11.0;
/// Knockback decay rate. Total knockback travel is `initial_speed / KNOCKBACK_DECAY`.
pub const KNOCKBACK_DECAY: f32 = 10.0;

/// Shared hit reaction for anything that hurts or shoves the hero.
///
/// * `push_dir` / `distance`: horizontal knockback; the hero travels about `distance` units over a
///   few frames (same total as the old instant position jump), colliding with walls on the way.
/// * `hurt`: a heart was lost. Adds the red edge flash, the invulnerability blink and a bigger
///   camera shake. Call it after setting `invuln_timer`, since the blink lasts that long.
pub fn hit_reaction(
    player: &mut Player,
    stats: &mut PlayerStats,
    push_dir: Vec3,
    distance: f32,
    hurt: bool,
) {
    // Getting hit (or shoved on a block) drops a heavy charge in progress.
    player.cancel_charge();
    let flat = Vec3::new(push_dir.x, 0.0, push_dir.z).normalize_or_zero();
    if distance > 0.0 && flat != Vec3::ZERO {
        player.knockback = flat * distance * KNOCKBACK_DECAY;
    }
    if hurt {
        stats.hurt_flash = 1.0;
        stats.shake_request = stats.shake_request.max(0.6);
        player.hurt_blink = player.invuln_timer;
    } else {
        stats.shake_request = stats.shake_request.max(0.25);
    }
}

/// OnExit(gameplay state): undo a blink that was mid-flash and drop leftover knockback, so a
/// cutscene or end card never inherits a hidden or sliding hero.
pub fn reset_hit_reaction(mut player_q: Query<(&mut Player, &mut Visibility)>) {
    for (mut player, mut vis) in &mut player_q {
        if player.blink_hidden {
            *vis = Visibility::Visible;
            player.blink_hidden = false;
        }
        player.hurt_blink = 0.0;
        player.knockback = Vec3::ZERO;
        player.jump_buffer = 0.0;
        player.block_active = false;
        player.block_spark = 0;
        player.block_recoil = 0.0;
        player.attack_hold = AttackHold::Idle;
    }
}

/// Collected pickups pop (quick swell, then shrink to nothing) instead of vanishing.
#[derive(Component)]
pub struct PickupPop {
    pub t: f32,
    pub base_scale: Vec3,
}

const PICKUP_POP_SECS: f32 = 0.2;

/// Turns a collected pickup into a short pop. Callers remove their own pickup marker first so
/// the entity can't be collected twice.
pub fn start_pickup_pop(commands: &mut Commands, entity: Entity, base_scale: Vec3) {
    if let Some(mut ec) = commands.get_entity(entity) {
        ec.insert(PickupPop { t: 0.0, base_scale });
    }
}

pub fn animate_pickup_pops(
    mut commands: Commands,
    time: Res<Time>,
    mut pop_q: Query<(Entity, &mut Transform, &mut PickupPop)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut tf, mut pop) in &mut pop_q {
        pop.t += dt;
        if pop.t >= PICKUP_POP_SECS {
            commands.entity(e).despawn_recursive();
            continue;
        }
        let swell_t = 0.07;
        let s = if pop.t < swell_t {
            1.0 + 0.45 * (pop.t / swell_t)
        } else {
            let x = ((pop.t - swell_t) / (PICKUP_POP_SECS - swell_t)).clamp(0.0, 1.0);
            1.45 * (1.0 - x * x)
        };
        tf.scale = pop.base_scale * s.max(0.001);
        tf.rotate_y(9.0 * dt);
    }
}

impl Default for Player {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: true,
            jumps_used: 0,
            dash_timer: 0.0,
            invuln_timer: 0.0,
            slow_timer: 0.0,
            facing_yaw: 0.0,
            walk_cycle: 0.0,
            last_step_half: 0,
            roll_timer: 0.0,
            dodge_counter_timer: 0.0,
            attack_timer: 0.0,
            attack_cooldown: 0.0,
            knockback: Vec3::ZERO,
            hurt_blink: 0.0,
            blink_hidden: false,
            jump_buffer: 0.0,
            squash: 0.0,
            squash_vel: 0.0,
            block_active: false,
            block_time: 0.0,
            block_down_time: 1.0,
            block_lockout: 0.0,
            block_spark: 0,
            block_recoil: 0.0,
            swing_side: 1.0,
            combo_timer: 0.0,
            swing_id: 0,
            hit_spark: None,
            hit_stop: 0.0,
            strafing: false,
            attack_hold: AttackHold::Idle,
        }
    }
}

pub fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_player_inner(&mut commands, &asset_server);
}

pub fn spawn_player_inner(commands: &mut Commands, asset_server: &AssetServer) {
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        LevelEntity,
        Player::default(),
        SceneRoot(hero_scene),
        crate::state::SceneRetryPath::new("models/plaid_hero.glb#Scene0"),
        Transform::from_xyz(0.0, 0.0, 29.0).with_scale(Vec3::splat(0.55)),
    ));
}

/// Orbit pitch limits. Negative pitch = looking up: the boom stays at `BOOM_PITCH_MIN` (camera
/// never drops below the hero's anchor, so it can't sink under the floor) and the view tilts up.
const PITCH_MIN: f32 = -0.3;
const PITCH_MAX: f32 = 1.32;
const BOOM_PITCH_MIN: f32 = 0.08;

/// Ordering for gameplay Update systems: look input, then everything that moves the hero,
/// then the follow camera, so the camera always frames this frame's player position.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CameraSet {
    Look,
    Motion,
    Follow,
}

/// Frame-rate independent smoothing factor for exponential approach at `rate` per second.
fn smooth_factor(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Beer stagger in -1..1: a few incommensurate sines, so the drift wanders instead of ticking
/// back and forth on a beat. Fades out over the last 2 s of `drunk_timer`. Callers scale it into
/// a heading offset (on foot) or a steering bias (scooter).
pub fn drunk_steer_drift(t: f32, drunk_timer: f32) -> f32 {
    let fade = (drunk_timer / 2.0).min(1.0);
    ((t * 0.83).sin() * 0.6 + (t * 1.91 + 1.3).sin() * 0.3 + (t * 3.7 + 0.4).sin() * 0.1) * fade
}

/// OnEnter(gameplay state): ease from the cutscene's last shot into the follow camera.
pub fn camera_begin_blend(mut orbit: ResMut<CameraOrbit>) {
    orbit.blend_request = true;
    orbit.snap_follow = true;
}

/// OnEnter(non-gameplay follow states): snap the follow point, no blend.
pub fn camera_snap_follow(mut orbit: ResMut<CameraOrbit>) {
    orbit.snap_follow = true;
}

pub fn camera_mouse_look(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut orbit: ResMut<CameraOrbit>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if stats.casino_open || stats.arcade_open || stats.bartender_open || stats.char_select_open || stats.club_dance_open {
        return;
    }
    if let Ok(mut window) = window_q.get_single_mut() {
        if mouse_buttons.just_pressed(MouseButton::Left) && touches.iter().next().is_none() {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
        }
        if keys.just_pressed(KeyCode::Escape) {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
    }

    // Track how long since the player last looked around so vehicle chase cams can back off.
    orbit.since_look = (orbit.since_look + time.delta_secs().min(0.05)).min(99.0);

    if mouse_motion.delta != Vec2::ZERO {
        orbit.yaw -= mouse_motion.delta.x * 0.0045;
        orbit.pitch = (orbit.pitch + mouse_motion.delta.y * 0.0035).clamp(PITCH_MIN, PITCH_MAX);
        orbit.since_look = 0.0;
    }

    // Touch drags and the gamepad right stick both arrive through this bridge.
    let t_dx = crate::audio::take_touch_look_dx();
    let t_dy = crate::audio::take_touch_look_dy();
    if t_dx != 0.0 || t_dy != 0.0 {
        orbit.yaw -= t_dx * 0.0065;
        orbit.pitch = (orbit.pitch + t_dy * 0.0050).clamp(PITCH_MIN, PITCH_MAX);
        orbit.since_look = 0.0;
    }

    let zoom_delta = crate::audio::take_touch_zoom_delta();
    let joy_active = crate::audio::touch_move_x().abs() > 0.001
        || crate::audio::touch_move_y().abs() > 0.001;
    if zoom_delta != 0.0 && !joy_active {
        orbit.distance = (orbit.distance + zoom_delta).clamp(3.0, 42.0);
    } else if mouse_scroll.delta.y != 0.0 {
        let step = match mouse_scroll.unit {
            bevy::input::mouse::MouseScrollUnit::Line => mouse_scroll.delta.y * 0.85,
            bevy::input::mouse::MouseScrollUnit::Pixel => mouse_scroll.delta.y * 0.012,
        };
        orbit.distance = (orbit.distance - step).clamp(3.0, 42.0);
    }
}

pub fn unlock_cursor(mut window_q: Query<&mut Window, With<PrimaryWindow>>) {
    crate::audio::set_touch_playing(false);
    if let Ok(mut window) = window_q.get_single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
}

pub fn lock_touch_playing() {
    crate::audio::set_touch_playing(true);
}

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut orbit: ResMut<CameraOrbit>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player, &mut Visibility)>,
    platform_q: Query<(&Transform, &PlatformBox), Without<Player>>,
    target_q: Query<
        (&Transform, Has<crate::boss::MomBoss>, Has<crate::city::LexieBoss>, Option<&crate::enemies::Enemy>),
        (
            Or<(
                With<crate::boss::MomBoss>,
                With<crate::city::LexieBoss>,
                With<crate::enemies::Enemy>,
            )>,
            Without<Player>,
        ),
    >,
    mut demo: Local<CombatDemo>,
    frames: Res<bevy::core::FrameCount>,
    mut last_frame: Local<u32>,
) {
    let Ok((mut tf, mut player, mut vis)) = player_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    // A skipped frame means this system was paused (help overlay, fade): drop any charge so the
    // release on resume can't fire a stale swing.
    if frames.0 != last_frame.wrapping_add(1) {
        player.cancel_charge();
    }
    *last_frame = frames.0;

    // Smoothly grow the hero toward their Katamari target scale (with 0.5s enraged swelling pulses for 10s)
    if stats.enraged && !stats.in_city {
        stats.rage_cooldown_timer = (stats.rage_cooldown_timer - dt).max(0.0);
        if stats.rage_cooldown_timer <= 0.0 {
            stats.enraged = false;
            stats.rage_swell_timer = 0.0;
            stats.rage_swell_ticks = 0;
            stats.rage_pulse_flash = 0.0;
            stats.hint_text = "Stan cooled down! Watch out for headphones!".to_string();
        } else {
            stats.rage_pulse_flash = (stats.rage_pulse_flash - dt * 2.2).max(0.0);
            stats.rage_swell_timer += dt;
            while stats.rage_swell_timer >= 0.5 {
                stats.rage_swell_timer -= 0.5;
                stats.rage_swell_ticks = stats.rage_swell_ticks.saturating_add(1);
                stats.rage_pulse_flash = 1.0;
                if stats.rage_swell_ticks <= 6 || stats.rage_swell_ticks % 2 == 0 {
                    crate::audio::play_sfx("rage_swell");
                }
            }
        }
    }
    let target_s = stats.target_scale();
    if (stats.player_scale - target_s).abs() > 0.001 {
        stats.player_scale += (target_s - stats.player_scale) * (dt * 6.0).min(1.0);
    } else {
        stats.player_scale = target_s;
    }
    // Squash & stretch: a damped spring kicked by jumps (stretch) and landings (squash), applied as
    // a volume-ish multiplier on top of the size scale. Feet stay planted (origin is at the feet).
    {
        let accel = -320.0 * player.squash - 20.0 * player.squash_vel;
        player.squash_vel += accel * dt;
        player.squash = (player.squash + player.squash_vel * dt).clamp(-0.3, 0.3);
    }
    let squash_y = 1.0 - 0.5 * player.squash;
    let squash_xz = 1.0 + 0.3 * player.squash;
    // Drawn size: human-proportioned beside pedestrians and cars in the city (`body_scale`).
    let body = stats.body_scale();
    if stats.enraged && !stats.in_city {
        let throb = 1.0 + stats.rage_pulse_flash * 0.16;
        tf.scale = Vec3::new(
            body * throb * squash_xz,
            body * (1.0 + stats.rage_pulse_flash * 0.22) * squash_y,
            body * throb * squash_xz,
        );
    } else {
        tf.scale = Vec3::new(body * squash_xz, body * squash_y, body * squash_xz);
    }
    let size_ratio = (stats.player_scale / 0.55).max(1.0);

    if player.dash_timer > 0.0 {
        player.dash_timer = (player.dash_timer - dt).max(0.0);
    }
    if player.roll_timer > 0.0 {
        player.roll_timer = (player.roll_timer - dt).max(0.0);
    }
    if player.dodge_counter_timer > 0.0 {
        player.dodge_counter_timer = (player.dodge_counter_timer - dt).max(0.0);
    }
    if player.attack_timer > 0.0 {
        player.attack_timer = (player.attack_timer - dt).max(0.0);
    }
    if player.attack_cooldown > 0.0 {
        player.attack_cooldown = (player.attack_cooldown - dt).max(0.0);
    }
    player.combo_timer = (player.combo_timer - dt).max(0.0);
    player.block_lockout = (player.block_lockout - dt).max(0.0);
    player.block_recoil = (player.block_recoil - dt * 5.0).max(0.0);
    if player.invuln_timer > 0.0 {
        player.invuln_timer = (player.invuln_timer - dt).max(0.0);
    }
    if player.slow_timer > 0.0 {
        player.slow_timer = (player.slow_timer - dt).max(0.0);
    }
    if stats.speed_boost_timer > 0.0 {
        stats.speed_boost_timer = (stats.speed_boost_timer - dt).max(0.0);
    }
    if stats.estus_drink_timer > 0.0 {
        stats.estus_drink_timer = (stats.estus_drink_timer - dt).max(0.0);
    }
    if stats.high_timer > 0.0 {
        stats.high_timer = (stats.high_timer - dt).max(0.0);
    }
    if stats.drunk_timer > 0.0 {
        stats.drunk_timer = (stats.drunk_timer - dt).max(0.0);
    }
    if stats.beer_cooldown > 0.0 {
        stats.beer_cooldown = (stats.beer_cooldown - dt).max(0.0);
    }
    if stats.stamina_regen_delay > 0.0 {
        stats.stamina_regen_delay = (stats.stamina_regen_delay - dt).max(0.0);
    } else if !player.block_active && player.charge_level().is_none() {
        // Holding the saber guard or charging a heavy pauses stamina regeneration.
        let dash_regen = if stats.enraged && !stats.in_city {
            62.0
        } else if stats.boss_mode {
            36.0
        } else {
            28.0
        };
        stats.dash_energy = (stats.dash_energy + dash_regen * dt).min(100.0);
    }

    // Drain screen-tap state so overlay transitions don't pick up stale taps
    let _ = crate::audio::take_touch_action_tap();

    // Damage blink (~7 Hz) while the post-hit invulnerability lasts. Vehicles own the hero's
    // visibility (the car hides them), so the blink stands down there.
    player.hurt_blink = (player.hurt_blink - dt).max(0.0);
    let in_vehicle = stats.driving_car.is_some() || stats.riding_scooter.is_some() || stats.riding_wheel;
    if in_vehicle {
        if player.blink_hidden {
            player.blink_hidden = false;
            if stats.driving_car.is_none() {
                *vis = Visibility::Visible;
            }
        }
    } else {
        let hide = player.hurt_blink > 0.0 && ((player.hurt_blink * 14.0) as i32) % 2 == 1;
        if hide != player.blink_hidden {
            *vis = if hide { Visibility::Hidden } else { Visibility::Visible };
            player.blink_hidden = hide;
        }
    }

    // Vehicles (and the Ferris wheel / Casino tables / Bartender modal) own Stan's position or UI focus.
    if stats.driving_car.is_some()
        || stats.riding_scooter.is_some()
        || stats.riding_wheel
        || stats.phone_open
        || stats.casino_open
        || stats.arcade_open
        || stats.bartender_open
        || stats.char_select_open
        || stats.club_dance_open
    {
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.jump_buffer = 0.0;
        player.block_active = false;
        player.cancel_charge();
        return;
    }

    // Camera-relative movement input (keyboard + mobile virtual stick)
    let mut forward_in = 0.0_f32;
    let mut right_in = 0.0_f32;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        forward_in += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        forward_in -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        right_in -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        right_in += 1.0;
    }
    forward_in += crate::audio::touch_move_y();
    right_in += crate::audio::touch_move_x();

    let forward = Vec3::new(-orbit.yaw.sin(), 0.0, -orbit.yaw.cos());
    let right = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());
    let raw_move = forward * forward_in + right * right_in;
    let move_mag = raw_move.length().min(1.0);
    let mut dir = raw_move.normalize_or_zero();
    // Beer: a slow, wandering drift on the walk direction (see `drunk_steer_drift`).
    if stats.in_city && stats.drunk_timer > 0.0 && dir != Vec3::ZERO {
        let a = drunk_steer_drift(time.elapsed_secs(), stats.drunk_timer) * 0.45;
        dir = Quat::from_rotation_y(a) * dir;
    }

    // Target Lock-On toggle [TAB] & Estus Tendy Flask [Q / 1]
    if keys.just_pressed(KeyCode::Tab) {
        stats.lock_on_active = !stats.lock_on_active;
        stats.hint_text = if stats.lock_on_active {
            "TARGET LOCK-ON ENGAGED! [TAB] to unlock".to_string()
        } else {
            "Target Lock-On released.".to_string()
        };
    }
    if (keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Digit1))
        && stats.estus_flasks > 0
        && stats.hearts < stats.max_hearts
        && stats.estus_drink_timer <= 0.0
    {
        stats.estus_flasks -= 1;
        stats.estus_drink_timer = 0.65;
        stats.hearts = (stats.hearts + 2).min(stats.max_hearts);
        crate::audio::play_sfx("estus_drink");
        stats.hint_text = format!(
            "CHUGGED HONEY-MUSTARD ESTUS! +2 Hearts, +100% Dignity ({}/{} left)",
            stats.estus_flasks, stats.max_estus_flasks
        );
    }

    // Dash outside boss mode; Dark Souls Dodge Roll + Light/Heavy Saber Slash in boss mode
    // In boss mode right mouse is the saber guard (hold), so the roll is Shift / ROLL only.
    let mut shift_pressed = keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || crate::audio::take_touch_dash();
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let touch_atk = crate::audio::take_touch_attack();
    let touch_block = crate::audio::touch_block_held();
    // Lock-on target (boss mode): Stan keeps facing it while strafing, and swings prefer it.
    let lock_target = if stats.boss_mode && stats.lock_on_active {
        pick_lock_target(tf.translation, target_q.iter().map(|(t, ..)| t.translation))
    } else {
        None
    };
    let mut strafe_input = false;
    if stats.boss_mode {
        // One attack button family: tap for a light swing, hold to charge and release a heavy.
        let ef = !stats.in_city;
        let mut atk_pressed = mouse_buttons.just_pressed(MouseButton::Left)
            || keys.just_pressed(KeyCode::KeyJ)
            || keys.just_pressed(KeyCode::KeyX)
            || (ef && (keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::KeyF)))
            || touch_atk;
        let mut atk_down = mouse_buttons.pressed(MouseButton::Left)
            || keys.pressed(KeyCode::KeyJ)
            || keys.pressed(KeyCode::KeyX)
            || (ef && (keys.pressed(KeyCode::KeyE) || keys.pressed(KeyCode::KeyF)))
            || crate::audio::touch_attack_held();
        let mut block_held = mouse_buttons.pressed(MouseButton::Right)
            || keys.pressed(KeyCode::KeyR)
            || touch_block;
        // Debug-only motion demo (`?debug=1&combat_demo=...`) feeds synthetic inputs.
        let demo_pose = combat_demo_inputs(
            &mut demo,
            dt,
            &mut player,
            &mut stats,
            &mut orbit,
            &mut tf,
            &mut atk_pressed,
            &mut atk_down,
            &mut block_held,
            &mut shift_pressed,
        );
        // Strafe: block held (the guard covers a front arc) or the dedicated key (V, gamepad L3).
        strafe_input = block_held || keys.pressed(KeyCode::KeyV);

        if shift_pressed && player.roll_timer <= 0.0 && stats.dash_energy >= 22.0 {
            stats.dash_energy -= 22.0;
            stats.stamina_regen_delay = 0.50;
            player.roll_timer = ROLL_SECS;
            player.invuln_timer = 0.58;
            player.dodge_counter_timer = 2.8;
            player.block_active = false;
            crate::audio::play_sfx("dodge_roll");
        }

        // Saber guard: held input raises it unless swinging, rolling or guard-broken.
        let can_guard = block_held
            && player.roll_timer <= 0.0
            && player.attack_timer <= 0.0
            && player.block_lockout <= 0.0
            && stats.estus_drink_timer <= 0.0;
        if can_guard {
            if !player.block_active {
                player.block_active = true;
                // Perfect window only re-arms after the guard has been down for a moment.
                player.block_time = if player.block_down_time >= BLOCK_REARM_SECS {
                    0.0
                } else {
                    PERFECT_BLOCK_SECS
                };
            } else {
                player.block_time += dt;
            }
            player.block_down_time = 0.0;
        } else {
            player.block_active = false;
            player.block_down_time += dt;
        }

        // Attack button: a press starts a hold; releasing before `HEAVY_CHARGE_SECS` is a light
        // swing (fired on release), releasing after it (or `CHARGE_AUTO_RELEASE_SECS`) a heavy.
        // Rolling and the guard cancel the hold until the button is let go.
        let mut fire: Option<bool> = None;
        if !demo_pose {
            if player.attack_hold == AttackHold::Spent && !atk_down {
                player.attack_hold = AttackHold::Idle;
            }
            if player.attack_hold == AttackHold::Idle && atk_pressed {
                player.attack_hold = AttackHold::Held(0.0);
            }
            if let AttackHold::Held(held) = player.attack_hold {
                // Only time held after the current swing counts, so a press during a swing
                // buffers into a tap instead of skipping the visible wind-up.
                let held = if player.attack_timer <= 0.0 { held + dt } else { held };
                player.attack_hold = if player.block_active || player.roll_timer > 0.0 {
                    AttackHold::Spent
                } else if !atk_down {
                    fire = Some(held >= HEAVY_CHARGE_SECS);
                    AttackHold::Idle
                } else if held >= CHARGE_AUTO_RELEASE_SECS {
                    fire = Some(true);
                    AttackHold::Spent
                } else {
                    AttackHold::Held(held)
                };
            }
        }
        if !player.block_active && !demo_pose {
            let ready = fire.is_some() && player.attack_cooldown <= 0.0;
            // Short on stamina for the heavy at release: fall back to a light swing.
            let starts_heavy = ready && fire == Some(true) && stats.dash_energy >= 32.0;
            let starts_light = ready && !starts_heavy && stats.dash_energy >= 16.0;
            // Attack assist: face (and step toward) a target in reach before the swing starts.
            let assist_gap = if starts_heavy || starts_light {
                player.swing_id = player.swing_id.wrapping_add(1);
                let targets: Vec<SaberTarget> = target_q
                    .iter()
                    .map(|(t, is_mom, is_lexie, enemy)| {
                        if is_mom {
                            SaberTarget::mom(t)
                        } else if is_lexie {
                            SaberTarget::lexie(t)
                        } else if let Some(e) = enemy {
                            SaberTarget::veggie(t, e.kind)
                        } else {
                            SaberTarget { pos: t.translation, radius: 0.5, height: 1.5 }
                        }
                    })
                    .collect();
                let cam_fwd = Vec3::new(-orbit.yaw.sin(), 0.0, -orbit.yaw.cos());
                attack_assist(&mut player, &mut tf, &stats, cam_fwd, lock_target, &targets, starts_heavy)
            } else {
                None
            };
            if starts_heavy {
                stats.dash_energy -= 32.0;
                stats.stamina_regen_delay = 0.65;
                stats.heavy_attack = true;
                player.attack_timer = HEAVY_SWING_SECS;
                player.attack_cooldown = 0.58;
                player.combo_timer = 0.0;
                let lunge_fwd = Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos());
                // Lunge scales with Stan's size, but never carries him into an assisted target.
                let s = stats.player_scale.max(0.55);
                let mut lunge = (0.28 * s).max(0.9);
                if let Some(gap) = assist_gap {
                    lunge = lunge.min((gap - 0.3 * s).max(0.0));
                }
                tf.translation += lunge_fwd * lunge;
                crate::audio::play_sfx("saber_swing");
            } else if starts_light {
                stats.dash_energy -= 16.0;
                stats.stamina_regen_delay = 0.42;
                stats.heavy_attack = false;
                player.attack_timer = LIGHT_SWING_SECS;
                player.attack_cooldown = 0.36;
                // Chained light swings alternate forehand / backhand.
                player.swing_side = if player.combo_timer > 0.0 { -player.swing_side } else { 1.0 };
                player.combo_timer = 0.75;
                crate::audio::play_sfx("saber_swing");
            }
        }
    } else if shift_pressed && player.dash_timer <= 0.0 && stats.dash_energy >= 30.0 {
        stats.dash_energy -= 30.0;
        stats.stamina_regen_delay = 0.45;
        player.dash_timer = 0.22;
        player.invuln_timer = 0.35;
        crate::audio::play_sfx("dash");
    }
    if !stats.boss_mode {
        player.block_active = false;
        player.attack_hold = AttackHold::Idle;
    }

    let sprinting = shift_held && move_mag > 0.15 && player.roll_timer <= 0.0 && stats.dash_energy > 2.0;
    if sprinting {
        stats.dash_energy = (stats.dash_energy - 15.0 * dt).max(0.0);
        stats.stamina_regen_delay = 0.35;
    }

    // Jump & Double-Jump (Space or Touch JUMP). A press that can't be used yet (both jumps spent)
    // stays buffered for JUMP_BUFFER_SECS, so pressing just before touching down still jumps.
    player.jump_buffer = (player.jump_buffer - dt).max(0.0);
    if keys.just_pressed(KeyCode::Space) || crate::audio::take_touch_jump() {
        player.jump_buffer = JUMP_BUFFER_SECS;
    }
    if player.jump_buffer > 0.0 && (player.grounded || player.jumps_used < 2) {
        player.jump_buffer = 0.0;
        player.squash_vel -= 5.0;
        if player.jumps_used == 0 {
            crate::audio::play_sfx("jump");
        } else {
            crate::audio::play_sfx("double_jump");
        }
        player.velocity.y = 9.2
            + (stats.player_scale - 0.55).max(0.0)
                * if stats.in_city || stats.in_casino_level || stats.in_club_level { 1.85 } else { 1.55 }
            + if stats.enraged && !stats.in_city { 2.2 } else { 0.0 };
        player.grounded = false;
        player.jumps_used += 1;
    }

    // Horizontal speed scales with player growth
    let effective_mag = if player.roll_timer > 0.0 && move_mag < 0.05 {
        dir = Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos());
        1.0
    } else {
        move_mag
    };
    let base_speed = if player.roll_timer > 0.0 {
        19.5
    } else if player.dash_timer > 0.0 {
        15.8
    } else {
        7.4
    };
    let mut speed = base_speed * size_ratio.powf(0.38);
    if sprinting {
        speed *= 1.36;
    }
    if stats.speed_boost_timer > 0.0 {
        speed *= 1.75;
    }
    if stats.enraged && !stats.in_city {
        speed *= 1.38 + 0.10 * (stats.rage_level.min(4) as f32);
    }
    if stats.estus_drink_timer > 0.0 {
        speed *= 0.38;
    }
    if player.slow_timer > 0.0 {
        speed *= 0.6;
    }
    // Strafe mode (boss mode only): lock-on, block held or the strafe key. Rolls still steer.
    player.strafing = stats.boss_mode && (stats.lock_on_active || strafe_input);
    if player.block_active {
        speed *= BLOCK_MOVE_MUL;
    } else if player.charge_level().is_some() {
        speed *= CHARGE_MOVE_MUL;
    } else if player.strafing && player.roll_timer <= 0.0 {
        speed *= STRAFE_MOVE_MUL;
    }

    // Ease toward the input velocity (snappier on the ground than in the air). Dashes and rolls
    // stay instant so their distances are unchanged, and anything above this state's top speed
    // (e.g. the frame a dash ends) is clamped at once rather than coasting.
    let target_h = Vec2::new(dir.x, dir.z) * speed * effective_mag;
    let mut cur_h = Vec2::new(player.velocity.x, player.velocity.z);
    if player.roll_timer > 0.0 || player.dash_timer > 0.0 {
        cur_h = target_h;
    } else {
        if cur_h.length() > speed {
            cur_h = cur_h.normalize_or_zero() * speed;
        }
        let rate = if !player.grounded {
            AIR_ACCEL_RATE
        } else if target_h.length_squared() >= cur_h.length_squared() {
            GROUND_ACCEL_RATE
        } else {
            GROUND_DECEL_RATE
        };
        cur_h += (target_h - cur_h) * smooth_factor(rate, dt);
        if target_h == Vec2::ZERO && cur_h.length_squared() < 0.0025 {
            cur_h = Vec2::ZERO;
        }
    }
    player.velocity.x = cur_h.x;
    player.velocity.z = cur_h.y;

    let horiz_speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    if horiz_speed > 0.25 {
        // Backpedalling while strafing runs the walk cycle in reverse so the legs step backwards.
        let fwd_v = Vec2::new(-player.facing_yaw.sin(), -player.facing_yaw.cos());
        let backwards = player.strafing && fwd_v.dot(Vec2::new(player.velocity.x, player.velocity.z)) < -0.3 * horiz_speed;
        let step = horiz_speed * dt * (2.2 / size_ratio.powf(0.28));
        player.walk_cycle += if backwards { -step } else { step };
    }

    if stats.boss_mode && player.attack_timer > 0.0 {
        // Facing is committed for the swing (set by the attack assist at swing start), so the
        // blade lands where it was aimed instead of following the stick mid-swing.
        tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    } else if player.strafing && player.roll_timer <= 0.0 {
        // Strafe: face the lock-on target, else the camera's forward; movement doesn't turn Stan.
        let target_yaw = match lock_target {
            Some(t) if Vec2::new(t.x - tf.translation.x, t.z - tf.translation.z).length_squared() > 0.01 => {
                (-(t.x - tf.translation.x)).atan2(-(t.z - tf.translation.z))
            }
            _ => orbit.yaw,
        };
        let diff = (target_yaw - player.facing_yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        player.facing_yaw += diff * (12.0 * dt).min(1.0);
        tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    } else if dir.length_squared() > 0.01 {
        // `plaid_hero.glb` is authored facing -Z at Quat::IDENTITY, so negate both components
        // to align the model's front (-Z) with the world movement vector `dir`.
        let target_yaw = (-dir.x).atan2(-dir.z);
        let diff = (target_yaw - player.facing_yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        player.facing_yaw += diff * (12.0 * dt).min(1.0);
        tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    }

    // Gravity & integrate
    let gravity = 22.0 + (stats.player_scale - 0.55).max(0.0) * 0.55;
    player.velocity.y -= gravity * dt;
    let was_grounded = player.grounded;
    let fall_speed = (-player.velocity.y).max(0.0);

    // Knockback: exact integral of an exponentially decaying impulse over this frame, so the total
    // shove distance is `initial_speed / KNOCKBACK_DECAY` regardless of frame rate.
    let knock_decay = (-KNOCKBACK_DECAY * dt).exp();
    let mut knock_disp = player.knockback * ((1.0 - knock_decay) / KNOCKBACK_DECAY);
    player.knockback *= knock_decay;
    if player.knockback.length_squared() < 0.01 {
        player.knockback = Vec3::ZERO;
    }

    let land_r = if stats.in_city || stats.in_casino_level || stats.in_club_level {
        (0.38 * stats.player_scale).clamp(0.55, 3.2)
    } else {
        (0.38 * stats.player_scale).clamp(0.38, 0.85)
    };
    let wall_r = if stats.in_city || stats.in_casino_level || stats.in_club_level {
        (0.28 * stats.player_scale).clamp(0.45, 2.8)
    } else {
        (0.28 * stats.player_scale).clamp(0.28, 0.72)
    };

    // Sub-step the move whenever one frame's horizontal travel exceeds half the wall radius
    // (kaiju sprint, honey-mustard boost, knockback). The push-out below picks the shallowest
    // axis, so a step deeper than half a thin wall's padded width would pop out the far side.
    let frame_h = Vec2::new(
        player.velocity.x * dt + knock_disp.x,
        player.velocity.z * dt + knock_disp.z,
    )
    .length();
    let max_step = wall_r * 0.5;
    let substeps = ((frame_h / max_step).ceil() as u32).clamp(1, 12);
    let sub_dt = dt / substeps as f32;
    knock_disp /= substeps as f32;

    player.grounded = false;
    for _ in 0..substeps {
        let prev_y = tf.translation.y;
        tf.translation += player.velocity * sub_dt + knock_disp;

        // Platform AABB landing (top surfaces)
        for (p_tf, p_box) in &platform_q {
            let min_x = p_tf.translation.x - p_box.half_extents.x - land_r;
            let max_x = p_tf.translation.x + p_box.half_extents.x + land_r;
            let min_z = p_tf.translation.z - p_box.half_extents.z - land_r;
            let max_z = p_tf.translation.z + p_box.half_extents.z + land_r;
            let top_y = p_tf.translation.y + p_box.half_extents.y;

            if tf.translation.x >= min_x
                && tf.translation.x <= max_x
                && tf.translation.z >= min_z
                && tf.translation.z <= max_z
                && player.velocity.y <= 0.0
                && prev_y >= top_y - 0.35
                && tf.translation.y <= top_y
            {
                tf.translation.y = top_y;
                player.velocity.y = 0.0;
                player.grounded = true;
                player.jumps_used = 0;
            }
        }

        // Solid side-wall pushout against all props, cabinets, walls, and obstacles (2 passes prevent corner-clipping)
        let player_top_y = tf.translation.y + (0.45 * stats.player_scale).max(0.55);
        for _ in 0..2 {
            for (p_tf, p_box) in &platform_q {
                let min_x = p_tf.translation.x - p_box.half_extents.x - wall_r;
                let max_x = p_tf.translation.x + p_box.half_extents.x + wall_r;
                let min_z = p_tf.translation.z - p_box.half_extents.z - wall_r;
                let max_z = p_tf.translation.z + p_box.half_extents.z + wall_r;
                let bottom_y = p_tf.translation.y - p_box.half_extents.y;
                let top_y = p_tf.translation.y + p_box.half_extents.y;

                if tf.translation.x > min_x
                    && tf.translation.x < max_x
                    && tf.translation.z > min_z
                    && tf.translation.z < max_z
                    && tf.translation.y < top_y - 0.05
                    && player_top_y > bottom_y + 0.05
                {
                    let dx_left = tf.translation.x - min_x;
                    let dx_right = max_x - tf.translation.x;
                    let dz_back = tf.translation.z - min_z;
                    let dz_front = max_z - tf.translation.z;
                    let m = dx_left.min(dx_right).min(dz_back).min(dz_front);
                    if (m - dx_left).abs() < 0.001 {
                        tf.translation.x = min_x;
                        player.velocity.x = player.velocity.x.min(0.0);
                        player.knockback.x = player.knockback.x.min(0.0);
                        knock_disp.x = knock_disp.x.min(0.0);
                    } else if (m - dx_right).abs() < 0.001 {
                        tf.translation.x = max_x;
                        player.velocity.x = player.velocity.x.max(0.0);
                        player.knockback.x = player.knockback.x.max(0.0);
                        knock_disp.x = knock_disp.x.max(0.0);
                    } else if (m - dz_back).abs() < 0.001 {
                        tf.translation.z = min_z;
                        player.velocity.z = player.velocity.z.min(0.0);
                        player.knockback.z = player.knockback.z.min(0.0);
                        knock_disp.z = knock_disp.z.min(0.0);
                    } else {
                        tf.translation.z = max_z;
                        player.velocity.z = player.velocity.z.max(0.0);
                        player.knockback.z = player.knockback.z.max(0.0);
                        knock_disp.z = knock_disp.z.max(0.0);
                    }
                }
            }
        }
    }

    // Landing squash, scaled by how hard the hero came down (tiny step-downs don't bounce).
    if player.grounded && !was_grounded {
        let impact = fall_speed / (9.0 + stats.player_scale * 1.2);
        if impact > 0.3 {
            player.squash_vel += 6.5 * impact.min(1.3);
        }
    }

    if player.grounded && horiz_speed > 0.25 {
        let half = (player.walk_cycle / std::f32::consts::PI).floor() as i32;
        if half != player.last_step_half {
            player.last_step_half = half;
            crate::audio::play_sfx("step");
            // City kaiju footsteps thump the camera, more the bigger Stan gets.
            if stats.in_city && stats.player_scale > 4.0 {
                let heft = ((stats.player_scale - 4.0) / 14.0).clamp(0.0, 1.0);
                orbit.shake = orbit.shake.max(0.12 + 0.33 * heft);
            }
        }
    }

    // Keep within level bounds or respawn on abyssal fall
    if stats.in_casino_level {
        let c = crate::casino::CASINO_LEVEL_ORIGIN;
        let lim = crate::casino::CASINO_WALK_LIMIT;
        tf.translation.x = tf.translation.x.clamp(c.x - lim, c.x + lim);
        tf.translation.z = tf.translation.z.clamp(c.z - lim, c.z + lim);
        if tf.translation.y < -10.0 {
            tf.translation = crate::casino::CASINO_SPAWN_POS;
            respawn_after_fall(&mut player, &mut orbit);
        }
    } else if stats.in_club_level {
        let c = crate::club::CLUB_LEVEL_ORIGIN;
        let lim = crate::club::CLUB_WALK_LIMIT;
        tf.translation.x = tf.translation.x.clamp(c.x - lim, c.x + lim);
        tf.translation.z = tf.translation.z.clamp(c.z - lim, c.z + lim);
        if tf.translation.y < -10.0 {
            tf.translation = crate::club::CLUB_SPAWN_POS;
            respawn_after_fall(&mut player, &mut orbit);
        }
    } else if stats.in_city {
        tf.translation.x = tf.translation.x.clamp(-126.0, 126.0);
        tf.translation.z = tf.translation.z.clamp(-126.0, 126.0);
        if tf.translation.y < -12.0 {
            tf.translation = Vec3::new(0.0, 1.0, 82.0);
            respawn_after_fall(&mut player, &mut orbit);
        }
    } else {
        tf.translation.x = tf.translation.x.clamp(-54.0, 54.0);
        tf.translation.z = tf.translation.z.clamp(-46.0, 46.0);
        if tf.translation.y < -12.0 {
            tf.translation = Vec3::new(0.0, 0.5, 29.0);
            respawn_after_fall(&mut player, &mut orbit);
        }
    }

    // Debug pose freeze holds Stan in place so the held pose can be inspected.
    if let Some(anchor) = demo.anchor {
        if demo.freeze.is_some() {
            tf.translation.x = anchor.x;
            tf.translation.z = anchor.z;
            player.knockback = Vec3::ZERO;
        }
    }
}

/// Debug-only combat motion demo, enabled with `?debug=1&combat_demo=<mode>`:
/// * `loop`: Stan repeats light (tap), light (backhand), charged heavy (hold, release), block,
///   roll in sequence.
/// * `light:<p>`, `light2:<p>`, `heavy:<p>`, `charge:<p>`, `roll:<p>`, `block`, `parry`: freeze
///   that pose at progress `p` (0..1) with Stan pinned in place.
///
/// An optional last `:<radians>` swings the camera around Stan (0 = behind, 1.57 = his side).
#[derive(Default)]
pub struct CombatDemo {
    init: bool,
    loop_mode: bool,
    freeze: Option<(String, f32)>,
    cam_offset: Option<f32>,
    anchor: Option<Vec3>,
    t: f32,
    step: usize,
}

#[allow(clippy::too_many_arguments)]
fn combat_demo_inputs(
    demo: &mut CombatDemo,
    dt: f32,
    player: &mut Player,
    stats: &mut PlayerStats,
    orbit: &mut CameraOrbit,
    tf: &mut Transform,
    atk_pressed: &mut bool,
    atk_down: &mut bool,
    block_held: &mut bool,
    shift_pressed: &mut bool,
) -> bool {
    if !demo.init {
        demo.init = true;
        if crate::audio::debug_keys_enabled() {
            let spec = crate::audio::debug_combat_demo();
            let parts: Vec<&str> = spec.split(':').filter(|s| !s.is_empty()).collect();
            if let Some(&mode) = parts.first() {
                if mode == "loop" {
                    demo.loop_mode = true;
                    demo.cam_offset = parts.get(1).and_then(|s| s.parse().ok());
                } else {
                    let p = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.4);
                    demo.freeze = Some((mode.to_string(), p));
                    demo.cam_offset = parts.get(2).and_then(|s| s.parse().ok());
                }
            }
        }
    }
    if !demo.loop_mode && demo.freeze.is_none() {
        return false;
    }
    // Keep the demo alive: no game over while it runs.
    stats.hearts = stats.max_hearts;
    stats.dash_energy = 100.0;
    if demo.anchor.is_none() {
        demo.anchor = Some(tf.translation);
    }
    if let Some(off) = demo.cam_offset {
        orbit.yaw = player.facing_yaw + off;
    }
    if let Some((mode, p)) = demo.freeze.clone() {
        let p = p.clamp(0.0, 0.999);
        match mode.as_str() {
            "light" | "light2" => {
                stats.heavy_attack = false;
                player.swing_side = if mode == "light2" { -1.0 } else { 1.0 };
                player.attack_timer = LIGHT_SWING_SECS * (1.0 - p);
                player.attack_cooldown = 1.0;
            }
            "heavy" => {
                stats.heavy_attack = true;
                player.attack_timer = HEAVY_SWING_SECS * (1.0 - p);
                player.attack_cooldown = 1.0;
            }
            "charge" => {
                player.attack_timer = 0.0;
                player.attack_hold =
                    AttackHold::Held(ATTACK_TAP_SECS + p * (HEAVY_CHARGE_SECS - ATTACK_TAP_SECS));
            }
            "roll" => {
                player.roll_timer = ROLL_SECS * (1.0 - p);
            }
            "block" | "parry" => {
                *block_held = true;
                if mode == "parry" {
                    player.block_recoil = 0.6;
                    demo.t += dt;
                    if demo.t > 0.5 {
                        demo.t = 0.0;
                        player.block_spark = 2;
                    }
                }
            }
            _ => {}
        }
        player.invuln_timer = player.invuln_timer.max(0.5);
        return true;
    }
    // Loop: light tap, light tap (backhand), attack held 0.6 s then released (charged heavy),
    // guard held for 1.2 s, roll; 4.6 s per cycle.
    const SCRIPT: [(f32, u8); 6] = [(0.0, 1), (0.45, 1), (0.9, 2), (2.2, 3), (3.4, 4), (4.6, 0)];
    demo.t += dt;
    if demo.step >= SCRIPT.len() {
        demo.step = 0;
        demo.t = 0.0;
    }
    while demo.step < SCRIPT.len() && demo.t >= SCRIPT[demo.step].0 {
        match SCRIPT[demo.step].1 {
            1 => *atk_pressed = true,
            2 => *atk_pressed = true,
            4 => *shift_pressed = true,
            _ => {}
        }
        demo.step += 1;
    }
    // The tap steps press and release in the same frame; the heavy step holds until 1.5 s.
    *atk_down = demo.t >= 0.9 && demo.t < 1.5;
    if demo.t >= 2.2 && demo.t < 3.4 {
        *block_held = true;
    }
    false
}

/// Fell out of the world: cut the camera straight to the respawn point (no long swoop) and give a
/// short blinking grace period so the reset reads as deliberate, not as a glitch.
fn respawn_after_fall(player: &mut Player, orbit: &mut CameraOrbit) {
    player.velocity = Vec3::ZERO;
    player.knockback = Vec3::ZERO;
    player.invuln_timer = player.invuln_timer.max(1.0);
    player.hurt_blink = player.hurt_blink.max(1.0);
    orbit.snap_follow = true;
}

fn ray_aabb_t(origin: Vec3, dir: Vec3, box_min: Vec3, box_max: Vec3) -> Option<(f32, f32)> {
    let mut t_min = -f32::INFINITY;
    let mut t_max = f32::INFINITY;
    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let mn = box_min[axis];
        let mx = box_max[axis];
        if d.abs() < 1e-6 {
            if o < mn || o > mx {
                return None;
            }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (mn - o) * inv;
            let mut t2 = (mx - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }
    if t_max > 0.0 {
        Some((t_min, t_max))
    } else {
        None
    }
}

pub fn update_camera(
    time: Res<Time>,
    mut orbit: ResMut<CameraOrbit>,
    mut stats: ResMut<PlayerStats>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<(&Transform, &Player), (Without<Camera3d>, Without<PlatformBox>)>,
    platform_q: Query<(&Transform, &PlatformBox), (Without<Player>, Without<Camera3d>)>,
    lock_q: Query<
        &Transform,
        (
            Or<(
                With<crate::boss::MomBoss>,
                With<crate::city::LexieBoss>,
                With<crate::enemies::Enemy>,
            )>,
            Without<Player>,
            Without<Camera3d>,
            Without<PlatformBox>,
        ),
    >,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>, Without<PlatformBox>)>,
) {
    // When seated at a Tendy Palace Casino game station, casino.rs drives the live 3D station cam.
    if stats.casino_open || stats.arcade_open || stats.club_dance_open {
        return;
    }
    // The camera frames the drawn body (smaller than the gameplay scale at human size in the
    // city), so the framing matches what it was tuned for.
    let cam_ps = stats.body_scale();
    let (Ok((p_tf, player)), Ok((mut c_tf, mut proj))) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let base_fov = std::f32::consts::FRAC_PI_4;

    // Lock-on yaw steering (disabled in cars and on the scooter, where the chase cam owns yaw).
    if stats.lock_on_active && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
        if let Some(target_pos) = pick_lock_target(p_tf.translation, lock_q.iter().map(|t| t.translation)) {
            let to_target = target_pos - p_tf.translation;
            // The camera sits at +(sin yaw, cos yaw) from Stan and looks along -(sin, cos), so
            // this yaw puts it behind Stan looking at the target (an extra half turn used to put
            // the camera on the target's side, looking away from it).
            let desired_yaw = (-to_target.x).atan2(-to_target.z);
            let mut diff = (desired_yaw - orbit.yaw) % std::f32::consts::TAU;
            if diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            } else if diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            orbit.yaw += diff * smooth_factor(7.5, dt);
        }
    }

    // Damped follow point: quick horizontally, softer vertically, lag capped relative to scale.
    // Snaps on state entry (flag) or on a teleport-sized jump in the raw player position.
    let p_pos = p_tf.translation;
    let follow_scale = cam_ps.max(0.55);
    let teleport_dist = 6.0 + follow_scale * 1.5;
    let snapping = orbit.snap_follow || p_pos.distance(orbit.last_player_pos) > teleport_dist;
    if snapping {
        orbit.follow_pos = p_pos;
        orbit.push_offset = Vec3::ZERO;
    } else {
        let kh = smooth_factor(16.0, dt);
        let kv = smooth_factor(7.0, dt);
        let mut f = orbit.follow_pos;
        f.x += (p_pos.x - f.x) * kh;
        f.z += (p_pos.z - f.z) * kh;
        f.y += (p_pos.y - f.y) * kv;
        let max_lag_h = 0.45 + 0.30 * follow_scale;
        let max_lag_v = 0.5 + 0.45 * follow_scale;
        let lag_h = Vec2::new(p_pos.x - f.x, p_pos.z - f.z);
        let lag_len = lag_h.length();
        if lag_len > max_lag_h {
            let keep = lag_h * (max_lag_h / lag_len);
            f.x = p_pos.x - keep.x;
            f.z = p_pos.z - keep.y;
        }
        let lag_v = p_pos.y - f.y;
        if lag_v.abs() > max_lag_v {
            f.y = p_pos.y - lag_v.signum() * max_lag_v;
        }
        orbit.follow_pos = f;
    }
    orbit.last_player_pos = p_pos;
    let follow = orbit.follow_pos;

    let anchor_y = if stats.driving_car.is_some() {
        1.95
    } else {
        0.55 + cam_ps * 0.92
    };
    let anchor = follow + Vec3::Y * anchor_y;
    // Widen camera distance slightly on narrow portrait viewports (mobile) so Stan stays framed.
    let aspect_mul = if let Ok(win) = window_q.get_single() {
        let aspect = win.width() / win.height().max(1.0);
        1.0 + (1.0 - aspect).clamp(0.0, 0.55) * 0.35
    } else {
        1.0
    };
    // Scale camera pullback with Stan's Katamari growth (linear in city/casino up to 18x kaiju, sqrt in apartment).
    let cam_scale_mul = if stats.driving_car.is_some() {
        0.92
    } else if stats.in_city || stats.in_casino_level || stats.in_club_level {
        0.72 + cam_ps * 0.24
    } else {
        0.82 + cam_ps.sqrt() * 0.34
    };
    // Pub 217 keeps a fixed tight interior scale; only count interior (not roof).
    let in_pub217 = stats.in_city && crate::city::is_in_pub217(p_pos, cam_ps);
    let pub_s = crate::city::PUB217_SCALE;
    let mut desired_dist = orbit.distance * cam_scale_mul * aspect_mul;
    // Clamp distance & pitch inside Pub 217 so the bar ceiling/walls don't occlude the player.
    if in_pub217 {
        desired_dist = desired_dist.min(4.2 * pub_s);
    }
    let min_cam_dist = if stats.in_casino_level || stats.in_club_level {
        (2.4 + cam_ps * 0.6).min(desired_dist)
    } else if stats.in_city {
        (2.2 + cam_ps * 1.8).min(desired_dist)
    } else {
        (2.2 + cam_ps * 0.45).min(desired_dist)
    };
    // The boom never pitches below BOOM_PITCH_MIN; any further "look up" tilts the view instead.
    let mut eff_pitch = (orbit.pitch.max(BOOM_PITCH_MIN) + (orbit.distance - 10.6).max(0.0) * 0.008)
        .clamp(BOOM_PITCH_MIN, 1.35);
    if in_pub217 {
        eff_pitch = eff_pitch.clamp(0.14, 0.28);
    }
    let look_up = (BOOM_PITCH_MIN - orbit.pitch).max(0.0);
    let dir = Vec3::new(
        orbit.yaw.sin() * eff_pitch.cos(),
        eff_pitch.sin(),
        orbit.yaw.cos() * eff_pitch.cos(),
    )
    .normalize();

    // Spring-arm ray-AABB sweep against large structural boxes (walls, fridge, buildings) that
    // actually cross the anchor->camera line. Small props & low ledges are ignored so they never
    // yank the camera in. Skipped for the NYC Skyscraper Titan, which looks over the city.
    let mut safe_dist = desired_dist;
    let sweep_pad = Vec3::splat(0.22);
    if !(stats.in_city && cam_ps >= 8.0) {
        for (p_tf_box, p_box) in &platform_q {
            if !stats.in_city && !stats.in_casino_level && !stats.in_club_level && p_tf_box.translation.z > 46.5 {
                continue;
            }
            let is_structural = p_box.half_extents.y >= 2.1
                && (p_box.half_extents.x >= 2.5 || p_box.half_extents.z >= 2.5);
            if !is_structural {
                continue;
            }
            let bmin = p_tf_box.translation - p_box.half_extents - sweep_pad;
            let bmax = p_tf_box.translation + p_box.half_extents + sweep_pad;
            // The boom always rises from the anchor, so boxes entirely below it can't block it.
            if bmax.y < anchor.y {
                continue;
            }
            if let Some((t_min, t_max)) = ray_aabb_t(anchor, dir, bmin, bmax) {
                if t_max > 0.0 && t_min < safe_dist {
                    if t_min > 0.0 {
                        safe_dist = safe_dist.min((t_min - 0.14).max(min_cam_dist));
                    } else if (p_tf_box.translation - anchor).dot(dir) > 0.0 {
                        safe_dist = min_cam_dist;
                    }
                }
            }
        }
    }

    // Clamp boom along room/city/casino bounds (allowing open camera pull-back at south spawn)
    let (room_min, room_max) = if stats.in_casino_level {
        let c = crate::casino::CASINO_LEVEL_ORIGIN;
        (
            Vec3::new(c.x - 21.0, 0.55, c.z - 21.0),
            Vec3::new(c.x + 21.0, 12.8, c.z + 21.0),
        )
    } else if stats.in_club_level {
        let c = crate::club::CLUB_LEVEL_ORIGIN;
        (
            Vec3::new(c.x - 18.2, 0.55, c.z - 18.2),
            Vec3::new(c.x + 18.2, 11.2, c.z + 18.2),
        )
    } else if stats.in_city {
        (
            Vec3::new(-165.0, -2.0, -165.0),
            Vec3::new(165.0, 260.0, 165.0),
        )
    } else {
        (
            Vec3::new(-55.2, -5.55, -47.2),
            Vec3::new(55.2, 24.0, 68.0),
        )
    };
    for axis in 0..3 {
        if dir[axis] > 1e-4 {
            let t = ((room_max[axis] - anchor[axis]) / dir[axis]).max(min_cam_dist);
            safe_dist = safe_dist.min(t);
        } else if dir[axis] < -1e-4 {
            let t = ((room_min[axis] - anchor[axis]) / dir[axis]).max(min_cam_dist);
            safe_dist = safe_dist.min(t);
        }
    }

    // Asymmetric, frame-rate independent boom smoothing: pull in fast when blocked, ease out slowly.
    if snapping {
        orbit.current_dist = safe_dist;
    } else {
        let rate = if safe_dist < orbit.current_dist { 30.0 } else { 7.0 };
        orbit.current_dist += (safe_dist - orbit.current_dist) * smooth_factor(rate, dt);
    }
    orbit.current_dist = orbit.current_dist.clamp(min_cam_dist, desired_dist);

    let mut cam_pos = anchor + dir * orbit.current_dist;
    if in_pub217 {
        let anch = crate::city::PUB217_ANCHOR;
        cam_pos.x = cam_pos.x.clamp(anch.x - 13.0 * pub_s, anch.x - 0.65 * pub_s);
        cam_pos.z = cam_pos.z.clamp(anch.z - 6.4 * pub_s, anch.z + 6.4 * pub_s);
        cam_pos.y = cam_pos.y.min(5.5 * pub_s);
    }

    // Camera-sphere push-out from any box it ends up inside. The correction is eased through
    // `push_offset` instead of applied in one frame, and skipped entirely for the city kaiju
    // (it would hop the camera onto skyscraper roofs; passing through is less jarring).
    let mut push_target = Vec3::ZERO;
    if !(stats.in_city && cam_ps >= 8.0) {
        let mut pushed = cam_pos;
        let cam_margin = Vec3::splat(0.38);
        for _ in 0..2 {
            for (p_tf_box, p_box) in &platform_q {
                if !stats.in_city && !stats.in_casino_level && !stats.in_club_level && p_tf_box.translation.z > 46.5 {
                    continue;
                }
                let top_y = p_tf_box.translation.y + p_box.half_extents.y;
                let bmin = p_tf_box.translation - p_box.half_extents - cam_margin;
                let bmax = p_tf_box.translation + p_box.half_extents + cam_margin;
                if pushed.x > bmin.x
                    && pushed.x < bmax.x
                    && pushed.y > bmin.y
                    && pushed.y < bmax.y
                    && pushed.z > bmin.z
                    && pushed.z < bmax.z
                {
                    let dx_l = pushed.x - bmin.x;
                    let dx_r = bmax.x - pushed.x;
                    let dy_b = pushed.y - bmin.y;
                    let dy_t = bmax.y - pushed.y;
                    let dz_b = pushed.z - bmin.z;
                    let dz_f = bmax.z - pushed.z;
                    let mut m = dx_l.min(dx_r).min(dy_t).min(dz_b).min(dz_f);
                    if anchor.y < top_y {
                        m = m.min(dy_b);
                    }
                    if anchor.y >= top_y && dy_t <= m + 0.45 {
                        pushed.y = bmax.y;
                    } else if (m - dy_t).abs() < 0.001 {
                        pushed.y = bmax.y;
                    } else if (m - dy_b).abs() < 0.001 && anchor.y < top_y {
                        pushed.y = bmin.y;
                    } else if (m - dx_l).abs() < 0.001 {
                        pushed.x = bmin.x;
                    } else if (m - dx_r).abs() < 0.001 {
                        pushed.x = bmax.x;
                    } else if (m - dz_b).abs() < 0.001 {
                        pushed.z = bmin.z;
                    } else {
                        pushed.z = bmax.z;
                    }
                }
            }
        }
        push_target = pushed - cam_pos;
    }
    if snapping {
        orbit.push_offset = push_target;
    } else {
        let k = smooth_factor(12.0, dt);
        let delta = (push_target - orbit.push_offset) * k;
        orbit.push_offset += delta;
    }
    cam_pos += orbit.push_offset;

    cam_pos = cam_pos.clamp(room_min, room_max);

    let zoom_ratio = (orbit.current_dist / desired_dist.max(0.01)).clamp(0.15, 1.0);
    let look_fwd = 1.0 * (0.45 + cam_ps.sqrt() * 0.22) * zoom_ratio;
    let mut look_target = follow
        + Vec3::new(0.0, anchor_y * 0.88, 0.0)
        - Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos()) * look_fwd;
    // Looking up past the boom limit: raise the look target so the view tilts upward.
    if look_up > 0.0 {
        let horiz = Vec2::new(look_target.x - cam_pos.x, look_target.z - cam_pos.z).length();
        look_target.y += horiz * (look_up * 0.7).tan();
    }

    // "Drunk" sway only while high in the city, eased in/out so it never pops.
    let wobble_target = if stats.in_city && stats.high_timer > 0.0 {
        (stats.high_timer / 3.0).min(1.0)
    } else {
        0.0
    };
    orbit.wobble_amp += (wobble_target - orbit.wobble_amp) * smooth_factor(1.6, dt);
    if orbit.wobble_amp < 0.001 && wobble_target == 0.0 {
        orbit.wobble_amp = 0.0;
    }
    let drunk_amp = orbit.wobble_amp;
    // Beer sway: slower and rollier than the high sway (a ship deck rather than a buzz).
    let beer_target = if stats.in_city && stats.drunk_timer > 0.0 {
        (stats.drunk_timer / 3.0).min(1.0)
    } else {
        0.0
    };
    orbit.beer_amp += (beer_target - orbit.beer_amp) * smooth_factor(1.2, dt);
    if orbit.beer_amp < 0.001 && beer_target == 0.0 {
        orbit.beer_amp = 0.0;
    }
    let beer_amp = orbit.beer_amp;

    // Small FOV kick while dashing.
    orbit.speed_kick_t = (orbit.speed_kick_t - dt).max(0.0);
    let kick_target = if orbit.speed_kick_t > 0.0 {
        0.11
    } else if player.dash_timer > 0.0 {
        0.06
    } else {
        0.0
    };
    let kick_rate = if kick_target > orbit.fov_kick { 18.0 } else { 6.0 };
    orbit.fov_kick += (kick_target - orbit.fov_kick) * smooth_factor(kick_rate, dt);

    let mut final_cam_pos = cam_pos;
    let mut final_look = look_target;
    let mut roll = 0.0;
    let mut fov = base_fov + orbit.fov_kick;
    if drunk_amp > 0.0 {
        let t = time.elapsed_secs();
        final_cam_pos += Vec3::new(
            (t * 1.65).sin() * 0.28 * drunk_amp,
            (t * 2.35).cos() * 0.16 * drunk_amp,
            (t * 1.25).cos() * 0.22 * drunk_amp,
        );
        final_look += Vec3::new(
            (t * 1.4).cos() * 0.24 * drunk_amp,
            (t * 1.9).sin() * 0.14 * drunk_amp,
            0.0,
        );
        roll = (t * 1.35).sin() * 0.065 * drunk_amp;
        fov += (t * 2.05).sin() * 0.075 * drunk_amp;
    }
    if beer_amp > 0.0 {
        let t = time.elapsed_secs();
        final_cam_pos += Vec3::new((t * 0.7).sin() * 0.45, (t * 1.1).sin() * 0.08, 0.0) * beer_amp;
        final_look += Vec3::new((t * 0.55).sin() * 0.35, 0.0, 0.0) * beer_amp;
        roll += ((t * 0.62).sin() * 0.13 + (t * 1.7).sin() * 0.025) * beer_amp;
        fov += (t * 0.8).sin() * 0.04 * beer_amp;
    }
    let mut goal = Transform::from_translation(final_cam_pos);
    goal.look_at(final_look, Vec3::Y);
    if roll != 0.0 {
        goal.rotate_local_z(roll);
    }

    // Impact shake: an explicit, quickly decaying intensity (hits, kaiju footsteps) turned into a
    // small rotational jitter. Rotation only, so it reads the same at every camera distance.
    if stats.shake_request > 0.0 {
        orbit.shake = orbit.shake.max(stats.shake_request.min(1.0));
        stats.shake_request = 0.0;
    }
    if orbit.shake > 0.0 {
        let t = time.elapsed_secs();
        let amp = orbit.shake * 0.03;
        goal.rotate_local_x(((t * 41.0).sin() * 0.6 + (t * 67.0 + 1.3).sin() * 0.4) * amp);
        goal.rotate_local_y(((t * 37.0 + 0.7).sin() * 0.6 + (t * 59.0 + 2.1).sin() * 0.4) * amp);
        goal.rotate_local_z((t * 47.0 + 0.4).sin() * amp * 0.5);
        orbit.shake *= (-7.0 * dt).exp();
        if orbit.shake < 0.01 {
            orbit.shake = 0.0;
        }
    }

    // Ease from the previous shot (e.g. a cutscene's last frame) into the follow pose,
    // snapping cleanly if switching between isolated stages (> 60m apart).
    if orbit.blend_request {
        orbit.blend_request = false;
        if c_tf.translation.distance(goal.translation) > 60.0 {
            orbit.blend_t = CAMERA_BLEND_SECS;
        } else {
            orbit.blend_t = 0.0;
            orbit.blend_from_pos = c_tf.translation;
            orbit.blend_from_rot = c_tf.rotation;
            orbit.blend_from_fov = match *proj {
                Projection::Perspective(ref p) => p.fov,
                _ => base_fov,
            };
        }
    }
    orbit.snap_follow = false;
    if orbit.blend_t < CAMERA_BLEND_SECS {
        orbit.blend_t = (orbit.blend_t + dt).min(CAMERA_BLEND_SECS);
        let x = orbit.blend_t / CAMERA_BLEND_SECS;
        let s = x * x * (3.0 - 2.0 * x);
        c_tf.translation = orbit.blend_from_pos.lerp(goal.translation, s);
        c_tf.rotation = orbit.blend_from_rot.slerp(goal.rotation, s);
        fov = orbit.blend_from_fov + (fov - orbit.blend_from_fov) * s;
    } else {
        c_tf.translation = goal.translation;
        c_tf.rotation = goal.rotation;
    }
    if let Projection::Perspective(ref mut p) = *proj {
        p.fov = fov;
    }
}

pub fn animate_and_collect_pickups(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    player_q: Query<(&Transform, &Player)>,
    mut tendy_q: Query<
        (Entity, &mut Transform, &TendyPickup),
        (Without<Player>, Without<crate::state::HeadphoneHazard>),
    >,
    mut power_q: Query<
        (Entity, &mut Transform, &PowerupPickup),
        (Without<Player>, Without<TendyPickup>, Without<crate::state::HeadphoneHazard>),
    >,
    headphone_q: Query<
        (Entity, &Transform, &crate::state::HeadphoneHazard),
        (
            Without<Player>,
            Without<TendyPickup>,
            Without<PowerupPickup>,
            Without<crate::state::FridgeGoal>,
        ),
    >,
    fridge_q: Query<
        &Transform,
        (
            With<crate::state::FridgeGoal>,
            Without<Player>,
            Without<TendyPickup>,
            Without<PowerupPickup>,
            Without<crate::state::HeadphoneHazard>,
        ),
    >,
    mut door_q: Query<
        (&Name, &mut Transform),
        (
            Without<Player>,
            Without<TendyPickup>,
            Without<PowerupPickup>,
            Without<crate::state::FridgeGoal>,
            Without<crate::state::HeadphoneHazard>,
        ),
    >,
) {
    // Debug scene skips (K = ending cutscene, B = boss, P = headphone cutscene) only with
    // `?debug=1` in the URL. In the city, P is the real phone key (scooter.rs); that system
    // doesn't run here.
    let debug_keys = (keys.just_pressed(KeyCode::KeyK)
        || keys.just_pressed(KeyCode::KeyB)
        || keys.just_pressed(KeyCode::KeyP))
        && crate::audio::debug_keys_enabled();
    if debug_keys && keys.just_pressed(KeyCode::KeyK) {
        next_state.set(GameState::Cutscene);
        return;
    }
    if debug_keys && keys.just_pressed(KeyCode::KeyB) {
        next_state.set(GameState::BossFight);
        return;
    }
    if debug_keys && keys.just_pressed(KeyCode::KeyP) {
        if let Ok((p_tf, player)) = player_q.get_single() {
            stats.saved_apartment_pos = p_tf.translation;
            stats.saved_apartment_scale = stats.player_scale;
            stats.saved_apartment_yaw = player.facing_yaw;
        }
        stats.headphone_return_boss = *state.get() == GameState::BossFight;
        next_state.set(GameState::HeadphoneCutscene);
        return;
    }
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let player_info = player_q
        .get_single()
        .map(|(tf, pl)| (tf.translation, pl.facing_yaw))
        .ok();
    let player_pos = player_info.map(|(p, _)| p);
    let pickup_r = 1.15 + stats.player_scale * 0.45;
    let player_mid_y = (0.55 * stats.player_scale).max(0.55);

    if let Some((p_pos, p_yaw)) = player_info {
        for (h_ent, h_tf, hazard) in &headphone_q {
            let flat_d = Vec2::new(p_pos.x - h_tf.translation.x, p_pos.z - h_tf.translation.z).length();
            if flat_d < hazard.radius + stats.player_scale * 0.28
                && (p_pos.y - h_tf.translation.y).abs() < 1.8
            {
                commands.entity(h_ent).despawn_recursive();
                stats.saved_apartment_pos = p_pos;
                stats.saved_apartment_scale = stats.player_scale;
                stats.saved_apartment_yaw = p_yaw;
                stats.headphone_return_boss = *state.get() == GameState::BossFight;
                next_state.set(GameState::HeadphoneCutscene);
                return;
            }
        }
    }

    for (entity, mut tf, tendy) in &mut tendy_q {
        tf.rotate_y(2.4 * dt);
        tf.translation.y = tendy.base_y + ((t * 3.2 + tendy.phase).sin() * 0.16);

        if let Some(p_pos) = player_pos {
            let d = (p_pos + Vec3::Y * player_mid_y).distance(tf.translation);
            if d < pickup_r {
                commands.entity(entity).remove::<TendyPickup>();
                start_pickup_pop(&mut commands, entity, tf.scale);
                stats.tendies += 1;
                stats.add_tendy_wendies(2);
                stats.score += 150;
                crate::audio::play_sfx("tendy");
                if stats.tendies >= stats.total_tendies {
                    stats.score += 1000;
                    stats.hint_text = "ADULT SIZE! Drop to the floor & open the Giant Fridge!".to_string();
                } else {
                    let next_cm = (stats.target_scale() * 30.0).round() as u32;
                    stats.hint_text = format!(
                        "Grew to {}cm! ({}/{} Tendies -> Fridge unlocks at 12/12)",
                        next_cm, stats.tendies, stats.total_tendies
                    );
                }
            }
        }
    }

    for (entity, mut tf, power) in &mut power_q {
        tf.rotate_y(1.8 * dt);
        tf.translation.y = power.base_y + ((t * 2.8 + power.phase).sin() * 0.14);

        if let Some(p_pos) = player_pos {
            if (p_pos + Vec3::Y * player_mid_y).distance(tf.translation) < pickup_r {
                commands.entity(entity).remove::<PowerupPickup>();
                start_pickup_pop(&mut commands, entity, tf.scale);
                crate::audio::play_sfx("powerup");
                match power.kind {
                    PowerupKind::HoneyMustard => stats.speed_boost_timer = 10.0,
                    PowerupKind::RanchShield => stats.has_ranch_shield = true,
                }
            }
        }
    }

    // Giant Refrigerator Goal trigger once Hero grows to Full Adult Size
    if let (Some(p_pos), Ok(f_tf)) = (player_pos, fridge_q.get_single()) {
        let horiz_d = Vec2::new(p_pos.x - f_tf.translation.x, p_pos.z - f_tf.translation.z).length();
        let dy = (p_pos.y - f_tf.translation.y).abs();
        if horiz_d < 4.5 && dy < 3.0 {
            if stats.tendies >= stats.total_tendies || stats.player_scale >= 5.95 {
                let squad = crate::audio::net_squad_info();
                let all_ready = squad[2] <= 1 || squad[1] >= squad[2];
                if !all_ready {
                    stats.hint_text = format!(
                        "WAITING FOR SQUAD TO EAT 12/12 TENDIES! ({}/{} READY)",
                        squad[1], squad[2]
                    );
                } else if stats.fridge_open <= 0.0 {
                    stats.fridge_open = 0.02;
                    stats.score += 5000;
                    crate::audio::play_sfx("win");
                    stats.hint_text = "FRIDGE OPEN! Tendy Feast Unlocked!".to_string();
                }
            } else {
                stats.hint_text = format!(
                    "Too tiny ({}cm) for the Fridge handle! Collect {}/{} Tendies.",
                    stats.height_cm(),
                    stats.tendies,
                    stats.total_tendies
                );
            }
        }
    }

    if stats.fridge_open > 0.0 {
        stats.fridge_open = (stats.fridge_open + dt * 1.45).min(1.0);
        for (name, mut d_tf) in &mut door_q {
            if name.as_str() == "FridgeDoor" {
                d_tf.rotation = Quat::from_rotation_y(stats.fridge_open * 2.05);
            }
        }
        if stats.fridge_open >= 0.72 {
            crate::audio::net_event(3, 1);
            next_state.set(GameState::Cutscene);
        }
    }
}

/// Saber grip (rotation of `HeroSaberBlade` in the right hand) as authored in `boss.rs`.
fn saber_rest_grip() -> Quat {
    Quat::from_rotation_x(-1.18)
}

/// Arm rotation from a heading (`az`, + turns toward Stan's left) and an elevation (`el`, 0 hangs
/// down, PI/2 points straight ahead, PI straight up). Built from two Euler steps so the arm never
/// flips its roll the way a shortest-arc rotation can near vertical.
fn arm_aim(az: f32, el: f32) -> Quat {
    Quat::from_rotation_y(az) * Quat::from_rotation_x(el)
}

/// A combat key pose in the chest frame (before the upper-body twist is applied).
#[derive(Clone, Copy)]
struct SwingKey {
    /// Progress (0..1) of the move at which this key is hit.
    at: f32,
    /// Upper-body twist (+ turns the chest to Stan's left) and lean (- bends forward).
    yaw: f32,
    pitch: f32,
    /// Right / left arm heading and elevation (see `arm_aim`).
    r_az: f32,
    r_el: f32,
    l_az: f32,
    l_el: f32,
    /// Blade direction in the chest frame (-Z ahead, +X Stan's right, +Y up).
    blade: Vec3,
    /// Leg swings (+ forward) and body drop.
    ll: f32,
    rl: f32,
    bob: f32,
}

impl SwingKey {
    fn lerp(a: &SwingKey, b: &SwingKey, t: f32) -> SwingKey {
        let m = |x: f32, y: f32| x + (y - x) * t;
        SwingKey {
            at: m(a.at, b.at),
            yaw: m(a.yaw, b.yaw),
            pitch: m(a.pitch, b.pitch),
            r_az: m(a.r_az, b.r_az),
            r_el: m(a.r_el, b.r_el),
            l_az: m(a.l_az, b.l_az),
            l_el: m(a.l_el, b.l_el),
            blade: a.blade.normalize().lerp(b.blade.normalize(), t).normalize_or(Vec3::NEG_Z),
            ll: m(a.ll, b.ll),
            rl: m(a.rl, b.rl),
            bob: m(a.bob, b.bob),
        }
    }

    fn mirrored(mut self) -> SwingKey {
        self.yaw = -self.yaw;
        self.r_az = -self.r_az;
        self.blade.x = -self.blade.x;
        // The free left hand swings out to the left for balance on backhands.
        self.l_az = 1.15;
        self.l_el = 0.55;
        self
    }
}

/// Sample a key track at progress `p` with smoothstep easing inside each segment.
fn sample_keys(keys: &[SwingKey], p: f32) -> SwingKey {
    if p <= keys[0].at {
        return keys[0];
    }
    for w in keys.windows(2) {
        if p <= w[1].at {
            let x = ((p - w[0].at) / (w[1].at - w[0].at).max(1e-4)).clamp(0.0, 1.0);
            let e = x * x * (3.0 - 2.0 * x);
            return SwingKey::lerp(&w[0], &w[1], e);
        }
    }
    keys[keys.len() - 1]
}

#[allow(clippy::too_many_arguments)]
const fn key(
    at: f32,
    yaw: f32,
    pitch: f32,
    r_az: f32,
    r_el: f32,
    l_az: f32,
    l_el: f32,
    blade: Vec3,
    ll: f32,
    rl: f32,
    bob: f32,
) -> SwingKey {
    SwingKey { at, yaw, pitch, r_az, r_el, l_az, l_el, blade, ll, rl, bob }
}

/// Light forehand slash, right to left. Short anticipation (the hit registers on the first
/// frames), the blade crosses the front around 30 to 45 percent, then follow-through.
const LIGHT_KEYS: [SwingKey; 5] = [
    key(0.00, -0.45, 0.05, -1.30, 1.20, 0.60, 0.60, Vec3::new(0.60, 0.45, 0.65), 0.30, -0.25, -0.02),
    key(0.14, -0.80, 0.08, -1.75, 1.25, 0.70, 0.55, Vec3::new(0.35, 0.55, 0.75), 0.35, -0.30, -0.04),
    key(0.32, 0.00, -0.14, -0.25, 1.45, 0.90, 0.50, Vec3::new(-0.15, 0.10, -1.00), 0.55, -0.40, -0.09),
    key(0.50, 0.70, -0.20, 1.10, 1.35, 0.95, 0.45, Vec3::new(-0.95, 0.00, -0.10), 0.60, -0.45, -0.11),
    key(1.00, 0.85, -0.10, 1.30, 0.90, 0.80, 0.40, Vec3::new(-0.55, -0.45, 0.65), 0.40, -0.30, -0.05),
];

/// Heavy overhead chop: raise two-handed behind the head, lean back, then drive down into a deep
/// lunge with the weight on the front foot.
const HEAVY_KEYS: [SwingKey; 4] = [
    key(0.00, -0.20, 0.10, 0.10, 2.00, -0.10, 1.90, Vec3::new(0.10, 0.90, 0.30), 0.30, -0.30, 0.00),
    key(0.20, -0.30, 0.24, 0.15, 2.75, -0.15, 2.65, Vec3::new(0.05, 0.20, 1.00), 0.45, -0.35, 0.03),
    key(0.42, 0.08, -0.52, 0.20, 1.15, -0.20, 1.20, Vec3::new(0.00, -0.50, -0.87), 0.95, -0.70, -0.16),
    key(1.00, 0.10, -0.45, 0.15, 0.85, -0.15, 0.85, Vec3::new(0.05, -0.85, -0.50), 0.90, -0.65, -0.17),
];

/// Saber guard: chest turned slightly, right hand forward at chest height, blade angled up and
/// across the body, left hand supporting, feet braced.
const GUARD_KEY: SwingKey =
    key(0.0, 0.35, -0.08, 0.35, 1.05, -0.35, 1.00, Vec3::new(-0.70, 0.68, -0.20), 0.35, -0.40, -0.07);

/// Full combat pose: limb targets plus upper-body twist, saber grip and the whole-body tumble.
struct CombatPose {
    ll: Quat,
    rl: Quat,
    la: Quat,
    ra: Quat,
    torso: Quat,
    head: Quat,
    bob: f32,
    upper: Quat,
    grip: Quat,
    /// Whole-body (scene root) transform for the dodge roll tumble.
    body: Option<Transform>,
}

impl CombatPose {
    fn from_key(k: &SwingKey) -> CombatPose {
        let ra = arm_aim(k.r_az, k.r_el);
        let grip = ra.inverse() * Quat::from_rotation_arc(Vec3::Y, k.blade.normalize_or(Vec3::NEG_Z));
        CombatPose {
            ll: Quat::from_rotation_x(k.ll),
            rl: Quat::from_rotation_x(k.rl),
            la: arm_aim(k.l_az, k.l_el),
            ra,
            torso: Quat::IDENTITY,
            // Head counter-turns so Stan keeps his eyes on the target through the twist.
            head: Quat::from_rotation_y(-k.yaw * 0.7) * Quat::from_rotation_x(-k.pitch * 0.5),
            bob: k.bob,
            upper: Quat::from_rotation_y(k.yaw) * Quat::from_rotation_x(k.pitch),
            grip,
            body: None,
        }
    }
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Dodge roll as a real forward tumble: dive and tuck, one full forward rotation low to the
/// ground with the body squashed into a ball, then unfurl. `p` is roll progress 0..1.
fn roll_pose(p: f32) -> CombatPose {
    let tuck = smooth01(p / 0.14) * (1.0 - smooth01((p - 0.80) / 0.20));
    let spin_t = ((p - 0.04) / 0.82).clamp(0.0, 1.0);
    // Smootherstep: quick through the middle, settling at both ends.
    let spin = spin_t * spin_t * spin_t * (spin_t * (spin_t * 6.0 - 15.0) + 10.0);
    let angle = spin * std::f32::consts::TAU;
    let rot = Quat::from_rotation_x(-angle);
    let scale = Vec3::new(1.0 - 0.06 * tuck, 1.0 - 0.32 * tuck, 1.0 - 0.06 * tuck);
    // Rotate about the hips (scaled), and lower the ball's centre toward the floor.
    let pivot = Vec3::new(0.0, 0.80, 0.0) * scale;
    let centre = Vec3::new(0.0, 0.80 - 0.18 * tuck, 0.0);
    let body = Transform {
        translation: centre - rot * pivot,
        rotation: rot,
        scale,
    };
    let k = key(
        0.0,
        0.0,
        -0.35 * tuck,
        0.35 * tuck,
        1.35 * tuck,
        -0.35 * tuck,
        1.35 * tuck,
        Vec3::new(0.0, 0.0, -1.0).lerp(Vec3::new(0.15, 0.55, 0.80), tuck),
        1.75 * tuck,
        1.55 * tuck,
        -0.10 * tuck,
    );
    let mut pose = CombatPose::from_key(&k);
    pose.torso = Quat::from_rotation_x(-0.45 * tuck);
    pose.head = Quat::from_rotation_x(-0.65 * tuck);
    pose.body = Some(body);
    pose
}

/// Landing out of the roll: a crouch that rises back to standing over `ROLL_RECOVER_SECS`.
const ROLL_RECOVER_SECS: f32 = 0.2;
fn roll_recover_pose(since_end: f32) -> CombatPose {
    let w = 1.0 - smooth01(since_end / ROLL_RECOVER_SECS);
    let k = key(
        0.0,
        0.0,
        -0.30 * w,
        -0.30 * w,
        0.70 * w,
        0.45 * w,
        0.75 * w,
        Vec3::new(0.15, 0.40, -0.90),
        0.70 * w,
        -0.35 * w,
        -0.14 * w,
    );
    CombatPose::from_key(&k)
}

/// Per-hero animation memory for `animate_player_limbs`.
///
/// This is a resource, not a `Local`: the system is registered once per game state, and a
/// `Local` would give every registration its own copy. A state change in the middle of a swing
/// (for example the killing blow on Mom) then recorded the twisted arm positions as the rest
/// pose, leaving the arms attached to Stan's front and back from that point on.
#[derive(Resource, Default)]
pub struct LimbAnimState {
    /// Smoothed upper-body rotation (applied to torso, head and arms about the hips).
    upper: Option<Quat>,
    prev_roll: f32,
    roll_total: f32,
    /// Seconds since the last roll ended (None before the first roll).
    since_roll_end: Option<f32>,
    /// Authored arm / head translations, captured the first time each part is seen.
    base: bevy::utils::HashMap<Entity, Vec3>,
    body_root: Option<Entity>,
    body_dirty: bool,
}

pub fn animate_player_limbs(
    time: Res<Time>,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    player_q: Query<(&Player, &Children)>,
    children_q: Query<&Children>,
    mut part_q: Query<(&Name, &mut Transform), Without<Player>>,
    mut unnamed_q: Query<&mut Transform, (Without<Player>, Without<Name>)>,
    saber_q: Query<Entity, With<crate::boss::HeroSaberBlade>>,
    mut anim: ResMut<LimbAnimState>,
) {
    let Ok((player, root_children)) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let horiz_speed = Vec2::new(player.velocity.x, player.velocity.z).length();

    // Roll bookkeeping: total length of the current roll (the city car bail-out uses a longer
    // one) and time since the last roll ended, for the landing crouch.
    if player.roll_timer > anim.prev_roll + 1e-4 {
        anim.roll_total = player.roll_timer.max(ROLL_SECS);
    }
    if player.roll_timer > 0.0 {
        anim.since_roll_end = Some(0.0);
    } else if let Some(s) = anim.since_roll_end.as_mut() {
        *s += dt;
    }
    anim.prev_roll = player.roll_timer;
    let since_roll_end = anim.since_roll_end.unwrap_or(f32::MAX);

    let scripted_pose = matches!(
        *state.get(),
        GameState::HeadphoneCutscene | GameState::MomVictoryCutscene | GameState::StageClear
    );
    let combat: Option<CombatPose> = if scripted_pose {
        None
    } else if player.roll_timer > 0.0 {
        let total = anim.roll_total.max(player.roll_timer).max(1e-3);
        Some(roll_pose(1.0 - player.roll_timer / total))
    } else if player.attack_timer > 0.0 {
        let k = if stats.heavy_attack {
            // Every heavy is released from the charged wind-up, so it starts at the raised key
            // instead of dipping back to the first one (hit timing is unchanged).
            let p = 1.0 - player.attack_timer / HEAVY_SWING_SECS;
            sample_keys(&HEAVY_KEYS, p.max(HEAVY_KEYS[1].at))
        } else {
            let k = sample_keys(&LIGHT_KEYS, 1.0 - player.attack_timer / LIGHT_SWING_SECS);
            if player.swing_side < 0.0 { k.mirrored() } else { k }
        };
        Some(CombatPose::from_key(&k))
    } else if let Some(c) = player.charge_level() {
        // Charging a heavy: rise into the two-handed overhead wind-up, coil back a little more as
        // the charge builds, and tremble once it is full.
        let rise = smooth01((c * 1.8).min(1.0));
        let mut k = sample_keys(&HEAVY_KEYS, HEAVY_KEYS[1].at * rise);
        let full = if c >= 1.0 { 1.0 } else { 0.0 };
        k.pitch += 0.06 * c + 0.015 * full * (t * 38.0).sin();
        k.yaw -= 0.08 * c;
        k.bob -= 0.03 * c;
        Some(CombatPose::from_key(&k))
    } else if player.block_active {
        let r = player.block_recoil;
        let mut k = GUARD_KEY;
        // Blocked hit: the guard is shoved back toward the chest and Stan rocks onto his heels.
        k.pitch += 0.18 * r;
        k.bob -= 0.05 * r;
        k.r_el -= 0.25 * r;
        k.blade = (k.blade + Vec3::new(0.0, 0.0, 0.45 * r)).normalize();
        Some(CombatPose::from_key(&k))
    } else if since_roll_end < ROLL_RECOVER_SECS && player.grounded {
        Some(roll_recover_pose(since_roll_end))
    } else {
        None
    };

    let (left_leg_rot, right_leg_rot, left_arm_rot, right_arm_rot, torso_rot, head_rot, bob_y) =
        if let Some(c) = &combat {
            (c.ll, c.rl, c.la, c.ra, c.torso, c.head, c.bob)
        } else if *state.get() == GameState::HeadphoneCutscene {
            let rage_shake = (t * 36.0).sin() * 0.09;
            if stats.cutscene_beat >= 4 {
                // Enraged Sacred Vow: swelling fury with shaking right fist raised high above head!
                let flex = stats.rage_pulse_flash * 0.25;
                (
                    Quat::from_rotation_x(0.08),
                    Quat::from_rotation_x(-0.08),
                    Quat::from_rotation_z(-0.42 - flex + rage_shake),
                    Quat::from_rotation_z(2.35 + rage_shake * 0.6) * Quat::from_rotation_x(0.55),
                    Quat::from_rotation_x(0.08 + flex * 0.2) * Quat::from_rotation_z(rage_shake * 0.3),
                    Quat::from_rotation_x(-0.14) * Quat::from_rotation_y(rage_shake * 0.6),
                    0.02 + stats.rage_pulse_flash * 0.05,
                )
            } else if stats.cutscene_beat == 3 {
                // Boiling fury / remorse over fallen devices
                (
                    Quat::from_rotation_x(-0.10),
                    Quat::from_rotation_x(0.10),
                    Quat::from_rotation_x(-1.85) * Quat::from_rotation_z(0.55 + rage_shake),
                    Quat::from_rotation_x(-1.85) * Quat::from_rotation_z(-0.55 - rage_shake),
                    Quat::from_rotation_x(-0.35) * Quat::from_rotation_z(rage_shake * 0.35),
                    Quat::from_rotation_x(-0.42) * Quat::from_rotation_y(rage_shake * 0.7),
                    -0.02 + stats.rage_pulse_flash * 0.04,
                )
            } else {
                // Shocked freeze staring down at crushed headphones
                (
                    Quat::from_rotation_x(0.14),
                    Quat::from_rotation_x(-0.26),
                    Quat::from_rotation_z(-0.48),
                    Quat::from_rotation_z(0.48),
                    Quat::from_rotation_x(-0.14),
                    Quat::from_rotation_x(-0.28),
                    0.0,
                )
            }
        } else if *state.get() == GameState::MomVictoryCutscene {
            let bow = if stats.cutscene_timer >= 2.2 && stats.cutscene_timer < 12.2 {
                1.0
            } else if stats.cutscene_timer < 2.2 {
                ((stats.cutscene_timer - 0.8) / 1.4).clamp(0.0, 1.0)
            } else {
                (1.0 - (stats.cutscene_timer - 12.2) / 1.8).clamp(0.0, 1.0)
            };
            (
                Quat::from_rotation_x(-0.18 * bow),
                Quat::from_rotation_x(-0.18 * bow),
                Quat::from_rotation_x(-0.35 * bow) * Quat::from_rotation_z(0.14 * bow),
                Quat::from_rotation_x(-0.35 * bow) * Quat::from_rotation_z(-0.14 * bow),
                Quat::from_rotation_x(-0.88 * bow),
                Quat::from_rotation_x(-0.38 * bow),
                -0.08 * bow,
            )
        } else if *state.get() == GameState::StageClear {
            let swing = (t * 10.0).sin();
            (
                Quat::from_rotation_x(swing * 0.55),
                Quat::from_rotation_x(-swing * 0.55),
                Quat::from_rotation_z(-2.0 + (t * 8.0).sin() * 0.35)
                    * Quat::from_rotation_x(0.35),
                Quat::from_rotation_z(2.0 - (t * 8.0).sin() * 0.35)
                    * Quat::from_rotation_x(0.35),
                Quat::from_rotation_z((t * 5.0).sin() * 0.12)
                    * Quat::from_rotation_x((t * 10.0).cos() * 0.08),
                Quat::from_rotation_y((t * 5.0).sin() * 0.25)
                    * Quat::from_rotation_x(-0.10 + swing * 0.08),
                swing.abs() * 0.05,
            )
        } else if player.dash_timer > 0.0 {
            (
                Quat::from_rotation_x(-0.65),
                Quat::from_rotation_x(-0.35),
                Quat::from_rotation_x(-0.85) * Quat::from_rotation_z(-0.25),
                Quat::from_rotation_x(-0.85) * Quat::from_rotation_z(0.25),
                Quat::from_rotation_x(-0.50),
                Quat::from_rotation_x(0.28),
                -0.06,
            )
        } else if !player.grounded {
            let air_t = (player.velocity.y / 9.5).clamp(-1.0, 1.0);
            (
                Quat::from_rotation_x(0.65),
                Quat::from_rotation_x(-0.45),
                Quat::from_rotation_z(-0.95 - air_t * 0.35) * Quat::from_rotation_x(0.45),
                Quat::from_rotation_z(0.95 + air_t * 0.35) * Quat::from_rotation_x(-0.25),
                Quat::from_rotation_x(-0.12 * air_t),
                Quat::from_rotation_x(0.10 * air_t),
                0.0,
            )
        } else if horiz_speed > 0.25 {
            let amp = (horiz_speed / 8.0).clamp(0.45, 1.15);
            let s = player.walk_cycle.sin();
            let bounce = (player.walk_cycle * 2.0).sin().abs();
            (
                Quat::from_rotation_x(s * 0.78 * amp),
                Quat::from_rotation_x(-s * 0.78 * amp),
                Quat::from_rotation_x(-s * 0.68 * amp) * Quat::from_rotation_z(-0.12),
                Quat::from_rotation_x(s * 0.68 * amp) * Quat::from_rotation_z(0.12),
                Quat::from_rotation_x(-0.12 * amp) * Quat::from_rotation_y(-s * 0.12 * amp),
                Quat::from_rotation_x(0.08 * amp) * Quat::from_rotation_y(s * 0.06 * amp),
                bounce * 0.06 * amp,
            )
        } else {
            let breath = (t * 2.6).sin();
            (
                Quat::IDENTITY,
                Quat::IDENTITY,
                Quat::from_rotation_z(-0.06 - breath * 0.03),
                Quat::from_rotation_z(0.06 + breath * 0.03),
                Quat::from_rotation_x(breath * 0.03),
                Quat::from_rotation_x(-breath * 0.03)
                    * Quat::from_rotation_y((t * 1.1).sin() * 0.08),
                breath * 0.015,
            )
        };

    // Emanating anger trembling & berserker flex layered during active enraged gameplay
    let (left_leg_rot, right_leg_rot, left_arm_rot, right_arm_rot, torso_rot, head_rot, bob_y) =
        if stats.enraged
            && !stats.in_city
            && matches!(*state.get(), GameState::Playing | GameState::BossFight)
            && player.roll_timer <= 0.0
            && player.attack_timer <= 0.0
            && combat.is_none()
        {
            let shake = (t * 36.0).sin() * 0.09;
            let flare = stats.rage_pulse_flash * 0.55;
            (
                left_leg_rot * Quat::from_rotation_x(shake * 0.4),
                right_leg_rot * Quat::from_rotation_x(-shake * 0.4),
                left_arm_rot * Quat::from_rotation_z(-0.48 - flare + shake) * Quat::from_rotation_x(-0.32),
                right_arm_rot * Quat::from_rotation_z(0.48 + flare - shake) * Quat::from_rotation_x(-0.32),
                torso_rot * Quat::from_rotation_x(-0.18 - flare * 0.15) * Quat::from_rotation_z(shake * 0.35),
                head_rot * Quat::from_rotation_y(shake * 0.7) * Quat::from_rotation_x(0.10),
                bob_y + (t * 28.0).sin().abs() * 0.035 + stats.rage_pulse_flash * 0.05,
            )
        } else {
            (
                left_leg_rot,
                right_leg_rot,
                left_arm_rot,
                right_arm_rot,
                torso_rot,
                head_rot,
                bob_y,
            )
        };

    // Combat moves are fast arcs: follow their keys more tightly than the ambient poses.
    let blend = if combat.is_some() { (dt * 30.0).min(1.0) } else { (dt * 16.0).min(1.0) };
    let upper_target = combat.as_ref().map(|c| c.upper).unwrap_or(Quat::IDENTITY);
    let upper = anim.upper.unwrap_or(Quat::IDENTITY).slerp(upper_target, blend);
    anim.upper = Some(upper);
    // Upper-body twist / lean pivots about the hips (the torso origin).
    let hip = Vec3::new(0.0, 0.72 + bob_y, 0.0);

    let mut body_root = None;
    for &top in root_children.iter() {
        let mut stack: Vec<Entity> = vec![top];
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children_q.get(entity) {
                stack.extend(kids.iter().copied());
            }
            if let Ok((name, mut tf)) = part_q.get_mut(entity) {
                let (rot, base_y) = match name.as_str() {
                    "HeroLeftLeg" => {
                        tf.rotation = tf.rotation.slerp(left_leg_rot, blend);
                        continue;
                    }
                    "HeroRightLeg" => {
                        tf.rotation = tf.rotation.slerp(right_leg_rot, blend);
                        continue;
                    }
                    "HeroLeftArm" => (left_arm_rot, 1.36 + bob_y * 0.8),
                    "HeroRightArm" => (right_arm_rot, 1.36 + bob_y * 0.8),
                    "HeroTorso" => {
                        body_root = Some(top);
                        tf.rotation = tf.rotation.slerp(upper * torso_rot, blend);
                        tf.translation.y = 0.72 + bob_y;
                        continue;
                    }
                    "HeroHead" => (head_rot, 1.52 + bob_y),
                    _ => continue,
                };
                let authored = *anim.base.entry(entity).or_insert(tf.translation);
                let rest = Vec3::new(authored.x, base_y, authored.z);
                tf.translation = hip + upper * (rest - hip);
                tf.rotation = tf.rotation.slerp(upper * rot, blend);
            }
        }
    }

    // Saber grip follows the swing / guard keys, and eases back to the authored grip otherwise.
    let grip_target = combat.as_ref().map(|c| c.grip).unwrap_or_else(saber_rest_grip);
    for saber in &saber_q {
        if let Ok(mut s_tf) = unnamed_q.get_mut(saber) {
            s_tf.rotation = s_tf.rotation.slerp(grip_target, blend);
        }
    }

    // Whole-body tumble for the dodge roll on the glTF scene root. The player entity itself never
    // rotates or moves for it, so the follow camera (which tracks the player transform) stays level.
    if let Some(root) = body_root {
        anim.body_root = Some(root);
    }
    let body_tf = combat.as_ref().and_then(|c| c.body);
    if let Some(root) = anim.body_root {
        if body_tf.is_some() || anim.body_dirty {
            let target = body_tf.unwrap_or(Transform::IDENTITY);
            if let Ok(mut r_tf) = unnamed_q.get_mut(root) {
                *r_tf = target;
            } else if let Ok((_, mut r_tf)) = part_q.get_mut(root) {
                *r_tf = target;
            }
            anim.body_dirty = body_tf.is_some();
        }
    }
}

/// Short-lived saber effects: swoosh trail segments along the blade arc, block sparks and the
/// perfect-block flash.
#[derive(Component)]
pub struct SaberFx {
    age: f32,
    life: f32,
    vel: Vec3,
    kind: SaberFxKind,
    size: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaberFxKind {
    Trail,
    Spark,
    Flash,
}

/// Swing progress window (0..1) in which the blade leaves a swoosh trail.
fn swing_trail_window(heavy: bool) -> (f32, f32) {
    if heavy { (0.16, 0.72) } else { (0.08, 0.62) }
}

/// Spawns and ages saber effects. Runs in every state so effects alive at a scene change still
/// fade out and despawn.
pub fn update_saber_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stats: Res<PlayerStats>,
    mut player_q: Query<&mut Player>,
    blade_q: Query<&GlobalTransform, With<crate::boss::HeroSaberBlade>>,
    mut fx_q: Query<(Entity, &mut SaberFx, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut prev_blade: Local<Option<(Vec3, Vec3)>>,
) {
    use bevy::pbr::NotShadowCaster;
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;

    let dt = time.delta_secs().min(0.05);
    for (e, mut fx, mut tf, mat) in &mut fx_q {
        fx.age += dt;
        if fx.age >= fx.life {
            commands.entity(e).despawn_recursive();
            continue;
        }
        let k = 1.0 - fx.age / fx.life;
        match fx.kind {
            SaberFxKind::Trail => {
                if let Some(m) = materials.get_mut(&mat.0) {
                    m.base_color.set_alpha(0.85 * k * k);
                }
            }
            SaberFxKind::Spark => {
                let v = fx.vel;
                tf.translation += v * dt;
                fx.vel = v * (1.0 - 5.0 * dt).max(0.0) - Vec3::Y * (14.0 * fx.size * dt);
                tf.scale = Vec3::splat(fx.size * (0.35 + 0.65 * k));
            }
            SaberFxKind::Flash => {
                tf.scale = Vec3::splat(fx.size * (0.35 + 1.4 * (1.0 - k)));
                if let Some(m) = materials.get_mut(&mat.0) {
                    m.base_color.set_alpha(0.7 * k * k);
                }
            }
        }
    }

    let Ok(mut player) = player_q.get_single_mut() else {
        *prev_blade = None;
        return;
    };
    let Ok(blade_gt) = blade_q.get_single() else {
        *prev_blade = None;
        player.block_spark = 0;
        return;
    };
    let base = blade_gt.transform_point(Vec3::new(0.0, 0.40, 0.0));
    let tip = blade_gt.transform_point(Vec3::new(0.0, 1.66, 0.0));
    let world_scale = ((tip - base).length() / 1.26).max(0.05);

    // Swoosh: a ribbon quad between last frame's blade and this frame's, bright at the tip edge
    // and fading toward the hilt, during the striking part of the swing.
    let striking = stats.boss_mode && player.attack_timer > 0.0 && {
        let total = if stats.heavy_attack { HEAVY_SWING_SECS } else { LIGHT_SWING_SECS };
        let p = 1.0 - player.attack_timer / total;
        let (a, b) = swing_trail_window(stats.heavy_attack);
        p >= a && p <= b
    };
    if let (true, Some((p_base, p_tip))) = (striking, *prev_blade) {
        if p_tip.distance(tip) > 0.03 * world_scale {
            let n = (p_tip - p_base).cross(tip - p_base).normalize_or(Vec3::Y);
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![p_base.to_array(), p_tip.to_array(), tip.to_array(), base.to_array()])
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![n.to_array(); 4])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]])
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_COLOR,
                    vec![[1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.65], [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.0]],
                )
                .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
            let mat = materials.add(StandardMaterial {
                // Unlit HDR colour (a little over 1) so the ribbon blooms softly.
                base_color: Color::linear_rgba(1.25, 0.70, 2.2, 0.85),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                crate::state::LevelEntity,
                SaberFx { age: 0.0, life: 0.16, vel: Vec3::ZERO, kind: SaberFxKind::Trail, size: 1.0 },
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                Transform::IDENTITY,
                NotShadowCaster,
            ));
        }
    }
    *prev_blade = Some((base, tip));

    // Saber hit spark at the contact point on the target: a hot burst plus a quick flash.
    if let Some(contact) = player.hit_spark.take() {
        let heavy = stats.heavy_attack;
        let t = time.elapsed_secs();
        let spark_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
        let spark_mat = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(2.4, 1.3, 3.0),
            unlit: true,
            ..default()
        });
        let count = if heavy { 18 } else { 12 };
        for i in 0..count {
            let a = i as f32 * 2.399 + t * 5.0;
            let up = 0.2 + 0.7 * ((i as f32 * 1.3 + t * 2.0).sin() * 0.5 + 0.5);
            let dir = Vec3::new(a.cos(), up, a.sin()).normalize();
            let speed = (if heavy { 10.0 } else { 7.5 }) * world_scale * (0.6 + 0.4 * ((i * 7 % 5) as f32 / 4.0));
            commands.spawn((
                crate::state::LevelEntity,
                SaberFx { age: 0.0, life: 0.30, vel: dir * speed, kind: SaberFxKind::Spark, size: 0.05 * world_scale },
                Mesh3d(spark_mesh.clone()),
                MeshMaterial3d(spark_mat.clone()),
                Transform::from_translation(contact).with_scale(Vec3::splat(0.05 * world_scale)),
                NotShadowCaster,
            ));
        }
        commands.spawn((
            crate::state::LevelEntity,
            SaberFx {
                age: 0.0,
                life: 0.18,
                vel: Vec3::ZERO,
                kind: SaberFxKind::Flash,
                size: (if heavy { 0.6 } else { 0.42 }) * world_scale,
            },
            Mesh3d(meshes.add(Sphere::new(1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::linear_rgba(2.0, 1.2, 2.6, 0.7),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(contact).with_scale(Vec3::splat(0.2 * world_scale)),
            NotShadowCaster,
        ));
    }

    // Block sparks at the blade, where the hit met the guard.
    if player.block_spark > 0 {
        let perfect = player.block_spark >= 2;
        player.block_spark = 0;
        let contact = blade_gt.transform_point(Vec3::new(0.0, 1.05, 0.0));
        let t = time.elapsed_secs();
        let spark_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
        let spark_mat = materials.add(StandardMaterial {
            base_color: if perfect {
                Color::linear_rgb(1.6, 2.2, 3.0)
            } else {
                Color::linear_rgb(2.6, 1.9, 0.9)
            },
            unlit: true,
            ..default()
        });
        let count = if perfect { 16 } else { 9 };
        for i in 0..count {
            let a = i as f32 * 2.399 + t * 7.0;
            let up = 0.35 + 0.6 * ((i as f32 * 1.7 + t * 3.0).sin() * 0.5 + 0.5);
            let dir = Vec3::new(a.cos(), up, a.sin()).normalize();
            let speed = (if perfect { 9.0 } else { 6.5 }) * world_scale * (0.6 + 0.4 * ((i * 7 % 5) as f32 / 4.0));
            commands.spawn((
                crate::state::LevelEntity,
                SaberFx {
                    age: 0.0,
                    life: if perfect { 0.38 } else { 0.28 },
                    vel: dir * speed,
                    kind: SaberFxKind::Spark,
                    size: 0.045 * world_scale,
                },
                Mesh3d(spark_mesh.clone()),
                MeshMaterial3d(spark_mat.clone()),
                Transform::from_translation(contact).with_scale(Vec3::splat(0.045 * world_scale)),
                NotShadowCaster,
            ));
        }
        if perfect {
            commands.spawn((
                crate::state::LevelEntity,
                SaberFx { age: 0.0, life: 0.22, vel: Vec3::ZERO, kind: SaberFxKind::Flash, size: 0.5 * world_scale },
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::linear_rgba(1.3, 1.6, 2.4, 0.7),
                    alpha_mode: AlphaMode::Add,
                    unlit: true,
                    ..default()
                })),
                Transform::from_translation(contact).with_scale(Vec3::splat(0.2 * world_scale)),
                NotShadowCaster,
            ));
        }
    }
}

/// Heavy charge glow: the hero saber's sheath thickens and brightens and its light swells as the
/// charge builds, pulsing faster once full. Base colour (not emissive) carries it, since unlit
/// materials draw their base colour and `gfx` caps emissive anyway; peaks stay under 2.
pub fn update_saber_charge_glow(
    time: Res<Time>,
    player_q: Query<&Player>,
    mut aura_q: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<crate::boss::HeroSaberAura>>,
    mut light_q: Query<&mut PointLight, With<crate::boss::HeroSaberLight>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut applied: Local<f32>,
) {
    let c = player_q.get_single().ok().and_then(|p| p.charge_level());
    let g = match c {
        Some(c) => {
            let t = time.elapsed_secs();
            let rate = if c >= 1.0 { 16.0 } else { 7.0 };
            c * (0.8 + 0.2 * (t * rate).sin())
        }
        None => 0.0,
    };
    if g == 0.0 && *applied == 0.0 {
        return;
    }
    *applied = g;
    let base = crate::boss::HERO_SABER_AURA_COLOR.to_linear();
    for (mut tf, mat) in &mut aura_q {
        let r = 1.0 + 0.5 * g;
        tf.scale = Vec3::new(r, 1.0, r);
        if let Some(m) = materials.get_mut(&mat.0) {
            let k = 1.0 + 0.9 * g;
            m.base_color = Color::linear_rgba(
                base.red * k,
                base.green * k,
                base.blue * k,
                (base.alpha + 0.3 * g).min(0.8),
            );
        }
    }
    for mut light in &mut light_q {
        light.intensity = crate::boss::HERO_SABER_LIGHT * (1.0 + 1.5 * g);
    }
}

/// Saber hit-stop: slows virtual time to a crawl for a few frames (timed in real seconds) when a
/// swing lands, then restores normal speed. Pausing (help) is separate and unaffected.
pub fn update_hit_stop(
    real: Res<Time<Real>>,
    mut virt: ResMut<Time<Virtual>>,
    mut player_q: Query<&mut Player>,
    mut left: Local<f32>,
) {
    if let Ok(mut player) = player_q.get_single_mut() {
        if player.hit_stop > 0.0 {
            *left = left.max(player.hit_stop);
            player.hit_stop = 0.0;
        }
    }
    if *left > 0.0 {
        *left -= real.delta_secs();
        let speed = if *left > 0.0 { 0.06 } else { 1.0 };
        if virt.relative_speed() != speed {
            virt.set_relative_speed(speed);
        }
    }
}

pub fn victory_dance(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<&mut Transform, With<Player>>,
    mut door_q: Query<(&Name, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.fridge_open = (stats.fridge_open + dt * 1.4).min(1.0);
    for (name, mut d_tf) in &mut door_q {
        if name.as_str() == "FridgeDoor" {
            d_tf.rotation = Quat::from_rotation_y(stats.fridge_open * 2.05);
        }
    }
    let Ok(mut tf) = player_q.get_single_mut() else {
        return;
    };
    let t = time.elapsed_secs();
    if stats.boss_mode {
        tf.scale = Vec3::splat(3.4);
        tf.translation.x = 24.0;
        tf.translation.z = -34.5;
        tf.rotation = Quat::from_rotation_y(std::f32::consts::PI + (t * 6.0).sin() * 0.35);
        tf.translation.y = -6.0 + (t * 8.0).sin().abs() * 0.9;
    } else {
        tf.scale = Vec3::splat(6.0);
        tf.translation.x = 24.0;
        tf.translation.z = -34.5;
        tf.rotation = Quat::from_rotation_y(std::f32::consts::PI + (t * 6.0).sin() * 0.35);
        tf.translation.y = -6.0 + (t * 8.0).sin().abs() * 1.1;
    }
}
