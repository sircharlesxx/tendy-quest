use bevy::prelude::*;
use crate::audio::{is_voice_speaking, play_cutscene_music, play_sfx, speak_voice};
use crate::state::{GameState, PlayerStats};

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
pub struct LexieMeltedActor;

#[derive(Resource, Default)]
pub struct LexieVictoryState {
    pub beat: u8,
    pub beat_t: f32,
}

const GROSS_BEATS: [(&str, &str, &str); 3] = [
    (
        "LEXIE (MELTING IN GARLIC BRINE)",
        "Ughhh... my fermented garlic brine... the jars are bursting inside me... *BLAAARGH*!! Everything's turning into warm pickled slime!!",
        "lexie",
    ),
    (
        "STAN (COVERED IN GARLIC JUICE)",
        "EWWWW!! It's chunky vinegar, fermented toes, and slimy garlic pulp! Derek! TJ! DO NOT BREATHE THIS IN!",
        "stan",
    ),
    (
        "STAN, TJ & DEREK (HOLDING THEIR NOSES)",
        "Cedar Rapids is saved! Lexie dissolved into a giant bubbling garlic puddle... even if 3rd Street smells like pickled feet forever!",
        "stan",
    ),
];

pub fn enter_lexie_victory_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut v_state: ResMut<LexieVictoryState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    *v_state = LexieVictoryState::default();
    let stage = Vec3::new(0.0, 0.0, -420.0);

    let street_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.11, 0.12),
        perceptual_roughness: 0.85,
        ..default()
    });
    let slime_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.46, 0.78, 0.14, 0.90),
        emissive: LinearRgba::new(0.28, 0.55, 0.05, 1.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.12,
        ..default()
    });
    let chunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.82, 0.36),
        perceptual_roughness: 0.25,
        ..default()
    });
    let stink_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.42, 0.65, 0.12, 0.36),
        emissive: LinearRgba::new(0.14, 0.28, 0.02, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let lexie_skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.76, 0.45),
        perceptual_roughness: 0.35,
        ..default()
    });
    let lexie_dress = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.18, 0.42),
        perceptual_roughness: 0.5,
        ..default()
    });
    let stan_plaid = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.14, 0.16),
        ..default()
    });
    let skin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.76, 0.66),
        ..default()
    });

    // Bright dramatic spotlights so the nighttime stage at z = -420 is vividly lit
    commands.spawn((
        LexieVictoryEntity,
        PointLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            intensity: 420_000.0,
            range: 34.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(1.5, 5.5, 2.5)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        PointLight {
            color: Color::srgb(0.65, 1.0, 0.30),
            intensity: 240_000.0,
            range: 22.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 2.8, -1.8)),
    ));

    // Street floor + giant bubbling green pickled garlic puddle
    commands.spawn((
        LexieVictoryEntity,
        Mesh3d(meshes.add(Cuboid::new(28.0, 0.2, 28.0))),
        MeshMaterial3d(street_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        LexieVictoryEntity,
        Mesh3d(meshes.add(Cylinder::new(4.2, 0.12))),
        MeshMaterial3d(slime_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 0.06, -1.6)),
    ));

    // Melting Lexie slumped in the bubbling green puddle
    commands
        .spawn((
            LexieVictoryEntity,
            LexieMeltedActor,
            Transform::from_translation(stage + Vec3::new(0.0, 0.1, -1.6)),
            Visibility::default(),
        ))
        .with_children(|lx| {
            lx.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.95, 1.1))),
                MeshMaterial3d(lexie_dress),
                Transform::from_xyz(0.0, 0.55, 0.0).with_scale(Vec3::new(1.2, 0.7, 1.2)),
            ));
            lx.spawn((
                Mesh3d(meshes.add(Sphere::new(0.55))),
                MeshMaterial3d(lexie_skin),
                Transform::from_xyz(0.0, 1.25, 0.15),
            ));
        });

    // Stan standing disgusted nearby
    commands
        .spawn((
            LexieVictoryEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 2.4))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
        ))
        .with_children(|st| {
            st.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.55, 1.25))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 0.9, 0.0),
            ));
            st.spawn((
                Mesh3d(meshes.add(Sphere::new(0.36))),
                MeshMaterial3d(skin_mat),
                Transform::from_xyz(0.0, 1.85, 0.0),
            ));
            st.spawn((
                Mesh3d(meshes.add(Cone::new(0.32, 0.45))),
                MeshMaterial3d(stan_plaid),
                Transform::from_xyz(0.0, 2.25, 0.0),
            ));
        });

    // Erupting disgusting slime & garlic chunk geyser blobs
    let blob_mesh = meshes.add(Sphere::new(0.18));
    for i in 0..24 {
        let ang = (i as f32 / 24.0) * std::f32::consts::TAU * 2.0;
        let spd = 1.6 + (i % 4) as f32 * 0.65;
        let mat = if i % 3 == 0 {
            chunk_mat.clone()
        } else {
            slime_mat.clone()
        };
        commands.spawn((
            LexieVictoryEntity,
            LexieGrossBlob {
                base_pos: stage + Vec3::new(0.0, 1.15, -1.4),
                vel: Vec3::new(ang.cos() * spd, 4.2 + (i % 3) as f32 * 1.1, ang.sin() * spd),
                phase: (i as f32) * 0.23,
            },
            Mesh3d(blob_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.15, -1.4)),
        ));
    }

    // Rising green stink clouds (positioned behind/around Lexie so they never block the camera)
    let cloud_mesh = meshes.add(Sphere::new(0.36));
    for i in 0..8 {
        let ang = std::f32::consts::PI + (i as f32 / 7.0) * std::f32::consts::PI;
        let pos = stage + Vec3::new(ang.cos() * 1.9, 0.35, -2.1 + ang.sin() * 1.1);
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
        c_tf.translation = stage + Vec3::new(1.8, 2.1, 1.4);
        c_tf.look_at(stage + Vec3::new(0.0, 1.0, -1.6), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.64;
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
                BackgroundColor(Color::srgba(0.06, 0.11, 0.04, 0.90)),
                BorderColor(Color::srgb(0.55, 0.92, 0.22)),
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
                    TextColor(Color::srgb(0.65, 1.0, 0.35)),
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

    play_cutscene_music();
    play_sfx("headphone_crunch");
    speak_voice(line, voice);
}

pub fn update_lexie_victory_cutscene(
    time: Res<Time>,
    mut v_state: ResMut<LexieVictoryState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut blob_q: Query<(&mut Transform, &LexieGrossBlob), Without<Camera3d>>,
    mut cloud_q: Query<(&mut Transform, &LexieStinkCloud), (Without<LexieGrossBlob>, Without<Camera3d>)>,
    mut lexie_q: Query<&mut Transform, (With<LexieMeltedActor>, Without<LexieGrossBlob>, Without<LexieStinkCloud>, Without<Camera3d>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<LexieVictorySpeakerText>>,
    mut sub_q: Query<&mut Text, (With<LexieVictorySubText>, Without<LexieVictorySpeakerText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    v_state.beat_t += dt;

    if v_state.beat_t >= 4.0 && (!is_voice_speaking() || v_state.beat_t >= 16.0) {
        if v_state.beat + 1 >= GROSS_BEATS.len() as u8 {
            next_state.set(GameState::StageClear);
            return;
        }
        v_state.beat += 1;
        v_state.beat_t = 0.0;
        let (spk, line, voice) = GROSS_BEATS[v_state.beat as usize];
        play_sfx("hit");
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
    }

    let bt = v_state.beat_t;
    let stage = Vec3::new(0.0, 0.0, -420.0);

    // Erupting slime geyser physics
    for (mut btf, blob) in &mut blob_q {
        let cycle = (bt * 1.15 + blob.phase) % 1.4;
        let y = (blob.vel.y * cycle - 4.9 * cycle * cycle).max(-1.05);
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

    // Lexie melting flatter into the puddle
    for mut ltf in &mut lexie_q {
        let melt = (1.0 - (v_state.beat as f32 * 0.25 + bt * 0.06)).clamp(0.18, 1.0);
        ltf.scale = Vec3::new(1.0 + (1.0 - melt) * 1.2, melt, 1.0 + (1.0 - melt) * 1.2);
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match v_state.beat {
            0 => {
                c_tf.translation = stage + Vec3::new(1.8 - push * 0.4, 2.1, 1.4 - push * 0.3);
                c_tf.look_at(stage + Vec3::new(0.0, 1.0, -1.6), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.64;
                }
            }
            1 => {
                c_tf.translation = stage + Vec3::new(-1.4 + push * 0.3, 2.0, 0.3 + push * 0.25);
                c_tf.look_at(stage + Vec3::new(0.0, 1.75, 2.4), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62;
                }
            }
            _ => {
                c_tf.translation = stage + Vec3::new(3.4, 2.8, 5.6 - push * 0.7);
                c_tf.look_at(stage + Vec3::new(0.0, 1.1, 0.2), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
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
}
