use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, speak_voice,
};
use crate::city::{pub217_scale, CityWorldMarker, PUB217_ANCHOR};
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

#[derive(Component)]
pub struct CityDerek {
    pub was_inside: bool,
}

#[derive(Component)]
pub struct DerekCutsceneEntity;

#[derive(Component)]
pub struct DerekCutsceneOverlay;

#[derive(Component)]
pub struct DerekSpeakerText;

#[derive(Component)]
pub struct DerekBadgeText;

#[derive(Component)]
pub struct DerekSubtitleText;

#[derive(Component)]
pub struct DerekCutsceneActor {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct DerekTearDrop {
    pub base_pos: Vec3,
    pub phase: f32,
    pub min_beat: u8,
}

#[derive(Component)]
pub struct DerekClashSpark {
    pub angle: f32,
    pub radius: f32,
}

#[derive(Component)]
pub struct DerekArmPivot {
    pub is_left: bool,
    pub is_stan: bool,
}

#[derive(Resource, Default)]
pub struct DerekCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
    pub fight_sfx_played: bool,
}

struct DerekMaterials {
    skin: Handle<StandardMaterial>,
    hoodie: Handle<StandardMaterial>,
    hoodie_dark: Handle<StandardMaterial>,
    beard: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    pants: Handle<StandardMaterial>,
    shoe: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
    tear: Handle<StandardMaterial>,
}

fn make_derek_materials(materials: &mut Assets<StandardMaterial>) -> DerekMaterials {
    DerekMaterials {
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.76, 0.66),
            perceptual_roughness: 0.65,
            ..default()
        }),
        hoodie: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.25, 0.28),
            perceptual_roughness: 0.85,
            ..default()
        }),
        hoodie_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.16, 0.18),
            perceptual_roughness: 0.9,
            ..default()
        }),
        beard: materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.08, 0.06),
            perceptual_roughness: 0.92,
            ..default()
        }),
        hair: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.10, 0.07),
            perceptual_roughness: 0.88,
            ..default()
        }),
        pants: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.14, 0.20),
            perceptual_roughness: 0.85,
            ..default()
        }),
        shoe: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.09),
            perceptual_roughness: 0.75,
            ..default()
        }),
        eye_white: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.3,
            ..default()
        }),
        eye_pupil: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.09, 0.08),
            perceptual_roughness: 0.2,
            ..default()
        }),
        tear: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.86, 1.0, 0.88),
            emissive: LinearRgba::new(0.18, 0.45, 0.75, 1.0),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        }),
    }
}

