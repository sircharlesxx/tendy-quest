use bevy::prelude::*;
use crate::audio::{play_bgm, play_hacking_music, play_sfx};
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::state::{CameraOrbit, LevelEntity, PlatformBox, PlayerStats};

pub const SCOOTER_RENT_COST: u32 = 12;

const MAZE: [[u8; 7]; 7] = [
    [0, 0, 1, 0, 0, 0, 1],
    [1, 0, 1, 0, 1, 0, 0],
    [0, 0, 0, 0, 1, 1, 0],
    [0, 1, 1, 0, 0, 0, 0],
    [0, 0, 1, 1, 1, 0, 1],
    [1, 0, 0, 0, 1, 0, 0],
    [1, 1, 1, 0, 0, 0, 2],
];

#[derive(Component)]
pub struct CityScooter {
    pub yaw: f32,
    pub speed: f32,
    pub paid: bool,
    pub occupied: bool,
}

#[derive(Component)]
pub struct PhoneMazeUi;

#[derive(Component)]
pub struct PhoneMazeCell {
    pub x: usize,
    pub y: usize,
}

#[derive(Component)]
pub struct PhoneMazeCellText {
    pub x: usize,
    pub y: usize,
}

#[derive(Component)]
pub struct PhoneMazeStatusText;

#[derive(Resource)]
pub struct PhoneMazeState {
    pub px: usize,
    pub py: usize,
    pub moves: u32,
}

impl Default for PhoneMazeState {
    fn default() -> Self {
        Self {
            px: 0,
            py: 0,
            moves: 0,
        }
    }
}

pub fn spawn_city_scooters(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let deck_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.14, 0.16),
        metallic: 0.4,
        ..default()
    });
    let lime_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.92, 0.36),
        emissive: LinearRgba::new(0.12, 0.72, 0.25, 1.0),
        ..default()
    });
    let glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.95, 0.85, 0.36),
        emissive: LinearRgba::new(0.1, 0.7, 0.65, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let tire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });

    let spots = [
        (Vec3::new(3.8, 0.0, 74.0), 0.25_f32),
        (Vec3::new(-4.5, 0.0, 46.0), -0.4),
        (Vec3::new(42.0, 0.0, 42.0), 1.2),
        (Vec3::new(-42.0, 0.0, 42.0), -1.1),
        (Vec3::new(42.0, 0.0, 0.0), 1.57),
        (Vec3::new(-42.0, 0.0, 0.0), -1.57),
        (Vec3::new(0.0, 0.0, -42.0), 0.0),
        (Vec3::new(42.0, 0.0, -46.0), 2.4),
    ];

    for (pos, yaw) in spots {
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityScooter {
                    yaw,
                    speed: 0.0,
                    paid: false,
                    occupied: false,
                },
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            ))
            .with_children(|s| {
                // Deck
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 0.24, 0.0).with_scale(Vec3::new(0.36, 0.08, 1.45)),
                ));
                // Lime trim
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::new(0.40, 0.05, 1.38)),
                ));
                // Stem
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.88, 0.56).with_scale(Vec3::new(0.045, 1.22, 0.045)),
                ));
                // Handlebars
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 1.48, 0.56)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.04, 0.62, 0.04)),
                ));
                // Wheels
                for wz in [0.62_f32, -0.62_f32] {
                    s.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(tire_mat.clone()),
                        Transform::from_xyz(0.0, 0.18, wz)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::new(0.18, 0.10, 0.18)),
                    ));
                }
                // Subtle cyan beacon ring on ground
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(glow_mat.clone()),
                    Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(1.05, 0.03, 1.05)),
                ));
            });
    }
}

