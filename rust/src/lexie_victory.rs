use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_headphone_music, play_sfx, speak_voice,
};
use crate::state::GameState;
use crate::ui::HudOverlay;

#[derive(Component)]
pub struct LexieVictoryEntity;

#[derive(Component)]
pub struct LexieVictoryOverlay;

#[derive(Component)]
pub struct LexieVictorySpeakerText;

#[derive(Component)]
pub struct LexieVictorySubText;

#[derive(Component)]
pub struct LexieGrossBlob {
    pub base_pos: Vec3,
    pub vel: Vec3,
    pub phase: f32,
}

#[derive(Component)]
pub struct LexieStinkCloud {
    pub base_pos: Vec3,
    pub phase: f32,
}

#[derive(Component)]
pub struct LexieSpiralRing {
    pub speed: f32,
    pub base_scale: f32,
}

#[derive(Component)]
pub struct LexieBoil {
    pub base_scale: f32,
    pub phase: f32,
}

#[derive(Component)]
pub struct LexieMeltedActor;

#[derive(Resource, Default)]
pub struct LexieVictoryState {
    pub beat: u8,
    pub beat_t: f32,
}

const GROSS_BEATS: [(&str, &str, &str); 4] = [
    (
        "LEXIE (CONSUMED BY THE GARLIC SPIRAL)",
        "HEHEHEHAHAHA!! YOU THINK YOU WON?! THE PICKLED GARLIC SPIRAL LIVES IN MY VEINS!! MY BOILS ARE BURSTING - DROWN IN MY WARM FERMENTED BRINE!!",
        "lexie",
    ),
    (
        "STAN (TO TJ & DEREK)",
        "EWWWW!! Look at her spiral eyes - green vinegar is shooting out of her ears!! Derek! TJ! DO NOT LET THAT CHUNKY GARLIC SLIME TOUCH YOU!!",
        "stan",
    ),
    (
        "DEREK & TJ (HORRIFIED)",
        "OH GOD SHE'S MELTING INTO PICKLED FEET JUICE!! MY EYES ARE WATERING - THE STENCH IS SO THICK YOU CAN CHEW IT!!",
        "tj",
    ),
    (
        "STAN, TJ & DEREK (SURVIVORS)",
        "Cedar Rapids is saved... we survived the spiral, boys. Now let's sprint back to Pub 217 and wash this stench out with a six-foot beer tower!",
        "stan",
    ),
];

