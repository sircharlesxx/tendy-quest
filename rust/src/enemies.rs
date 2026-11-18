use bevy::prelude::*;
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    BroccoliBrute,
    CarrotLancer,
    KaleSlime,
}

/// Carrot Lancers cycle through Patrol -> Telegraph (0.8s windup) -> Lunge -> Stunned
/// so players have a readable window to jump or dodge before the high-speed fork charge.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarrotState {
    Patrol,
    Telegraph,
    Lunge,
    Stunned,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub origin: Vec3,
    pub angle: f32,
    pub state_timer: f32,
    pub carrot_state: CarrotState,
    pub lunge_dir: Vec3,
    pub trail_timer: f32,
    pub knockback: Vec3,
    pub walk_phase: f32,
    pub hp: i32,
    pub hit_cooldown: f32,
}

#[derive(Component)]
pub struct FiberPuddle {
    pub lifetime: f32,
}

#[derive(Resource)]
pub struct WaveTimer(pub Timer);

pub fn spawn_initial_enemies(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_initial_enemies_inner(&mut commands, &asset_server);
}

pub fn spawn_initial_enemies_inner(commands: &mut Commands, asset_server: &AssetServer) {
    commands.insert_resource(WaveTimer(Timer::from_seconds(16.0, TimerMode::Repeating)));

    let spots = [
        // South Starter Breakfast Island
        (EnemyKind::BroccoliBrute, Vec3::new(-4.5, 0.0, 26.5)),
        (EnemyKind::CarrotLancer, Vec3::new(4.8, 0.0, 24.5)),
        // 1,200 Sq Ft Apartment Floor (West, Central, Living, Dining, North)
        (EnemyKind::KaleSlime, Vec3::new(-18.0, -6.0, 2.0)),
        (EnemyKind::BroccoliBrute, Vec3::new(16.0, -6.0, -8.0)),
        (EnemyKind::CarrotLancer, Vec3::new(-28.0, -6.0, -14.0)),
        (EnemyKind::BroccoliBrute, Vec3::new(26.0, -6.0, -24.0)),
        // North Kitchen Countertop
        (EnemyKind::CarrotLancer, Vec3::new(-16.0, 0.0, -40.5)),
    ];
    for (kind, pos) in spots {
        spawn_enemy(commands, asset_server, kind, pos);
    }
}

fn spawn_enemy(commands: &mut Commands, asset_server: &AssetServer, kind: EnemyKind, pos: Vec3) {
    spawn_enemy_at(commands, asset_server, kind, pos);
}

pub fn spawn_enemy_at(commands: &mut Commands, asset_server: &AssetServer, kind: EnemyKind, pos: Vec3) {
    let path = match kind {
        EnemyKind::BroccoliBrute => "models/broccoli_brute.glb#Scene0",
        EnemyKind::CarrotLancer => "models/carrot_lancer.glb#Scene0",
        EnemyKind::KaleSlime => "models/kale_slime.glb#Scene0",
    };
    let scene: Handle<Scene> = asset_server.load(path);
    commands.spawn((
        LevelEntity,
        SceneRoot(scene),
        crate::state::SceneRetryPath::new(path),
        Transform::from_translation(pos),
        Enemy {
            kind,
            origin: pos,
            angle: pos.z * 0.3,
            state_timer: 0.0,
            carrot_state: CarrotState::Patrol,
            lunge_dir: Vec3::NEG_Z,
            trail_timer: 0.0,
            knockback: Vec3::ZERO,
            walk_phase: pos.z * 0.5,
            hp: 2,
            hit_cooldown: 0.0,
        },
    ));
}