fn spawn_phone_maze_ui(commands: &mut Commands, maze: &PhoneMazeState) {
    commands
        .spawn((
            PhoneMazeUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.02, 0.03, 0.52)),
        ))
        .with_children(|root| {
            // Smartphone bezel
            root.spawn((
                Node {
                    width: Val::Px(320.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.06, 0.07)),
                BorderColor(Color::srgb(0.12, 0.92, 0.58)),
                BorderRadius::all(Val::Px(20.0)),
            ))
            .with_children(|phone| {
                phone.spawn((
                    Text::new("SCOOTR.OS // 0x7F_AUTH_PROXY"),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.18, 0.98, 0.62)),
                ));
                phone.spawn((
                    PhoneMazeStatusText,
                    Text::new(format!(
                        "SIG: 0xDEADBEEF | PKT ({},{}) -> $TOKEN",
                        maze.px, maze.py
                    )),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.55, 0.92, 0.86)),
                ));
                phone.spawn((
                    Text::new("ROUTE [@] TO [$] THROUGH FIREWALL"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.85, 0.78)),
                ));

                // 7x7 Obscure Maze Grid
                phone
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(3.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.02, 0.03, 0.04)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|grid| {
                        for y in 0..7_usize {
                            grid.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for x in 0..7_usize {
                                    let (bg, fg, sym) = cell_visual(x, y, maze);
                                    row.spawn((
                                        PhoneMazeCell { x, y },
                                        Node {
                                            width: Val::Px(34.0),
                                            height: Val::Px(34.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        BackgroundColor(bg),
                                        BorderRadius::all(Val::Px(4.0)),
                                    ))
                                    .with_children(|cell| {
                                        cell.spawn((
                                            PhoneMazeCellText { x, y },
                                            Text::new(sym),
                                            TextFont {
                                                font_size: 13.0,
                                                ..default()
                                            },
                                            TextColor(fg),
                                        ));
                                    });
                                }
                            });
                        }
                    });

                phone.spawn((
                    Text::new("[WASD / ARROWS] Route  |  [P / ESC] Close\nCOST AFTER UNLOCK: 12 TENDY CREDITS"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.82, 0.28)),
                ));
            });
        });
}

fn cell_visual(x: usize, y: usize, maze: &PhoneMazeState) -> (Color, Color, &'static str) {
    if x == maze.px && y == maze.py {
        (
            Color::srgb(0.08, 0.45, 0.55),
            Color::srgb(0.4, 1.0, 1.0),
            "@",
        )
    } else if MAZE[y][x] == 1 {
        (
            Color::srgb(0.24, 0.06, 0.08),
            Color::srgb(0.75, 0.22, 0.25),
            "#",
        )
    } else if MAZE[y][x] == 2 {
        (
            Color::srgb(0.32, 0.25, 0.05),
            Color::srgb(1.0, 0.88, 0.22),
            "$",
        )
    } else {
        (
            Color::srgb(0.05, 0.11, 0.08),
            Color::srgb(0.22, 0.65, 0.42),
            ".",
        )
    }
}