fn spawn_derek_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &DerekMaterials,
    is_cutscene: bool,
) {
    // Legs & boots
    for sx in [-0.14_f32, 0.14] {
        parent.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.11, 0.48))),
            MeshMaterial3d(mats.pants.clone()),
            Transform::from_xyz(sx, 0.36, 0.0),
        ));
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.20, 0.12, 0.30))),
            MeshMaterial3d(mats.shoe.clone()),
            Transform::from_xyz(sx, 0.06, 0.04),
        ));
    }

    // Broad dark-grey hoodie torso
    parent.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.68, 0.82, 0.40))),
        MeshMaterial3d(mats.hoodie.clone()),
        Transform::from_xyz(0.0, 1.05, 0.0),
    ));
    // Hoodie collar / hood bunched behind neck
    parent.spawn((
        Mesh3d(meshes.add(Torus::new(0.09, 0.24))),
        MeshMaterial3d(mats.hoodie_dark.clone()),
        Transform::from_xyz(0.0, 1.44, -0.04).with_rotation(Quat::from_rotation_x(0.35)),
    ));
    // Hoodie drawstrings
    for sx in [-0.07_f32, 0.07] {
        parent.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.013, 0.32))),
            MeshMaterial3d(mats.hoodie_dark.clone()),
            Transform::from_xyz(sx, 1.24, 0.21),
        ));
    }

    // Arms (pivoted if cutscene so Derek can clash/swing!)
    for (idx, sx) in [-0.44_f32, 0.44].iter().enumerate() {
        let mut arm = parent.spawn((
            Transform::from_xyz(*sx, 1.38, 0.0),
            Visibility::default(),
        ));
        if is_cutscene {
            arm.insert(DerekArmPivot {
                is_left: idx == 0,
                is_stan: false,
            });
        }
        arm.with_children(|a| {
            a.spawn((
                Mesh3d(meshes.add(Capsule3d::new(0.10, 0.46))),
                MeshMaterial3d(mats.hoodie.clone()),
                Transform::from_xyz(0.0, -0.26, 0.0),
            ));
            a.spawn((
                Mesh3d(meshes.add(Sphere::new(0.10))),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, -0.56, 0.02),
            ));
        });
    }

    // Head
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.28))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.76, 0.0),
    ));

    // Pulled-back dark brown hair + back bun
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.275))),
        MeshMaterial3d(mats.hair.clone()),
        Transform::from_xyz(0.0, 1.82, -0.05).with_scale(Vec3::new(0.96, 0.85, 0.98)),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.11))),
        MeshMaterial3d(mats.hair.clone()),
        Transform::from_xyz(0.0, 1.80, -0.32),
    ));

    // Thick full dark beard & mustache (matching Derek's photo)
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.265))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.63, 0.07).with_scale(Vec3::new(0.96, 0.92, 0.88)),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.24, 0.065, 0.09))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.69, 0.25),
    ));

    // Nose & brows
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.052))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.75, 0.28),
    ));
    for sx in [-0.095_f32, 0.095] {
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.10, 0.026, 0.04))),
            MeshMaterial3d(mats.beard.clone()),
            Transform::from_xyz(sx, 1.86, 0.245)
                .with_rotation(Quat::from_rotation_z(if sx < 0.0 { -0.14 } else { 0.14 })),
        ));
        // Eyes
        parent.spawn((
            Mesh3d(meshes.add(Sphere::new(0.042))),
            MeshMaterial3d(mats.eye_white.clone()),
            Transform::from_xyz(sx, 1.80, 0.242),
        ));
        parent.spawn((
            Mesh3d(meshes.add(Sphere::new(0.024))),
            MeshMaterial3d(mats.eye_pupil.clone()),
            Transform::from_xyz(sx, 1.80, 0.268),
        ));
    }
}

pub fn spawn_city_derek(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mats = make_derek_materials(materials);
    let pos = PUB217_ANCHOR + Vec3::new(-9.6, 0.0, -3.4);
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityDerek { was_inside: false },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(0.65)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_derek_rig(p, meshes, &mats, false);
        });
}

pub fn update_city_derek(
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<PlayerStats>,
    player_q: Query<&Transform, (With<Player>, Without<CityDerek>)>,
    mut derek_q: Query<(&mut Transform, &mut CityDerek), Without<Player>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut d_state: ResMut<DerekCutsceneState>,
) {
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let pub_s = pub217_scale(stats.player_scale);
    for (mut d_tf, mut derek) in &mut derek_q {
        d_tf.translation = PUB217_ANCHOR + Vec3::new(-9.6, 0.0, -3.4) * pub_s;
        d_tf.scale = Vec3::splat(pub_s);
        let to_p = p_tf.translation - d_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);
        if flat.length_squared() > 0.01 {
            d_tf.rotation = Quat::from_rotation_y(flat.x.atan2(flat.z));
        }

        let dist = flat.length();
        let trigger_r = 2.2 * pub_s;
        let inside = dist < trigger_r;
        let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

        if stats.driving_car.is_none()
            && stats.riding_scooter.is_none()
            && ((inside && !derek.was_inside) || (dist < 4.5 * pub_s && interact))
        {
            derek.was_inside = true;
            *d_state = DerekCutsceneState::default();
            next_state.set(GameState::DerekCutscene);
            return;
        }
        derek.was_inside = inside;
    }
}