pub fn update_enemies(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut wave_timer: ResMut<WaveTimer>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut enemy_q: Query<(&mut Transform, &mut Enemy), Without<Player>>,
    mut puddle_q: Query<(Entity, &Transform, &mut FiberPuddle), (Without<Player>, Without<Enemy>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };

    // Wave reinforcements (up to Wave 3) across Starter Island & 1,200 Sq Ft Apartment Floor
    if wave_timer.0.tick(time.delta()).just_finished() && stats.wave < 3 {
        stats.wave += 1;
        let spawns = [
            (EnemyKind::BroccoliBrute, Vec3::new(-10.0, 0.0, 27.0)),
            (EnemyKind::CarrotLancer, Vec3::new(-22.0, -6.0, 6.0)),
            (EnemyKind::KaleSlime, Vec3::new(20.0, -6.0, -16.0)),
        ];
        for (kind, pos) in spawns {
            spawn_enemy(&mut commands, &asset_server, kind, pos);
        }
    }

    for (mut e_tf, mut enemy) in &mut enemy_q {
        if enemy.hit_cooldown > 0.0 {
            enemy.hit_cooldown = (enemy.hit_cooldown - dt).max(0.0);
        }
        let (min_x, max_x, min_z, max_z) = if enemy.origin.y < -1.0 {
            (-48.0_f32, 48.0_f32, -36.0_f32, 14.0_f32)
        } else if enemy.origin.z < -30.0 {
            (-40.0_f32, 10.0_f32, -45.0_f32, -36.0_f32)
        } else {
            (-14.5_f32, 14.5_f32, 21.0_f32, 34.5_f32)
        };

        if enemy.knockback.length_squared() > 0.05 {
            e_tf.translation += enemy.knockback * dt;
            enemy.knockback *= (1.0 - 6.0 * dt).max(0.0);
        }

        let to_player = p_tf.translation - e_tf.translation;
        let same_level = (p_tf.translation.y - e_tf.translation.y).abs() < 2.2;
        let flat_to_player = Vec3::new(to_player.x, 0.0, to_player.z);
        let dist = flat_to_player.length();

        let mut move_vec = Vec3::ZERO;
        match enemy.kind {
            EnemyKind::BroccoliBrute => {
                if same_level && dist < 9.0 && dist > 0.2 {
                    move_vec = flat_to_player.normalize() * 3.8;
                } else {
                    enemy.angle += dt * 0.9;
                    let target = enemy.origin
                        + Vec3::new(enemy.angle.cos() * 2.5, 0.0, enemy.angle.sin() * 2.5);
                    let d = target - e_tf.translation;
                    if d.length() > 0.15 {
                        move_vec = d.normalize() * 2.2;
                    }
                }
            }
            EnemyKind::CarrotLancer => match enemy.carrot_state {
                CarrotState::Patrol => {
                    enemy.angle += dt * 1.1;
                    let target = enemy.origin
                        + Vec3::new(enemy.angle.cos() * 2.8, 0.0, enemy.angle.sin() * 2.8);
                    let d = target - e_tf.translation;
                    if d.length() > 0.15 {
                        move_vec = d.normalize() * 2.6;
                    }
                    if same_level && dist < 10.5 && dist > 0.2 {
                        enemy.carrot_state = CarrotState::Telegraph;
                        enemy.state_timer = 0.8;
                        enemy.lunge_dir = flat_to_player.normalize();
                    }
                }
                CarrotState::Telegraph => {
                    enemy.state_timer -= dt;
                    if dist > 0.2 {
                        enemy.lunge_dir = flat_to_player.normalize();
                        let yaw = (-enemy.lunge_dir.x).atan2(-enemy.lunge_dir.z);
                        e_tf.rotation = Quat::from_rotation_y(yaw);
                    }
                    if enemy.state_timer <= 0.0 {
                        enemy.carrot_state = CarrotState::Lunge;
                        enemy.state_timer = 0.6;
                        crate::audio::play_sfx("lunge");
                    }
                }
                CarrotState::Lunge => {
                    enemy.state_timer -= dt;
                    move_vec = enemy.lunge_dir * 11.5;
                    if e_tf.translation.x <= min_x + 0.1
                        || e_tf.translation.x >= max_x - 0.1
                        || enemy.state_timer <= 0.0
                    {
                        enemy.carrot_state = CarrotState::Stunned;
                        enemy.state_timer = 1.1;
                    }
                }
                CarrotState::Stunned => {
                    enemy.state_timer -= dt;
                    if enemy.state_timer <= 0.0 {
                        enemy.carrot_state = CarrotState::Patrol;
                    }
                }
            },
            EnemyKind::KaleSlime => {
                if same_level && dist < 9.5 && dist > 0.2 {
                    move_vec = flat_to_player.normalize() * 2.1;
                } else {
                    enemy.angle += dt * 0.7;
                    let target = enemy.origin
                        + Vec3::new(enemy.angle.cos() * 2.4, 0.0, enemy.angle.sin() * 2.4);
                    let d = target - e_tf.translation;
                    if d.length() > 0.15 {
                        move_vec = d.normalize() * 1.8;
                    }
                }
                enemy.trail_timer += dt;
                if enemy.trail_timer >= 1.4 {
                    enemy.trail_timer = 0.0;
                    commands.spawn((
                        LevelEntity,
                        Mesh3d(meshes.add(Cylinder::new(0.85, 0.06))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgba(0.12, 0.45, 0.15, 0.85),
                            alpha_mode: AlphaMode::Blend,
                            ..default()
                        })),
                        Transform::from_xyz(
                            e_tf.translation.x,
                            enemy.origin.y + 0.03,
                            e_tf.translation.z,
                        ),
                        FiberPuddle { lifetime: 4.5 },
                    ));
                }
            }
        }

        let rage_mul = if stats.boss_mode { 1.25 } else { 1.0 };
        let move_speed = move_vec.length() * rage_mul;
        enemy.walk_phase += move_speed * dt * 2.4;

        e_tf.translation += move_vec * rage_mul * dt;
        e_tf.translation.x = e_tf.translation.x.clamp(min_x, max_x);
        e_tf.translation.z = e_tf.translation.z.clamp(min_z, max_z);
        e_tf.translation.y = enemy.origin.y;
        if move_vec.length_squared() > 0.05 {
            let yaw = (-move_vec.x).atan2(-move_vec.z);
            e_tf.rotation = Quat::from_rotation_y(yaw);
        }

        // Hit check against player (big Katamari Hero bowls veggies aside outside boss mode; Dodge & Roll evades!)
        let xz_dist = Vec2::new(
            p_tf.translation.x - e_tf.translation.x,
            p_tf.translation.z - e_tf.translation.z,
        )
        .length();
        let y_overlap =
            (p_tf.translation.y - e_tf.translation.y).abs() < (1.25 + stats.player_scale * 0.45);
        let hit_radius = if stats.boss_mode {
            1.65
        } else {
            1.05 + stats.player_scale * 0.25
        };
        if xz_dist < hit_radius
            && y_overlap
            && player.invuln_timer <= 0.0
            && player.roll_timer <= 0.0
            && !(stats.boss_mode && player.attack_timer > 0.0)
        {
            if !stats.boss_mode && (stats.enraged || stats.player_scale >= 2.2) {
                enemy.knockback =
                    -flat_to_player.normalize_or_zero() * (14.0 + stats.player_scale * 1.5);
                player.invuln_timer = 0.25;
                stats.score += 250;
                crate::audio::play_sfx("shield");
            } else if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
                player.invuln_timer = 0.8;
                enemy.knockback = -flat_to_player.normalize_or_zero() * 12.0;
                crate::audio::play_sfx("shield");
            } else {
                stats.hearts -= 1;
                player.invuln_timer = 1.2;
                let push = flat_to_player.normalize_or_zero() * 8.0;
                p_tf.translation += push * 0.25;
                enemy.knockback = -flat_to_player.normalize_or_zero() * 14.0;
                player.velocity.y = 5.0;
                crate::audio::play_sfx("hit");
                if stats.hearts <= 0 {
                    crate::audio::play_sfx("lose");
                    next_state.set(GameState::GameOver);
                }
            }
        }
    }

    // Fiber Puddles slow small heroes on matching floor height
    for (entity, pud_tf, mut puddle) in &mut puddle_q {
        puddle.lifetime -= dt;
        if puddle.lifetime <= 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let xz_d = Vec2::new(
            p_tf.translation.x - pud_tf.translation.x,
            p_tf.translation.z - pud_tf.translation.z,
        )
        .length();
        if xz_d < 1.0
            && (p_tf.translation.y - pud_tf.translation.y).abs() < 0.55
            && !stats.enraged
            && stats.player_scale < 2.2
        {
            player.slow_timer = 0.4;
        }
    }
}

