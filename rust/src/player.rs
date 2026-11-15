use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::state::{
    CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats, PowerupKind, PowerupPickup,
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
        Transform::from_xyz(0.0, 0.0, 17.5).with_scale(Vec3::splat(0.55)),
    ));
}

pub fn camera_mouse_look(
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

    if mouse_motion.delta != Vec2::ZERO {
        orbit.yaw -= mouse_motion.delta.x * 0.0045;
        orbit.pitch = (orbit.pitch + mouse_motion.delta.y * 0.0035).clamp(0.06, 1.15);
    }

    let t_dx = crate::audio::take_touch_look_dx();
    let t_dy = crate::audio::take_touch_look_dy();
    if t_dx != 0.0 || t_dy != 0.0 {
        orbit.yaw -= t_dx * 0.0065;
        orbit.pitch = (orbit.pitch + t_dy * 0.0050).clamp(0.06, 1.15);
    }

    if mouse_scroll.delta.y != 0.0 {
        orbit.distance = (orbit.distance - mouse_scroll.delta.y * 0.5).clamp(3.5, 18.0);
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
    orbit: Res<CameraOrbit>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    platform_q: Query<(&Transform, &PlatformBox), Without<Player>>,
) {
    let Ok((mut tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);

    // Smoothly grow the hero toward their Katamari target scale
    let target_s = stats.target_scale();
    if (stats.player_scale - target_s).abs() > 0.001 {
        stats.player_scale += (target_s - stats.player_scale) * (dt * 6.0).min(1.0);
    } else {
        stats.player_scale = target_s;
    }
    tf.scale = Vec3::splat(stats.player_scale);
    let size_ratio = (stats.player_scale / 0.55).max(1.0);

    if player.dash_timer > 0.0 {
        player.dash_timer = (player.dash_timer - dt).max(0.0);
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
    stats.dash_energy = (stats.dash_energy + 24.0 * dt).min(100.0);

    // Drain screen-tap state so overlay transitions don't pick up stale taps
    let _ = crate::audio::take_touch_action_tap();

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
    let dir = raw_move.normalize_or_zero();

    // Dash (Shift or Touch DASH)
    let shift_pressed = keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || crate::audio::take_touch_dash();
    if shift_pressed && player.dash_timer <= 0.0 && stats.dash_energy >= 35.0 {
        stats.dash_energy -= 35.0;
        player.dash_timer = 0.22;
        player.invuln_timer = 0.35;
        crate::audio::play_sfx("dash");
    }

    // Jump & Double-Jump (Space or Touch JUMP)
    if (keys.just_pressed(KeyCode::Space) || crate::audio::take_touch_jump())
        && (player.grounded || player.jumps_used < 2)
    {
        if player.jumps_used == 0 {
            crate::audio::play_sfx("jump");
        } else {
            crate::audio::play_sfx("double_jump");
        }
        player.velocity.y = 9.2 + (stats.player_scale - 0.55).max(0.0) * 1.55;
        player.grounded = false;
        player.jumps_used += 1;
    }

    // Horizontal speed scales with player growth
    let base_speed = if player.dash_timer > 0.0 { 15.8 } else { 7.4 };
    let mut speed = base_speed * size_ratio.powf(0.38);
    if stats.speed_boost_timer > 0.0 {
        speed *= 1.75;
    }
    if player.slow_timer > 0.0 {
        speed *= 0.6;
    }

    player.velocity.x = dir.x * speed * move_mag;
    player.velocity.z = dir.z * speed * move_mag;

    let horiz_speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    if horiz_speed > 0.25 {
        player.walk_cycle += horiz_speed * dt * (2.2 / size_ratio.powf(0.28));
    }

    if dir.length_squared() > 0.01 {
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
    let prev_y = tf.translation.y;
    tf.translation += player.velocity * dt;

    // Platform AABB landing (top surfaces)
    player.grounded = false;
    let land_r = (0.38 * stats.player_scale).clamp(0.38, 0.85);
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

    // Solid side-wall pushout against tall platforms (cabinets, island, fridge)
    let wall_r = (0.24 * stats.player_scale).clamp(0.26, 0.72);
    let player_top_y = tf.translation.y + (0.35 * stats.player_scale).max(0.5);
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
            && tf.translation.y < top_y - 0.35
            && player_top_y > bottom_y + 0.15
        {
            let dx_left = tf.translation.x - min_x;
            let dx_right = max_x - tf.translation.x;
            let dz_back = tf.translation.z - min_z;
            let dz_front = max_z - tf.translation.z;
            let m = dx_left.min(dx_right).min(dz_back).min(dz_front);
            if (m - dx_left).abs() < 0.001 {
                tf.translation.x = min_x;
            } else if (m - dx_right).abs() < 0.001 {
                tf.translation.x = max_x;
            } else if (m - dz_back).abs() < 0.001 {
                tf.translation.z = min_z;
            } else {
                tf.translation.z = max_z;
            }
        }
    }

    if player.grounded && horiz_speed > 0.25 {
        let half = (player.walk_cycle / std::f32::consts::PI).floor() as i32;
        if half != player.last_step_half {
            player.last_step_half = half;
            crate::audio::play_sfx("step");
        }
    }

    // Keep within giant kitchen room walls or respawn on abyssal fall
    tf.translation.x = tf.translation.x.clamp(-20.5, 22.5);
    tf.translation.z = tf.translation.z.clamp(-24.0, 24.0);
    if tf.translation.y < -12.0 {
        tf.translation = Vec3::new(0.0, 0.5, 17.5);
        player.velocity = Vec3::ZERO;
    }
}

pub fn update_camera(
    time: Res<Time>,
    orbit: Res<CameraOrbit>,
    stats: Res<PlayerStats>,
    player_q: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(p_tf), Ok(mut c_tf)) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    let cam_dist = orbit.distance * (0.72 + stats.player_scale * 0.35);
    let anchor_y = 0.45 + stats.player_scale * 0.95;
    let anchor = p_tf.translation + Vec3::new(0.0, anchor_y, 0.0);
    let offset = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    ) * cam_dist;
    let target_pos = anchor + offset;
    let dt = (time.delta_secs() * 12.0).min(1.0);
    c_tf.translation = c_tf.translation.lerp(target_pos, dt);
    let look_fwd = 1.2 * (0.5 + stats.player_scale * 0.25);
    let look_target = p_tf.translation + Vec3::new(0.0, anchor_y * 0.92, 0.0)
        - Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos()) * look_fwd;
    c_tf.look_at(look_target, Vec3::Y);
}