const DEREK_BEATS: [(&str, &str, &str, &str); 6] = [
    (
        "DEREK (FORMER ARCH-RIVAL)",
        "BEAT 1/6  |  TEARS BY THE BEER TOWER",
        "Stan... I swore the next time we met under these ceiling tiles, one of us wouldn't walk out. Remember the 1990 Ninja Turtles arcade cabinet? You stole my Michelangelo quarter and knocked over a six-foot beer tower!",
        "tj",
    ),
    (
        "STAN",
        "BEAT 2/6  |  CONFESSION IN TEARS",
        "I didn't mean to spill the Donatello pitcher, Derek! Leonardo taught us honor, Raphael taught us fury... and every duel since has left nothing but broken plastic cups and shame!",
        "stan",
    ),
    (
        "DEREK & STAN (DUEL CLASH!)",
        "BEAT 3/6  |  ONE LAST EXCHANGE",
        "Ten years of blood, sweat, and shattered beer towers! Cowabunga was never meant to end like this—TAKE MY SPLINTER-STYLE HAYMAKER!",
        "tj",
    ),
    (
        "STAN (LOCKED IN CLASH)",
        "BEAT 4/6  |  THE EXCEPTION",
        "Wait—look outside! Lexie's pickled garlic is drowning Cedar Rapids! Even Splinter wouldn't let Shredder wreck Pub 217! Today we make an exception!",
        "stan",
    ),
    (
        "DEREK (TEARS OF BROTHERHOOD)",
        "BEAT 5/6  |  THE BEER TOWER TRUCE",
        "Heh... four brothers, one pizza, one impossible beer tower. For tonight only, Stan... our duel waits. Go take down Lexie—and when you get back, the next tower is on me!",
        "tj",
    ),
    (
        "STAN & DEREK",
        "BEAT 6/6  |  COWABUNGA, BROTHER",
        "Cowabunga, brother! Cedar Rapids stands—and when the dust settles, we finish that beer tower together!",
        "stan",
    ),
];

