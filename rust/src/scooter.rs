use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::audio::{
    play_bgm, play_hacking_music, play_headphone_music, play_scooter_music, play_sfx,
    set_uzumaki_mode, speak_voice,
};
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::ui::HudOverlay;

pub const SCOOTER_RENT_COST: u32 = 12;

// 0 = open, 1 = firewall (#), 2 = $ AUTH_TOKEN, 3 = pulsing gate (%), 4 = KEY_A, 5 = KEY_B
const MAZE: [[u8; 11]; 11] = [
    [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 5, 0],
    [0, 0, 0, 0, 1, 0, 0, 3, 1, 1, 0],
    [0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1],
    [0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0],
    [1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 3, 0, 1, 1, 0],
    [0, 4, 1, 1, 1, 0, 1, 0, 0, 3, 0],
    [0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 2],
];

#[derive(Component)]
pub struct CityScooter {
    pub yaw: f32,
    pub speed: f32,
    pub paid: bool,
    pub occupied: bool,
}

#[derive(Component)]
pub struct CityMike;

#[derive(Component)]
pub struct PhoneMazeUi;

#[derive(Component)]
pub struct ScootPromptUi;

#[derive(Component)]
pub struct ScootPromptButton;

#[derive(Component)]
pub struct ScootPromptText;

#[derive(SystemParam)]
pub struct ScootUiParams<'w, 's> {
    pub ui_q: Query<'w, 's, Entity, With<ScootPromptUi>>,
    pub btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ScootPromptButton>)>,
    pub txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<ScootPromptText>,
            Without<PhoneMazeCellText>,
            Without<PhoneMazeStatusText>,
        ),
    >,
}

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
    pub key_a: bool,
    pub key_b: bool,
    pub gate_open: bool,
    pub gate_timer: f32,
    pub visited: [[bool; 11]; 11],
}

impl Default for PhoneMazeState {
    fn default() -> Self {
        let mut visited = [[false; 11]; 11];
        visited[0][0] = true;
        Self {
            px: 0,
            py: 0,
            moves: 0,
            key_a: false,
            key_b: false,
            gate_open: true,
            gate_timer: 0.0,
            visited,
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
        (Vec3::new(22.5, 0.0, 44.0), 0.25_f32),
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
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 0.24, 0.0).with_scale(Vec3::new(0.36, 0.08, 1.45)),
                ));
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::new(0.40, 0.05, 1.38)),
                ));
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.88, 0.56).with_scale(Vec3::new(0.045, 1.22, 0.045)),
                ));
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 1.48, 0.56)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.04, 0.62, 0.04)),
                ));
                for wz in [0.62_f32, -0.62_f32] {
                    s.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(tire_mat.clone()),
                        Transform::from_xyz(0.0, 0.18, wz)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::new(0.18, 0.10, 0.18)),
                    ));
                }
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(glow_mat.clone()),
                    Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(1.05, 0.03, 1.05)),
                ));
            });
    }
}