pub fn animate_and_collect_pickups(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    player_q: Query<&Transform, With<Player>>,
    mut tendy_q: Query<(Entity, &mut Transform, &TendyPickup), Without<Player>>,
    mut power_q: Query<(Entity, &mut Transform, &PowerupPickup), (Without<Player>, Without<TendyPickup>)>,
    fridge_q: Query<
        &Transform,
        (
            With<crate::state::FridgeGoal>,
            Without<Player>,
            Without<TendyPickup>,
            Without<PowerupPickup>,
        ),
    >,
    mut door_q: Query<
        (&Name, &mut Transform),
        (
            Without<Player>,
            Without<TendyPickup>,
            Without<PowerupPickup>,
            Without<crate::state::FridgeGoal>,
        ),
    >,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let player_pos = player_q.get_single().map(|p| p.translation).ok();
    let pickup_r = 1.15 + stats.player_scale * 0.45;
    let player_mid_y = (0.55 * stats.player_scale).max(0.55);

    for (entity, mut tf, tendy) in &mut tendy_q {
        tf.rotate_y(2.4 * dt);
        tf.translation.y = tendy.base_y + ((t * 3.2 + tendy.phase).sin() * 0.16);

        if let Some(p_pos) = player_pos {
            let d = (p_pos + Vec3::Y * player_mid_y).distance(tf.translation);
            if d < pickup_r {
                commands.entity(entity).despawn_recursive();
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
                commands.entity(entity).despawn_recursive();
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
            if stats.tendies >= stats.total_tendies {
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
            next_state.set(GameState::StageClear);
        }
    }
}

pub fn animate_player_limbs(
    time: Res<Time>,
    state: Res<State<GameState>>,
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
        if *state.get() == GameState::StageClear {
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
    tf.scale = Vec3::splat(6.0);
    tf.translation.x = 13.0;
    tf.translation.z = -12.6;
    tf.rotation = Quat::from_rotation_y(std::f32::consts::PI + (t * 6.0).sin() * 0.35);
    tf.translation.y = -6.0 + (t * 8.0).sin().abs() * 1.1;
}