pub fn enter_lexie_victory_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut v_state: ResMut<LexieVictoryState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut clear_col: ResMut<ClearColor>,
) {
    *v_state = LexieVictoryState::default();
    for mut hv in &mut hud_q {
        *hv = Visibility::Hidden;
    }
    clear_col.0 = Color::srgb(0.03, 0.01, 0.06);
    let stage = Vec3::new(0.0, 0.0, -420.0);

    // Bright dramatic lighting so the entire horror stage pops in vivid sickly color
    commands.spawn((
        LexieVictoryEntity,
        PointLight {
            color: Color::srgb(1.0, 0.94, 0.88),
            intensity: 520_000.0,
            range: 40.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 6.0, 3.0)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        PointLight {
            color: Color::srgb(0.60, 1.0, 0.20),
            intensity: 300_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 2.5, -2.5)),
    ));

    let street_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.08, 0.12),
        perceptual_roughness: 0.85,
        ..default()
    });
    let slime_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.48, 0.86, 0.12, 0.92),
        emissive: LinearRgba::new(0.45, 0.95, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        ..default()
    });
    let chunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.86, 0.32),
        emissive: LinearRgba::new(0.25, 0.22, 0.04, 1.0),
        perceptual_roughness: 0.25,
        ..default()
    });
    let boil_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.18, 0.52),
        emissive: LinearRgba::new(0.65, 0.08, 0.45, 1.0),
        perceptual_roughness: 0.2,
        ..default()
    });
    let stink_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.45, 0.78, 0.12, 0.34),
        emissive: LinearRgba::new(0.18, 0.42, 0.04, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // Colored Uzumaki spirals spinning behind Lexie (acid green, bruised magenta, sickly gold)
    let spiral_colors = [
        LinearRgba::new(0.4, 3.2, 0.2, 1.0),
        LinearRgba::new(3.0, 0.2, 2.2, 1.0),
        LinearRgba::new(2.8, 2.4, 0.15, 1.0),
    ];
    for ring_i in 0..9 {
        let sc = 0.85 + ring_i as f32 * 0.58;
        let em = spiral_colors[ring_i % spiral_colors.len()];
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: em,
            unlit: true,
            ..default()
        });
        commands.spawn((
            LexieVictoryEntity,
            LexieSpiralRing {
                speed: if ring_i % 2 == 0 { 1.8 } else { -2.2 },
                base_scale: sc,
            },
            Mesh3d(meshes.add(Torus::new(0.055, 0.88))),
            MeshMaterial3d(ring_mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.8, -4.4))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(sc)),
        ));
    }

    // Street floor + giant bubbling green pickled garlic puddle
    commands.spawn((
        LexieVictoryEntity,
        Mesh3d(meshes.add(Cuboid::new(30.0, 0.2, 30.0))),
        MeshMaterial3d(street_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        Mesh3d(meshes.add(Cylinder::new(4.4, 0.12))),
        MeshMaterial3d(slime_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 0.06, -2.2)),
    ));

    // Maniacal Lexie with bulging colored Uzumaki spiral eyes, gaping jaw, wild hair & pulsing boils
    let lexie_skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.84, 0.48),
        perceptual_roughness: 0.35,
        ..default()
    });
    let lexie_dress = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.12, 0.46),
        perceptual_roughness: 0.5,
        ..default()
    });
    let lexie_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.06, 0.16),
        perceptual_roughness: 0.7,
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.86),
        emissive: LinearRgba::new(0.6, 0.35, 0.25, 1.0),
        ..default()
    });
    let iris_spiral = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.10, 0.95),
        emissive: LinearRgba::new(2.5, 0.2, 3.0, 1.0),
        ..default()
    });
    let pupil_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        ..default()
    });
    let mouth_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.18, 0.03),
        emissive: LinearRgba::new(0.15, 0.45, 0.02, 1.0),
        ..default()
    });

    commands
        .spawn((
            LexieVictoryEntity,
            LexieMeltedActor,
            Transform::from_translation(stage + Vec3::new(0.0, 0.1, -2.2)),
            Visibility::default(),
        ))
        .with_children(|lx| {
            // Torso & flailing arms
            lx.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.85, 1.25))),
                MeshMaterial3d(lexie_dress.clone()),
                Transform::from_xyz(0.0, 0.65, 0.0),
            ));
            for sx in [-0.95_f32, 0.95] {
                lx.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.18, 0.75))),
                    MeshMaterial3d(lexie_skin.clone()),
                    Transform::from_xyz(sx, 1.15, 0.25)
                        .with_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.0, sx * 0.4)),
                ));
            }
            // Head
            lx.spawn((
                Mesh3d(meshes.add(Sphere::new(0.58))),
                MeshMaterial3d(lexie_skin.clone()),
                Transform::from_xyz(0.0, 1.72, 0.05),
            ));
            // Wild flared hair spikes
            for (hx, hy, hz, rz) in [
                (-0.48_f32, 2.02_f32, -0.08_f32, 0.55_f32),
                (0.48, 2.02, -0.08, -0.55),
                (0.0, 2.25, -0.14, 0.0),
                (-0.56, 1.72, -0.18, 0.85),
                (0.56, 1.72, -0.18, -0.85),
            ] {
                lx.spawn((
                    Mesh3d(meshes.add(Cone::new(0.24, 0.65))),
                    MeshMaterial3d(lexie_hair.clone()),
                    Transform::from_xyz(hx, hy, hz).with_rotation(Quat::from_rotation_z(rz)),
                ));
            }
            // Asymmetrical bulging maniacal spiral eyes!
            for (idx, ex) in [-0.20_f32, 0.20].iter().enumerate() {
                let eye_sc = if idx == 0 { 1.25 } else { 0.92 };
                lx.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.13 * eye_sc))),
                    MeshMaterial3d(eye_white.clone()),
                    Transform::from_xyz(*ex, 1.84, 0.52),
                ));
                lx.spawn((
                    Mesh3d(meshes.add(Torus::new(0.025, 0.075 * eye_sc))),
                    MeshMaterial3d(iris_spiral.clone()),
                    Transform::from_xyz(*ex, 1.84, 0.60)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
                lx.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.036))),
                    MeshMaterial3d(pupil_mat.clone()),
                    Transform::from_xyz(*ex, 1.84, 0.63),
                ));
            }
            // Unhinged gaping mouth spewing slime
            lx.spawn((
                Mesh3d(meshes.add(Sphere::new(0.24))),
                MeshMaterial3d(mouth_dark),
                Transform::from_xyz(0.0, 1.50, 0.52).with_scale(Vec3::new(1.15, 1.35, 0.65)),
            ));
            // Pulsing disgusting boils on the sides of her cheeks & forehead (never blocking her spiral eyes)
            for (i, (bx, by, bz)) in [
                (-0.42_f32, 1.62_f32, 0.34_f32),
                (0.42, 1.64, 0.34),
                (-0.22, 2.08, 0.38),
                (0.24, 2.06, 0.38),
            ]
            .iter()
            .enumerate()
            {
                lx.spawn((
                    LexieBoil {
                        base_scale: 0.06 + (i % 2) as f32 * 0.02,
                        phase: i as f32 * 1.1,
                    },
                    Mesh3d(meshes.add(Sphere::new(1.0))),
                    MeshMaterial3d(boil_mat.clone()),
                    Transform::from_xyz(*bx, *by, *bz).with_scale(Vec3::splat(0.07)),
                ));
            }
        });

    // Shared materials for Stan, TJ & Derek standing together facing Lexie!
    let skin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.76, 0.66),
        perceptual_roughness: 0.65,
        ..default()
    });
    let stan_plaid = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.14, 0.16),
        ..default()
    });
    let stan_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.82, 0.22),
        metallic: 0.6,
        ..default()
    });
    let dark_beard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.08, 0.06),
        ..default()
    });
    let tj_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.34, 0.62),
        ..default()
    });
    let derek_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.25, 0.28),
        ..default()
    });
    let pants_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.14, 0.20),
        ..default()
    });
    let clean_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    // 1. STAN (center, z = +2.4, facing Lexie at -Z)
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 2.4))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
        ))
        .with_children(|st| {
            for sx in [-0.14_f32, 0.14] {
                st.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.45))),
                    MeshMaterial3d(stan_plaid.clone()),
                    Transform::from_xyz(sx, 0.36, 0.0),
                ));
            }
            st.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.35, 0.80))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 1.04, 0.0),
            ));
            st.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.365, 0.12))),
                MeshMaterial3d(stan_gold),
                Transform::from_xyz(0.0, 0.82, 0.0),
            ));
            st.spawn((
                Mesh3d(meshes.add(Sphere::new(0.27))),
                MeshMaterial3d(skin_mat.clone()),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            st.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(dark_beard.clone()),
                Transform::from_xyz(0.0, 1.60, 0.07).with_scale(Vec3::new(0.95, 0.88, 0.85)),
            ));
            st.spawn((
                Mesh3d(meshes.add(Cone::new(0.24, 0.36))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 2.02, -0.02),
            ));
            for ex in [-0.09_f32, 0.09] {
                st.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.042))),
                    MeshMaterial3d(clean_white.clone()),
                    Transform::from_xyz(ex, 1.76, 0.235),
                ));
                st.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.022))),
                    MeshMaterial3d(pupil_mat.clone()),
                    Transform::from_xyz(ex, 1.76, 0.258),
                ));
            }
        });

    // 2. DEREK (left of Stan, x = -1.25, z = +2.55 — dark grey hoodie, back bun, thick beard)
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(-1.25, 0.0, 2.55))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI - 0.18)),
            Visibility::default(),
        ))
        .with_children(|dk| {
            for sx in [-0.14_f32, 0.14] {
                dk.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.45))),
                    MeshMaterial3d(pants_mat.clone()),
                    Transform::from_xyz(sx, 0.36, 0.0),
                ));
            }
            dk.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.66, 0.80, 0.38))),
                MeshMaterial3d(derek_hoodie),
                Transform::from_xyz(0.0, 1.04, 0.0),
            ));
            dk.spawn((
                Mesh3d(meshes.add(Sphere::new(0.28))),
                MeshMaterial3d(skin_mat.clone()),
                Transform::from_xyz(0.0, 1.76, 0.0),
            ));
            dk.spawn((
                Mesh3d(meshes.add(Sphere::new(0.27))),
                MeshMaterial3d(dark_beard.clone()),
                Transform::from_xyz(0.0, 1.62, 0.07).with_scale(Vec3::new(0.96, 0.92, 0.88)),
            ));
            dk.spawn((
                Mesh3d(meshes.add(Sphere::new(0.11))),
                MeshMaterial3d(dark_beard.clone()),
                Transform::from_xyz(0.0, 1.80, -0.30),
            ));
            for ex in [-0.095_f32, 0.095] {
                dk.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.042))),
                    MeshMaterial3d(clean_white.clone()),
                    Transform::from_xyz(ex, 1.80, 0.242),
                ));
                dk.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.022))),
                    MeshMaterial3d(pupil_mat.clone()),
                    Transform::from_xyz(ex, 1.80, 0.268),
                ));
            }
        });

    // 3. TJ (right of Stan, x = +1.25, z = +2.55 — blue hoodie, dark beard, glasses)
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(1.25, 0.0, 2.55))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI + 0.18)),
            Visibility::default(),
        ))
        .with_children(|tj| {
            for sx in [-0.14_f32, 0.14] {
                tj.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.45))),
                    MeshMaterial3d(pants_mat.clone()),
                    Transform::from_xyz(sx, 0.36, 0.0),
                ));
            }
            tj.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.62, 0.78, 0.36))),
                MeshMaterial3d(tj_hoodie),
                Transform::from_xyz(0.0, 1.04, 0.0),
            ));
            tj.spawn((
                Mesh3d(meshes.add(Sphere::new(0.26))),
                MeshMaterial3d(skin_mat.clone()),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            tj.spawn((
                Mesh3d(meshes.add(Sphere::new(0.24))),
                MeshMaterial3d(dark_beard.clone()),
                Transform::from_xyz(0.0, 1.59, 0.07).with_scale(Vec3::new(0.92, 0.88, 0.85)),
            ));
            for ex in [-0.09_f32, 0.09] {
                tj.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.04))),
                    MeshMaterial3d(clean_white.clone()),
                    Transform::from_xyz(ex, 1.76, 0.23),
                ));
                tj.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.022))),
                    MeshMaterial3d(pupil_mat.clone()),
                    Transform::from_xyz(ex, 1.76, 0.255),
                ));
            }
        });

    // Erupting disgusting slime & garlic chunk geyser blobs strictly around Lexie (never hitting camera or Stan/Derek/TJ)
    let blob_mesh = meshes.add(Sphere::new(0.10));
    for i in 0..24 {
        let ang = (i as f32 / 24.0) * std::f32::consts::TAU * 2.0;
        let spd = 0.8 + (i % 4) as f32 * 0.32;
        let mat = if i % 3 == 0 {
            chunk_mat.clone()
        } else {
            slime_mat.clone()
        };
        commands.spawn((
            LexieVictoryEntity,
            LexieGrossBlob {
                base_pos: stage + Vec3::new(0.0, 1.45, -2.1),
                vel: Vec3::new(ang.cos() * spd, 3.2 + (i % 3) as f32 * 0.85, ang.sin() * spd * 0.6),
                phase: (i as f32) * 0.21,
            },
            Mesh3d(blob_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.45, -2.1)),
        ));
    }

    // Rising green stink clouds behind Lexie
    let cloud_mesh = meshes.add(Sphere::new(0.36));
    for i in 0..8 {
        let ang = std::f32::consts::PI + (i as f32 / 7.0) * std::f32::consts::PI;
        let pos = stage + Vec3::new(ang.cos() * 2.1, 0.35, -2.5 + ang.sin() * 1.1);
        commands.spawn((
            LexieVictoryEntity,
            LexieStinkCloud {
                base_pos: pos,
                phase: i as f32 * 0.45,
            },
            Mesh3d(cloud_mesh.clone()),
            MeshMaterial3d(stink_mat.clone()),
            Transform::from_translation(pos),
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.45, 1.85, 0.85);
        c_tf.look_at(stage + Vec3::new(0.0, 1.65, -2.2), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.70;
        }
    }

    let (spk, line, voice) = GROSS_BEATS[0];
    commands
        .spawn((
            LexieVictoryOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(20.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // Colored Uzumaki manga horror SFX banner at top
            root.spawn((
                Node {
                    width: Val::Percent(92.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
            ))
            .with_children(|sfx_row| {
                sfx_row.spawn((
                    Text::new("*SQUELCH*  *SPLORCH*"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.55, 1.0, 0.18)),
                ));
                sfx_row.spawn((
                    Text::new("UZUMAKI!!  *BLAAARGH*"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.22, 0.78)),
                ));
            });

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
                BackgroundColor(Color::srgba(0.06, 0.02, 0.10, 0.90)),
                BorderColor(Color::srgb(0.65, 1.0, 0.22)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    LexieVictorySpeakerText,
                    Text::new(spk),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 1.0, 0.30)),
                ));
                card.spawn((
                    LexieVictorySubText,
                    Text::new(format!("\"{}\"", line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    // Scary Uzumaki horror synth + crunch + thunder!
    play_headphone_music(2);
    play_sfx("headphone_crunch");
    play_sfx("thunder");
    speak_voice(line, voice);
}

#[allow(clippy::type_complexity)]
pub fn update_lexie_victory_cutscene(
    time: Res<Time>,
    mut v_state: ResMut<LexieVictoryState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut blob_q: Query<(&mut Transform, &LexieGrossBlob), Without<Camera3d>>,
    mut cloud_q: Query<(&mut Transform, &LexieStinkCloud), (Without<LexieGrossBlob>, Without<Camera3d>)>,
    mut spiral_q: Query<(&mut Transform, &LexieSpiralRing), (Without<LexieGrossBlob>, Without<LexieStinkCloud>, Without<Camera3d>)>,
    mut boil_q: Query<(&mut Transform, &LexieBoil), (Without<LexieGrossBlob>, Without<LexieStinkCloud>, Without<LexieSpiralRing>, Without<Camera3d>)>,
    mut lexie_q: Query<&mut Transform, (With<LexieMeltedActor>, Without<LexieGrossBlob>, Without<LexieStinkCloud>, Without<LexieSpiralRing>, Without<LexieBoil>, Without<Camera3d>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<LexieVictorySpeakerText>>,
    mut sub_q: Query<&mut Text, (With<LexieVictorySubText>, Without<LexieVictorySpeakerText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    v_state.beat_t += dt;

    if v_state.beat_t >= 4.0 && (!is_voice_speaking() || v_state.beat_t >= 18.0) {
        if v_state.beat + 1 >= GROSS_BEATS.len() as u8 {
            next_state.set(GameState::StageClear);
            return;
        }
        v_state.beat += 1;
        v_state.beat_t = 0.0;
        let (spk, line, voice) = GROSS_BEATS[v_state.beat as usize];
        if v_state.beat == 2 {
            play_sfx("headphone_crunch");
            play_sfx("thunder");
        } else {
            play_sfx("hit");
        }
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
    }

    let bt = v_state.beat_t;
    let t = time.elapsed_secs();
    let stage = Vec3::new(0.0, 0.0, -420.0);

    // Spin colored Uzumaki spirals flat-facing the camera behind Lexie
    for (mut stf, sp) in &mut spiral_q {
        stf.rotate_z(sp.speed * dt);
        let pulse = 1.0 + (t * 2.8 + sp.base_scale).sin() * 0.08;
        stf.scale = Vec3::splat(sp.base_scale * pulse);
    }

    // Pulse disgusting boils on Lexie's face
    for (mut btf, boil) in &mut boil_q {
        let s = boil.base_scale * (1.0 + (t * 8.0 + boil.phase).sin().abs() * 0.75);
        btf.scale = Vec3::splat(s);
    }

    // Erupting slime geyser physics
    for (mut btf, blob) in &mut blob_q {
        let cycle = (bt * 1.15 + blob.phase) % 1.4;
        let y = (blob.vel.y * cycle - 4.9 * cycle * cycle).max(-1.35);
        btf.translation = blob.base_pos + Vec3::new(blob.vel.x * cycle, y, blob.vel.z * cycle);
        let sq = 0.75 + (cycle * 6.0).sin().abs() * 0.55;
        btf.scale = Vec3::splat(sq);
    }

    // Rising green stink clouds
    for (mut ctf, cl) in &mut cloud_q {
        let rise = (bt * 0.55 + cl.phase) % 2.4;
        ctf.translation = cl.base_pos + Vec3::new((bt + cl.phase).sin() * 0.35, rise * 1.4, (bt + cl.phase).cos() * 0.35);
        ctf.scale = Vec3::splat(0.7 + rise * 0.55);
    }

    // Lexie stays maniacally upright in beats 0-1, then melts into the garlic puddle in beats 2-3!
    for mut ltf in &mut lexie_q {
        if v_state.beat < 2 {
            let shake = (t * 24.0).sin() * 0.04;
            ltf.translation = stage + Vec3::new(shake, 0.1, -2.2);
            ltf.scale = Vec3::splat(1.0 + (t * 5.0).sin() * 0.04);
        } else {
            let melt = (1.0 - ((v_state.beat - 2) as f32 * 0.42 + bt * 0.10)).clamp(0.16, 1.0);
            ltf.scale = Vec3::new(1.0 + (1.0 - melt) * 1.3, melt, 1.0 + (1.0 - melt) * 1.3);
        }
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match v_state.beat {
            0 => {
                // Close-up on maniacal Lexie + spinning colored Uzumaki spirals!
                c_tf.translation = stage + Vec3::new(0.45 - push * 0.20, 1.85, 0.85 - push * 0.25);
                c_tf.look_at(stage + Vec3::new(0.0, 1.65, -2.2), Vec3::Y);
                c_tf.rotate_local_z((t * 1.4).sin() * 0.035);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.70 - push * 0.06;
                }
            }
            1 => {
                // Close-up framing Stan, Derek & TJ together reacting in disgust!
                c_tf.translation = stage + Vec3::new(0.0, 1.80, -0.45 + push * 0.20);
                c_tf.look_at(stage + Vec3::new(0.0, 1.60, 2.5), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.78;
                }
            }
            2 => {
                // Wide 3/4 view of Lexie melting into the bubbling green puddle while Derek & TJ watch!
                c_tf.translation = stage + Vec3::new(3.8, 2.5, 1.8 - push * 0.35);
                c_tf.look_at(stage + Vec3::new(0.0, 1.0, -1.6), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
                }
            }
            _ => {
                // 3/4 group shot showing Stan, Derek & TJ's faces alongside the melted Lexie puddle & spirals
                c_tf.translation = stage + Vec3::new(3.4 - push * 0.4, 2.2, 0.35);
                c_tf.look_at(stage + Vec3::new(-0.3, 1.45, 0.8), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.74;
                }
            }
        }
    }
}

pub fn exit_lexie_victory_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<LexieVictoryEntity>>,
    ov_q: Query<Entity, With<LexieVictoryOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    for mut hv in &mut hud_q {
        *hv = Visibility::Inherited;
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
}