pub fn spawn_city_mike(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    _seed: u64,
) {
    // Mike hangs out in the alley right next to Pub 217 (217 3rd Ave SE)
    let pos = Vec3::new(28.5, 0.0, 32.0);

    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));

    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let beard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.14, 0.09),
        ..default()
    });
    let wrestling_tee = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });
    let belt_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.16),
        emissive: LinearRgba::new(0.45, 0.32, 0.04, 1.0),
        ..default()
    });
    let cap_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.12, 0.14),
        ..default()
    });
    let jeans = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.24, 0.42),
        ..default()
    });
    let weed_ring = materials.add(StandardMaterial {
        base_color: Color::srgba(0.22, 0.92, 0.36, 0.35),
        emissive: LinearRgba::new(0.12, 0.8, 0.22, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityMike,
            Transform::from_translation(pos),
        ))
        .with_children(|m| {
            // Legs
            for sx in [-0.14_f32, 0.14_f32] {
                m.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(jeans.clone()),
                    Transform::from_xyz(sx, 0.48, 0.0).with_scale(Vec3::new(0.20, 0.96, 0.22)),
                ));
            }
            // Black wrestling t-shirt torso + gold championship belt graphic
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(wrestling_tee.clone()),
                Transform::from_xyz(0.0, 1.28, 0.0).with_scale(Vec3::new(0.62, 0.72, 0.34)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(belt_gold.clone()),
                Transform::from_xyz(0.0, 1.32, 0.18).with_scale(Vec3::new(0.42, 0.20, 0.04)),
            ));
            // Head + Beard
            m.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.86, 0.0).with_scale(Vec3::splat(0.24)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(beard.clone()),
                Transform::from_xyz(0.0, 1.75, 0.11).with_scale(Vec3::new(0.28, 0.20, 0.20)),
            ));
            // Baseball Cap (crown + front brim)
            m.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(cap_red.clone()),
                Transform::from_xyz(0.0, 2.02, 0.0).with_scale(Vec3::new(0.25, 0.14, 0.25)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(cap_red.clone()),
                Transform::from_xyz(0.0, 1.98, 0.22).with_scale(Vec3::new(0.32, 0.04, 0.24)),
            ));
            // Alley weed aura ring
            m.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(weed_ring),
                Transform::from_xyz(0.0, 0.04, 0.0).with_scale(Vec3::new(1.5, 0.03, 1.5)),
            ));
        });
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
            BackgroundColor(Color::srgba(0.0, 0.02, 0.03, 0.58)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(340.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.06, 0.07)),
                BorderColor(Color::srgb(0.12, 0.92, 0.58)),
                BorderRadius::all(Val::Px(20.0)),
            ))
            .with_children(|phone| {
                phone.spawn((
                    Text::new("SCOOTR.OS // 0x7F_KERNEL_MAZE"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.18, 0.98, 0.62)),
                ));
                phone.spawn((
                    PhoneMazeStatusText,
                    Text::new("KEYS: 0/2 [K1,K2] | GATE: OPEN | PKT (0,0)"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.55, 0.92, 0.86)),
                ));
                phone.spawn((
                    Text::new("GRAB K1 & K2 BEFORE [$] | AVOID BLOCKED [%] GATES"),
                    TextFont {
                        font_size: 9.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.85, 0.78)),
                ));

                phone
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.02, 0.03, 0.04)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|grid| {
                        for y in 0..11_usize {
                            grid.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(2.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for x in 0..11_usize {
                                    let (bg, fg, sym) = cell_visual(x, y, maze);
                                    row.spawn((
                                        PhoneMazeCell { x, y },
                                        Node {
                                            width: Val::Px(24.0),
                                            height: Val::Px(24.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        BackgroundColor(bg),
                                        BorderRadius::all(Val::Px(3.0)),
                                    ))
                                    .with_children(|cell| {
                                        cell.spawn((
                                            PhoneMazeCellText { x, y },
                                            Text::new(sym),
                                            TextFont {
                                                font_size: 10.5,
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
                    Text::new("[WASD / ARROWS] Route | [P / ESC] Close | Cost: 12 Credits"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.82, 0.28)),
                ));
            });
        });
}

fn cell_visual(x: usize, y: usize, maze: &PhoneMazeState) -> (Color, Color, &'static str) {
    if x == maze.px && y == maze.py {
        return (
            Color::srgb(0.08, 0.45, 0.55),
            Color::srgb(0.4, 1.0, 1.0),
            "@",
        );
    }
    let dist = x.abs_diff(maze.px) + y.abs_diff(maze.py);
    let cell = MAZE[y][x];
    if dist > 3 && !maze.visited[y][x] && cell != 2 {
        return (
            Color::srgb(0.04, 0.05, 0.06),
            Color::srgb(0.18, 0.26, 0.24),
            "?",
        );
    }
    match cell {
        1 => (
            Color::srgb(0.24, 0.06, 0.08),
            Color::srgb(0.75, 0.22, 0.25),
            "#",
        ),
        2 => {
            if maze.key_a && maze.key_b {
                (
                    Color::srgb(0.32, 0.25, 0.05),
                    Color::srgb(1.0, 0.92, 0.25),
                    "$",
                )
            } else {
                (
                    Color::srgb(0.20, 0.14, 0.06),
                    Color::srgb(0.8, 0.48, 0.20),
                    "X",
                )
            }
        }
        3 => {
            if maze.gate_open {
                (
                    Color::srgb(0.08, 0.22, 0.28),
                    Color::srgb(0.35, 0.95, 1.0),
                    "%",
                )
            } else {
                (
                    Color::srgb(0.35, 0.08, 0.16),
                    Color::srgb(1.0, 0.35, 0.45),
                    "%",
                )
            }
        }
        4 => {
            if maze.key_a {
                (Color::srgb(0.05, 0.11, 0.08), Color::srgb(0.22, 0.65, 0.42), ".")
            } else {
                (Color::srgb(0.14, 0.28, 0.08), Color::srgb(0.65, 1.0, 0.35), "K1")
            }
        }
        5 => {
            if maze.key_b {
                (Color::srgb(0.05, 0.11, 0.08), Color::srgb(0.22, 0.65, 0.42), ".")
            } else {
                (Color::srgb(0.14, 0.28, 0.08), Color::srgb(0.65, 1.0, 0.35), "K2")
            }
        }
        _ => (
            Color::srgb(0.05, 0.11, 0.08),
            Color::srgb(0.22, 0.65, 0.42),
            ".",
        ),
    }
}

pub fn update_city_scooter_and_phone(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut maze: ResMut<PhoneMazeState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_q: Query<
        (&mut Transform, &mut Player),
        (Without<CityScooter>, Without<PlatformBox>, Without<CityMike>),
    >,
    mut scooter_q: Query<
        (Entity, &mut Transform, &mut CityScooter),
        (Without<Player>, Without<PlatformBox>, Without<CityMike>),
    >,
    mut mike_q: Query<
        (&Transform, &mut CityMike),
        (Without<Player>, Without<CityScooter>, Without<PlatformBox>),
    >,
    platform_q: Query<
        (&Transform, &PlatformBox),
        (Without<Player>, Without<CityScooter>, Without<CityMike>),
    >,
    phone_ui_q: Query<Entity, With<PhoneMazeUi>>,
    mut cell_bg_q: Query<(&PhoneMazeCell, &mut BackgroundColor)>,
    mut cell_txt_q: Query<(&PhoneMazeCellText, &mut Text, &mut TextColor)>,
    mut status_txt_q: Query<&mut Text, (With<PhoneMazeStatusText>, Without<PhoneMazeCellText>)>,
    mut scoot_ui: ScootUiParams,
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

        maze.gate_timer += dt;
        let mut gate_toggled = false;
        if maze.gate_timer >= 2.0 {
            maze.gate_timer -= 2.0;
            maze.gate_open = !maze.gate_open;
            gate_toggled = true;
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

        let mut moved = false;
        if dx != 0 || dy != 0 {
            let nx = maze.px as isize + dx;
            let ny = maze.py as isize + dy;
            let valid_bounds = nx >= 0 && nx < 11 && ny >= 0 && ny < 11;
            let cell = if valid_bounds { MAZE[ny as usize][nx as usize] } else { 1 };
            let passable = valid_bounds && cell != 1 && (cell != 3 || maze.gate_open);

            if passable {
                let mx = nx as usize;
                let my = ny as usize;
                maze.px = mx;
                maze.py = my;
                maze.moves += 1;
                maze.visited[my][mx] = true;
                moved = true;

                if cell == 4 {
                    maze.key_a = true;
                    play_sfx("hack_success");
                } else if cell == 5 {
                    maze.key_b = true;
                    play_sfx("hack_success");
                } else if cell == 2 {
                    if maze.key_a && maze.key_b {
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
                    } else {
                        play_sfx("hack_deny");
                    }
                } else {
                    play_sfx("hack_blip");
                }
            } else {
                play_sfx("hack_deny");
            }
        }

        if moved || gate_toggled {
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
                let kc = maze.key_a as u8 + maze.key_b as u8;
                st.0 = format!(
                    "KEYS: {}/2 | GATE: {} | PKT ({},{})",
                    kc,
                    if maze.gate_open { "OPEN" } else { "LOCKED" },
                    maze.px,
                    maze.py
                );
            }
        }
        return;
    }

    let scoot_btn_clicked = scoot_ui.btn_q.iter().any(|i| *i == Interaction::Pressed);
    let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE) || scoot_btn_clicked;

    // Check Mike in alleyway (beard, baseball cap, wrestling shirt) — triggers trippy cutscene on every interaction!
    for (m_tf, _mike) in &mut mike_q {
        let d = Vec2::new(
            m_tf.translation.x - p_tf.translation.x,
            m_tf.translation.z - p_tf.translation.z,
        )
        .length();
        if d < 3.4 && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
            if interact {
                stats.saved_city_pos = p_tf.translation;
                stats.mike_return_city = true;
                next_state.set(GameState::MikeCutscene);
                return;
            } else {
                stats.hint_text = "[F / E] Talk & Smoke Weed with Mike in the Alley".to_string();
            }
        }
    }

    // On foot: check nearby scooter
    if stats.riding_scooter.is_none() {
        if stats.driving_car.is_some() || stats.player_scale >= 6.0 {
            for e in &scoot_ui.ui_q {
                commands.entity(e).despawn_recursive();
            }
            return;
        }
        let mut nearest: Option<(Entity, f32)> = None;
        for (se, stf, _) in &scooter_q {
            let d = Vec2::new(
                stf.translation.x - p_tf.translation.x,
                stf.translation.z - p_tf.translation.z,
            )
            .length();
            if d < 5.2 && nearest.map_or(true, |(_, bd)| d < bd) {
                nearest = Some((se, d));
            }
        }

        if let Some((se, _)) = nearest {
            let btn_label = if !stats.scooter_app_unlocked {
                "SCOOT! (HACK PHONE APP [F/E])".to_string()
            } else if let Ok((_, _, sc)) = scooter_q.get(se) {
                if sc.paid {
                    "SCOOT! [F/E]".to_string()
                } else {
                    format!("SCOOT! (RENT: {} CREDITS)", SCOOTER_RENT_COST)
                }
            } else {
                "SCOOT! [F/E]".to_string()
            };

            if scoot_ui.ui_q.is_empty() {
                commands
                    .spawn((
                        ScootPromptUi,
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(26.0),
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ))
                    .with_children(|root| {
                        root.spawn((
                            ScootPromptButton,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.08, 0.72, 0.30)),
                            BorderColor(Color::srgb(0.45, 1.0, 0.65)),
                            BorderRadius::all(Val::Px(10.0)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                ScootPromptText,
                                Text::new(btn_label.clone()),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });
            } else if let Ok(mut st) = scoot_ui.txt_q.get_single_mut() {
                if st.0 != btn_label {
                    st.0 = btn_label;
                }
            }

            if !stats.scooter_app_unlocked {
                if interact {
                    for e in &scoot_ui.ui_q {
                        commands.entity(e).despawn_recursive();
                    }
                    stats.phone_open = true;
                    *maze = PhoneMazeState::default();
                    spawn_phone_maze_ui(&mut commands, &maze);
                    play_hacking_music();
                } else {
                    stats.hint_text =
                        "SCOOTER LOCKED! Click [SCOOT!] or press [P] to hack SCOOTR.OS!"
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
                        play_scooter_music();
                        stats.hint_text =
                            "RIDING SCOOTER! [WASD] Zip | [SHIFT] Turbo | [F/E] Hop Off"
                                .to_string();
                    }
                } else if sc.paid {
                    stats.hint_text = "Click [SCOOT!] or press [F / E] to Ride Scooter".to_string();
                } else {
                    stats.hint_text = format!(
                        "Click [SCOOT!] or press [F / E] to Rent ({} Credits | Have {})",
                        SCOOTER_RENT_COST, stats.tendy_credits
                    );
                }
            }
        } else {
            for e in &scoot_ui.ui_q {
                commands.entity(e).despawn_recursive();
            }
        }
        return;
    }

    if let Ok(mut st) = scoot_ui.txt_q.get_single_mut() {
        let lbl = "HOP OFF SCOOTER [F/E]";
        if st.0 != lbl {
            st.0 = lbl.to_string();
        }
    }

    let Some(se) = stats.riding_scooter else {
        return;
    };

    if interact {
        if let Ok((_, stf, mut sc)) = scooter_q.get_mut(se) {
            sc.occupied = false;
            sc.speed = 0.0;
            p_tf.translation = Vec3::new(stf.translation.x + 1.4, 0.0, stf.translation.z);
        }
        stats.riding_scooter = None;
        play_sfx("car_door");
        play_bgm();
        stats.hint_text = "HOPPED OFF SCOOTER!".to_string();
        return;
    }

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

#[derive(Component)]
pub struct MikeCutsceneEntity;

#[derive(Component)]
pub struct MikeCutsceneOverlay;

#[derive(Component)]
pub struct MikeCutsceneSpeakerText;

#[derive(Component)]
pub struct MikeCutsceneSubtitleText;

#[derive(Component)]
pub struct MikeCutsceneSmoke {
    pub base_pos: Vec3,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Resource, Default)]