pub fn animate_enemies(
    time: Res<Time>,
    mut enemy_q: Query<(&Enemy, &mut Transform, Option<&Children>)>,
    children_q: Query<&Children>,
    mut part_q: Query<(&Name, &mut Transform), Without<Enemy>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let blend = (dt * 14.0).min(1.0);

    for (enemy, mut e_tf, maybe_children) in &mut enemy_q {
        if enemy.kind == EnemyKind::KaleSlime {
            let pulse = (t * 4.5 + enemy.angle).sin();
            e_tf.scale = Vec3::new(1.0 + pulse * 0.14, 1.0 - pulse * 0.16, 1.0 + pulse * 0.14);
            continue;
        }

        let Some(root_children) = maybe_children else {
            continue;
        };
        let mut stack: Vec<Entity> = root_children.iter().copied().collect();
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children_q.get(entity) {
                stack.extend(kids.iter().copied());
            }
            let Ok((name, mut tf)) = part_q.get_mut(entity) else {
                continue;
            };

            match enemy.kind {
                EnemyKind::BroccoliBrute => {
                    let s = enemy.walk_phase.sin();
                    let waddle = enemy.walk_phase.cos() * 0.08;
                    let bounce = (enemy.walk_phase * 2.0).sin().abs() * 0.05;
                    match name.as_str() {
                        "BruteLeftLeg" => tf.rotation = tf.rotation.slerp(Quat::from_rotation_x(s * 0.65), blend),
                        "BruteRightLeg" => tf.rotation = tf.rotation.slerp(Quat::from_rotation_x(-s * 0.65), blend),
                        "BruteLeftArm" => {
                            let rot = Quat::from_rotation_x(-s * 0.55) * Quat::from_rotation_z(-0.15);
                            tf.rotation = tf.rotation.slerp(rot, blend);
                            tf.translation.y = 1.24 + bounce;
                        }
                        "BruteRightArm" => {
                            let rot = Quat::from_rotation_x(s * 0.55) * Quat::from_rotation_z(0.15);
                            tf.rotation = tf.rotation.slerp(rot, blend);
                            tf.translation.y = 1.24 + bounce;
                        }
                        "BruteBody" => {
                            let rot = Quat::from_rotation_z(waddle) * Quat::from_rotation_x(-0.08);
                            tf.rotation = tf.rotation.slerp(rot, blend);
                            tf.translation.y = 0.48 + bounce;
                        }
                        _ => {}
                    }
                }
                EnemyKind::CarrotLancer => {
                    let s = enemy.walk_phase.sin();
                    let (ll, rl, la, ra, body_rot, body_y) = match enemy.carrot_state {
                        CarrotState::Patrol => (
                            Quat::from_rotation_x(s * 0.70),
                            Quat::from_rotation_x(-s * 0.70),
                            Quat::from_rotation_x(-s * 0.55),
                            Quat::from_rotation_x(-0.18 + s * 0.25),
                            Quat::from_rotation_z(enemy.walk_phase.cos() * 0.06),
                            0.38 + (enemy.walk_phase * 2.0).sin().abs() * 0.04,
                        ),
                        CarrotState::Telegraph => {
                            let jitter = (t * 28.0).sin() * 0.08;
                            (
                                Quat::from_rotation_x(0.35),
                                Quat::from_rotation_x(-0.35),
                                Quat::from_rotation_x(0.45),
                                Quat::from_rotation_x(0.55 + jitter),
                                Quat::from_rotation_x(0.22),
                                0.35,
                            )
                        }
                        CarrotState::Lunge => {
                            let sprint = (t * 18.0).sin();
                            (
                                Quat::from_rotation_x(sprint * 0.85),
                                Quat::from_rotation_x(-sprint * 0.85),
                                Quat::from_rotation_x(-0.80),
                                Quat::from_rotation_x(-1.25),
                                Quat::from_rotation_x(-0.28),
                                0.38,
                            )
                        }
                        CarrotState::Stunned => {
                            let wobble = (t * 9.0).sin() * 0.22;
                            (
                                Quat::IDENTITY,
                                Quat::IDENTITY,
                                Quat::from_rotation_z(-0.45),
                                Quat::from_rotation_z(0.45),
                                Quat::from_rotation_z(wobble) * Quat::from_rotation_x(0.15),
                                0.38,
                            )
                        }
                    };
                    match name.as_str() {
                        "CarrotLeftLeg" => tf.rotation = tf.rotation.slerp(ll, blend),
                        "CarrotRightLeg" => tf.rotation = tf.rotation.slerp(rl, blend),
                        "CarrotLeftArm" => {
                            tf.rotation = tf.rotation.slerp(la, blend);
                            tf.translation.y = 1.05 + (body_y - 0.38);
                        }
                        "CarrotRightArm" => {
                            tf.rotation = tf.rotation.slerp(ra, blend);
                            tf.translation.y = 1.05 + (body_y - 0.38);
                        }
                        "CarrotBody" => {
                            tf.rotation = tf.rotation.slerp(body_rot, blend);
                            tf.translation.y = body_y;
                        }
                        _ => {}
                    }
                }
                EnemyKind::KaleSlime => {}
            }
        }
    }
}
