use bevy::prelude::*;
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    BroccoliBrute,
    CarrotLancer,
    KaleSlime,
}

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
        (EnemyKind::BroccoliBrute, Vec3::new(0.0, 0.0, 10.0)),
        (EnemyKind::CarrotLancer, Vec3::new(-3.2, 0.0, 3.0)),
        (EnemyKind::KaleSlime, Vec3::new(2.8, 0.0, -3.5)),
        (EnemyKind::BroccoliBrute, Vec3::new(-1.8, 0.0, -11.0)),
        (EnemyKind::CarrotLancer, Vec3::new(2.6, 0.0, -13.5)),
    ];
    for (kind, pos) in spots {
        spawn_enemy(commands, asset_server, kind, pos);
    }
}

fn spawn_enemy(commands: &mut Commands, asset_server: &AssetServer, kind: EnemyKind, pos: Vec3) {
    let path = match kind {
        EnemyKind::BroccoliBrute => "assets/models/broccoli_brute.glb#Scene0",
        EnemyKind::CarrotLancer => "assets/models/carrot_lancer.glb#Scene0",
        EnemyKind::KaleSlime => "assets/models/kale_slime.glb#Scene0",
    };
    let scene: Handle<Scene> = asset_server.load(path);
    commands.spawn((
        LevelEntity,
        SceneRoot(scene),
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

    // Wave reinforcements (up to Wave 3)
    if wave_timer.0.tick(time.delta()).just_finished() && stats.wave < 3 {
        stats.wave += 1;
        let spawns = [
            (EnemyKind::BroccoliBrute, Vec3::new(-4.0, 0.0, 4.0)),
            (EnemyKind::CarrotLancer, Vec3::new(4.0, 0.0, -7.0)),
            (EnemyKind::KaleSlime, Vec3::new(0.0, 0.0, -14.0)),
        ];
        for (kind, pos) in spawns {
            spawn_enemy(&mut commands, &asset_server, kind, pos);
        }
    }

    for (mut e_tf, mut enemy) in &mut enemy_q {
        if enemy.knockback.length_squared() > 0.05 {
            e_tf.translation += enemy.knockback * dt;
            enemy.knockback *= (1.0 - 6.0 * dt).max(0.0);
        }

        let to_player = p_tf.translation - e_tf.translation;
        let flat_to_player = Vec3::new(to_player.x, 0.0, to_player.z);
        let dist = flat_to_player.length();

        let mut move_vec = Vec3::ZERO;
        match enemy.kind {
            EnemyKind::BroccoliBrute => {
                if dist < 9.0 && dist > 0.2 {
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
                    if dist < 10.5 && dist > 0.2 {
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
                    }
                }
                CarrotState::Lunge => {
                    enemy.state_timer -= dt;
                    move_vec = enemy.lunge_dir * 11.5;
                    if e_tf.translation.x.abs() >= 6.4 || enemy.state_timer <= 0.0 {
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
                if dist < 9.5 && dist > 0.2 {
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
                        Transform::from_xyz(e_tf.translation.x, 0.03, e_tf.translation.z),
                        FiberPuddle { lifetime: 4.5 },
                    ));
                }
            }
        }

        e_tf.translation += move_vec * dt;
        e_tf.translation.x = e_tf.translation.x.clamp(-6.5, 6.5);
        e_tf.translation.z = e_tf.translation.z.clamp(-20.5, 20.5);
        if move_vec.length_squared() > 0.05 {
            let yaw = (-move_vec.x).atan2(-move_vec.z);
            e_tf.rotation = Quat::from_rotation_y(yaw);
        }

        // Hit check against player
        let hit_dist = p_tf.translation.distance(e_tf.translation);
        if hit_dist < 1.15 && (p_tf.translation.y - e_tf.translation.y).abs() < 1.3 && player.invuln_timer <= 0.0 {
            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
                player.invuln_timer = 0.8;
                enemy.knockback = -flat_to_player.normalize_or_zero() * 12.0;
            } else {
                stats.hearts -= 1;
                player.invuln_timer = 1.2;
                let push = flat_to_player.normalize_or_zero() * 8.0;
                p_tf.translation += push * 0.25;
                player.velocity.y = 5.0;
                if stats.hearts <= 0 {
                    next_state.set(GameState::GameOver);
                }
            }
        }
    }

    // Fiber Puddles slow player
    for (entity, pud_tf, mut puddle) in &mut puddle_q {
        puddle.lifetime -= dt;
        if puddle.lifetime <= 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        if (p_tf.translation - pud_tf.translation).length() < 1.0 && p_tf.translation.y < 0.4 {
            player.slow_timer = 0.4;
        }
    }
}