pub struct MikeCutsceneDialog {
    pub variant: usize,
}

const MIKE_BACKSTORIES: [(&str, &str); 4] = [
    (
        "Are you sure you want this, traveler? Look at the bricks behind Pub 217... Cedar Rapids is built inside a giant spiral. Smoke this and see the truth.",
        "Every street... every tendy... it's all twisting into a spiral, Mike...",
    ),
    (
        "Are you sure you want this, traveler? Time coils inward behind this alley. Take a hit—the spiral will heal your wounds, if your mind survives it.",
        "I can hear the spirals spinning in my skull... thanks for the wisdom, Mike.",
    ),
    (
        "Are you sure you want this, traveler? My championship belt was forged at the center of the Cedar River vortex... inhale the sacred alley smoke.",
        "The ink... the spirals... everything in Downtown Cedar Rapids is connected!",
    ),
    (
        "Are you sure you want this, traveler? Once you step into the spiral behind Pub 217, you never truly leave. Let the smoke mend your body.",
        "My hearts are healed, but the spirals won't stop turning...",
    ),
];

pub fn enter_mike_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut dialog: ResMut<MikeCutsceneDialog>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    for mut n in &mut hud_q {
        n.display = Display::None;
    }

    let mut rbytes = [0u8; 4];
    let _ = getrandom::getrandom(&mut rbytes);
    dialog.variant = (u32::from_le_bytes(rbytes) as usize) % MIKE_BACKSTORIES.len();
    stats.cutscene_timer = 0.0;
    stats.cutscene_beat = 0;

    // Intense Uzumaki stark black-and-white manga ink aesthetic
    set_uzumaki_mode(2);
    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.02)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.95, 0.95),
        brightness: 1100.0,
    });

    let stage = Vec3::new(0.0, 0.0, -210.0);
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let spiral_torus = meshes.add(Torus::new(0.10, 1.0));

    let brick = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.08),
        ..default()
    });
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.04),
        ..default()
    });
    let smoke_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.92, 0.92, 0.42),
        emissive: LinearRgba::new(0.75, 0.75, 0.75, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let spiral_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(1.6, 1.6, 1.6, 1.0),
        unlit: true,
        ..default()
    });

    // Alley floor & brick walls
    commands.spawn((
        MikeCutsceneEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.25, 0.0))
            .with_scale(Vec3::new(16.0, 0.5, 16.0)),
    ));
    for sx in [-4.5_f32, 4.5_f32] {
        commands.spawn((
            MikeCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick.clone()),
            Transform::from_translation(stage + Vec3::new(sx, 3.5, 0.0))
                .with_scale(Vec3::new(1.0, 7.5, 16.0)),
        ));
    }

    // Spawn Mike (beard, red baseball cap, black wrestling shirt + gold belt)
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let beard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.14, 0.09),
        ..default()
    });
    let wrestling_tee = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });
    let belt_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.16),
        emissive: LinearRgba::new(0.5, 0.35, 0.05, 1.0),
        ..default()
    });
    let cap_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.14, 0.16),
        ..default()
    });
    let jeans = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.24, 0.42),
        ..default()
    });
    let plaid_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.14, 0.16),
        ..default()
    });

    // Mike facing Stan
    commands
        .spawn((
            MikeCutsceneEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.6)),
        ))
        .with_children(|m| {
            for sx in [-0.15_f32, 0.15_f32] {
                m.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(jeans.clone()),
                    Transform::from_xyz(sx, 0.48, 0.0).with_scale(Vec3::new(0.22, 0.96, 0.24)),
                ));
            }
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(wrestling_tee.clone()),
                Transform::from_xyz(0.0, 1.30, 0.0).with_scale(Vec3::new(0.68, 0.76, 0.36)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(belt_gold.clone()),
                Transform::from_xyz(0.0, 1.34, 0.19).with_scale(Vec3::new(0.46, 0.22, 0.04)),
            ));
            m.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.88, 0.0).with_scale(Vec3::splat(0.26)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(beard.clone()),
                Transform::from_xyz(0.0, 1.77, 0.12).with_scale(Vec3::new(0.30, 0.22, 0.22)),
            ));
            m.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(cap_red.clone()),
                Transform::from_xyz(0.0, 2.05, 0.0).with_scale(Vec3::new(0.27, 0.14, 0.27)),
            ));
            m.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(cap_red.clone()),
                Transform::from_xyz(0.0, 2.01, 0.24).with_scale(Vec3::new(0.34, 0.04, 0.26)),
            ));
        });

    // Stan silhouette facing Mike
    commands
        .spawn((
            MikeCutsceneEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.6))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(plaid_red),
                Transform::from_xyz(0.0, 1.1, 0.0).with_scale(Vec3::new(0.75, 1.3, 0.45)),
            ));
            s.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 1.95, 0.0).with_scale(Vec3::splat(0.28)),
            ));
        });

    // Swirling Uzumaki concentric spirals & ink smoke clouds
    for i in 0..16 {
        let ang = i as f32 * 0.42;
        let rad = 0.6 + (i % 4) as f32 * 0.55;
        let bp = stage + Vec3::new(ang.cos() * rad, 0.4 + (i % 5) as f32 * 0.55, ang.sin() * rad);
        commands.spawn((
            MikeCutsceneEntity,
            MikeCutsceneSmoke {
                base_pos: bp,
                speed: 0.7 + (i % 3) as f32 * 0.35,
                phase: i as f32 * 0.8,
            },
            Mesh3d(unit_sphere.clone()),
            MeshMaterial3d(smoke_mat.clone()),
            Transform::from_translation(bp).with_scale(Vec3::splat(0.32 + (i % 3) as f32 * 0.16)),
        ));
    }
    for ring_i in 0..8 {
        let sc = 0.8 + ring_i as f32 * 0.65;
        commands.spawn((
            MikeCutsceneEntity,
            Mesh3d(spiral_torus.clone()),
            MeshMaterial3d(spiral_white.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.7, -2.8))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(sc)),
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(2.6, 2.2, 3.4);
        c_tf.look_at(stage + Vec3::new(0.0, 1.5, -0.4), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }

    // Subtitle overlay
    let (mike_line, _) = MIKE_BACKSTORIES[dialog.variant];
    commands
        .spawn((
            MikeCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(Val::Px(22.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(88.0),
                    max_width: Val::Px(780.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.02, 0.14, 0.86)),
                BorderColor(Color::srgb(0.35, 0.98, 0.55)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    MikeCutsceneSpeakerText,
                    Text::new("MIKE (ALLEY ORACLE)  |  [SPACE / CLICK] Skip"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.40, 1.0, 0.62)),
                ));
                card.spawn((
                    MikeCutsceneSubtitleText,
                    Text::new(format!("\"{}\"", mike_line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    play_headphone_music(2);
    play_sfx("smoke_puff");
    speak_voice(mike_line, "tj");
}

pub fn update_mike_cutscene(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
    dialog: Res<MikeCutsceneDialog>,
    mut next_state: ResMut<NextState<GameState>>,
    mut smoke_q: Query<(&mut Transform, &MikeCutsceneSmoke), Without<Camera3d>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut speaker_q: Query<&mut Text, With<MikeCutsceneSpeakerText>>,
    mut sub_q: Query<&mut Text, (With<MikeCutsceneSubtitleText>, Without<MikeCutsceneSpeakerText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    let t = stats.cutscene_timer;

    if keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Escape)
        || mouse_buttons.just_pressed(MouseButton::Left)
    {
        next_state.set(GameState::CityPlay);
        return;
    }

    let (mike_line, stan_line) = MIKE_BACKSTORIES[dialog.variant];
    if t >= 6.8 && stats.cutscene_beat == 0 {
        stats.cutscene_beat = 1;
        play_sfx("smoke_puff");
        speak_voice(stan_line, "stan");
        if let Ok(mut sp) = speaker_q.get_single_mut() {
            sp.0 = "STAN (ENLIGHTENED)  |  [SPACE / CLICK] Skip".to_string();
        }
        if let Ok(mut sb) = sub_q.get_single_mut() {
            sb.0 = format!("\"{}\"", stan_line);
        }
    }
    if t >= 12.2 {
        next_state.set(GameState::CityPlay);
        return;
    }

    // Animate swirling weed smoke clouds
    for (mut stf, sm) in &mut smoke_q {
        let rise = ((t * sm.speed + sm.phase) % 2.8) * 1.1;
        stf.translation.x = sm.base_pos.x + (t * 1.4 + sm.phase).sin() * 0.45;
        stf.translation.y = sm.base_pos.y + rise;
        stf.translation.z = sm.base_pos.z + (t * 1.1 + sm.phase).cos() * 0.45;
    }

    // Trippy spiral + kaleidoscope FOV camera
    let stage = Vec3::new(0.0, 0.0, -210.0);
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let angle = 0.55 + t * 0.35;
        let dist = 3.8 + (t * 1.3).sin() * 0.55;
        c_tf.translation = stage + Vec3::new(angle.sin() * dist, 1.9 + (t * 1.6).sin() * 0.35, angle.cos() * dist);
        c_tf.look_at(stage + Vec3::new(0.0, 1.55, if stats.cutscene_beat == 0 { -1.1 } else { 0.8 }), Vec3::Y);
        c_tf.rotate_local_z((t * 1.75).sin() * 0.12);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4 + (t * 2.4).sin() * 0.14;
        }
    }
    let _ = mike_line;
}

pub fn exit_mike_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<MikeCutsceneEntity>>,
    ov_q: Query<Entity, With<MikeCutsceneOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
    set_uzumaki_mode(0);
    stats.hearts = (stats.hearts + 3).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.high_timer = 30.0;
    play_bgm();
}

