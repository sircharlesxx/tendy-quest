use bevy::prelude::*;
use crate::state::{
    GameState, LevelEntity, PlatformBox, PlayerStats, PowerupKind, PowerupPickup, TendyPickup,
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
        }
    }
}

pub fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_player_inner(&mut commands, &asset_server);
}

pub fn spawn_player_inner(commands: &mut Commands, asset_server: &AssetServer) {
    let hero_scene: Handle<Scene> = asset_server.load("assets/models/plaid_hero.glb#Scene0");
    commands.spawn((
        LevelEntity,
        Player::default(),
        SceneRoot(hero_scene),
        Transform::from_xyz(0.0, 0.0, 17.5),
    ));
}

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    platform_q: Query<(&Transform, &PlatformBox), Without<Player>>,
) {
    let Ok((mut tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);

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

    // Input direction
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        dir.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        dir.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    if dir.length_squared() > 0.0 {
        dir = dir.normalize();
    }

    // Dash (Shift)
    let shift_pressed =
        keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight);
    if shift_pressed && player.dash_timer <= 0.0 && stats.dash_energy >= 35.0 {
        stats.dash_energy -= 35.0;
        player.dash_timer = 0.22;
        player.invuln_timer = 0.35;
    }

    // Jump & Double-Jump (Space)
    if keys.just_pressed(KeyCode::Space) && (player.grounded || player.jumps_used < 2) {
        player.velocity.y = 9.2;
        player.grounded = false;
        player.jumps_used += 1;
    }

    // Horizontal speed
    let mut speed = if player.dash_timer > 0.0 { 16.5 } else { 7.8 };
    if stats.speed_boost_timer > 0.0 {
        speed *= 1.75;
    }
    if player.slow_timer > 0.0 {
        speed *= 0.6;
    }

    player.velocity.x = dir.x * speed;
    player.velocity.z = dir.z * speed;

    if dir.length_squared() > 0.01 {
        // Blender +Y -> glTF -Z forward, so yaw = atan2(-dir.x, -dir.z)
        let target_yaw = (-dir.x).atan2(-dir.z);
        let diff = (target_yaw - player.facing_yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        player.facing_yaw += diff * (12.0 * dt).min(1.0);
        tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    }

    // Gravity & integrate
    player.velocity.y -= 22.0 * dt;
    let prev_y = tf.translation.y;
    tf.translation += player.velocity * dt;

    // Platform AABB landing
    player.grounded = false;
    let player_r = 0.45;
    for (p_tf, p_box) in &platform_q {
        let min_x = p_tf.translation.x - p_box.half_extents.x - player_r;
        let max_x = p_tf.translation.x + p_box.half_extents.x + player_r;
        let min_z = p_tf.translation.z - p_box.half_extents.z - player_r;
        let max_z = p_tf.translation.z + p_box.half_extents.z + player_r;
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

    // Keep within countertop bounds or respawn on fall
    tf.translation.x = tf.translation.x.clamp(-6.8, 6.8);
    tf.translation.z = tf.translation.z.clamp(-21.2, 21.2);
    if tf.translation.y < -6.0 {
        tf.translation = Vec3::new(0.0, 0.5, 17.5);
        player.velocity = Vec3::ZERO;
    }
}

pub fn update_camera(
    time: Res<Time>,
    player_q: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(p_tf), Ok(mut c_tf)) = (player_q.get_single(), cam_q.get_single_mut()) else {
        return;
    };
    let target_pos = p_tf.translation + Vec3::new(0.0, 4.4, 7.2);
    let dt = (time.delta_secs() * 7.0).min(1.0);
    c_tf.translation = c_tf.translation.lerp(target_pos, dt);
    let look_at = p_tf.translation + Vec3::new(0.0, 1.3, -2.5);
    c_tf.look_at(look_at, Vec3::Y);
}

pub fn animate_and_collect_pickups(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    player_q: Query<&Transform, With<Player>>,
    mut tendy_q: Query<(Entity, &mut Transform, &TendyPickup), Without<Player>>,
    mut power_q: Query<(Entity, &mut Transform, &PowerupPickup), (Without<Player>, Without<TendyPickup>)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let player_pos = player_q.get_single().map(|p| p.translation).ok();

    for (entity, mut tf, tendy) in &mut tendy_q {
        tf.rotate_y(2.4 * dt);
        tf.translation.y = tendy.base_y + ((t * 3.2 + tendy.phase).sin() * 0.16);

        if let Some(p_pos) = player_pos {
            let d = (p_pos + Vec3::Y * 0.8).distance(tf.translation);
            if d < 1.35 {
                commands.entity(entity).despawn_recursive();
                stats.tendies += 1;
                stats.score += 100;
                if tendy.is_final_goal || stats.tendies >= stats.total_tendies {
                    stats.score += 5000;
                    next_state.set(GameState::StageClear);
                }
            }
        }
    }

    for (entity, mut tf, power) in &mut power_q {
        tf.rotate_y(1.8 * dt);
        tf.translation.y = power.base_y + ((t * 2.8 + power.phase).sin() * 0.14);

        if let Some(p_pos) = player_pos {
            if (p_pos + Vec3::Y * 0.8).distance(tf.translation) < 1.35 {
                commands.entity(entity).despawn_recursive();
                match power.kind {
                    PowerupKind::HoneyMustard => stats.speed_boost_timer = 10.0,
                    PowerupKind::RanchShield => stats.has_ranch_shield = true,
                }
            }
        }
    }
}

pub fn victory_dance(
    time: Res<Time>,
    mut player_q: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut tf) = player_q.get_single_mut() else {
        return;
    };
    let t = time.elapsed_secs();
    // Face the camera (+Z) and bounce happily!
    tf.rotation = Quat::from_rotation_y(std::f32::consts::PI + (t * 6.0).sin() * 0.35);
    tf.translation.y = 1.9 + (t * 8.0).sin().abs() * 0.35;
}
