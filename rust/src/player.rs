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
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut orbit: ResMut<CameraOrbit>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
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
) {
    let Ok((mut tf, mut player, mut vis)) = player_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);

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
    if stats.enraged && !stats.in_city {
        let throb = 1.0 + stats.rage_pulse_flash * 0.16;
        tf.scale = Vec3::new(
            stats.player_scale * throb * squash_xz,
            stats.player_scale * (1.0 + stats.rage_pulse_flash * 0.22) * squash_y,
            stats.player_scale * throb * squash_xz,
        );
    } else {
        tf.scale = Vec3::new(
            stats.player_scale * squash_xz,
            stats.player_scale * squash_y,
            stats.player_scale * squash_xz,
        );
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
    if stats.stamina_regen_delay > 0.0 {
        stats.stamina_regen_delay = (stats.stamina_regen_delay - dt).max(0.0);
    } else {
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
    let in_vehicle = stats.driving_car.is_some() || stats.riding_scooter.is_some();
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

    if stats.driving_car.is_some() || stats.riding_scooter.is_some() || stats.phone_open {
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.jump_buffer = 0.0;
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
    let shift_pressed = keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || crate::audio::take_touch_dash()
        || (stats.boss_mode && mouse_buttons.just_pressed(MouseButton::Right));
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let touch_atk = crate::audio::take_touch_attack();
    if stats.boss_mode {
        if shift_pressed && player.roll_timer <= 0.0 && stats.dash_energy >= 22.0 {
            stats.dash_energy -= 22.0;
            stats.stamina_regen_delay = 0.50;
            player.roll_timer = 0.45;
            player.invuln_timer = 0.58;
            player.dodge_counter_timer = 2.8;
            crate::audio::play_sfx("dodge_roll");
        }
        let light_btn = mouse_buttons.just_pressed(MouseButton::Left)
            || keys.just_pressed(KeyCode::KeyJ)
            || keys.just_pressed(KeyCode::KeyX)
            || (!stats.in_city && (keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::KeyF)))
            || touch_atk;
        let heavy_btn = keys.just_pressed(KeyCode::KeyC);
        if heavy_btn && player.attack_cooldown <= 0.0 && stats.dash_energy >= 32.0 {
            stats.dash_energy -= 32.0;
            stats.stamina_regen_delay = 0.65;
            stats.heavy_attack = true;
            player.attack_timer = 0.40;
            player.attack_cooldown = 0.58;
            let lunge_fwd = Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos());
            tf.translation += lunge_fwd * 0.9;
            crate::audio::play_sfx("saber_swing");
        } else if light_btn && player.attack_cooldown <= 0.0 && stats.dash_energy >= 16.0 {
            stats.dash_energy -= 16.0;
            stats.stamina_regen_delay = 0.42;
            stats.heavy_attack = false;
            player.attack_timer = 0.28;
            player.attack_cooldown = 0.36;
            crate::audio::play_sfx("saber_swing");
        }
    } else if shift_pressed && player.dash_timer <= 0.0 && stats.dash_energy >= 30.0 {
        stats.dash_energy -= 30.0;
        stats.stamina_regen_delay = 0.45;
        player.dash_timer = 0.22;
        player.invuln_timer = 0.35;
        crate::audio::play_sfx("dash");
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
            + (stats.player_scale - 0.55).max(0.0) * if stats.in_city { 1.85 } else { 1.55 }
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
        player.walk_cycle += horiz_speed * dt * (2.2 / size_ratio.powf(0.28));
    }

    if dir.length_squared() > 0.01 {
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

    let land_r = if stats.in_city {
        (0.38 * stats.player_scale).clamp(0.55, 3.2)
    } else {
        (0.38 * stats.player_scale).clamp(0.38, 0.85)
    };
    let wall_r = if stats.in_city {
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
    if stats.in_city {
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
    let (Ok((p_tf, player)), Ok((mut c_tf, mut proj))) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let base_fov = std::f32::consts::FRAC_PI_4;

    // Lock-on yaw steering (disabled in cars and on the scooter, where the chase cam owns yaw).
    if stats.lock_on_active && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
        let mut best_pos: Option<Vec3> = None;
        let mut best_dist = 70.0_f32;
        for t in &lock_q {
            let d = p_tf.translation.distance(t.translation);
            if d > 0.5 && d < best_dist {
                best_dist = d;
                best_pos = Some(t.translation);
            }
        }
        if let Some(target_pos) = best_pos {
            let to_target = target_pos - p_tf.translation;
            let desired_yaw = (-to_target.x).atan2(-to_target.z) + std::f32::consts::PI;
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
    let follow_scale = stats.player_scale.max(0.55);
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
        0.55 + stats.player_scale * 0.92
    };
    let anchor = follow + Vec3::Y * anchor_y;
    // Widen camera distance slightly on narrow portrait viewports (mobile) so Stan stays framed.
    let aspect_mul = if let Ok(win) = window_q.get_single() {
        let aspect = win.width() / win.height().max(1.0);
        1.0 + (1.0 - aspect).clamp(0.0, 0.55) * 0.35
    } else {
        1.0
    };
    // Scale camera pullback with Stan's Katamari growth (linear in city up to 18x kaiju, sqrt in apartment).
    let cam_scale_mul = if stats.driving_car.is_some() {
        0.92
    } else if stats.in_city {
        0.72 + stats.player_scale * 0.24
    } else {
        0.82 + stats.player_scale.sqrt() * 0.34
    };
    let in_pub217 = stats.in_city && crate::city::is_in_pub217(p_pos, stats.player_scale);
    let pub_s = crate::city::pub217_scale(stats.player_scale);
    let mut desired_dist = orbit.distance * cam_scale_mul * aspect_mul;
    // Clamp distance & pitch inside Pub 217 so the bar ceiling/walls don't occlude the player.
    if in_pub217 {
        desired_dist = desired_dist.min(4.2 * pub_s);
    }
    let min_cam_dist = if stats.in_city {
        (2.2 + stats.player_scale * 1.8).min(desired_dist)
    } else {
        (2.2 + stats.player_scale * 0.45).min(desired_dist)
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
    if !(stats.in_city && stats.player_scale >= 8.0) {
        for (p_tf_box, p_box) in &platform_q {
            if !stats.in_city && p_tf_box.translation.z > 46.5 {
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

    // Clamp boom along room/city bounds (allowing open camera pull-back at south spawn)
    let (room_min, room_max) = if stats.in_city {
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
    if !(stats.in_city && stats.player_scale >= 8.0) {
        let mut pushed = cam_pos;
        let cam_margin = Vec3::splat(0.38);
        for _ in 0..2 {
            for (p_tf_box, p_box) in &platform_q {
                if !stats.in_city && p_tf_box.translation.z > 46.5 {
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
    let look_fwd = 1.0 * (0.45 + stats.player_scale.sqrt() * 0.22) * zoom_ratio;
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

    // Small FOV kick while dashing.
    let kick_target = if player.dash_timer > 0.0 { 0.06 } else { 0.0 };
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

    // Ease from the previous shot (e.g. a cutscene's last frame) into the follow pose.
    if orbit.blend_request {
        orbit.blend_request = false;
        orbit.blend_t = 0.0;
        orbit.blend_from_pos = c_tf.translation;
        orbit.blend_from_rot = c_tf.rotation;
        orbit.blend_from_fov = match *proj {
            Projection::Perspective(ref p) => p.fov,
            _ => base_fov,
        };
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
                if stats.fridge_open <= 0.0 {
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
            next_state.set(GameState::Cutscene);
        }
    }
}

pub fn animate_player_limbs(
    time: Res<Time>,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    player_q: Query<(&Player, &Children)>,
    children_q: Query<&Children>,
    mut part_q: Query<(&Name, &mut Transform), Without<Player>>,
) {
    let Ok((player, root_children)) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let horiz_speed = Vec2::new(player.velocity.x, player.velocity.z).length();

    let (left_leg_rot, right_leg_rot, left_arm_rot, right_arm_rot, torso_rot, head_rot, bob_y) =
        if *state.get() == GameState::HeadphoneCutscene {
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
        } else if player.roll_timer > 0.0 {
            let p = (1.0 - (player.roll_timer / 0.42)).clamp(0.0, 1.0);
            let spin = p * std::f32::consts::TAU;
            (
                Quat::from_rotation_x(-1.15),
                Quat::from_rotation_x(-1.15),
                Quat::from_rotation_x(-0.95) * Quat::from_rotation_z(-0.2),
                Quat::from_rotation_x(-0.95) * Quat::from_rotation_z(0.2),
                Quat::from_rotation_x(-spin),
                Quat::from_rotation_x(0.35),
                -0.18 * (p * std::f32::consts::PI).sin(),
            )
        } else if player.attack_timer > 0.0 {
            let sw = (1.0 - (player.attack_timer / 0.28)).clamp(0.0, 1.0);
            (
                Quat::from_rotation_x(0.38),
                Quat::from_rotation_x(-0.42),
                Quat::from_rotation_x(-0.45) * Quat::from_rotation_z(-0.35),
                Quat::from_rotation_x(-1.45 + sw * 0.65)
                    * Quat::from_rotation_y(-1.15 + sw * 2.45)
                    * Quat::from_rotation_z(0.55 - sw * 1.1),
                Quat::from_rotation_x(-0.20) * Quat::from_rotation_y(-0.48 + sw * 0.96),
                Quat::from_rotation_x(0.12) * Quat::from_rotation_y(0.20 - sw * 0.40),
                0.04,
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

    let blend = (dt * 16.0).min(1.0);
    let mut stack: Vec<Entity> = root_children.iter().copied().collect();
    while let Some(entity) = stack.pop() {
        if let Ok(kids) = children_q.get(entity) {
            stack.extend(kids.iter().copied());
        }
        if let Ok((name, mut tf)) = part_q.get_mut(entity) {
            match name.as_str() {
                "HeroLeftLeg" => tf.rotation = tf.rotation.slerp(left_leg_rot, blend),
                "HeroRightLeg" => tf.rotation = tf.rotation.slerp(right_leg_rot, blend),
                "HeroLeftArm" => {
                    tf.rotation = tf.rotation.slerp(left_arm_rot, blend);
                    tf.translation.y = 1.36 + bob_y * 0.8;
                }
                "HeroRightArm" => {
                    tf.rotation = tf.rotation.slerp(right_arm_rot, blend);
                    tf.translation.y = 1.36 + bob_y * 0.8;
                }
                "HeroTorso" => {
                    tf.rotation = tf.rotation.slerp(torso_rot, blend);
                    tf.translation.y = 0.72 + bob_y;
                }
                "HeroHead" => {
                    tf.rotation = tf.rotation.slerp(head_rot, blend);
                    tf.translation.y = 1.52 + bob_y;
                }
                _ => {}
            }
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