pub fn update_city_scooter_and_phone(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut maze: ResMut<PhoneMazeState>,
    mut player_q: Query<(&mut Transform, &mut Player), (Without<CityScooter>, Without<PlatformBox>)>,
    mut scooter_q: Query<(Entity, &mut Transform, &mut CityScooter), (Without<Player>, Without<PlatformBox>)>,
    platform_q: Query<(&Transform, &PlatformBox), (Without<Player>, Without<CityScooter>)>,
    phone_ui_q: Query<Entity, With<PhoneMazeUi>>,
    mut cell_bg_q: Query<(&PhoneMazeCell, &mut BackgroundColor)>,
    mut cell_txt_q: Query<(&PhoneMazeCellText, &mut Text, &mut TextColor)>,
    mut status_txt_q: Query<
        &mut Text,
        (With<PhoneMazeStatusText>, Without<PhoneMazeCellText>),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };

    // Toggle phone with [P]
    if keys.just_pressed(KeyCode::KeyP) && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
        if stats.phone_open {
            stats.phone_open = false;
            for e in &phone_ui_q {
                commands.entity(e).despawn_recursive();
            }
            play_bgm();
            return;
        } else if !stats.scooter_app_unlocked {
            stats.phone_open = true;
            *maze = PhoneMazeState::default();
            spawn_phone_maze_ui(&mut commands, &maze);
            play_hacking_music();
            return;
        }
    }

    // Handle active Phone Hacking Maze minigame
    if stats.phone_open {
        if keys.just_pressed(KeyCode::Escape) {
            stats.phone_open = false;
            for e in &phone_ui_q {
                commands.entity(e).despawn_recursive();
            }
            play_bgm();
            return;
        }

        let mut dx: isize = 0;
        let mut dy: isize = 0;
        if keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp) {
            dy = -1;
        } else if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
            dy = 1;
        } else if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft) {
            dx = -1;
        } else if keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight) {
            dx = 1;
        }

        if dx != 0 || dy != 0 {
            let nx = maze.px as isize + dx;
            let ny = maze.py as isize + dy;
            if nx >= 0 && nx < 7 && ny >= 0 && ny < 7 && MAZE[ny as usize][nx as usize] != 1 {
                maze.px = nx as usize;
                maze.py = ny as usize;
                maze.moves += 1;

                if MAZE[maze.py][maze.px] == 2 {
                    stats.scooter_app_unlocked = true;
                    stats.phone_open = false;
                    for e in &phone_ui_q {
                        commands.entity(e).despawn_recursive();
                    }
                    play_sfx("hack_success");
                    play_bgm();
                    stats.hint_text = format!(
                        "SCOOTR APP HACKED! Press [F/E] near a Scooter ({} Tendy Credits)!",
                        SCOOTER_RENT_COST
                    );
                    return;
                }
                play_sfx("hack_blip");
            } else {
                play_sfx("hack_deny");
            }

            for (cell, mut bg) in &mut cell_bg_q {
                let (new_bg, _, _) = cell_visual(cell.x, cell.y, &maze);
                bg.0 = new_bg;
            }
            for (ct, mut t, mut tc) in &mut cell_txt_q {
                let (_, new_fg, sym) = cell_visual(ct.x, ct.y, &maze);
                t.0 = sym.to_string();
                tc.0 = new_fg;
            }
            if let Ok(mut st) = status_txt_q.get_single_mut() {
                st.0 = format!(
                    "SIG: 0x{:04X} | PKT ({},{}) -> $TOKEN",
                    0xDEAD_u16.wrapping_add(maze.moves as u16 * 17),
                    maze.px,
                    maze.py
                );
            }
        }
        return;
    }

    let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

    // On foot: check nearby scooter
    if stats.riding_scooter.is_none() {
        if stats.driving_car.is_some() || stats.player_scale >= 6.0 {
            return;
        }
        let mut nearest: Option<(Entity, f32)> = None;
        for (se, stf, _) in &scooter_q {
            let d = Vec2::new(
                stf.translation.x - p_tf.translation.x,
                stf.translation.z - p_tf.translation.z,
            )
            .length();
            if d < 3.6 && nearest.map_or(true, |(_, bd)| d < bd) {
                nearest = Some((se, d));
            }
        }

        if let Some((se, _)) = nearest {
            if !stats.scooter_app_unlocked {
                if interact {
                    stats.phone_open = true;
                    *maze = PhoneMazeState::default();
                    spawn_phone_maze_ui(&mut commands, &maze);
                    play_hacking_music();
                } else {
                    stats.hint_text =
                        "SCOOTER LOCKED! Press [P] or [F/E] to hack SCOOTR.OS on your phone!"
                            .to_string();
                }
            } else if let Ok((_, stf, mut sc)) = scooter_q.get_mut(se) {
                if interact {
                    if !sc.paid && stats.tendy_credits < SCOOTER_RENT_COST {
                        play_sfx("hack_deny");
                        stats.hint_text = format!(
                            "EXPENSIVE! Need {} Tendy Credits for Scooter (Have {}/{})!",
                            SCOOTER_RENT_COST, stats.tendy_credits, SCOOTER_RENT_COST
                        );
                    } else {
                        if !sc.paid {
                            stats.tendy_credits -= SCOOTER_RENT_COST;
                            sc.paid = true;
                        }
                        sc.occupied = true;
                        stats.riding_scooter = Some(se);
                        p_tf.translation = stf.translation + Vec3::Y * 0.25;
                        player.velocity = Vec3::ZERO;
                        play_sfx("hack_success");
                        stats.hint_text =
                            "RIDING SCOOTER! [WASD] Zip | [SHIFT] Turbo | [F/E] Hop Off"
                                .to_string();
                    }
                } else if sc.paid {
                    stats.hint_text = "[F / E] Ride Unlocked Scooter".to_string();
                } else {
                    stats.hint_text = format!(
                        "[F / E] Rent Scooter ({} Tendy Credits | You have {})",
                        SCOOTER_RENT_COST, stats.tendy_credits
                    );
                }
            }
        }
        return;
    }

    let Some(se) = stats.riding_scooter else {
        return;
    };

    // Dismount scooter
    if interact {
        if let Ok((_, stf, mut sc)) = scooter_q.get_mut(se) {
            sc.occupied = false;
            sc.speed = 0.0;
            p_tf.translation = Vec3::new(stf.translation.x + 1.4, 0.0, stf.translation.z);
        }
        stats.riding_scooter = None;
        play_sfx("car_door");
        stats.hint_text = "HOPPED OFF SCOOTER!".to_string();
        return;
    }

    // Active scooter riding physics
    if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
        let mut throttle = 0.0_f32;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            throttle += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            throttle -= 1.0;
        }
        let mut steer = 0.0_f32;
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            steer += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            steer -= 1.0;
        }

        let turbo = (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
            && stats.dash_energy > 2.0
            && throttle > 0.1;
        if turbo {
            stats.dash_energy = (stats.dash_energy - 28.0 * dt).max(0.0);
        }

        let max_spd = if turbo { 42.0 } else { 30.0 };
        if throttle > 0.05 {
            sc.speed = (sc.speed + 36.0 * dt).min(max_spd);
        } else if throttle < -0.05 {
            sc.speed = (sc.speed - 38.0 * dt).max(-12.0);
        } else {
            sc.speed *= (1.0 - 4.0 * dt).max(0.0);
        }

        let turn_scale = (sc.speed.abs() / 6.0).clamp(0.25, 1.0);
        sc.yaw += steer * 3.1 * turn_scale * dt;

        let fwd = Vec2::new(sc.yaw.sin(), sc.yaw.cos());
        stf.translation.x = (stf.translation.x + fwd.x * sc.speed * dt).clamp(-126.0, 126.0);
        stf.translation.z = (stf.translation.z + fwd.y * sc.speed * dt).clamp(-126.0, 126.0);
        stf.translation.y = 0.0;

        // Skyscraper collision
        let wall_r = 0.82;
        for (b_tf, b_box) in &platform_q {
            if b_tf.translation.y - b_box.half_extents.y > 2.0 {
                continue;
            }
            let min_x = b_tf.translation.x - b_box.half_extents.x - wall_r;
            let max_x = b_tf.translation.x + b_box.half_extents.x + wall_r;
            let min_z = b_tf.translation.z - b_box.half_extents.z - wall_r;
            let max_z = b_tf.translation.z + b_box.half_extents.z + wall_r;
            if stf.translation.x > min_x
                && stf.translation.x < max_x
                && stf.translation.z > min_z
                && stf.translation.z < max_z
            {
                let dx_l = stf.translation.x - min_x;
                let dx_r = max_x - stf.translation.x;
                let dz_b = stf.translation.z - min_z;
                let dz_f = max_z - stf.translation.z;
                let m = dx_l.min(dx_r).min(dz_b).min(dz_f);
                if (m - dx_l).abs() < 0.001 {
                    stf.translation.x = min_x;
                } else if (m - dx_r).abs() < 0.001 {
                    stf.translation.x = max_x;
                } else if (m - dz_b).abs() < 0.001 {
                    stf.translation.z = min_z;
                } else {
                    stf.translation.z = max_z;
                }
                sc.speed *= 0.35;
            }
        }

        let lean = (-steer * sc.speed * 0.006).clamp(-0.16, 0.16);
        stf.rotation = Quat::from_rotation_y(sc.yaw) * Quat::from_rotation_z(lean);

        p_tf.translation = stf.translation + Vec3::Y * 0.25;
        p_tf.rotation = Quat::from_rotation_y(sc.yaw + std::f32::consts::PI);

        if sc.speed.abs() > 2.0 {
            let target_cam_yaw = sc.yaw + std::f32::consts::PI;
            let mut diff = (target_cam_yaw - orbit.yaw) % std::f32::consts::TAU;
            if diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            } else if diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            orbit.yaw += diff * (dt * 3.0).min(1.0);
        }
    }
}