pub fn enter_derek_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut d_state: ResMut<DerekCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    *d_state = DerekCutsceneState::default();
    stats.cutscene_timer = 0.0;
    stats.cutscene_beat = 0;

    let stage = Vec3::new(0.0, 0.0, -310.0);
    let mats = make_derek_materials(&mut materials);

    // Dramatic dark Pub 217 floor & ceiling tiles
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.06, 0.05),
        perceptual_roughness: 0.75,
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.06, 0.08),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        DerekCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 0.2, 16.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        DerekCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 0.2, 16.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 5.2, 0.0)),
    ));

    // Glowing Beer Tower Pyramid in the background between them
    let beer_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.68, 0.12, 0.90),
        emissive: LinearRgba::new(0.85, 0.52, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let rim_silver = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.85, 0.90),
        metallic: 0.8,
        perceptual_roughness: 0.25,
        ..default()
    });
    for tier in 0..5 {
        let count = 5 - tier;
        let y = 0.45 + tier as f32 * 0.78;
        for c in 0..count {
            let x_off = -2.4 + (c as f32 - (count - 1) as f32 * 0.5) * 0.42;
            commands.spawn((
                DerekCutsceneEntity,
                Mesh3d(meshes.add(Cylinder::new(0.16, 0.72))),
                MeshMaterial3d(beer_gold.clone()),
                Transform::from_translation(stage + Vec3::new(x_off, y, -2.9)),
            ));
            commands.spawn((
                DerekCutsceneEntity,
                Mesh3d(meshes.add(Cylinder::new(0.17, 0.05))),
                MeshMaterial3d(rim_silver.clone()),
                Transform::from_translation(stage + Vec3::new(x_off, y + 0.36, -2.9)),
            ));
        }
    }

    // Dramatic warm spotlight
    commands.spawn((
        DerekCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.65),
            intensity: 260_000.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 4.2, 0.5)),
    ));

    // Spawn Derek actor at stage -Z facing +Z
    commands
        .spawn((
            DerekCutsceneEntity,
            DerekCutsceneActor { is_stan: false },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.55)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_derek_rig(p, &mut meshes, &mats, true);
        });

    // Spawn Stan actor at stage +Z facing -Z
    let stan_plaid = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.14, 0.16),
        perceptual_roughness: 0.75,
        ..default()
    });
    let stan_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.82, 0.22),
        metallic: 0.65,
        ..default()
    });
    let stan_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.15, 0.65),
        perceptual_roughness: 0.45,
        ..default()
    });
    commands
        .spawn((
            DerekCutsceneEntity,
            DerekCutsceneActor { is_stan: true },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.55))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
        ))
        .with_children(|p| {
            for sx in [-0.14_f32, 0.14] {
                p.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.45))),
                    MeshMaterial3d(stan_plaid.clone()),
                    Transform::from_xyz(sx, 0.36, 0.0),
                ));
            }
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.34, 0.78))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 1.04, 0.0),
            ));
            // Vikings belt
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.355, 0.12))),
                MeshMaterial3d(stan_gold.clone()),
                Transform::from_xyz(0.0, 0.82, 0.0),
            ));
            for (idx, sx) in [-0.42_f32, 0.42].iter().enumerate() {
                p.spawn((
                    DerekArmPivot {
                        is_left: idx == 0,
                        is_stan: true,
                    },
                    Transform::from_xyz(*sx, 1.32, 0.0),
                    Visibility::default(),
                ))
                .with_children(|a| {
                    a.spawn((
                        Mesh3d(meshes.add(Capsule3d::new(0.09, 0.42))),
                        MeshMaterial3d(stan_plaid.clone()),
                        Transform::from_xyz(0.0, -0.24, 0.0),
                    ));
                    if idx == 1 {
                        a.spawn((
                            Mesh3d(meshes.add(Cylinder::new(0.04, 0.95))),
                            MeshMaterial3d(stan_purple.clone()),
                            Transform::from_xyz(0.0, -0.52, 0.35)
                                .with_rotation(Quat::from_rotation_x(1.1)),
                        ));
                    }
                });
            }
            // Stan head + beard + Santa/Viking cap
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.27))),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(mats.hair.clone()),
                Transform::from_xyz(0.0, 1.60, 0.07).with_scale(Vec3::new(0.95, 0.88, 0.85)),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cone::new(0.24, 0.36))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 2.02, -0.02),
            ));
            for sx in [-0.09_f32, 0.09] {
                p.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.04))),
                    MeshMaterial3d(mats.eye_white.clone()),
                    Transform::from_xyz(sx, 1.76, 0.235),
                ));
                p.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.022))),
                    MeshMaterial3d(mats.eye_pupil.clone()),
                    Transform::from_xyz(sx, 1.76, 0.258),
                ));
            }
        });

    // Animated 3D teardrops on Derek's & Stan's cheeks
    let tear_mesh = meshes.add(Sphere::new(0.022));
    for (i, sx) in [-0.095_f32, 0.095].iter().enumerate() {
        commands.spawn((
            DerekCutsceneEntity,
            DerekTearDrop {
                base_pos: stage + Vec3::new(*sx, 1.76, -1.29),
                phase: i as f32 * 0.65,
                min_beat: 0,
            },
            Mesh3d(tear_mesh.clone()),
            MeshMaterial3d(mats.tear.clone()),
            Transform::from_translation(stage + Vec3::new(*sx, 1.76, -1.29)),
        ));
        commands.spawn((
            DerekCutsceneEntity,
            DerekTearDrop {
                base_pos: stage + Vec3::new(*sx, 1.72, 1.30),
                phase: i as f32 * 0.55 + 0.3,
                min_beat: 1,
            },
            Mesh3d(tear_mesh.clone()),
            MeshMaterial3d(mats.tear.clone()),
            Transform::from_translation(stage + Vec3::new(*sx, 1.72, 1.30)),
            Visibility::Hidden,
        ));
    }

    // Anime clash sparks for Beat 2 & 3
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.25),
        emissive: LinearRgba::new(4.0, 2.8, 0.4, 1.0),
        ..default()
    });
    let spark_mesh = meshes.add(Cuboid::new(0.05, 0.05, 0.38));
    for i in 0..10 {
        let ang = (i as f32 / 10.0) * std::f32::consts::TAU;
        commands.spawn((
            DerekCutsceneEntity,
            DerekClashSpark {
                angle: ang,
                radius: 0.45 + (i % 3) as f32 * 0.25,
            },
            Mesh3d(spark_mesh.clone()),
            MeshMaterial3d(spark_mat.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.35, 0.0)),
            Visibility::Hidden,
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.45, 1.72, -0.72);
        c_tf.look_at(stage + Vec3::new(0.0, 1.79, -1.55), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.56;
        }
    }

    let (spk, badge, line, voice) = DEREK_BEATS[0];
    commands
        .spawn((
            DerekCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
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
                    max_width: Val::Px(820.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.05, 0.11, 0.88)),
                BorderColor(Color::srgb(0.95, 0.72, 0.22)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        DerekSpeakerText,
                        Text::new(spk),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.80, 0.28)),
                    ));
                    hdr.spawn((
                        DerekBadgeText,
                        Text::new(format!("{}  |  [SPACE / CLICK] Skip", badge)),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.65, 0.85, 1.0)),
                    ));
                });
                card.spawn((
                    DerekSubtitleText,
                    Text::new(format!("\"{}\"", line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    play_cutscene_music();
    play_sfx("thunder");
    speak_voice(line, voice);
}

pub fn update_derek_cutscene(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut d_state: ResMut<DerekCutsceneState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut actor_q: Query<(&mut Transform, &DerekCutsceneActor), Without<Camera3d>>,
    mut arm_q: Query<(&mut Transform, &DerekArmPivot), (Without<DerekCutsceneActor>, Without<Camera3d>)>,
    mut tear_q: Query<(&mut Transform, &mut Visibility, &DerekTearDrop), (Without<DerekCutsceneActor>, Without<DerekArmPivot>, Without<Camera3d>)>,
    mut spark_q: Query<(&mut Transform, &mut Visibility, &DerekClashSpark), (Without<DerekCutsceneActor>, Without<DerekArmPivot>, Without<DerekTearDrop>, Without<Camera3d>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<DerekSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<DerekBadgeText>, Without<DerekSpeakerText>)>,
    mut sub_q: Query<&mut Text, (With<DerekSubtitleText>, Without<DerekSpeakerText>, Without<DerekBadgeText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    d_state.beat_t += dt;

    if keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Escape)
        || mouse_buttons.just_pressed(MouseButton::Left)
    {
        next_state.set(GameState::CityPlay);
        return;
    }

    if d_state.beat_t >= 4.0 && (!is_voice_speaking() || d_state.beat_t >= 18.0) {
        if d_state.beat + 1 >= DEREK_BEATS.len() as u8 {
            next_state.set(GameState::CityPlay);
            return;
        }
        d_state.beat += 1;
        d_state.beat_t = 0.0;
        let (spk, badge, line, voice) = DEREK_BEATS[d_state.beat as usize];
        if d_state.beat == 2 {
            play_sfx("sword_clash");
        } else if d_state.beat == 4 {
            play_sfx("thunder");
        }
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut b) = badge_q.get_single_mut() {
            b.0 = format!("{}  |  [SPACE / CLICK] Skip", badge);
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
    }

    let beat = d_state.beat;
    let bt = d_state.beat_t;
    let stage = Vec3::new(0.0, 0.0, -310.0);

    // Animate actor positions (close clash in beats 2-3, truce stand side-by-side in beat 5)
    for (mut tf, act) in &mut actor_q {
        if beat == 2 || beat == 3 {
            let clash_z = if beat == 2 {
                0.55 + (bt * 9.0).sin().abs() * 0.25
            } else {
                0.52
            };
            tf.translation = stage + Vec3::new(0.0, 0.0, if act.is_stan { clash_z } else { -clash_z });
            tf.rotation = Quat::from_rotation_y(if act.is_stan { std::f32::consts::PI } else { 0.0 });
        } else if beat == 5 {
            tf.translation = stage + Vec3::new(if act.is_stan { 0.72 } else { -0.72 }, 0.0, 0.4);
            tf.rotation = Quat::from_rotation_y(0.0);
        } else {
            tf.translation = stage + Vec3::new(0.0, 0.0, if act.is_stan { 1.55 } else { -1.55 });
            tf.rotation = Quat::from_rotation_y(if act.is_stan { std::f32::consts::PI } else { 0.0 });
        }
    }

    // Animate fighting arms during beats 2-3
    for (mut atf, piv) in &mut arm_q {
        if beat == 2 {
            let swing = (bt * 11.0 + if piv.is_left { 0.0 } else { 1.8 }).sin() * 1.15;
            atf.rotation = Quat::from_rotation_x(-1.1 + swing);
        } else if beat == 3 {
            atf.rotation = Quat::from_rotation_x(-1.35 + (bt * 4.0).sin() * 0.12);
        } else {
            atf.rotation = Quat::IDENTITY;
        }
    }

    // Animate glistening teardrops streaming down cheeks
    for (mut ttf, mut vis, tear) in &mut tear_q {
        if beat >= tear.min_beat && beat != 2 {
            *vis = Visibility::Visible;
            let fall = ((bt * 0.42 + tear.phase) % 1.0) * 0.14;
            ttf.translation = tear.base_pos - Vec3::Y * fall;
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Animate clash sparks in beats 2-3
    for (mut stf, mut vis, sp) in &mut spark_q {
        if beat == 2 || beat == 3 {
            *vis = Visibility::Visible;
            let pulse = ((bt * 7.5 + sp.angle) % 1.0) * sp.radius;
            let a = sp.angle + bt * 3.5;
            stf.translation = stage + Vec3::new(a.cos() * pulse, 1.35 + a.sin() * pulse * 0.7, 0.0);
            stf.rotation = Quat::from_rotation_z(a);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Dramatic close-up & fighting camera angles
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match beat {
            0 => {
                // Extreme low-angle close-up on Derek's face & tears
                c_tf.translation = stage + Vec3::new(0.32 - push * 0.10, 1.66, -0.78 + push * 0.10);
                c_tf.look_at(stage + Vec3::new(0.0, 1.79, -1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54 - push * 0.06;
                }
            }
            1 => {
                // Extreme close-up on Stan's face & tears
                c_tf.translation = stage + Vec3::new(-0.34 + push * 0.08, 1.66, 0.76 - push * 0.10);
                c_tf.look_at(stage + Vec3::new(0.0, 1.74, 1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54 - push * 0.05;
                }
            }
            2 => {
                // Dynamic fighting orbit & close action cuts
                let ang = 0.75 + bt * 0.85;
                let dist = 1.95 + (bt * 4.0).sin() * 0.22;
                c_tf.translation = stage + Vec3::new(ang.sin() * dist, 1.55 + (bt * 3.2).cos() * 0.25, ang.cos() * dist);
                c_tf.look_at(stage + Vec3::new(0.0, 1.45, 0.0), Vec3::Y);
                c_tf.rotate_local_z((bt * 5.0).sin() * 0.06);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.68;
                }
            }
            3 => {
                // Ultra-tight nose-to-nose lock-up close-up
                c_tf.translation = stage + Vec3::new(1.05 - push * 0.18, 1.74, 0.12);
                c_tf.look_at(stage + Vec3::new(0.0, 1.73, 0.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.52;
                }
            }
            4 => {
                // Emotional close-up on Derek dropping his guard + Beer Tower Pyramid behind
                c_tf.translation = stage + Vec3::new(0.45, 1.72, -0.55 - push * 0.15);
                c_tf.look_at(stage + Vec3::new(-0.15, 1.76, -1.65), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.56;
                }
            }
            _ => {
                // Heroic low-angle shot of Stan & Derek side-by-side in front of the Beer Tower Pyramid
                c_tf.translation = stage + Vec3::new(0.25, 1.15, 3.25 - push * 0.35);
                c_tf.look_at(stage + Vec3::new(0.0, 1.65, -0.5), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
        }
    }
}

pub fn exit_derek_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<DerekCutsceneEntity>>,
    ov_q: Query<Entity, With<DerekCutsceneOverlay>>,
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
    stats.hearts = (stats.hearts + 2).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.tendy_credits += 25;
    stats.hint_text = "DEREK'S BEER TOWER TRUCE! +2 HEARTS & +25 CREDITS!".to_string();
    play_viking_chant_music();
}
