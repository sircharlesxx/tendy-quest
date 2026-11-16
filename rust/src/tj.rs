use std::f32::consts::PI;

use bevy::prelude::*;

use crate::boss::HeroCosmetic;
use crate::player::Player;
use crate::state::{GameState, PlayerStats};
use crate::ui::HudOverlay;

#[derive(Component)]
pub struct TjCityActor;

#[derive(Component)]
pub struct TjCityBeacon;

#[derive(Component)]
pub struct TjCutsceneEntity;

#[derive(Component)]
pub struct TjCutsceneOverlay;

#[derive(Component)]
pub struct TjSceneBadgeText;

#[derive(Component)]
pub struct TjSpeakerText;

#[derive(Component)]
pub struct TjSubtitleText;

#[derive(Component)]
pub struct TjCutsceneActor;

#[derive(Component)]
pub struct TjLegPivot {
    pub is_left: bool,
}

#[derive(Component)]
pub struct TjAnkleCast;

#[derive(Component)]
pub struct TjCrackFlash;

/// Spawns TJ with a short city-style haircut, trimmed beard, and rectangular glasses.
pub fn spawn_tj_character(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    with_cast: bool,
) -> Entity {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.74, 0.62),
        perceptual_roughness: 0.72,
        ..default()
    });
    let hair_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.11, 0.08),
        perceptual_roughness: 0.85,
        ..default()
    });
    let frame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        metallic: 0.45,
        perceptual_roughness: 0.3,
        ..default()
    });
    let lens_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.68, 0.86, 1.0, 0.34),
        emissive: LinearRgba::new(0.18, 0.32, 0.55, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let shirt_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.28, 0.42),
        perceptual_roughness: 0.82,
        ..default()
    });
    let collar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.90, 0.96),
        perceptual_roughness: 0.75,
        ..default()
    });
    let pants_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.15, 0.19),
        perceptual_roughness: 0.86,
        ..default()
    });
    let shoe_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.18, 0.16),
        perceptual_roughness: 0.65,
        ..default()
    });
    let eye_w = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let eye_p = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.07, 0.10),
        unlit: true,
        ..default()
    });
    let cast_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.96, 0.98),
        perceptual_roughness: 0.9,
        ..default()
    });

    commands
        .spawn((
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
            Visibility::default(),
        ))
        .with_children(|tj| {
            // Torso & Collar
            tj.spawn((
                Mesh3d(meshes.add(Capsule3d::new(0.30, 0.55))),
                MeshMaterial3d(shirt_mat.clone()),
                Transform::from_xyz(0.0, 1.15, 0.0),
            ));
            tj.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.18, 0.10))),
                MeshMaterial3d(collar_mat),
                Transform::from_xyz(0.0, 1.54, 0.0),
            ));

            // Arms
            for sx in [-1.0_f32, 1.0_f32] {
                tj.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.085, 0.56))),
                    MeshMaterial3d(shirt_mat.clone()),
                    Transform::from_xyz(sx * 0.37, 1.18, 0.06)
                        .with_rotation(Quat::from_rotation_x(-0.22) * Quat::from_rotation_z(sx * -0.14)),
                ));
                tj.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.082))),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.41, 0.90, 0.13),
                ));
            }

            // Left & Right Legs (pivoted at hip so TJ can kick his feet onto the table!)
            for (sx, is_left) in [(-0.15_f32, true), (0.15_f32, false)] {
                tj.spawn((
                    TjLegPivot { is_left },
                    Transform::from_xyz(sx, 0.78, 0.0),
                ))
                .with_children(|leg| {
                    leg.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.105, 0.70))),
                        MeshMaterial3d(pants_mat.clone()),
                        Transform::from_xyz(0.0, -0.35, 0.0),
                    ));
                    // Sneaker facing +Z
                    leg.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.17, 0.11, 0.30)))),
                        MeshMaterial3d(shoe_mat.clone()),
                        Transform::from_xyz(0.0, -0.72, 0.06),
                    ));
                    if is_left {
                        leg.spawn((
                            TjAnkleCast,
                            Mesh3d(meshes.add(Cylinder::new(0.125, 0.24))),
                            MeshMaterial3d(cast_mat.clone()),
                            Transform::from_xyz(0.0, -0.60, 0.0),
                            if with_cast {
                                Visibility::Inherited
                            } else {
                                Visibility::Hidden
                            },
                        ));
                    }
                });
            }

            // Head (+Z is forward face)
            tj.spawn((
                Mesh3d(meshes.add(Sphere::new(0.28))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.84, 0.0),
            ));

            // Short city-style haircut: neat top sweep + tapered sides/back
            tj.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.46, 0.15, 0.44)))),
                MeshMaterial3d(hair_mat.clone()),
                Transform::from_xyz(0.0, 2.09, -0.02)
                    .with_rotation(Quat::from_rotation_x(-0.10)),
            ));
            tj.spawn((
                Mesh3d(meshes.add(Sphere::new(0.275))),
                MeshMaterial3d(hair_mat.clone()),
                Transform::from_xyz(0.0, 1.92, -0.08).with_scale(Vec3::new(1.0, 0.85, 0.85)),
            ));

            // Full trimmed beard + mustache
            tj.spawn((
                Mesh3d(meshes.add(Sphere::new(0.24))),
                MeshMaterial3d(hair_mat.clone()),
                Transform::from_xyz(0.0, 1.72, 0.08).with_scale(Vec3::new(0.95, 0.72, 0.85)),
            ));
            tj.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.20, 0.04, 0.08)))),
                MeshMaterial3d(hair_mat),
                Transform::from_xyz(0.0, 1.77, 0.25),
            ));

            // Eyes + Rectangular Glasses
            for sx in [-1.0_f32, 1.0_f32] {
                tj.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.044))),
                    MeshMaterial3d(eye_w.clone()),
                    Transform::from_xyz(sx * 0.095, 1.87, 0.24),
                ));
                tj.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.023))),
                    MeshMaterial3d(eye_p.clone()),
                    Transform::from_xyz(sx * 0.095, 1.87, 0.275),
                ));
                // Glass lens & dark rim
                tj.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.145, 0.095, 0.015)))),
                    MeshMaterial3d(frame_mat.clone()),
                    Transform::from_xyz(sx * 0.10, 1.875, 0.285),
                ));
                tj.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.118, 0.070, 0.018)))),
                    MeshMaterial3d(lens_mat.clone()),
                    Transform::from_xyz(sx * 0.10, 1.875, 0.287),
                ));
                // Temple arms
                tj.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.02, 0.02, 0.22)))),
                    MeshMaterial3d(frame_mat.clone()),
                    Transform::from_xyz(sx * 0.18, 1.88, 0.18),
                ));
            }
            // Glasses nose bridge
            tj.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.06, 0.02, 0.02)))),
                MeshMaterial3d(frame_mat),
                Transform::from_xyz(0.0, 1.885, 0.29),
            ));
        })
        .id()
}

