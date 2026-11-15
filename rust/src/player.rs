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
    pub roll_timer: f32,
    pub dodge_counter_timer: f32,
    pub attack_timer: f32,
    pub attack_cooldown: f32,
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
        orbit.pitch = (orbit.pitch + mouse_motion.delta.y * 0.0035).clamp(0.15, 1.32);
    }

    let t_dx = crate::audio::take_touch_look_dx();
    let t_dy = crate::audio::take_touch_look_dy();
    if t_dx != 0.0 || t_dy != 0.0 {
        orbit.yaw -= t_dx * 0.0065;
        orbit.pitch = (orbit.pitch + t_dy * 0.0050).clamp(0.15, 1.32);
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
    orbit: Res<CameraOrbit>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    platform_q: Query<(&Transform, &PlatformBox), Without<Player>>,
) {
    let Ok((mut tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);

    // Smoothly grow the hero toward their Katamari target scale (with 0.5s enraged swelling pulses)
    if stats.enraged && !stats.in_city {
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
    let target_s = stats.target_scale();
    if (stats.player_scale - target_s).abs() > 0.001 {
        stats.player_scale += (target_s - stats.player_scale) * (dt * 6.0).min(1.0);
    } else {
        stats.player_scale = target_s;
    }
    if stats.enraged && !stats.in_city {
        let throb = 1.0 + stats.rage_pulse_flash * 0.16;
        tf.scale = Vec3::new(
            stats.player_scale * throb,
            stats.player_scale * (1.0 + stats.rage_pulse_flash * 0.22),
            stats.player_scale * throb,
        );
    } else {
        tf.scale = Vec3::splat(stats.player_scale);
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
    let dash_regen = if stats.enraged && !stats.in_city {
        60.0
    } else if stats.boss_mode {
        34.0
    } else {
        24.0
    };
    stats.dash_energy = (stats.dash_energy + dash_regen * dt).min(100.0);

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
    let mut dir = raw_move.normalize_or_zero();

    // Dash outside boss mode; Dodge & Roll + Lightsaber Slash in boss mode
    let shift_pressed = keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || crate::audio::take_touch_dash()
        || (stats.boss_mode && mouse_buttons.just_pressed(MouseButton::Right));
    let touch_atk = crate::audio::take_touch_attack();
    if stats.boss_mode {
        if shift_pressed && player.roll_timer <= 0.0 && stats.dash_energy >= 20.0 {
            stats.dash_energy -= 20.0;
            player.roll_timer = 0.45;
            player.invuln_timer = 0.60;
            player.dodge_counter_timer = 2.8;
            crate::audio::play_sfx("dodge_roll");
        }
        if (mouse_buttons.just_pressed(MouseButton::Left)
            || keys.just_pressed(KeyCode::KeyE)
            || keys.just_pressed(KeyCode::KeyF)
            || touch_atk)
            && player.attack_cooldown <= 0.0
        {
            player.attack_timer = 0.28;
            player.attack_cooldown = 0.34;
            crate::audio::play_sfx("saber_swing");
        }
    } else if shift_pressed && player.dash_timer <= 0.0 && stats.dash_energy >= 35.0 {
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
    if stats.speed_boost_timer > 0.0 {
        speed *= 1.75;
    }
    if stats.enraged && !stats.in_city {
        speed *= 1.38 + 0.10 * (stats.rage_level.min(4) as f32);
    }
    if player.slow_timer > 0.0 {
        speed *= 0.6;
    }

    player.velocity.x = dir.x * speed * effective_mag;
    player.velocity.z = dir.z * speed * effective_mag;

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
    let land_r = if stats.in_city {
        (0.38 * stats.player_scale).clamp(0.55, 3.2)
    } else {
        (0.38 * stats.player_scale).clamp(0.38, 0.85)
    };
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
    let wall_r = if stats.in_city {
        (0.28 * stats.player_scale).clamp(0.45, 2.8)
    } else {
        (0.28 * stats.player_scale).clamp(0.28, 0.72)
    };
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
                } else if (m - dx_right).abs() < 0.001 {
                    tf.translation.x = max_x;
                    player.velocity.x = player.velocity.x.max(0.0);
                } else if (m - dz_back).abs() < 0.001 {
                    tf.translation.z = min_z;
                    player.velocity.z = player.velocity.z.min(0.0);
                } else {
                    tf.translation.z = max_z;
                    player.velocity.z = player.velocity.z.max(0.0);
                }
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

    // Keep within level bounds or respawn on abyssal fall
    if stats.in_city {
        tf.translation.x = tf.translation.x.clamp(-126.0, 126.0);
        tf.translation.z = tf.translation.z.clamp(-126.0, 126.0);
        if tf.translation.y < -12.0 {
            tf.translation = Vec3::new(0.0, 1.0, 82.0);
            player.velocity = Vec3::ZERO;
        }
    } else {
        tf.translation.x = tf.translation.x.clamp(-54.0, 54.0);
        tf.translation.z = tf.translation.z.clamp(-46.0, 46.0);
        if tf.translation.y < -12.0 {
            tf.translation = Vec3::new(0.0, 0.5, 29.0);
            player.velocity = Vec3::ZERO;
        }
    }
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
    stats: Res<PlayerStats>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<&Transform, (With<Player>, Without<Camera3d>, Without<PlatformBox>)>,
    platform_q: Query<(&Transform, &PlatformBox), (Without<Player>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<PlatformBox>)>,
) {
    let (Ok(p_tf), Ok(mut c_tf)) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    let anchor_y = 0.55 + stats.player_scale * 0.92;
    let anchor = p_tf.translation + Vec3::Y * anchor_y;
    let aspect_mul = if let Ok(win) = window_q.get_single() {
        let aspect = win.width() / win.height().max(1.0);
        1.0 + (1.0 - aspect).clamp(0.0, 0.55) * 0.35
    } else {
        1.0
    };
    let cam_scale_mul = if stats.in_city {
        0.72 + stats.player_scale * 0.24
    } else {
        0.82 + stats.player_scale.sqrt() * 0.34
    };
    let desired_dist = orbit.distance * cam_scale_mul * aspect_mul;
    let min_cam_dist = if stats.in_city {
        (2.2 + stats.player_scale * 1.8).min(desired_dist)
    } else {
        (2.2 + stats.player_scale * 0.45).min(desired_dist)
    };
    let eff_pitch = (orbit.pitch + (orbit.distance - 10.6).max(0.0) * 0.008).clamp(0.15, 1.35);
    let dir = Vec3::new(
        orbit.yaw.sin() * eff_pitch.cos(),
        eff_pitch.sin(),
        orbit.yaw.cos() * eff_pitch.cos(),
    )
    .normalize();

    // Spring-arm ray-AABB sweep against tall structural wall/fridge occluders only
    // (small props & low ledges never yank the camera close to the character; skipped for NYC Skyscraper Titan)
    let mut safe_dist = desired_dist;
    let sweep_pad = Vec3::splat(0.22);
    let desired_cam_y = anchor.y + dir.y * desired_dist;
    if !(stats.in_city && stats.player_scale >= 8.0) {
        for (p_tf_box, p_box) in &platform_q {
            if !stats.in_city && p_tf_box.translation.z > 46.5 {
                continue;
            }
            let box_top = p_tf_box.translation.y + p_box.half_extents.y;
            let is_tall_wall = p_box.half_extents.y >= 2.1
                && (p_box.half_extents.x >= 2.5 || p_box.half_extents.z >= 2.5)
                && box_top > desired_cam_y + 0.2;
            if !is_tall_wall {
                continue;
            }
            let bmin = p_tf_box.translation - p_box.half_extents - sweep_pad;
            let bmax = p_tf_box.translation + p_box.half_extents + sweep_pad;
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

    // Asymmetric distance smoothing along spherical arm (prevents chord corner clipping)
    let dt = time.delta_secs().min(0.05);
    let rate = if safe_dist < orbit.current_dist {
        30.0
    } else {
        7.0
    };
    orbit.current_dist += (safe_dist - orbit.current_dist) * (dt * rate).min(1.0);
    orbit.current_dist = orbit.current_dist.clamp(min_cam_dist, desired_dist);

    let mut cam_pos = anchor + dir * orbit.current_dist;

    // 2-pass 3D camera sphere pushout around any solid geometry touched by the camera sphere
    let cam_margin = Vec3::splat(0.38);
    for _ in 0..2 {
        for (p_tf_box, p_box) in &platform_q {
            if !stats.in_city && p_tf_box.translation.z > 46.5 {
                continue;
            }
            let top_y = p_tf_box.translation.y + p_box.half_extents.y;
            let bmin = p_tf_box.translation - p_box.half_extents - cam_margin;
            let bmax = p_tf_box.translation + p_box.half_extents + cam_margin;
            if cam_pos.x > bmin.x
                && cam_pos.x < bmax.x
                && cam_pos.y > bmin.y
                && cam_pos.y < bmax.y
                && cam_pos.z > bmin.z
                && cam_pos.z < bmax.z
            {
                let dx_l = cam_pos.x - bmin.x;
                let dx_r = bmax.x - cam_pos.x;
                let dy_b = cam_pos.y - bmin.y;
                let dy_t = bmax.y - cam_pos.y;
                let dz_b = cam_pos.z - bmin.z;
                let dz_f = bmax.z - cam_pos.z;
                let mut m = dx_l.min(dx_r).min(dy_t).min(dz_b).min(dz_f);
                if anchor.y < top_y {
                    m = m.min(dy_b);
                }
                if anchor.y >= top_y && dy_t <= m + 0.45 {
                    cam_pos.y = bmax.y;
                } else if (m - dy_t).abs() < 0.001 {
                    cam_pos.y = bmax.y;
                } else if (m - dy_b).abs() < 0.001 && anchor.y < top_y {
                    cam_pos.y = bmin.y;
                } else if (m - dx_l).abs() < 0.001 {
                    cam_pos.x = bmin.x;
                } else if (m - dx_r).abs() < 0.001 {
                    cam_pos.x = bmax.x;
                } else if (m - dz_b).abs() < 0.001 {
                    cam_pos.z = bmin.z;
                } else {
                    cam_pos.z = bmax.z;
                }
            }
        }
    }

    cam_pos = cam_pos.clamp(room_min, room_max);

    let zoom_ratio = (orbit.current_dist / desired_dist.max(0.01)).clamp(0.15, 1.0);
    let look_fwd = 1.0 * (0.45 + stats.player_scale.sqrt() * 0.22) * zoom_ratio;
    let look_target = p_tf.translation
        + Vec3::new(0.0, anchor_y * 0.88, 0.0)
        - Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos()) * look_fwd;
    c_tf.translation = cam_pos;
    c_tf.look_at(look_target, Vec3::Y);
}

pub fn animate_and_collect_pickups(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
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
    if keys.just_pressed(KeyCode::KeyK) {
        next_state.set(GameState::Cutscene);
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        next_state.set(GameState::BossFight);
        return;
    }
    if keys.just_pressed(KeyCode::KeyP) {
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