/// Ensures TJ exists in the NYC city map and handles meeting / companion following.
pub fn update_city_tj(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    player_q: Query<(&Transform, &Player), Without<TjCityActor>>,
    mut tj_q: Query<(Entity, &mut Transform), With<TjCityActor>>,
    mut beacon_q: Query<(&mut Visibility, &mut Transform), (With<TjCityBeacon>, Without<TjCityActor>, Without<Player>)>,
    mut cast_q: Query<&mut Visibility, (With<TjAnkleCast>, Without<TjCityBeacon>, Without<TjCityActor>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if tj_q.is_empty() {
        let spawn_pos = Vec3::new(14.0, 0.0, 48.0);
        let tj_ent = spawn_tj_character(
            &mut commands,
            &mut meshes,
            &mut materials,
            spawn_pos,
            0.0,
            1.8,
            stats.tj_recruited,
        );
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.90, 1.0, 0.85),
            emissive: LinearRgba::new(0.4, 2.4, 3.6, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let marker_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.95, 1.0),
            emissive: LinearRgba::new(0.6, 3.0, 4.2, 1.0),
            unlit: true,
            ..default()
        });
        commands.entity(tj_ent).insert(TjCityActor).with_children(|p| {
            p.spawn((
                TjCityBeacon,
                Mesh3d(meshes.add(Torus::new(0.10, 1.25))),
                MeshMaterial3d(ring_mat),
                Transform::from_xyz(0.0, 0.12, 0.0),
                if stats.tj_recruited {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
            ));
            p.spawn((
                TjCityBeacon,
                Mesh3d(meshes.add(Cone::new(0.22, 0.45))),
                MeshMaterial3d(marker_mat),
                Transform::from_xyz(0.0, 2.75, 0.0).with_rotation(Quat::from_rotation_x(PI)),
                if stats.tj_recruited {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
            ));
        });
        return;
    }

    let Ok((p_tf, player)) = player_q.get_single() else {
        return;
    };
    let Ok((_, mut tj_tf)) = tj_q.get_single_mut() else {
        return;
    };

    for (mut vis, mut b_tf) in &mut beacon_q {
        *vis = if stats.tj_recruited {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if !stats.tj_recruited {
            let s = 1.0 + (t * 3.5).sin() * 0.14;
            b_tf.scale = Vec3::splat(s);
            b_tf.rotate_y(dt * 1.8);
        }
    }
    for mut c_vis in &mut cast_q {
        *c_vis = if stats.tj_recruited {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if !stats.tj_recruited {
        let flat = p_tf.translation - tj_tf.translation;
        let dist = Vec2::new(flat.x, flat.z).length();
        if dist > 0.2 {
            tj_tf.rotation = Quat::from_rotation_y(flat.x.atan2(flat.z));
        }
        if dist < 5.5 {
            stats.saved_city_pos = p_tf.translation;
            stats.tj_return_city = true;
            next_state.set(GameState::TjCutscene);
        }
    } else {
        // Companion mode: TJ runs/limps alongside Stan and scales up slightly with him
        let fwd = Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos());
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        let follow_scale = (stats.player_scale * 0.72).clamp(1.65, 9.0);
        let target_pos = Vec3::new(
            p_tf.translation.x + right.x * (follow_scale * 1.65) - fwd.x * (follow_scale * 1.4),
            0.0,
            p_tf.translation.z + right.z * (follow_scale * 1.65) - fwd.z * (follow_scale * 1.4),
        );
        let delta = Vec3::new(
            target_pos.x - tj_tf.translation.x,
            0.0,
            target_pos.z - tj_tf.translation.z,
        );
        let d = delta.length();
        if d > 0.35 {
            let dir = delta / d;
            let spd = (d * 3.4).clamp(7.0, 32.0);
            tj_tf.translation += dir * spd * dt;
            tj_tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
            tj_tf.translation.y = (t * 9.0).sin().abs() * 0.22;
        } else {
            tj_tf.translation.y = 0.0;
        }
        tj_tf.scale = Vec3::splat(follow_scale);
    }
}

fn tj_cutscene_script(
    beat: u8,
) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    match beat {
        1 => (
            "NYC OFFICE 3:00 AM | PATCHING WINDOW 1/7",
            "NARRATOR",
            "\"3:00 AM. An empty Manhattan office during a Linux server patching window. TJ's dual monitors glow in the dark.\"",
            "3:00 AM. An empty Manhattan office during a Linux server patching window. T J's dual monitors glow in the dark.",
            "narrator",
        ),
        2 => (
            "NYC OFFICE 3:00 AM | CONSPIRACY 2/7",
            "TJ",
            "\"Stan, I'm telling you! Look at these patching logs and the farmland deeds... Bill Gates owns John Deere! Wake up!\"",
            "Stan, I'm telling you! Look at these patching logs and the farmland deeds. Bill Gates owns John Deere! Wake up!",
            "tj",
        ),
        3 => (
            "NYC OFFICE 3:00 AM | MONUMENTAL MOMENT 3/7",
            "NARRATOR",
            "\"A monumental, unforgivable moment... TJ casually swings both feet straight up onto the table between them!\"",
            "A monumental, unforgivable moment. T J casually swings both feet straight up onto the table between them.",
            "narrator",
        ),
        4 => (
            "NYC OFFICE 3:00 AM | STAN'S FACE 4/7",
            "STAN (INTERNAL MONOLOGUE)",
            "\"Scenario 1: I let his shoes stay on this table. The John Deere madness spreads straight into production.\"",
            "Scenario 1: I let his shoes stay on this table. The John Deere madness spreads straight into production.",
            "stan_thought",
        ),
        5 => (
            "NYC OFFICE 3:00 AM | THE DECISION 5/7",
            "STAN (INTERNAL MONOLOGUE)",
            "\"Scenario 2: I ask politely and he boots the Linux kernel. Scenario 3: Break TJ's ankle. Restore table honor.\"",
            "Scenario 2: I ask politely and he boots the Linux kernel. Scenario 3: Break T J's ankle. Restore table honor.",
            "stan_thought",
        ),
        6 => (
            "NYC OFFICE 3:00 AM | ANKLE BROKEN! 6/7",
            "TJ (SHOCKED)",
            "\"CRACK!! OW!! WHY DID YOU DO THAT?! YOU BROKE MY ANKLE!! We were just talking about Bill Gates and tractors!!\"",
            "Ow!! Why did you do that?! You broke my ankle!! We were just talking about Bill Gates and tractors!!",
            "tj_upset",
        ),
        _ => (
            "NYC OFFICE 3:00 AM | ALLIANCE FORGED 7/7",
            "TJ (JOINING FORCES)",
            "\"Actually... that snapped me out of it. Lexie is wrecking NYC with pickled garlic! Let's find tendies and stop her together!\"",
            "Actually, that snapped me out of it. Lexie is wrecking N Y C with pickled garlic! Let's find tendies and stop her together!",
            "tj",
        ),
    }
}

/// Spawns TJ's exact 3:00 AM office desk (curved blue cubicle arch, dual Acer monitors w/ Linux terminal + fiery nebula,
/// HP tower, Cisco VoIP phone, wood water bottle, open textbook, notepad) plus the table in between Stan and TJ.
pub fn enter_tj_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    old_q: Query<Entity, With<TjCutsceneEntity>>,
    old_ui_q: Query<Entity, With<TjCutsceneOverlay>>,
    mut cosmetic_q: Query<&mut Visibility, With<HeroCosmetic>>,
) {
    for e in &old_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &old_ui_q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    for mut vis in &mut cosmetic_q {
        *vis = Visibility::Hidden;
    }

    crate::audio::set_touch_playing(false);
    crate::audio::play_cutscene_music();

    let start_t = crate::audio::take_cutscene_start_time();
    stats.cutscene_timer = if start_t >= 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;

    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.08)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.84, 0.90, 1.0),
        brightness: 240.0,
    });

    let base_y = 240.0_f32;
    let carpet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.15, 0.18),
        perceptual_roughness: 0.96,
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.27, 0.25),
        perceptual_roughness: 0.95,
        ..default()
    });
    let blue_arch_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.62, 0.90),
        perceptual_roughness: 0.72,
        ..default()
    });
    let desk_wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.62, 0.44),
        perceptual_roughness: 0.52,
        ..default()
    });
    let table_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.56, 0.44, 0.30),
        perceptual_roughness: 0.48,
        ..default()
    });
    let dark_metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.12),
        metallic: 0.45,
        perceptual_roughness: 0.42,
        ..default()
    });
    let silver_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.75, 0.79),
        metallic: 0.65,
        perceptual_roughness: 0.3,
        ..default()
    });
    let wood_bottle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.54, 0.34, 0.18),
        perceptual_roughness: 0.35,
        ..default()
    });
    let phone_grey = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.34, 0.38),
        perceptual_roughness: 0.55,
        ..default()
    });
    let term_bg = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.06, 0.08),
        emissive: LinearRgba::new(0.08, 0.12, 0.16, 1.0),
        unlit: true,
        ..default()
    });
    let term_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.96, 0.55),
        emissive: LinearRgba::new(0.45, 1.8, 0.85, 1.0),
        unlit: true,
        ..default()
    });
    let term_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.90, 0.96),
        emissive: LinearRgba::new(0.7, 0.8, 0.95, 1.0),
        unlit: true,
        ..default()
    });
    let nebula_orange = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.15),
        emissive: LinearRgba::new(2.2, 0.95, 0.20, 1.0),
        unlit: true,
        ..default()
    });
    let nebula_blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.45, 0.95),
        emissive: LinearRgba::new(0.3, 0.85, 1.9, 1.0),
        unlit: true,
        ..default()
    });
    let light_screen = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.94, 0.98),
        emissive: LinearRgba::new(1.15, 1.25, 1.38, 1.0),
        unlit: true,
        ..default()
    });
    let log_line_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.52, 0.62),
        unlit: true,
        ..default()
    });
    let paper_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.93),
        perceptual_roughness: 0.9,
        ..default()
    });
    let clock_red = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.22, 0.18),
        emissive: LinearRgba::new(3.2, 0.3, 0.2, 1.0),
        unlit: true,
        ..default()
    });

    // Office room floor + cubicle walls + curved blue acoustic topper arch
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(16.0, 0.3, 14.0)))),
        MeshMaterial3d(carpet_mat),
        Transform::from_xyz(0.0, base_y - 0.15, 0.0),
    ));
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(6.4, 2.2, 0.18)))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(0.0, base_y + 1.1, -2.55),
    ));
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.18, 2.2, 3.2)))),
        MeshMaterial3d(wall_mat),
        Transform::from_xyz(-3.1, base_y + 1.1, -1.05),
    ));
    // Signature curved sky-blue acoustic arch behind TJ's dual monitors (like the photo)
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cylinder::new(2.2, 0.12))),
        MeshMaterial3d(blue_arch_mat),
        Transform::from_xyz(0.25, base_y + 1.78, -2.42)
            .with_rotation(Quat::from_rotation_x(PI * 0.5))
            .with_scale(Vec3::new(1.18, 1.0, 0.52)),
    ));

    // TJ's Light-Wood Desk at z = -1.65 facing +Z
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(5.2, 0.10, 1.70)))),
        MeshMaterial3d(desk_wood),
        Transform::from_xyz(0.0, base_y + 1.05, -1.65),
    ));
    // Wood-grain bottle (left), HP Desktop Tower, Caddy, Cisco VoIP Phone
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cylinder::new(0.10, 0.56))),
        MeshMaterial3d(wood_bottle),
        Transform::from_xyz(-2.25, base_y + 1.38, -1.85),
    ));
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cylinder::new(0.075, 0.08))),
        MeshMaterial3d(silver_mat.clone()),
        Transform::from_xyz(-2.25, base_y + 1.68, -1.85),
    ));
    // Black HP Desktop Tower
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.42, 0.88, 0.72)))),
        MeshMaterial3d(dark_metal.clone()),
        Transform::from_xyz(-1.72, base_y + 1.54, -1.92),
    ));
    // Cisco VoIP Desk Phone
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.42, 0.14, 0.36)))),
        MeshMaterial3d(phone_grey),
        Transform::from_xyz(-1.08, base_y + 1.18, -1.88)
            .with_rotation(Quat::from_rotation_x(0.28)),
    ));

    // Left Monitor: Dark Linux Terminal (left half) + Fiery Nebula Wallpaper (right half)
    for mx in [-0.24_f32, 1.08_f32] {
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(1.18, 0.74, 0.06)))),
            MeshMaterial3d(dark_metal.clone()),
            Transform::from_xyz(mx, base_y + 1.62, -1.95),
        ));
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cylinder::new(0.04, 0.24))),
            MeshMaterial3d(dark_metal.clone()),
            Transform::from_xyz(mx, base_y + 1.22, -1.98),
        ));
    }
    // Left screen panel: Linux terminal + nebula art
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.55, 0.63, 0.02)))),
        MeshMaterial3d(term_bg),
        Transform::from_xyz(-0.52, base_y + 1.62, -1.915),
    ));
    for row in 0..8 {
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(
                if row % 2 == 0 { 0.44 } else { 0.34 },
                0.024,
                0.025,
            )))),
            MeshMaterial3d(if row % 3 == 0 {
                term_green.clone()
            } else {
                term_white.clone()
            }),
            Transform::from_xyz(-0.54, base_y + 1.84 - (row as f32) * 0.062, -1.91),
        ));
    }
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.54, 0.63, 0.02)))),
        MeshMaterial3d(nebula_blue),
        Transform::from_xyz(0.04, base_y + 1.62, -1.915),
    ));
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Sphere::new(0.22))),
        MeshMaterial3d(nebula_orange),
        Transform::from_xyz(0.08, base_y + 1.60, -1.91).with_scale(Vec3::new(0.9, 1.2, 0.1)),
    ));

    // Right Monitor: Light-theme patching log screen
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(1.08, 0.63, 0.02)))),
        MeshMaterial3d(light_screen),
        Transform::from_xyz(1.08, base_y + 1.62, -1.915),
    ));
    for row in 0..7 {
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.88, 0.022, 0.025)))),
            MeshMaterial3d(log_line_mat.clone()),
            Transform::from_xyz(1.06, base_y + 1.84 - (row as f32) * 0.068, -1.91),
        ));
    }

    // Keyboard, Open Textbook, Mouse & Notepad
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.92, 0.04, 0.30)))),
        MeshMaterial3d(dark_metal.clone()),
        Transform::from_xyz(-0.08, base_y + 1.12, -1.42),
    ));
    // Open Book
    for bx in [-0.52_f32, -0.18_f32] {
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.32, 0.05, 0.44)))),
            MeshMaterial3d(paper_white.clone()),
            Transform::from_xyz(bx, base_y + 1.15, -1.16)
                .with_rotation(Quat::from_rotation_z(if bx < -0.35 { 0.12 } else { -0.12 })),
        ));
    }
    // Notepad & Pen on right
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.50, 0.04, 0.42)))),
        MeshMaterial3d(paper_white),
        Transform::from_xyz(1.65, base_y + 1.13, -1.24),
    ));
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cylinder::new(0.018, 0.32))),
        MeshMaterial3d(dark_metal),
        Transform::from_xyz(1.65, base_y + 1.17, -1.22)
            .with_rotation(Quat::from_rotation_z(PI * 0.5) * Quat::from_rotation_y(0.3)),
    ));

    // Digital 3:00 AM indicator light block on cubicle divider
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.68, 0.18, 0.06)))),
        MeshMaterial3d(clock_red),
        Transform::from_xyz(-2.15, base_y + 2.02, -2.42),
    ));

    // THE TABLE IN BETWEEN TJ AND STAN (z = 1.15)
    commands.spawn((
        TjCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(2.70, 0.12, 1.30)))),
        MeshMaterial3d(table_mat.clone()),
        Transform::from_xyz(0.0, base_y + 0.92, 1.15),
    ));
    for (lx, lz) in [(-1.15, 0.60), (1.15, 0.60), (-1.15, 1.70), (1.15, 1.70)] {
        commands.spawn((
            TjCutsceneEntity,
            Mesh3d(meshes.add(Cylinder::new(0.06, 0.92))),
            MeshMaterial3d(table_mat.clone()),
            Transform::from_xyz(lx, base_y + 0.46, lz),
        ));
    }

    // Office moody lighting (cool overhead + warm monitor glow)
    commands.spawn((
        TjCutsceneEntity,
        PointLight {
            color: Color::srgb(0.68, 0.84, 1.0),
            intensity: 110_000.0,
            range: 18.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.2, base_y + 3.8, 0.5),
    ));
    commands.spawn((
        TjCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.72, 0.40),
            intensity: 48_000.0,
            range: 9.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.2, base_y + 2.1, -1.2),
    ));

    // Crack flash orb (shown briefly on Beat 6 ankle break)
    let flash_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.92, 0.45, 0.92),
        emissive: LinearRgba::new(6.0, 3.5, 0.4, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        TjCutsceneEntity,
        TjCrackFlash,
        Mesh3d(meshes.add(Sphere::new(0.32))),
        MeshMaterial3d(flash_mat),
        Transform::from_xyz(-0.1, base_y + 1.05, 0.95),
        Visibility::Hidden,
    ));

    // Spawn TJ on the -Z side of the table facing +Z toward Stan
    let tj_ent = spawn_tj_character(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y, 0.02),
        0.0,
        1.15,
        false,
    );
    commands.entity(tj_ent).insert((TjCutsceneEntity, TjCutsceneActor));

    // Position Stan on the +Z side of the table facing -Z toward TJ
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(0.0, base_y, 2.38);
        p_tf.rotation = Quat::IDENTITY;
        p_tf.scale = Vec3::splat(1.22);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0;
    }

    // Cinematic bottom subtitle box
    let (badge, speaker, sub, _, _) = tj_cutscene_script(1);
    commands
        .spawn((
            TjCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                left: Val::Percent(6.0),
                width: Val::Percent(88.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.09, 0.92)),
            BorderColor(Color::srgb(0.28, 0.72, 1.0)),
            BorderRadius::all(Val::Px(12.0)),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        TjSpeakerText,
                        Text::new(speaker),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));
                    hdr.spawn((
                        TjSceneBadgeText,
                        Text::new(badge),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.78, 0.30)),
                    ));
                });
            panel.spawn((
                TjSubtitleText,
                Text::new(sub),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub fn update_tj_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_q: Query<&mut Transform, (With<Player>, Without<Camera3d>, Without<TjCutsceneActor>, Without<TjCrackFlash>)>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<TjCutsceneActor>, Without<TjCrackFlash>)>,
    mut leg_q: Query<(&TjLegPivot, &mut Transform), (Without<Camera3d>, Without<Player>, Without<TjCrackFlash>)>,
    mut cast_q: Query<&mut Visibility, (With<TjAnkleCast>, Without<TjCrackFlash>)>,
    mut flash_q: Query<(&mut Visibility, &mut Transform), (With<TjCrackFlash>, Without<Camera3d>, Without<Player>, Without<TjLegPivot>)>,
    mut badge_q: Query<&mut Text, With<TjSceneBadgeText>>,
    mut spk_q: Query<&mut Text, (With<TjSpeakerText>, Without<TjSceneBadgeText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<TjSubtitleText>,
            Without<TjSceneBadgeText>,
            Without<TjSpeakerText>,
        ),
    >,
) {
    const BEAT_ENDS: [f32; 7] = [6.8, 13.8, 20.8, 28.0, 35.2, 41.8, 49.0];
    let dt = time.delta_secs().min(0.05);
    let mut t = stats.cutscene_timer + dt;

    let mut target_beat: u8 = 8;
    for (idx, &end_t) in BEAT_ENDS.iter().enumerate() {
        if t < end_t {
            target_beat = (idx as u8) + 1;
            break;
        }
    }

    if stats.cutscene_beat > 0 && target_beat > stats.cutscene_beat {
        if crate::audio::is_voice_speaking() {
            let hold_idx = (stats.cutscene_beat.saturating_sub(1) as usize).min(6);
            t = BEAT_ENDS[hold_idx] - 0.01;
            target_beat = stats.cutscene_beat;
        } else if target_beat == 8 {
            stats.cutscene_timer = t;
            stats.tj_recruited = true;
            next_state.set(GameState::CityPlay);
            return;
        }
    } else if target_beat == 8 && !crate::audio::is_voice_speaking() {
        stats.cutscene_timer = t;
        stats.tj_recruited = true;
        next_state.set(GameState::CityPlay);
        return;
    }

    stats.cutscene_timer = t;

    if target_beat != stats.cutscene_beat && target_beat <= 7 {
        stats.cutscene_beat = target_beat;
        let (badge, speaker, sub, voice, role) = tj_cutscene_script(target_beat);
        if let Ok(mut b) = badge_q.get_single_mut() {
            b.0 = badge.to_string();
        }
        if let Ok(mut sp) = spk_q.get_single_mut() {
            sp.0 = speaker.to_string();
        }
        if let Ok(mut sb) = sub_q.get_single_mut() {
            sb.0 = sub.to_string();
        }
        if target_beat == 6 {
            crate::audio::play_sfx("hit");
        }
        crate::audio::speak_voice(voice, role);
    }

    let base_y = 240.0_f32;
    let beat = stats.cutscene_beat.max(1);

    // Animate TJ putting his feet up on the table (Beats 3..5) and snapping down on Beat 6!
    let feet_up = matches!(beat, 3 | 4 | 5);
    let target_leg_pitch = if feet_up { -1.32 } else { 0.0 };
    for (leg, mut l_tf) in &mut leg_q {
        let side_splay = if feet_up {
            if leg.is_left { -0.10 } else { 0.10 }
        } else {
            0.0
        };
        let target_rot = Quat::from_rotation_x(target_leg_pitch) * Quat::from_rotation_z(side_splay);
        l_tf.rotation = l_tf.rotation.slerp(target_rot, (dt * 9.0).min(1.0));
    }

    // Show ankle cast once broken (Beat 6 & 7)
    for mut c_vis in &mut cast_q {
        *c_vis = if beat >= 6 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Flash on Beat 6 ankle break
    for (mut f_vis, mut f_tf) in &mut flash_q {
        if beat == 6 && t < 36.2 {
            *f_vis = Visibility::Inherited;
            let pulse = 1.0 + ((t - 35.2) * 8.0).sin().abs() * 1.4;
            f_tf.scale = Vec3::splat(pulse);
        } else {
            *f_vis = Visibility::Hidden;
        }
    }

    // Stan lunge on Beat 6
    if let Ok(mut p_tf) = player_q.get_single_mut() {
        let target_z = if beat == 6 && t < 36.5 {
            1.65
        } else if beat >= 7 {
            1.95
        } else {
            2.38
        };
        p_tf.translation.z += (target_z - p_tf.translation.z) * (dt * 10.0).min(1.0);
    }

    // Dramatic Cinematography: Desk/Linux Terminal Pan -> Table/Feet Zoom -> Stan Face Hyper-Zoom -> Ankle Break
    if let Ok(mut c_tf) = cam_q.get_single_mut() {
        let (cam_pos, look_at) = match beat {
            1 => {
                // Dramatic pan from wide 3 AM cubicle onto TJ's Linux terminal & dual monitors
                let u = (t / 6.8).clamp(0.0, 1.0);
                (
                    Vec3::new(-2.4 + u * 2.15, base_y + 2.55 - u * 0.72, 1.6 - u * 2.25),
                    Vec3::new(-0.35 + u * 0.05, base_y + 1.62, -1.92),
                )
            }
            2 => {
                // Framing TJ & Stan across the table while discussing Bill Gates / John Deere
                let u = ((t - 6.8) / 7.0).clamp(0.0, 1.0);
                (
                    Vec3::new(2.75 - u * 0.55, base_y + 2.25, 2.35),
                    Vec3::new(0.0, base_y + 1.50, 0.75),
                )
            }
            3 => {
                // Monumental Zoom-In directly onto the table with TJ's feet resting on top!
                let u = ((t - 13.8) / 7.0).clamp(0.0, 1.0);
                (
                    Vec3::new(1.55 - u * 0.75, base_y + 1.65 - u * 0.28, 2.05 - u * 0.55),
                    Vec3::new(0.0, base_y + 1.06, 0.92),
                )
            }
            4 => {
                // Hyper-Detailed Zoom-In on Stan's face (front +Z looking -Z, so camera at z ~ 1.25 looking at z = 2.38)
                let u = ((t - 20.8) / 7.2).clamp(0.0, 1.0);
                (
                    Vec3::new(0.18 - u * 0.14, base_y + 2.15, 1.05 + u * 0.25),
                    Vec3::new(0.0, base_y + 2.12, 2.22),
                )
            }
            5 => {
                // Even tighter push onto Stan's eyes & brow during Scenario 2 & 3
                let u = ((t - 28.0) / 7.2).clamp(0.0, 1.0);
                (
                    Vec3::new(-0.08 + u * 0.08, base_y + 2.14, 1.25 + u * 0.18),
                    Vec3::new(0.0, base_y + 2.13, 2.24),
                )
            }
            6 => {
                // Ankle break impact shake!
                let u = ((t - 35.2) / 6.6).clamp(0.0, 1.0);
                let shake = if u < 0.25 { (t * 42.0).sin() * 0.12 } else { 0.0 };
                (
                    Vec3::new(1.65 + shake, base_y + 1.45 + shake * 0.5, 1.85),
                    Vec3::new(0.0, base_y + 1.00, 0.55),
                )
            }
            _ => {
                // Alliance forged wide shot
                let u = ((t - 41.8) / 7.2).clamp(0.0, 1.0);
                (
                    Vec3::new(-2.4 + u * 0.8, base_y + 2.10, 3.35),
                    Vec3::new(0.0, base_y + 1.45, 0.85),
                )
            }
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, Vec3::Y);
    }
}

pub fn exit_tj_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<TjCutsceneEntity>>,
    ui_q: Query<Entity, With<TjCutsceneOverlay>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut cosmetic_q: Query<&mut Visibility, With<HeroCosmetic>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
    for mut vis in &mut cosmetic_q {
        *vis = Visibility::Inherited;
    }
    stats.tj_recruited = true;
}
