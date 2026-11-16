use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music,
    set_touch_boss_mode, set_touch_playing, speak_voice, take_cutscene_start_time,
};
use crate::boss::{CutsceneEyeReflection, HeroCosmetic, spawn_mom_mesh_hierarchy};
use crate::enemies::{Enemy, FiberPuddle};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats, TendyPickup};
use crate::ui::HudOverlay;

const CITY_HALF_EXTENT: f32 = 135.0;
const ROAD_COORDS: [f32; 5] = [-92.0, -46.0, 0.0, 46.0, 92.0];
const BLOCK_COORDS: [f32; 4] = [-69.0, -23.0, 23.0, 69.0];
const CITADEL_POS: Vec3 = Vec3::new(0.0, 0.0, -82.0);
const LEXIE_TERRACE_POS: Vec3 = Vec3::new(0.0, 38.0, -75.5);

#[derive(Component)]
pub struct CityWorldMarker;

#[derive(Component)]
pub struct CityTendy;

#[derive(Component)]
pub struct AnimeStageEntity;

#[derive(Component)]
pub struct AnimeOverlay;

#[derive(Component)]
pub struct AnimeSpeakerText;

#[derive(Component)]
pub struct AnimeSubtitleText;

#[derive(Component)]
pub struct AnimeBeatBadge;

#[derive(Component)]
pub struct AnimeSakuraPetal {
    pub drift_phase: f32,
    pub fall_speed: f32,
    pub sway_amp: f32,
}

#[derive(Component)]
pub struct AnimeCutsceneMom;

#[derive(Component)]
pub struct CityIntroOverlay;

#[derive(Component)]
pub struct CityIntroSpeakerText;

#[derive(Component)]
pub struct CityIntroSubtitleText;

#[derive(Component)]
pub struct CityIntroBeatBadge;

#[derive(Component)]
pub struct CityPlayUi;

#[derive(Component)]
pub struct LexieBossHudRoot;

#[derive(Component)]
pub struct CityMinimapPlayerDot;

#[derive(Component)]
pub struct CityMinimapBossDot;

#[derive(Component)]
pub struct CityMinimapTendyDot;

#[derive(Component)]
pub struct LexieBossHudText;

#[derive(Component)]
pub struct LexieBossHpFill;

#[derive(Component)]
pub struct CityCar {
    pub axis_x: bool,
    pub dir: f32,
    pub speed: f32,
    pub horn_cooldown: f32,
    pub occupied: bool,
    pub parked: bool,
    pub yaw: f32,
    pub velocity: Vec2,
    pub pitch_lean: f32,
    pub roll_lean: f32,
    pub steer_input: f32,
    pub knock_vel: Vec2,
    pub drift_sfx_cooldown: f32,
    pub car_hp: f32,
}

#[derive(Component)]
pub struct CityCarWheel {
    pub is_front: bool,
}

#[derive(Component)]
pub struct CityNpc {
    pub home_x: f32,
    pub home_z: f32,
    pub patrol_radius: f32,
    pub angle: f32,
    pub speed: f32,
    pub walk_phase: f32,
}

#[derive(Component)]
pub struct CityNpcLimb {
    pub side: f32,
    pub is_arm: bool,
}

#[derive(Component)]
pub struct LexieBoss {
    pub hp: f32,
    pub max_hp: f32,
    pub on_tower: bool,
    pub throw_cooldown: f32,
    pub chew_timer: f32,
    pub debris_timer: f32,
    pub hit_flash: f32,
    pub roar_cooldown: f32,
}

#[derive(Component)]
pub struct LexieThrowArm;

#[derive(Component)]
pub struct GarlicProjectile {
    pub velocity: Vec3,
    pub spin: f32,
    pub age: f32,
}

#[derive(Component)]
pub struct GarlicStenchCloud {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub radius: f32,
    pub damage_tick: f32,
}

#[derive(Component)]
pub struct CityTowerDebris {
    pub velocity: Vec3,
    pub spin: Vec3,
    pub age: f32,
}

// =========================================================================
// PART 1: POST-MOM JAPANESE ANIME THANK-YOU CUTSCENE
// =========================================================================

pub fn enter_mom_victory_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    for e in &old_level_q {
        commands.entity(e).despawn_recursive();
    }

    let start_t = take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;
    stats.boss_mode = false;
    stats.mom_quote.clear();
    stats.hint_text.clear();

    set_touch_playing(false);
    set_touch_boss_mode(false);
    play_cutscene_music();
    play_sfx("powerup");

    // Warm Japanese anime twilight sunset sky & cherry-blossom ambiance
    commands.insert_resource(ClearColor(Color::srgb(0.95, 0.46, 0.34)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.78, 0.62),
        brightness: 820.0,
    });

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        stats.player_scale = 3.2;
        p_tf.translation = Vec3::new(0.0, 0.0, 2.2);
        p_tf.scale = Vec3::splat(3.2);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0; // Facing -Z straight toward Mom
        p_tf.rotation = Quat::from_rotation_y(0.0);
    }

    // Spawn Mom proudly smiling in anime lighting facing Stan (+Z)
    let mom_ent = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, 0.0, -3.4),
        3.2,
    );
    commands.entity(mom_ent).insert((AnimeStageEntity, AnimeCutsceneMom));
    commands.entity(mom_ent).insert(
        Transform::from_xyz(0.0, 0.0, -3.4)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            .with_scale(Vec3::splat(3.2)),
    );

    // Anime Dojo / Shrine Courtyard Stage
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.24, 0.14),
        perceptual_roughness: 0.68,
        ..default()
    });
    let mat_runner = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.12, 0.18),
        perceptual_roughness: 0.75,
        ..default()
    });
    let torii_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.14, 0.12),
        perceptual_roughness: 0.35,
        ..default()
    });
    let sun_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.45),
        emissive: LinearRgba::rgb(4.2, 2.4, 0.6),
        unlit: true,
        ..default()
    });
    let speedline_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.65),
        emissive: LinearRgba::rgb(2.6, 1.8, 0.5),
        unlit: true,
        ..default()
    });
    let stone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.52, 0.49),
        perceptual_roughness: 0.9,
        ..default()
    });
    let lantern_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.42),
        emissive: LinearRgba::rgb(3.5, 2.2, 0.6),
        ..default()
    });

    // Raised wooden shrine floor + crimson ceremonial runner
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(26.0, 0.6, 26.0))),
        MeshMaterial3d(wood_mat),
        Transform::from_xyz(0.0, -0.3, -1.0),
    ));
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(4.2, 0.06, 14.0))),
        MeshMaterial3d(mat_runner),
        Transform::from_xyz(0.0, 0.03, -1.0),
    ));

    // Anime Red Torii Gate framing the sunset horizon
    for sx in [-4.5_f32, 4.5_f32] {
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cylinder::new(0.42, 10.5))),
            MeshMaterial3d(torii_red.clone()),
            Transform::from_xyz(sx, 5.2, -9.8),
        ));
    }
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(13.2, 0.72, 0.95))),
        MeshMaterial3d(torii_red.clone()),
        Transform::from_xyz(0.0, 10.3, -9.8),
    ));
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(10.8, 0.52, 0.70))),
        MeshMaterial3d(torii_red),
        Transform::from_xyz(0.0, 8.6, -9.8),
    ));

    // Giant glowing Anime Rising Sun halo + radial sunbeams
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cylinder::new(6.8, 0.25))),
        MeshMaterial3d(sun_gold),
        Transform::from_xyz(0.0, 5.8, -14.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
    for i in 0..14 {
        let ang = (i as f32 / 14.0) * std::f32::consts::TAU;
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.24, 15.0, 0.1))),
            MeshMaterial3d(speedline_gold.clone()),
            Transform::from_xyz(ang.cos() * 8.0, 5.8 + ang.sin() * 8.0, -14.0)
                .with_rotation(Quat::from_rotation_z(ang + std::f32::consts::FRAC_PI_2)),
        ));
    }

    // Traditional glowing stone lanterns flanking the ceremony
    for (lx, lz) in [(-5.4_f32, -2.0_f32), (5.4, -2.0), (-5.4, 3.6), (5.4, 3.6)] {
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.9, 1.8, 0.9))),
            MeshMaterial3d(stone_mat.clone()),
            Transform::from_xyz(lx, 0.9, lz),
        ));
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.68, 0.72, 0.68))),
            MeshMaterial3d(lantern_glow.clone()),
            Transform::from_xyz(lx, 2.1, lz),
        ));
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cone::new(0.88, 0.65))),
            MeshMaterial3d(stone_mat.clone()),
            Transform::from_xyz(lx, 2.75, lz),
        ));
    }

    // 42 Drifting Anime Sakura Cherry Blossom Petals
    let petal_mesh = meshes.add(Cuboid::new(0.18, 0.03, 0.12));
    let petal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.74, 0.86),
        emissive: LinearRgba::rgb(0.45, 0.18, 0.28),
        ..default()
    });
    for i in 0..42 {
        let fi = i as f32;
        let x = ((fi * 3.17).sin()) * 9.5;
        let y = 1.2 + ((fi * 1.91).rem_euclid(11.0));
        let z = -9.5 + ((fi * 2.43).cos()) * 6.5;
        commands.spawn((
            AnimeStageEntity,
            AnimeSakuraPetal {
                drift_phase: fi * 0.73,
                fall_speed: 1.15 + (i % 5) as f32 * 0.28,
                sway_amp: 0.7 + (i % 4) as f32 * 0.35,
            },
            Mesh3d(petal_mesh.clone()),
            MeshMaterial3d(petal_mat.clone()),
            Transform::from_xyz(x, y, z),
        ));
    }

    // Spawn stylized Anime Subtitle Banner UI
    commands
        .spawn((
            AnimeOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(14.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                    width: Val::Percent(94.0),
                    max_width: Val::Px(780.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.03, 0.08, 0.90)),
                BorderColor(Color::srgb(1.0, 0.65, 0.78)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        AnimeSpeakerText,
                        Text::new("STAN - HONORABLE SON"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.78, 0.88)),
                    ));
                    hdr.spawn((
                        AnimeBeatBadge,
                        Text::new("ANIME FINALE | SCENE 1/4"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.86, 0.45)),
                    ));
                });
                card.spawn((
                    AnimeSubtitleText,
                    Text::new(
                        "\"Okasan... Forgive this foolish warrior for raising his tendy-blade against his own mother!\"",
                    ),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    if stats.cutscene_timer < 1.0 {
        speak_voice(
            "Mother-san! Forgive this foolish warrior for raising his tendy blade against his own mother!",
            "stan",
        );
    }
}

pub fn update_mom_victory_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<AnimeSakuraPetal>)>,
    mut petal_q: Query<(&mut Transform, &mut AnimeSakuraPetal), Without<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    let t = stats.cutscene_timer;
    let total_elapsed = time.elapsed_secs();

    // Animate drifting sakura blossom petals across shrine
    for (mut tf, petal) in &mut petal_q {
        tf.translation.y -= petal.fall_speed * dt;
        tf.translation.x += (total_elapsed * 1.6 + petal.drift_phase).sin() * petal.sway_amp * dt;
        tf.translation.z += (total_elapsed * 1.1 + petal.drift_phase).cos() * 0.35 * dt;
        tf.rotate_y(2.4 * dt);
        tf.rotate_x(1.7 * dt);
        if tf.translation.y < 0.08 {
            tf.translation.y = 12.2;
        }
    }

    let (beat_idx, speech, role, sfx): (u8, &str, &str, Option<&str>) = if t < 5.0 {
        (0, "", "stan", None)
    } else if t < 11.2 {
        (
            1,
            "Arigatou gozaimasu, Mother! Every crispy tendy and every veggie lesson forged my Viking spirit! I am who I am today because of you!",
            "stan",
            Some("tendy"),
        )
    } else if t < 17.5 {
        (
            2,
            "Rise, my sweet Viking samurai, Stan. You have surpassed your mother! Now go to New York City. A massive titan named Lexie is destroying Manhattan with pickled garlic!",
            "mom",
            Some("powerup"),
        )
    } else {
        (
            3,
            "With honor restored and gratitude in his heart, Stan embarks for a vast new world. Next stop: New York City!",
            "narrator",
            Some("thunder"),
        )
    };

    // Hold on previous beat cleanly until TTS speech finishes!
    let mut active_beat = stats.cutscene_beat;
    if beat_idx > stats.cutscene_beat && !is_voice_speaking() {
        stats.cutscene_beat = beat_idx;
        active_beat = beat_idx;
        if let Some(name) = sfx {
            play_sfx(name);
        }
        if !speech.is_empty() {
            speak_voice(speech, role);
        }
    } else if beat_idx > stats.cutscene_beat && is_voice_speaking() {
        stats.cutscene_timer -= dt;
    }

    // Update camera with dramatic anime framing per beat
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        let (eye, target) = match active_beat {
            0 => (
                Vec3::new(6.2, 3.8 + (t * 0.12).min(0.6), 4.4),
                Vec3::new(0.0, 3.0, -1.0),
            ),
            1 => {
                // Heroic anime three-quarter shot framing bowing Stan & proud Mom beneath Torii & Rising Sun
                let u = ((t - 5.0) / 5.5).clamp(0.0, 1.0);
                (
                    Vec3::new(9.2 - u * 1.4, 3.8, 2.6 - u * 1.0),
                    Vec3::new(0.0, 2.8, -0.8),
                )
            }
            2 => (
                // Dramatic anime close-up of proud 3.2x Mom beneath the Torii gate & Rising Sun
                Vec3::new(2.2, 5.2, 1.8),
                Vec3::new(0.0, 5.1, -3.4),
            ),
            _ => {
                // Slow anime pull-up toward sunset sky before Manhattan transition
                let u = ((t - 17.5) / 4.0).clamp(0.0, 1.0);
                (
                    Vec3::new(0.0, 5.2 + u * 6.5, 10.5 + u * 5.0),
                    Vec3::new(0.0, 4.2 + u * 2.5, -6.0),
                )
            }
        };
        cam_tf.translation = cam_tf.translation.lerp(eye, (dt * 4.5).min(1.0));
        cam_tf.look_at(target, Vec3::Y);
    }

    // Wait: subtitle text updates run in a tiny helper system or inline — keep mutable UI updates separate!
    if active_beat >= 3 && t >= 22.8 && !is_voice_speaking() {
        next_state.set(GameState::CityIntroCutscene);
    }
}

pub fn update_mom_victory_subtitles(
    stats: Res<PlayerStats>,
    mut spk_q: Query<&mut Text, With<AnimeSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<AnimeBeatBadge>, Without<AnimeSpeakerText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<AnimeSubtitleText>,
            Without<AnimeSpeakerText>,
            Without<AnimeBeatBadge>,
        ),
    >,
) {
    let (speaker, badge, line) = match stats.cutscene_beat {
        0 => (
            "STAN - HONORABLE SON",
            "ANIME FINALE | SCENE 1/4",
            "\"Okasan... Forgive this foolish warrior for raising his tendy-blade against the one who gave him life!\"",
        ),
        1 => (
            "STAN - DEEPEST BOW",
            "ANIME FINALE | SCENE 2/4",
            "\"Arigatou gozaimasu, Mother! Every crispy tendy, every veggie warning, forged my Viking spirit! I am who I am today because of you!\"",
        ),
        2 => (
            "MOM - PROUD SENSEI",
            "ANIME FINALE | SCENE 3/4",
            "\"Rise, my sweet Viking samurai. You have surpassed your old mother! Now fly to New York... an enormous menace named Lexie is wrecking Manhattan!\"",
        ),
        _ => (
            "NARRATOR - NEXT CHAPTER",
            "ANIME FINALE | SCENE 4/4",
            "\"With tears of honor and fifty Manhattan tendies calling his name, Stan leaps into a new world... NEXT STAGE: NEW YORK CITY!\"",
        ),
    };
    if let Ok(mut t) = spk_q.get_single_mut() {
        t.0 = speaker.to_string();
    }
    if let Ok(mut t) = badge_q.get_single_mut() {
        t.0 = badge.to_string();
    }
    if let Ok(mut t) = sub_q.get_single_mut() {
        t.0 = line.to_string();
    }
}

pub fn exit_mom_victory_cutscene(
    mut commands: Commands,
    stage_q: Query<Entity, With<AnimeStageEntity>>,
    ui_q: Query<Entity, With<AnimeOverlay>>,
    cosmetic_q: Query<Entity, With<HeroCosmetic>>,
    refl_q: Query<Entity, With<CutsceneEyeReflection>>,
) {
    for e in &stage_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &cosmetic_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &refl_q {
        commands.entity(e).despawn_recursive();
    }
}

// =========================================================================
// PART 2: NEW YORK CITY LEVEL GENERATION & LEXIE COLOSSAL BOSS MODEL
// =========================================================================

fn is_clear_of_skyscrapers(x: f32, z: f32, margin: f32) -> bool {
    // Empire Citadel Tower footprint: 21x21 centered at (0, -82)
    if (x - CITADEL_POS.x).abs() < 11.2 + margin && (z - CITADEL_POS.z).abs() < 11.2 + margin {
        return false;
    }
    // Inner 4x4 city grid blocks (17m buildings at bx, bz in [-69, -23, 23, 69])
    for &bx in &BLOCK_COORDS {
        for &bz in &BLOCK_COORDS {
            if (x - bx).abs() < 9.0 + margin && (z - bz).abs() < 9.0 + margin {
                return false;
            }
        }
    }
    // Outer skyline blocks at -114 / +114
    for &bx in &[-114.0_f32, -69.0, -23.0, 23.0, 69.0, 114.0] {
        for &bz in &[-114.0_f32, 114.0] {
            if (x - bx).abs() < 9.2 + margin && (z - bz).abs() < 9.2 + margin {
                return false;
            }
        }
    }
    for &bz in &BLOCK_COORDS {
        for &bx in &[-114.0_f32, 114.0] {
            if (x - bx).abs() < 9.2 + margin && (z - bz).abs() < 9.2 + margin {
                return false;
            }
        }
    }
    true
}

pub fn spawn_nyc_world_if_needed(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    existing_city_q: &Query<Entity, With<CityWorldMarker>>,
    old_level_q: &Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    if !existing_city_q.is_empty() {
        return;
    }
    for e in old_level_q {
        commands.entity(e).despawn_recursive();
    }

    commands.insert_resource(ClearColor(Color::srgb(0.45, 0.67, 0.89)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 790.0,
    });

    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let unit_cone = meshes.add(Cone::new(1.0, 1.0));

    let asphalt = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.21),
        perceptual_roughness: 0.92,
        ..default()
    });
    let sidewalk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.56, 0.56, 0.58),
        perceptual_roughness: 0.88,
        ..default()
    });
    let yellow_stripe = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.78, 0.14),
        perceptual_roughness: 0.55,
        ..default()
    });
    let white_stripe = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.92, 0.94),
        perceptual_roughness: 0.55,
        ..default()
    });

    // Solid Manhattan street slab covering full 270m x 270m grid (top_y = 0.0)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(Cuboid::new(CITY_HALF_EXTENT * 2.0, 1.0, CITY_HALF_EXTENT * 2.0))),
        MeshMaterial3d(asphalt),
        Transform::from_xyz(0.0, -0.5, 0.0),
        PlatformBox {
            half_extents: Vec3::new(CITY_HALF_EXTENT, 0.5, CITY_HALF_EXTENT),
        },
    ));

    // Yellow Avenue double centerlines + White cross-street dashes
    for &r in &ROAD_COORDS {
        for off in [-0.28_f32, 0.28_f32] {
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(yellow_stripe.clone()),
                Transform::from_xyz(r + off, 0.015, 0.0)
                    .with_scale(Vec3::new(0.22, 0.02, CITY_HALF_EXTENT * 1.96)),
            ));
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(yellow_stripe.clone()),
                Transform::from_xyz(0.0, 0.015, r + off)
                    .with_scale(Vec3::new(CITY_HALF_EXTENT * 1.96, 0.02, 0.22)),
            ));
        }
        // Dashed lane dividers along avenues & streets
        for d in (-120..=120).step_by(14) {
            for side in [-5.0_f32, 5.0_f32] {
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(white_stripe.clone()),
                    Transform::from_xyz(r + side, 0.016, d as f32)
                        .with_scale(Vec3::new(0.18, 0.02, 4.2)),
                ));
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(white_stripe.clone()),
                    Transform::from_xyz(d as f32, 0.016, r + side)
                        .with_scale(Vec3::new(4.2, 0.02, 0.18)),
                ));
            }
        }
    }

    // Architectural palette for 36 Manhattan Skyscrapers
    let building_mats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.34, 0.46),
            metallic: 0.45,
            perceptual_roughness: 0.28,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.42, 0.38),
            perceptual_roughness: 0.72,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.19, 0.26),
            metallic: 0.60,
            perceptual_roughness: 0.22,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.54, 0.28, 0.22),
            perceptual_roughness: 0.80,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.42, 0.48),
            metallic: 0.35,
            perceptual_roughness: 0.40,
            ..default()
        }),
    ];
    let window_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.68),
        emissive: LinearRgba::rgb(1.15, 0.92, 0.45),
        ..default()
    });
    let billboard_mats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.80, 0.12),
            emissive: LinearRgba::rgb(2.4, 1.4, 0.2),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.14, 0.78),
            emissive: LinearRgba::rgb(1.4, 0.4, 2.2),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.92, 0.48),
            emissive: LinearRgba::rgb(0.3, 1.8, 0.7),
            ..default()
        }),
    ];

    // Spawn 16 inner skyscrapers + 20 outer skyline towers
    let mut b_idx = 0_usize;
    let block_all = [-114.0_f32, -69.0, -23.0, 23.0, 69.0, 114.0];
    for &bx in &block_all {
        for &bz in &block_all {
            let is_outer = bx.abs() > 90.0 || bz.abs() > 90.0;
            // Sidewalk base on every inner block (no PlatformBox so player feet never snag)
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(sidewalk_mat.clone()),
                Transform::from_xyz(bx, 0.04, bz).with_scale(Vec3::new(26.5, 0.08, 26.5)),
            ));

            let height = 24.0 + ((b_idx * 17 + 7) % 34) as f32 + if is_outer { 12.0 } else { 0.0 };
            let footprint = if is_outer { 18.0 } else { 17.0 };
            let b_mat = building_mats[b_idx % building_mats.len()].clone();

            commands
                .spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(meshes.add(Cuboid::new(footprint, height, footprint))),
                    MeshMaterial3d(b_mat.clone()),
                    Transform::from_xyz(bx, height * 0.5, bz),
                    PlatformBox {
                        half_extents: Vec3::new(footprint * 0.5, height * 0.5, footprint * 0.5),
                    },
                ))
                .with_children(|b| {
                    // Architectural setback roof penthouse & spire
                    b.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(b_mat.clone()),
                        Transform::from_xyz(0.0, height * 0.5 + 3.2, 0.0)
                            .with_scale(Vec3::new(footprint * 0.62, 6.4, footprint * 0.62)),
                    ));
                    if b_idx % 2 == 0 {
                        b.spawn((
                            Mesh3d(unit_cyl.clone()),
                            MeshMaterial3d(white_stripe.clone()),
                            Transform::from_xyz(0.0, height * 0.5 + 10.0, 0.0)
                                .with_scale(Vec3::new(0.28, 7.8, 0.28)),
                        ));
                    }
                    // Illuminated floor window bands on all 4 facades
                    let bands = ((height - 6.0) / 5.5).floor() as i32;
                    for floor_i in 0..bands.min(9) {
                        let ly = -height * 0.5 + 4.5 + floor_i as f32 * 5.2;
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(window_glow.clone()),
                            Transform::from_xyz(0.0, ly, 0.0)
                                .with_scale(Vec3::new(footprint + 0.22, 1.35, footprint * 0.84)),
                        ));
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(window_glow.clone()),
                            Transform::from_xyz(0.0, ly, 0.0)
                                .with_scale(Vec3::new(footprint * 0.84, 1.35, footprint + 0.22)),
                        ));
                    }
                    // Times Square Neon Billboard facing 5th Avenue on central towers
                    if bx.abs() < 35.0 && !is_outer {
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(billboard_mats[b_idx % billboard_mats.len()].clone()),
                            Transform::from_xyz(0.0, -height * 0.5 + 11.0, footprint * 0.5 + 0.35)
                                .with_scale(Vec3::new(10.5, 4.4, 0.4)),
                        ));
                    }
                });
            b_idx += 1;
        }
    }

    // Iconic Tiered EMPIRE CITADEL TOWER at (0, 0, -82)
    let citadel_stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.72, 0.68),
        perceptual_roughness: 0.52,
        metallic: 0.22,
        ..default()
    });
    let citadel_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.24),
        metallic: 0.82,
        perceptual_roughness: 0.24,
        ..default()
    });
    // Base Lobby (top = 16.0)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(Cuboid::new(22.0, 16.0, 22.0))),
        MeshMaterial3d(citadel_stone.clone()),
        Transform::from_xyz(CITADEL_POS.x, 8.0, CITADEL_POS.z),
        PlatformBox {
            half_extents: Vec3::new(11.0, 8.0, 11.0),
        },
    ));
    // Lexie Main Balcony Tier (y = 16..38 -> top = 38.0!)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(Cuboid::new(18.0, 22.0, 18.0))),
        MeshMaterial3d(citadel_stone.clone()),
        Transform::from_xyz(CITADEL_POS.x, 27.0, CITADEL_POS.z),
        PlatformBox {
            half_extents: Vec3::new(9.0, 11.0, 9.0),
        },
    ));
    // Upper King-Kong Spire Shaft + Golden Needle behind Lexie Terrace (y = 38..84)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(meshes.add(Cuboid::new(9.5, 24.0, 9.5))),
            MeshMaterial3d(citadel_stone),
            Transform::from_xyz(CITADEL_POS.x, 50.0, CITADEL_POS.z - 4.0),
        ))
        .with_children(|sp| {
            sp.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(citadel_gold.clone()),
                Transform::from_xyz(0.0, 14.5, 0.0).with_scale(Vec3::new(4.2, 5.5, 4.2)),
            ));
            sp.spawn((
                Mesh3d(unit_cone.clone()),
                MeshMaterial3d(citadel_gold),
                Transform::from_xyz(0.0, 22.0, 0.0).with_scale(Vec3::new(1.5, 11.0, 1.5)),
            ));
        });

    // Spawn Colossal Overweight Blonde LEXIE atop Empire Citadel Tower Terrace!
    spawn_lexie_boss(
        commands,
        meshes,
        materials,
        unit_box.clone(),
        unit_sphere.clone(),
        unit_cyl.clone(),
    );

    // Spawn 50 Collectible Golden Tendies across open Manhattan Streets, Avenues & Plazas
    spawn_50_city_tendies(commands, meshes, materials);

    // Spawn 28 Moving NYC Cars & Taxis on Avenues & Streets
    spawn_city_traffic(
        commands,
        materials,
        unit_box.clone(),
        unit_cyl.clone(),
    );

    // Spawn 38 Sidewalk Manhattan NPCs
    spawn_city_npcs(commands, materials, unit_box, unit_sphere);
}

fn spawn_lexie_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    unit_box: Handle<Mesh>,
    unit_sphere: Handle<Mesh>,
    unit_cyl: Handle<Mesh>,
) {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.68),
        perceptual_roughness: 0.45,
        ..default()
    });
    let blonde_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.84, 0.28),
        perceptual_roughness: 0.38,
        metallic: 0.08,
        ..default()
    });
    let pink_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.22, 0.56),
        perceptual_roughness: 0.62,
        ..default()
    });
    let denim_shorts = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.36, 0.62),
        perceptual_roughness: 0.78,
        ..default()
    });
    let eye_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.05, 0.07),
        ..default()
    });
    let red_lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.14, 0.24),
        perceptual_roughness: 0.25,
        ..default()
    });
    let jar_glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.68, 0.92, 0.84, 0.46),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.12,
        metallic: 0.1,
        ..default()
    });
    let garlic_clove = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.94, 0.74),
        emissive: LinearRgba::rgb(0.32, 0.36, 0.08),
        perceptual_roughness: 0.32,
        ..default()
    });
    let garlic_label = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.74, 0.28),
        emissive: LinearRgba::rgb(0.18, 0.48, 0.12),
        ..default()
    });
    let sneaker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.5,
        ..default()
    });

    // King-Kong Colossus Scale (~29m tall total silhouette!)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            LexieBoss {
                hp: 300.0,
                max_hp: 300.0,
                on_tower: true,
                throw_cooldown: 4.8,
                chew_timer: 0.0,
                debris_timer: 0.3,
                hit_flash: 0.0,
                roar_cooldown: 4.0,
            },
            Transform::from_translation(LEXIE_TERRACE_POS).with_scale(Vec3::splat(5.8)),
        ))
        .with_children(|lx| {
            // Chunky sneakers + thick legs & wide hips (overweight King-Kong titan silhouette)
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(sneaker.clone()),
                    Transform::from_xyz(sx * 0.46, 0.18, 0.10).with_scale(Vec3::new(0.46, 0.36, 0.68)),
                ));
                lx.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.46, 0.88, 0.0).with_scale(Vec3::new(0.32, 1.25, 0.34)),
                ));
                lx.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(denim_shorts.clone()),
                    Transform::from_xyz(sx * 0.44, 1.56, 0.0).with_scale(Vec3::new(0.56, 0.46, 0.54)),
                ));
            }
            // Wide denim waist + prominent rounded belly & curvy broad upper torso
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(denim_shorts),
                Transform::from_xyz(0.0, 1.82, 0.04).with_scale(Vec3::new(1.12, 0.52, 0.94)),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::from_xyz(0.0, 2.38, 0.24).with_scale(Vec3::new(1.18, 0.84, 1.06)),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::from_xyz(0.0, 2.98, 0.0).with_scale(Vec3::new(1.06, 0.72, 0.82)),
            ));

            // Plump Head, Double Chin, Glaring Eyes, Red Lipstick & Golden-Blonde Cascading Hair
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 3.92, 0.06).with_scale(Vec3::new(0.64, 0.60, 0.58)),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 3.55, 0.20).with_scale(Vec3::new(0.48, 0.24, 0.44)),
            ));
            // Eyes + Angry Brows + Chewing Red Mouth (Local +Z faces down 5th Avenue toward Stan!)
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(eye_dark.clone()),
                    Transform::from_xyz(sx * 0.20, 4.00, 0.58).with_scale(Vec3::splat(0.085)),
                ));
                lx.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(sx * 0.21, 4.13, 0.59)
                        .with_rotation(Quat::from_rotation_z(-sx * 0.28))
                        .with_scale(Vec3::new(0.24, 0.06, 0.06)),
                ));
            }
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(red_lip),
                Transform::from_xyz(0.0, 3.74, 0.62).with_scale(Vec3::new(0.18, 0.09, 0.08)),
            ));

            // Lush Golden Blonde Hair: crown + cascading side locks + back mane + messy bun
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 4.18, -0.05).with_scale(Vec3::new(0.76, 0.56, 0.72)),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 3.62, -0.34).with_scale(Vec3::new(0.92, 0.96, 0.52)),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 4.72, -0.14).with_scale(Vec3::splat(0.42)),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(sx * 0.54, 3.65, 0.02).with_scale(Vec3::new(0.32, 0.85, 0.42)),
                ));
            }

            // Left Arm holding giant PICKLED GARLIC CLOVES Glass Jar
            lx.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::from_xyz(-1.12, 2.74, 0.36)
                    .with_rotation(Quat::from_rotation_x(-0.85) * Quat::from_rotation_z(-0.25))
                    .with_scale(Vec3::new(0.34, 1.05, 0.34)),
            ));
            // Jar Glass + Green Label + Floating Ivory Pickled Garlic Cloves
            lx.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(jar_glass),
                Transform::from_xyz(-1.18, 2.66, 1.02).with_scale(Vec3::new(0.52, 0.95, 0.52)),
            ));
            lx.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(garlic_label),
                Transform::from_xyz(-1.18, 2.66, 1.02).with_scale(Vec3::new(0.535, 0.36, 0.535)),
            ));
            for &(gx, gy, gz) in &[
                (-1.26_f32, 2.48_f32, 1.00_f32),
                (-1.08, 2.58, 1.08),
                (-1.20, 2.82, 0.96),
                (-1.14, 2.94, 1.04),
            ] {
                lx.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(garlic_clove.clone()),
                    Transform::from_xyz(gx, gy, gz).with_scale(Vec3::new(0.14, 0.22, 0.13)),
                ));
            }

            // Right Throwing Arm gripping a glowing Pickled Garlic Clove
            lx.spawn((
                LexieThrowArm,
                Mesh3d(unit_cyl),
                MeshMaterial3d(pink_hoodie),
                Transform::from_xyz(1.16, 3.16, 0.28)
                    .with_rotation(Quat::from_rotation_x(0.65) * Quat::from_rotation_z(0.28))
                    .with_scale(Vec3::new(0.34, 1.15, 0.34)),
            ))
            .with_children(|arm| {
                arm.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(skin),
                    Transform::from_xyz(0.0, 0.58, 0.0).with_scale(Vec3::splat(0.58)),
                ));
                arm.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.36))),
                    MeshMaterial3d(garlic_clove),
                    Transform::from_xyz(0.0, 0.92, 0.0).with_scale(Vec3::new(0.85, 1.25, 0.85)),
                ));
            });
        });
}

fn spawn_50_city_tendies(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let tendy_mesh = meshes.add(Capsule3d::new(0.45, 0.95));
    let tendy_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.64, 0.16),
        emissive: LinearRgba::rgb(0.62, 0.32, 0.04),
        perceptual_roughness: 0.42,
        ..default()
    });
    let beacon_mesh = meshes.add(Cylinder::new(0.28, 12.0));
    let beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.25, 0.32),
        emissive: LinearRgba::rgb(1.4, 0.95, 0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // Generate exactly 50 well-distributed street & avenue coordinates guaranteed clear of buildings
    let mut spots = Vec::with_capacity(50);
    // 25 along the 5 N-S Avenues (-92, -46, 0, 46, 92)
    for (ai, &ax) in ROAD_COORDS.iter().enumerate() {
        for zi in 0..5 {
            let z = -104.0 + zi as f32 * 52.0 + if ai == 2 { 10.0 } else { 0.0 };
            let candidate = Vec3::new(ax + if zi % 2 == 0 { -6.5 } else { 6.5 }, 1.0, z);
            if is_clear_of_skyscrapers(candidate.x, candidate.z, 1.5) {
                spots.push(candidate);
            }
        }
    }
    // 25 along East-West cross streets & sidewalk corners
    for &sz in &ROAD_COORDS {
        for xi in 0..6 {
            if spots.len() >= 50 {
                break;
            }
            let x = -112.0 + xi as f32 * 44.5;
            let candidate = Vec3::new(x, 1.0, sz + if xi % 2 == 0 { 6.0 } else { -6.0 });
            let too_close = spots
                .iter()
                .any(|p| p.distance_squared(candidate) < 85.0);
            if !too_close && is_clear_of_skyscrapers(candidate.x, candidate.z, 1.5) {
                spots.push(candidate);
            }
        }
    }
    // Deterministically top up to exactly 50 if any overlap check trimmed one
    let mut step = 0_usize;
    while spots.len() < 50 {
        let ax = ROAD_COORDS[step % ROAD_COORDS.len()];
        let sz = -108.0 + ((step * 29) % 216) as f32;
        let cand = Vec3::new(ax, 1.0, sz);
        let too_close = spots.iter().any(|p| p.distance_squared(cand) < 42.0);
        if !too_close && is_clear_of_skyscrapers(cand.x, cand.z, 1.5) {
            spots.push(cand);
        }
        step += 1;
    }

    for (i, pos) in spots.into_iter().enumerate() {
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityTendy,
                TendyPickup {
                    base_y: pos.y,
                    phase: i as f32 * 0.55,
                    is_final_goal: false,
                },
                Mesh3d(tendy_mesh.clone()),
                MeshMaterial3d(tendy_mat.clone()),
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_z(0.55)),
            ))
            .with_children(|t| {
                // Tall faint golden sky-beacon so players spot distant Manhattan tendies!
                t.spawn((
                    Mesh3d(beacon_mesh.clone()),
                    MeshMaterial3d(beacon_mat.clone()),
                    Transform::from_xyz(0.0, 5.5, 0.0)
                        .with_rotation(Quat::from_rotation_z(-0.55)),
                ));
            });
    }
}

fn spawn_city_traffic(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    unit_box: Handle<Mesh>,
    unit_cyl: Handle<Mesh>,
) {
    let body_colors = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.78, 0.08), // NYC Yellow Cab
            perceptual_roughness: 0.25,
            metallic: 0.3,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.76, 0.08), // Another NYC Yellow Cab
            perceptual_roughness: 0.25,
            metallic: 0.3,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.16, 0.18), // Red Sedan
            perceptual_roughness: 0.28,
            metallic: 0.4,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.42, 0.86), // NYPD / Blue Cruiser
            perceptual_roughness: 0.28,
            metallic: 0.4,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.90, 0.90, 0.92), // Delivery Van
            perceptual_roughness: 0.35,
            ..default()
        }),
    ];
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.22, 0.32),
        metallic: 0.6,
        perceptual_roughness: 0.15,
        ..default()
    });
    let tire = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.09, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let headlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.98, 0.80),
        emissive: LinearRgba::rgb(2.2, 2.0, 1.3),
        ..default()
    });
    let taillight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.15, 0.15),
        emissive: LinearRgba::rgb(2.2, 0.2, 0.2),
        ..default()
    });

    // Spawn 28 moving NYC vehicles distributed across both N-S Avenues and E-W Cross-Streets
    for i in 0..28_usize {
        let axis_x = i % 2 == 1;
        let dir = if (i / 2) % 2 == 0 { 1.0 } else { -1.0 };
        let road = ROAD_COORDS[(i / 3) % ROAD_COORDS.len()];
        let lane_offset = if dir > 0.0 { 3.6 } else { -3.6 };
        let lane_coord = road + lane_offset;
        let progress = -120.0 + ((i as f32 * 37.0) % 240.0);
        let pos = if axis_x {
            Vec3::new(progress, 0.0, lane_coord)
        } else {
            Vec3::new(lane_coord, 0.0, progress)
        };
        let yaw = if axis_x {
            if dir > 0.0 {
                std::f32::consts::FRAC_PI_2
            } else {
                -std::f32::consts::FRAC_PI_2
            }
        } else if dir > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };

        let speed = 13.5 + (i % 5) as f32 * 2.4;
        let mat = body_colors[i % body_colors.len()].clone();

        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityCar {
                    axis_x,
                    dir,
                    speed,
                    horn_cooldown: 0.0,
                    occupied: false,
                    parked: false,
                    yaw,
                    velocity: Vec2::ZERO,
                    pitch_lean: 0.0,
                    roll_lean: 0.0,
                    steer_input: 0.0,
                    knock_vel: Vec2::ZERO,
                    drift_sfx_cooldown: 0.0,
                    car_hp: 100.0,
                },
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            ))
            .with_children(|car| {
                // Chassis
                car.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, 0.72, 0.0).with_scale(Vec3::new(2.15, 0.82, 4.45)),
                ));
                // Cabin glass
                car.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(glass.clone()),
                    Transform::from_xyz(0.0, 1.38, -0.15).with_scale(Vec3::new(1.88, 0.62, 2.25)),
                ));
                // Headlights (+Z front) & Taillights (-Z rear)
                for sx in [-0.72_f32, 0.72_f32] {
                    car.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(headlight.clone()),
                        Transform::from_xyz(sx, 0.68, 2.24).with_scale(Vec3::new(0.42, 0.24, 0.08)),
                    ));
                    car.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(taillight.clone()),
                        Transform::from_xyz(sx, 0.68, -2.24).with_scale(Vec3::new(0.42, 0.24, 0.08)),
                    ));
                }
                // 4 Wheels
                for (wx, wz, is_front) in [
                    (-1.05_f32, 1.32_f32, true),
                    (1.05, 1.32, true),
                    (-1.05, -1.32, false),
                    (1.05, -1.32, false),
                ] {
                    car.spawn((
                        CityCarWheel { is_front },
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(tire.clone()),
                        Transform::from_xyz(wx, 0.36, wz)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::new(0.36, 0.28, 0.36)),
                    ));
                }
            });
    }
}

fn spawn_city_npcs(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    unit_box: Handle<Mesh>,
    unit_sphere: Handle<Mesh>,
) {
    let coats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.42, 0.78),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.24, 0.26),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.64, 0.36),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.62, 0.16),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.30, 0.78),
            ..default()
        }),
    ];
    let pants = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.20, 0.24),
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.12, 0.10),
        ..default()
    });

    // Spawn 38 New Yorkers patrolling sidewalks safely outside skyscraper footprints
    for i in 0..38_usize {
        let bx = BLOCK_COORDS[i % BLOCK_COORDS.len()];
        let bz = BLOCK_COORDS[(i / 4) % BLOCK_COORDS.len()];
        let side = (i % 4) as f32;
        let offset = 10.6;
        let home = match i % 4 {
            0 => Vec3::new(bx + offset, 0.08, bz),
            1 => Vec3::new(bx - offset, 0.08, bz),
            2 => Vec3::new(bx, 0.08, bz + offset),
            _ => Vec3::new(bx, 0.08, bz - offset),
        };
        let coat = coats[i % coats.len()].clone();

        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityNpc {
                    home_x: home.x,
                    home_z: home.z,
                    patrol_radius: 10.5,
                    angle: side * 1.57,
                    speed: 2.2 + (i % 4) as f32 * 0.45,
                    walk_phase: i as f32 * 0.85,
                },
                Transform::from_translation(home),
            ))
            .with_children(|n| {
                // Torso
                n.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(coat),
                    Transform::from_xyz(0.0, 1.05, 0.0).with_scale(Vec3::new(0.52, 0.72, 0.30)),
                ));
                // Head + hair
                n.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, 1.62, 0.0).with_scale(Vec3::splat(0.22)),
                ));
                n.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(hair.clone()),
                    Transform::from_xyz(0.0, 1.69, -0.03).with_scale(Vec3::splat(0.225)),
                ));
                // Animated legs & arms
                for &sx in &[-1.0_f32, 1.0_f32] {
                    n.spawn((
                        CityNpcLimb { side: sx, is_arm: false },
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(pants.clone()),
                        Transform::from_xyz(sx * 0.14, 0.36, 0.0)
                            .with_scale(Vec3::new(0.18, 0.72, 0.18)),
                    ));
                    n.spawn((
                        CityNpcLimb { side: sx, is_arm: true },
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(sx * 0.34, 1.04, 0.0)
                            .with_scale(Vec3::new(0.14, 0.56, 0.14)),
                    ));
                }
            });
    }
}

// =========================================================================
// PART 3: CINEMATIC AERIAL NEW YORK CITY INTRO CUTSCENE
// =========================================================================

pub fn enter_city_intro_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    existing_city_q: Query<Entity, With<CityWorldMarker>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::None;
    }

    stats.in_city = true;
    stats.boss_mode = false;
    stats.tendies = 0;
    stats.total_tendies = 50;
    stats.wave = 4;
    stats.hearts = stats.max_hearts;
    stats.player_scale = 1.8;
    stats.lexie_defeated = false;
    stats.hint_text.clear();

    let start_t = take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;

    set_touch_playing(false);
    set_touch_boss_mode(false);

    spawn_nyc_world_if_needed(
        &mut commands,
        &mut meshes,
        &mut materials,
        &existing_city_q,
        &old_level_q,
    );

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(0.0, 0.0, 82.0);
        p_tf.scale = Vec3::splat(1.8);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0; // Facing North (-Z) straight down 5th Avenue toward Empire Citadel!
        p_tf.rotation = Quat::from_rotation_y(0.0);
    }

    play_viking_chant_music();
    play_sfx("thunder");

    commands
        .spawn((
            CityIntroOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(14.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                    width: Val::Percent(94.0),
                    max_width: Val::Px(800.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.07, 0.14, 0.90)),
                BorderColor(Color::srgb(1.0, 0.80, 0.18)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        CityIntroSpeakerText,
                        Text::new("NEW YORK CITY - AERIAL OVERLOOK"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.84, 0.25)),
                    ));
                    hdr.spawn((
                        CityIntroBeatBadge,
                        Text::new("MANHATTAN | SCENE 1/4"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));
                });
                card.spawn((
                    CityIntroSubtitleText,
                    Text::new(
                        "Welcome to New York City! Towering skyscrapers, rushing taxis, crowds, and 50 golden tendies stretch across Manhattan!",
                    ),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    if stats.cutscene_timer < 1.0 {
        speak_voice(
            "Welcome to New York City! Towering skyscrapers, speeding cars, and fifty golden tendies stretch across Manhattan!",
            "narrator",
        );
    }
}

pub fn update_city_intro_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    let t = stats.cutscene_timer;

    let (beat_idx, speech, role, sfx): (u8, &str, &str, Option<&str>) = if t < 5.6 {
        (0, "", "narrator", None)
    } else if t < 11.2 {
        (
            1,
            "High atop the Empire Citadel Tower, a King-Kong-sized menace is tearing Manhattan apart!",
            "narrator",
            Some("thunder"),
        )
    } else if t < 17.5 {
        (
            2,
            "Ha ha ha! Nobody stops Lexie and her extra-stinky pickled garlic cloves! Eat garlic fumes, tiny city!",
            "lexie",
            Some("garlic_throw"),
        )
    } else {
        (
            3,
            "Collect fifty golden tendies using your radar map, grow as tall as a skyscraper, and defeat Lexie!",
            "narrator",
            Some("tendy"),
        )
    };

    let mut active_beat = stats.cutscene_beat;
    if beat_idx > stats.cutscene_beat && !is_voice_speaking() {
        stats.cutscene_beat = beat_idx;
        active_beat = beat_idx;
        if let Some(name) = sfx {
            play_sfx(name);
        }
        if !speech.is_empty() {
            speak_voice(speech, role);
        }
    } else if beat_idx > stats.cutscene_beat && is_voice_speaking() {
        stats.cutscene_timer -= dt;
    }

    // Sweeping Helicopter Aerial Camera panning massive Manhattan & Empire Citadel Tower
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        let (eye, target) = match active_beat {
            0 => {
                // High aerial sweep orbiting the huge 270m skyline!
                let u = (t / 5.6).clamp(0.0, 1.0);
                (
                    Vec3::new(42.0 + u * 26.0, 74.0 - u * 14.0, 96.0 - u * 18.0),
                    Vec3::new(0.0, 20.0, -32.0),
                )
            }
            1 => {
                // Fast canyon race straight up 5th Avenue toward Empire Citadel Tower
                let u = ((t - 5.6) / 5.6).clamp(0.0, 1.0);
                (
                    Vec3::new(11.0 * (1.0 - u) + 18.0 * u, 22.0 + u * 36.0, 62.0 - u * 104.0),
                    Vec3::new(0.0, 26.0 + u * 22.0, -82.0),
                )
            }
            2 => {
                // Dramatic King-Kong orbit around colossal Lexie smashing tower & eating pickled garlic
                let u = ((t - 11.2) / 6.3).clamp(0.0, 1.0);
                let ang = 0.55 - u * 1.25;
                let rumble = (t * 28.0).sin() * 0.45;
                (
                    Vec3::new(
                        ang.sin() * 34.0 + rumble,
                        56.0 + u * 6.0 + rumble * 0.5,
                        -75.5 + ang.cos() * 32.0,
                    ),
                    Vec3::new(0.0, 52.0, -76.5),
                )
            }
            _ => {
                // Swoop down to Stan at South Broadway looking up the entire avenue at colossal Lexie
                let u = ((t - 17.5) / 5.0).clamp(0.0, 1.0);
                (
                    Vec3::new(6.0 * (1.0 - u), 6.0 + u * 14.0, 92.0 + u * 14.0),
                    Vec3::new(0.0, 4.0 + u * 28.0, 48.0 - u * 96.0),
                )
            }
        };
        cam_tf.translation = cam_tf.translation.lerp(eye, (dt * 4.0).min(1.0));
        cam_tf.look_at(target, Vec3::Y);
    }

    if active_beat >= 3 && t >= 23.0 && !is_voice_speaking() {
        next_state.set(GameState::CityPlay);
    }
}

pub fn update_city_intro_subtitles(
    stats: Res<PlayerStats>,
    mut spk_q: Query<&mut Text, With<CityIntroSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<CityIntroBeatBadge>, Without<CityIntroSpeakerText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<CityIntroSubtitleText>,
            Without<CityIntroSpeakerText>,
            Without<CityIntroBeatBadge>,
        ),
    >,
) {
    let (speaker, badge, line) = match stats.cutscene_beat {
        0 => (
            "NEW YORK CITY - AERIAL OVERLOOK",
            "MANHATTAN | SCENE 1/4",
            "\"Welcome to New York City! Towering skyscrapers, speeding cars, crowds, and 50 golden tendies span across Manhattan!\"",
        ),
        1 => (
            "EMPIRE CITADEL TOWER",
            "MANHATTAN | SCENE 2/4",
            "\"Look! High atop the Empire Citadel Tower, a King-Kong-sized colossus is tearing the skyline apart!\"",
        ),
        2 => (
            "LEXIE - PICKLED GARLIC TITAN",
            "MANHATTAN | SCENE 3/4",
            "\"HAHAHA! Nobody stops Lexie and her extra-stinky Pickled Garlic Cloves! Eat garlic fumes, tiny New York!\"",
        ),
        _ => (
            "MISSION BRIEFING - SKYSCRAPER TITAN",
            "MANHATTAN | SCENE 4/4",
            "\"Use the corner radar to collect all 50 TENDIES! Grow to SKYSCRAPER SCALE, dodge the cars & garlic fumes, and defeat Lexie!\"",
        ),
    };
    if let Ok(mut t) = spk_q.get_single_mut() {
        t.0 = speaker.to_string();
    }
    if let Ok(mut t) = badge_q.get_single_mut() {
        t.0 = badge.to_string();
    }
    if let Ok(mut t) = sub_q.get_single_mut() {
        t.0 = line.to_string();
    }
}

pub fn exit_city_intro_cutscene(
    mut commands: Commands,
    ui_q: Query<Entity, With<CityIntroOverlay>>,
) {
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
}

// =========================================================================
// PART 4: NEW YORK CITY GAMEPLAY, MINIMAP, TRAFFIC, NPCS & LEXIE GARLIC BOSS
// =========================================================================

pub fn enter_city_play(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    existing_city_q: Query<Entity, With<CityWorldMarker>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }

    spawn_nyc_world_if_needed(
        &mut commands,
        &mut meshes,
        &mut materials,
        &existing_city_q,
        &old_level_q,
    );

    let resuming_from_tj = stats.tj_return_city;
    stats.tj_return_city = false;
    let is_titan_preview = !resuming_from_tj && stats.in_city && stats.tendies >= 50;
    stats.in_city = true;
    stats.boss_mode = true;
    stats.total_tendies = 50;
    stats.wave = 4;

    if resuming_from_tj {
        stats.hint_text =
            "TJ JOINED YOUR QUEST! Collect 50 Tendies & take down Lexie together!".to_string();
    } else {
        stats.boss_hp = 300.0;
        stats.boss_max_hp = 300.0;
        stats.hearts = stats.max_hearts;
        stats.estus_flasks = stats.max_estus_flasks;
        stats.lexie_defeated = false;

        if is_titan_preview {
            stats.player_scale = 18.0;
            stats.hint_text =
                "SKYSCRAPER KAIJU! Slash Lexie with your Lightsaber [CLICK / SLASH] & Dodge Roll garlic!"
                    .to_string();
        } else {
            stats.tendies = 0;
            stats.player_scale = 1.8;
            stats.hint_text =
                "Collect 50 Tendies (use top-right Radar)! Meet TJ & Dodge Lexie's Pickled Garlic!"
                    .to_string();
        }
    }

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = if resuming_from_tj {
            stats.saved_city_pos
        } else {
            Vec3::new(
                if is_titan_preview { 4.2 } else { 0.0 },
                0.0,
                if is_titan_preview { -18.0 } else { 82.0 },
            )
        };
        p_tf.scale = Vec3::splat(stats.player_scale);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0;
        p_tf.rotation = Quat::from_rotation_y(0.0);
    }

    orbit.yaw = if is_titan_preview { -0.24 } else { 0.0 };
    orbit.pitch = if is_titan_preview { 0.42 } else { 0.36 };
    orbit.distance = if is_titan_preview { 15.5 } else { 11.2 };
    orbit.current_dist = orbit.distance;

    set_touch_playing(true);
    set_touch_boss_mode(true);
    play_viking_chant_music();

    // Spawn Lexie Top-Center Boss Bar + Corner NYC Tendy Radar Minimap
    commands
        .spawn((
            CityPlayUi,
            LexieBossHudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                bottom: Val::Auto,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Percent(44.0),
                    min_width: Val::Px(220.0),
                    max_width: Val::Px(460.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.04, 0.12, 0.88)),
                BorderColor(Color::srgb(0.95, 0.32, 0.68)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    LexieBossHudText,
                    Text::new("LEXIE - KING-KONG PICKLED GARLIC QUEEN"),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.84, 0.32)),
                ));
                bar.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.20, 0.14, 0.18)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|track| {
                    track.spawn((
                        LexieBossHpFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.94, 0.22, 0.56)),
                        BorderRadius::all(Val::Px(6.0)),
                    ));
                });
            });
        });

    // Top-Right Corner NYC Radar Minimap (clears Score box and mobile Help/Sound pills)
    commands
        .spawn((
            CityPlayUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(132.0),
                right: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.88)),
            BorderColor(Color::srgb(0.45, 0.86, 1.0)),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|box_ui| {
            box_ui.spawn((
                Text::new("NYC TENDY RADAR"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.28)),
            ));
            box_ui
                .spawn((
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(112.0),
                        height: Val::Px(112.0),
                        overflow: Overflow::clip(),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.13, 0.20, 0.95)),
                    BorderColor(Color::srgba(0.55, 0.72, 0.88, 0.45)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|radar| {
                    // Subtle Avenue & Cross-Street radar grid lines
                    for pct in [16.0_f32, 33.0, 50.0, 67.0, 84.0] {
                        radar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(pct),
                                top: Val::Px(0.0),
                                width: Val::Px(1.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.45, 0.58, 0.72, 0.22)),
                        ));
                        radar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Percent(pct),
                                left: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Px(1.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.45, 0.58, 0.72, 0.22)),
                        ));
                    }

                    // 50 Pooled Golden Tendy Dots
                    for _ in 0..50 {
                        radar.spawn((
                            CityMinimapTendyDot,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(50.0),
                                top: Val::Percent(50.0),
                                width: Val::Px(4.0),
                                height: Val::Px(4.0),
                                margin: UiRect::all(Val::Px(-2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.84, 0.14)),
                            BorderRadius::all(Val::Px(2.0)),
                        ));
                    }

                    // Magenta Boss Dot (Lexie)
                    radar.spawn((
                        CityMinimapBossDot,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(20.0),
                            width: Val::Px(9.0),
                            height: Val::Px(9.0),
                            margin: UiRect::all(Val::Px(-4.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.96, 0.22, 0.62)),
                        BorderRadius::all(Val::Px(5.0)),
                    ));

                    // Bright Cyan Player Dot (Stan)
                    radar.spawn((
                        CityMinimapPlayerDot,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(80.0),
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            margin: UiRect::all(Val::Px(-4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.96, 1.0)),
                        BorderRadius::all(Val::Px(4.0)),
                    ));
                });
        });
}

pub fn exit_city_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ui_q: Query<Entity, With<CityPlayUi>>,
    mut player_q: Query<&mut Visibility, With<Player>>,
) {
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    stats.driving_car = None;
    if let Ok(mut vis) = player_q.get_single_mut() {
        *vis = Visibility::Visible;
    }
    set_touch_boss_mode(false);
}

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

pub fn update_city_driving(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player, &mut Visibility)>,
    mut car_q: Query<
        (Entity, &mut Transform, &mut CityCar, &Children),
        (
            Without<Player>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut wheel_q: Query<
        (&mut Transform, &CityCarWheel),
        (
            Without<Player>,
            Without<CityCar>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    platform_q: Query<
        (&Transform, &PlatformBox),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut npc_q: Query<
        (&mut Transform, &mut CityNpc),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut lexie_q: Query<
        (&Transform, &mut LexieBoss),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<GarlicProjectile>,
        ),
    >,
    garlic_q: Query<
        (Entity, &Transform),
        (
            With<GarlicProjectile>,
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut p_tf, mut player, mut p_vis)) = player_q.get_single_mut() else {
        return;
    };

    let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

    // 1. If currently on foot, check for nearby hijackable car
    if stats.driving_car.is_none() {
        if stats.player_scale >= 6.0 {
            return;
        }
        let mut nearest: Option<(Entity, f32)> = None;
        for (car_e, car_tf, _car, _) in &car_q {
            let d = Vec2::new(
                car_tf.translation.x - p_tf.translation.x,
                car_tf.translation.z - p_tf.translation.z,
            )
            .length();
            if d < 5.4 && nearest.map_or(true, |(_, bd)| d < bd) {
                nearest = Some((car_e, d));
            }
        }
        if let Some((car_e, _)) = nearest {
            if interact {
                if let Ok((_, mut car_tf, mut car, _)) = car_q.get_mut(car_e) {
                    car.occupied = true;
                    car.parked = false;
                    car.car_hp = car.car_hp.max(40.0);
                    let fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
                    if !car.parked && car.velocity.length_squared() < 1.0 {
                        car.velocity = fwd * (car.speed * 0.45);
                    }
                    stats.driving_car = Some(car_e);
                    *p_vis = Visibility::Hidden;
                    p_tf.translation = car_tf.translation;
                    player.velocity = Vec3::ZERO;
                    player.grounded = true;
                    orbit.yaw = wrap_angle(car.yaw + std::f32::consts::PI);
                    car_tf.rotation = Quat::from_rotation_y(car.yaw);
                    play_sfx("car_door");
                    play_sfx("car_horn");
                    stats.hint_text =
                        "HIJACKED! [WASD] Drive | [SPACE] Drift | [SHIFT] Nitro | [G] Horn | [F/E] Exit"
                            .to_string();
                }
            } else if stats.hint_text.starts_with("Collect 50")
                || stats.hint_text.starts_with("TJ JOINED")
                || stats.hint_text.starts_with("EXITED")
                || stats.hint_text.is_empty()
            {
                stats.hint_text = "[F / E] Hijack & Drive Car!".to_string();
            }
        } else if stats.hint_text == "[F / E] Hijack & Drive Car!" {
            stats.hint_text =
                "Collect 50 Tendies (use top-right Radar)! Meet TJ & Dodge Lexie's Pickled Garlic!"
                    .to_string();
        }
        return;
    }

    let Some(driven_e) = stats.driving_car else {
        return;
    };

    // 2. Handle exit / GTA IV high-speed bail-out
    if interact {
        if let Ok((_, car_tf, mut car, _)) = car_q.get_mut(driven_e) {
            car.occupied = false;
            car.parked = true;
            let right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
            let speed = car.velocity.length();
            p_tf.translation = Vec3::new(
                car_tf.translation.x - right.x * 2.35,
                0.0,
                car_tf.translation.z - right.y * 2.35,
            );
            if speed > 14.0 {
                player.velocity = Vec3::new(car.velocity.x * 0.55, 5.2, car.velocity.y * 0.55);
                player.roll_timer = 0.55;
                player.invuln_timer = 0.8;
                play_sfx("dash");
                stats.hint_text = "GTA BAIL-OUT! Find another car or collect Tendies!".to_string();
            } else {
                player.velocity = Vec3::ZERO;
                play_sfx("car_door");
                stats.hint_text = "EXITED CAR! Press [F / E] near any car to drive.".to_string();
            }
        }
        stats.driving_car = None;
        *p_vis = Visibility::Visible;
        return;
    }

    // 3. Driving physics for the occupied car
    let mut driven_pos = Vec3::ZERO;
    let mut driven_vel = Vec2::ZERO;
    let mut driven_yaw = 0.0_f32;
    let honking = keys.just_pressed(KeyCode::KeyG) || mouse_buttons.just_pressed(MouseButton::Left);

    if let Ok((_, mut car_tf, mut car, children)) = car_q.get_mut(driven_e) {
        if car.horn_cooldown > 0.0 {
            car.horn_cooldown = (car.horn_cooldown - dt).max(0.0);
        }
        if car.drift_sfx_cooldown > 0.0 {
            car.drift_sfx_cooldown = (car.drift_sfx_cooldown - dt).max(0.0);
        }

        let mut throttle = 0.0_f32;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            throttle += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            throttle -= 1.0;
        }
        throttle += crate::audio::touch_move_y();
        throttle = throttle.clamp(-1.0, 1.0);

        let mut steer_cmd = 0.0_f32;
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            steer_cmd += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            steer_cmd -= 1.0;
        }
        steer_cmd -= crate::audio::touch_move_x();
        steer_cmd = steer_cmd.clamp(-1.0, 1.0);

        let handbrake = keys.pressed(KeyCode::Space) || crate::audio::take_touch_jump();
        let nitro_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let nitro_active = nitro_held && throttle > 0.1 && stats.dash_energy > 2.0;
        if nitro_active {
            stats.dash_energy = (stats.dash_energy - 38.0 * dt).max(0.0);
        }

        // Smooth steering input
        car.steer_input += (steer_cmd - car.steer_input) * (dt * 9.0).min(1.0);

        let fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
        let right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
        let mut fwd_speed = car.velocity.dot(fwd);
        let mut lat_speed = car.velocity.dot(right);

        // Longitudinal acceleration / braking / reverse
        let max_fwd = if nitro_active { 52.0 } else { 36.0 };
        let accel_force = if nitro_active { 46.0 } else { 29.0 };
        let mut pitch_target = 0.0_f32;

        if throttle > 0.05 {
            if fwd_speed < -1.0 {
                // Braking from reverse
                fwd_speed += 44.0 * dt;
                pitch_target = 0.085;
            } else {
                fwd_speed = (fwd_speed + accel_force * throttle * dt).min(max_fwd);
                pitch_target = if nitro_active { -0.095 } else { -0.055 };
            }
        } else if throttle < -0.05 {
            if fwd_speed > 1.5 {
                // Heavy GTA IV brake nose-dive
                fwd_speed = (fwd_speed - 48.0 * (-throttle) * dt).max(0.0);
                pitch_target = 0.105;
            } else {
                fwd_speed = (fwd_speed - 17.5 * (-throttle) * dt).max(-15.5);
                pitch_target = 0.04;
            }
        } else {
            // Rolling coast resistance
            fwd_speed *= (1.0 - 1.35 * dt).max(0.0);
        }

        if handbrake {
            fwd_speed *= (1.0 - 2.6 * dt).max(0.0);
            pitch_target += 0.05;
        }

        // Speed-sensitive steering (reverse flips steering direction)
        let speed_factor = (fwd_speed.abs() / 11.0).clamp(0.0, 1.0);
        let high_speed_damp = 1.0 - (fwd_speed.abs() / 65.0).clamp(0.0, 0.35);
        let dir_sign = if fwd_speed >= -0.4 { 1.0 } else { -1.0 };
        let drift_turn_mul = if handbrake { 1.48 } else { 1.0 };
        let turn_rate = 2.45 * speed_factor * high_speed_damp * drift_turn_mul * dir_sign;
        car.yaw = wrap_angle(car.yaw + car.steer_input * turn_rate * dt);

        // Recompute basis after yaw update, then damp lateral slip (handbrake = drift!)
        let new_fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
        let new_right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
        let lat_grip = if handbrake { 1.65 } else { 7.8 };
        lat_speed *= (1.0 - lat_grip * dt).max(0.0);

        if (handbrake || lat_speed.abs() > 7.5) && fwd_speed.abs() > 8.0 && car.drift_sfx_cooldown <= 0.0
        {
            play_sfx("car_drift");
            car.drift_sfx_cooldown = 0.28;
        }

        car.velocity = new_fwd * fwd_speed + new_right * lat_speed;

        // GTA IV heavy body roll & pitch squat/dive
        let roll_target = (-lat_speed * 0.017).clamp(-0.18, 0.18);
        car.roll_lean += (roll_target - car.roll_lean) * (dt * 8.5).min(1.0);
        car.pitch_lean += (pitch_target - car.pitch_lean) * (dt * 9.0).min(1.0);

        car_tf.translation.x = (car_tf.translation.x + car.velocity.x * dt).clamp(-128.0, 128.0);
        car_tf.translation.z = (car_tf.translation.z + car.velocity.y * dt).clamp(-128.0, 128.0);
        car_tf.translation.y = 0.0;

        // Building AABB collisions (bounce + scrape cleanly along skyscrapers)
        let wall_r = 1.95;
        for _ in 0..2 {
            for (b_tf, b_box) in &platform_q {
                if b_tf.translation.y - b_box.half_extents.y > 2.2 {
                    continue;
                }
                let min_x = b_tf.translation.x - b_box.half_extents.x - wall_r;
                let max_x = b_tf.translation.x + b_box.half_extents.x + wall_r;
                let min_z = b_tf.translation.z - b_box.half_extents.z - wall_r;
                let max_z = b_tf.translation.z + b_box.half_extents.z + wall_r;
                if car_tf.translation.x > min_x
                    && car_tf.translation.x < max_x
                    && car_tf.translation.z > min_z
                    && car_tf.translation.z < max_z
                {
                    let dx_l = car_tf.translation.x - min_x;
                    let dx_r = max_x - car_tf.translation.x;
                    let dz_b = car_tf.translation.z - min_z;
                    let dz_f = max_z - car_tf.translation.z;
                    let m = dx_l.min(dx_r).min(dz_b).min(dz_f);
                    let impact;
                    if (m - dx_l).abs() < 0.001 {
                        car_tf.translation.x = min_x;
                        impact = car.velocity.x.abs();
                        car.velocity.x = -car.velocity.x.max(0.0) * 0.28;
                    } else if (m - dx_r).abs() < 0.001 {
                        car_tf.translation.x = max_x;
                        impact = car.velocity.x.abs();
                        car.velocity.x = -car.velocity.x.min(0.0) * 0.28;
                    } else if (m - dz_b).abs() < 0.001 {
                        car_tf.translation.z = min_z;
                        impact = car.velocity.y.abs();
                        car.velocity.y = -car.velocity.y.max(0.0) * 0.28;
                    } else {
                        car_tf.translation.z = max_z;
                        impact = car.velocity.y.abs();
                        car.velocity.y = -car.velocity.y.min(0.0) * 0.28;
                    }
                    if impact > 9.5 && car.horn_cooldown <= 0.0 {
                        play_sfx("car_crash");
                        car.horn_cooldown = 0.35;
                        car.pitch_lean = 0.14;
                    }
                }
            }
        }

        car_tf.rotation = Quat::from_rotation_y(car.yaw)
            * Quat::from_rotation_x(car.pitch_lean)
            * Quat::from_rotation_z(car.roll_lean);

        // Steer front wheels visually
        for &child in children.iter() {
            if let Ok((mut w_tf, w)) = wheel_q.get_mut(child) {
                let steer_yaw = if w.is_front { car.steer_input * 0.42 } else { 0.0 };
                w_tf.rotation = Quat::from_rotation_y(steer_yaw)
                    * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            }
        }

        if honking && car.horn_cooldown <= 0.0 {
            play_sfx("car_horn");
            car.horn_cooldown = 0.32;
        }

        // Sync Stan's position to car for Tendy pickups, radar, and TJ trigger
        p_tf.translation = car_tf.translation;
        p_tf.rotation = Quat::from_rotation_y(car.yaw);

        // Smooth GTA IV chase camera auto-align while steering/driving
        if car.velocity.length() > 2.5 && !mouse_buttons.pressed(MouseButton::Right) {
            let target_cam_yaw = wrap_angle(car.yaw + std::f32::consts::PI);
            let diff = wrap_angle(target_cam_yaw - orbit.yaw);
            orbit.yaw = wrap_angle(orbit.yaw + diff * (dt * 2.8).min(1.0));
        }

        driven_pos = car_tf.translation;
        driven_vel = car.velocity;
        driven_yaw = car.yaw;
    }

    // 4. Car-to-Car GTA collisions
    let my_speed = driven_vel.length();
    let mut car_hit_impulse = Vec2::ZERO;
    for (other_e, mut other_tf, mut other_car, _) in &mut car_q {
        if other_e == driven_e {
            continue;
        }
        let delta = Vec2::new(
            other_tf.translation.x - driven_pos.x,
            other_tf.translation.z - driven_pos.z,
        );
        let dist = delta.length();
        if dist < 3.4 && dist > 0.001 {
            let n = delta / dist;
            other_tf.translation.x += n.x * (3.4 - dist);
            other_tf.translation.z += n.y * (3.4 - dist);
            if my_speed > 5.0 {
                other_car.knock_vel += n * (my_speed * 0.85) + driven_vel * 0.35;
                other_car.yaw = wrap_angle(other_car.yaw + (n.x * driven_vel.y - n.y * driven_vel.x) * 0.03);
                if other_car.horn_cooldown <= 0.0 {
                    play_sfx("car_crash");
                    play_sfx("car_horn");
                    other_car.horn_cooldown = 0.9;
                }
                car_hit_impulse -= n * (my_speed * 0.32);
            }
        }
    }
    if car_hit_impulse.length_squared() > 0.01 {
        if let Ok((_, _, mut car, _)) = car_q.get_mut(driven_e) {
            car.velocity += car_hit_impulse;
            car.pitch_lean = 0.12;
        }
    }

    // 5. Pedestrians scatter away from speeding/honking driven car
    if my_speed > 6.0 || honking {
        for (mut npc_tf, mut npc) in &mut npc_q {
            let diff = Vec2::new(
                npc_tf.translation.x - driven_pos.x,
                npc_tf.translation.z - driven_pos.z,
            );
            let dist = diff.length();
            if dist < 9.5 && dist > 0.01 {
                let away = diff / dist;
                npc_tf.translation.x += away.x * 14.0 * dt;
                npc_tf.translation.z += away.y * 14.0 * dt;
                npc.home_x = npc_tf.translation.x;
                npc.home_z = npc_tf.translation.z;
            }
        }
    }

    // 6. Smash Lexie's Garlic projectiles & ram Lexie at high speed!
    for (g_e, g_tf) in &garlic_q {
        let gd = Vec2::new(g_tf.translation.x - driven_pos.x, g_tf.translation.z - driven_pos.z).length();
        if gd < 2.9 && (g_tf.translation.y - driven_pos.y).abs() < 2.6 {
            commands.entity(g_e).despawn_recursive();
            play_sfx("car_crash");
        }
    }
    if my_speed > 11.0 {
        for (l_tf, mut lexie) in &mut lexie_q {
            if stats.lexie_defeated || lexie.on_tower {
                continue;
            }
            let ld = Vec2::new(l_tf.translation.x - driven_pos.x, l_tf.translation.z - driven_pos.z).length();
            if ld < 4.5 && lexie.hit_flash <= 0.0 {
                let dmg = 14.0 + my_speed * 0.35;
                stats.boss_hp = (stats.boss_hp - dmg).max(0.0);
                stats.score += 350;
                lexie.hit_flash = 0.45;
                play_sfx("car_crash");
                play_sfx("car_horn");
                stats.hint_text = format!("CAR RAMMED LEXIE! ({:.0} DMG!)", dmg);
                if let Ok((_, _, mut car, _)) = car_q.get_mut(driven_e) {
                    let bounce = Vec2::new(driven_pos.x - l_tf.translation.x, driven_pos.z - l_tf.translation.z).normalize_or_zero();
                    car.velocity = bounce * 10.0;
                    car.yaw = wrap_angle(driven_yaw + 0.35);
                }
            }
        }
    }
}

pub fn update_city_traffic(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut car_q: Query<(&mut Transform, &mut CityCar), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let in_active_play = *state.get() == GameState::CityPlay;
    let is_driving = stats.driving_car.is_some();
    let hit_radius = 1.9 + stats.player_scale * 0.22;

    for (mut car_tf, mut car) in &mut car_q {
        if car.occupied {
            continue;
        }
        if car.horn_cooldown > 0.0 {
            car.horn_cooldown = (car.horn_cooldown - dt).max(0.0);
        }

        if car.knock_vel.length_squared() > 0.05 {
            car_tf.translation.x = (car_tf.translation.x + car.knock_vel.x * dt).clamp(-132.0, 132.0);
            car_tf.translation.z = (car_tf.translation.z + car.knock_vel.y * dt).clamp(-132.0, 132.0);
            car.knock_vel *= (1.0 - 3.6 * dt).max(0.0);
        }

        if car.parked {
            car_tf.translation.x = (car_tf.translation.x + car.velocity.x * dt).clamp(-132.0, 132.0);
            car_tf.translation.z = (car_tf.translation.z + car.velocity.y * dt).clamp(-132.0, 132.0);
            car.velocity *= (1.0 - 2.2 * dt).max(0.0);
            car.pitch_lean *= (1.0 - 6.0 * dt).max(0.0);
            car.roll_lean *= (1.0 - 6.0 * dt).max(0.0);
            car_tf.rotation = Quat::from_rotation_y(car.yaw)
                * Quat::from_rotation_x(car.pitch_lean)
                * Quat::from_rotation_z(car.roll_lean);
            continue;
        }

        if car.axis_x {
            car_tf.translation.x += car.dir * car.speed * dt;
            if car_tf.translation.x > CITY_HALF_EXTENT {
                car_tf.translation.x = -CITY_HALF_EXTENT;
            } else if car_tf.translation.x < -CITY_HALF_EXTENT {
                car_tf.translation.x = CITY_HALF_EXTENT;
            }
        } else {
            car_tf.translation.z += car.dir * car.speed * dt;
            if car_tf.translation.z > CITY_HALF_EXTENT {
                car_tf.translation.z = -CITY_HALF_EXTENT;
            } else if car_tf.translation.z < -CITY_HALF_EXTENT {
                car_tf.translation.z = CITY_HALF_EXTENT;
            }
        }
        car_tf.rotation = Quat::from_rotation_y(car.yaw);

        if is_driving {
            continue;
        }

        // Check car impact with Stan (flying above street height clears cars cleanly!)
        let vertical_gap = (p_tf.translation.y - car_tf.translation.y).abs();
        let flat_dist = Vec2::new(
            p_tf.translation.x - car_tf.translation.x,
            p_tf.translation.z - car_tf.translation.z,
        )
        .length();
        if in_active_play
            && flat_dist < hit_radius
            && vertical_gap < (1.6 + stats.player_scale * 0.35)
            && player.roll_timer <= 0.0
            && player.invuln_timer <= 0.0
        {
            if car.horn_cooldown <= 0.0 {
                play_sfx("car_horn");
                car.horn_cooldown = 1.4;
            }
            let knock = (p_tf.translation - car_tf.translation)
                .with_y(0.0)
                .normalize_or_zero();
            p_tf.translation += knock * 2.2;
            player.velocity.y = 4.5;
            player.invuln_timer = 1.1;

            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
                stats.hint_text = "Ranch Shield blocked a speeding NYC Taxi!".to_string();
            } else {
                stats.hearts -= 1;
                play_sfx("hit");
                stats.hint_text =
                    "HIT BY NYC TRAFFIC! Jump or [SHIFT / ROLL] through cars!".to_string();
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }
    }
}

pub fn update_city_npcs(
    time: Res<Time>,
    mut npc_q: Query<(&mut Transform, &mut CityNpc, &Children)>,
    mut limb_q: Query<(&mut Transform, &CityNpcLimb), Without<CityNpc>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut tf, mut npc, children) in &mut npc_q {
        npc.angle += npc.speed * 0.16 * dt;
        npc.walk_phase += npc.speed * dt * 3.8;
        let nx = npc.home_x + npc.angle.cos() * 2.2;
        let nz = npc.home_z + npc.angle.sin() * npc.patrol_radius * 0.45;
        let dx = nx - tf.translation.x;
        let dz = nz - tf.translation.z;
        tf.translation.x = nx;
        tf.translation.z = nz;
        if dx * dx + dz * dz > 1e-5 {
            let yaw = dx.atan2(dz);
            tf.rotation = Quat::from_rotation_y(yaw);
        }
        let swing = npc.walk_phase.sin() * 0.55;
        for &ch in children {
            if let Ok((mut l_tf, limb)) = limb_q.get_mut(ch) {
                let s = if limb.is_arm { -swing * limb.side } else { swing * limb.side };
                l_tf.rotation = Quat::from_rotation_x(s);
            }
        }
    }
}

pub fn update_city_pickups(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    mut tendy_q: Query<(Entity, &mut Transform, &TendyPickup), (With<CityTendy>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let pickup_r = 2.1 + stats.player_scale * 0.45;
    let mid_y = 0.55 * stats.player_scale;

    for (entity, mut tf, tendy) in &mut tendy_q {
        tf.rotate_y(2.4 * dt);
        tf.translation.y = tendy.base_y + (t * 3.2 + tendy.phase).sin() * 0.22;

        let d = (p_tf.translation + Vec3::Y * mid_y).distance(tf.translation);
        if d < pickup_r {
            commands.entity(entity).despawn_recursive();
            stats.tendies += 1;
            stats.score += 200;
            play_sfx("tendy");
            if stats.tendies >= stats.total_tendies {
                stats.score += 5000;
                stats.hint_text =
                    "50/50 TENDIES - FULL SKYSCRAPER KAIJU! Slash Lexie [CLICK / SLASH]!"
                        .to_string();
                speak_voice(
                    "Maximum skyscraper power reached! Finish Lexie now, Stan!",
                    "narrator",
                );
            } else {
                stats.hint_text = format!(
                    "NYC Tendy {}/{}! Size: {} ({}cm)",
                    stats.tendies,
                    stats.total_tendies,
                    stats.size_tier(),
                    stats.height_cm()
                );
            }
        }
    }
}

pub fn update_city_lexie(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut lexie_q: Query<
        (Entity, &mut Transform, &mut LexieBoss, &Children),
        (Without<Player>, Without<LexieThrowArm>),
    >,
    mut arm_q: Query<
        &mut Transform,
        (With<LexieThrowArm>, Without<Player>, Without<LexieBoss>),
    >,
    mut garlic_q: Query<
        (Entity, &mut Transform, &mut GarlicProjectile),
        (Without<Player>, Without<LexieBoss>, Without<LexieThrowArm>),
    >,
    mut stench_q: Query<
        (Entity, &mut Transform, &mut GarlicStenchCloud),
        (
            Without<Player>,
            Without<LexieBoss>,
            Without<LexieThrowArm>,
            Without<GarlicProjectile>,
        ),
    >,
    mut debris_q: Query<
        (Entity, &mut Transform, &mut CityTowerDebris),
        (
            Without<Player>,
            Without<LexieBoss>,
            Without<LexieThrowArm>,
            Without<GarlicProjectile>,
            Without<GarlicStenchCloud>,
        ),
    >,
    mut hp_fill_q: Query<&mut Node, With<LexieBossHpFill>>,
    mut label_q: Query<&mut Text, With<LexieBossHudText>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };

    if let Ok((lexie_ent, mut lx_tf, mut lexie, lx_children)) = lexie_q.get_single_mut() {
        // Tower demolition chunks falling off Empire Citadel while Lexie rampages
        lexie.debris_timer -= dt;
        if lexie.debris_timer <= 0.0 && lexie.on_tower {
            lexie.debris_timer = 0.55;
            let chunk_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.62, 0.58, 0.54),
                perceptual_roughness: 0.85,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                CityTowerDebris {
                    velocity: Vec3::new(
                        (t * 7.3).sin() * 9.0,
                        4.0 + (t * 4.1).cos().abs() * 5.0,
                        4.0 + (t * 5.7).cos() * 6.0,
                    ),
                    spin: Vec3::new(3.2, 2.4, 4.1),
                    age: 0.0,
                },
                Mesh3d(meshes.add(Cuboid::new(1.5, 1.3, 1.4))),
                MeshMaterial3d(chunk_mat),
                Transform::from_xyz(
                    (t * 3.7).sin() * 5.0,
                    44.0 + (t * 2.1).cos() * 4.0,
                    CITADEL_POS.z + 5.5,
                ),
            ));
        }

        // Transition Lexie down to 5th Avenue Brawl once Stan grows large (>= 36 tendies)
        if lexie.on_tower && (stats.tendies >= 36 || p_tf.translation.z < -42.0 && stats.tendies >= 25) {
            lexie.on_tower = false;
            lx_tf.translation = Vec3::new(0.0, 0.0, -44.0);
            play_sfx("thunder");
            speak_voice(
                "Hey oversized tendy boy! Eat fresh pickled garlic cloves up close!",
                "lexie",
            );
            stats.hint_text =
                "LEXIE LEAPT OFF THE TOWER! Collect all 50 Tendies & SLASH her!".to_string();
        }

        // Face toward Stan
        let to_p = p_tf.translation - lx_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);
        let dist = flat.length();
        if flat.length_squared() > 0.1 {
            let yaw = flat.x.atan2(flat.z);
            lx_tf.rotation = Quat::from_rotation_y(yaw);
        }

        if !lexie.on_tower && dist > 16.0 {
            let dir = flat.normalize_or_zero();
            let next_pos = lx_tf.translation + dir * 7.5 * dt;
            if is_clear_of_skyscrapers(next_pos.x, next_pos.z, 3.5) {
                lx_tf.translation = next_pos;
            }
            lx_tf.translation.y = (t * 5.5).sin().abs() * 0.45;
        }

        // Pickled garlic chew & throw animation
        lexie.throw_cooldown -= dt;
        lexie.roar_cooldown -= dt;
        if lexie.hit_flash > 0.0 {
            lexie.hit_flash = (lexie.hit_flash - dt).max(0.0);
        }

        if lexie.chew_timer > 0.0 {
            lexie.chew_timer = (lexie.chew_timer - dt).max(0.0);
        }
        for &ch in lx_children {
            if let Ok(mut arm_tf) = arm_q.get_mut(ch) {
                let swing = if lexie.chew_timer > 0.0 {
                    -1.75 + (t * 12.0).sin() * 0.25
                } else if lexie.throw_cooldown < 0.45 {
                    -1.25
                } else {
                    0.55 + (t * 4.5).sin() * 0.35
                };
                arm_tf.rotation = Quat::from_rotation_x(swing) * Quat::from_rotation_z(0.28);
            }
        }

        // Throw Pickled Garlic Clove volley toward Stan!
        if lexie.throw_cooldown <= 0.0 {
            lexie.throw_cooldown = if lexie.on_tower { 3.6 } else { 2.8 };
            lexie.chew_timer = 0.85;
            play_sfx("garlic_throw");
            let hand_world = lx_tf.translation + Vec3::new(0.0, 20.0, 4.0);
            let target_pos = p_tf.translation + Vec3::Y * (0.5 * stats.player_scale);
            let count = if lexie.on_tower { 2 } else { 3 };
            let clove_mesh = meshes.add(Sphere::new(1.35));
            let clove_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.94, 0.68),
                emissive: LinearRgba::rgb(0.45, 0.65, 0.10),
                perceptual_roughness: 0.3,
                ..default()
            });
            for k in 0..count {
                let spread = (k as f32 - (count as f32 - 1.0) * 0.5) * 9.0;
                let aimed = target_pos + Vec3::new(spread, 0.0, spread * 0.35);
                let flight_t = 1.45_f32;
                let g = 24.0_f32;
                let vx = (aimed.x - hand_world.x) / flight_t;
                let vz = (aimed.z - hand_world.z) / flight_t;
                let vy = (aimed.y - hand_world.y + 0.5 * g * flight_t * flight_t) / flight_t;
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    GarlicProjectile {
                        velocity: Vec3::new(vx, vy, vz),
                        spin: 5.5 + k as f32,
                        age: 0.0,
                    },
                    Mesh3d(clove_mesh.clone()),
                    MeshMaterial3d(clove_mat.clone()),
                    Transform::from_translation(hand_world).with_scale(Vec3::new(0.85, 1.35, 0.85)),
                ));
            }
        }

        // Take Stan's Skyscraper Lightsaber Slash!
        let saber_reach = 5.5 + stats.player_scale * 1.45;
        let hit_dist = p_tf.translation.distance(lx_tf.translation);
        if player.attack_timer > 0.14 && hit_dist < saber_reach && lexie.hit_flash <= 0.0 {
            lexie.hit_flash = 0.32;
            play_sfx("saber_hit");
            let heavy_mul = if stats.heavy_attack { 1.85 } else { 1.0 };
            if stats.tendies < 50 {
                // Chip damage capped until all 50 tendies give Skyscraper Titan Power
                lexie.hp = (lexie.hp - 18.0 * heavy_mul).max(75.0);
                stats.hint_text = format!(
                    "Collect all 50 TENDIES ({}/50) for Full Skyscraper Titan Power to finish Lexie!",
                    stats.tendies
                );
            } else {
                lexie.hp -= 85.0;
                stats.score += 1200;
            }
            stats.boss_hp = lexie.hp.max(0.0);

            if lexie.hp <= 0.0 {
                stats.score += 15000;
                stats.lexie_defeated = true;
                stats.hint_text = "LEXIE DEFEATED! NEW YORK CITY SAVED!".to_string();
                play_sfx("win");
                speak_voice(
                    "Oh no! My pickled garlic empire! You win, giant Viking Stan!",
                    "lexie",
                );
                commands.entity(lexie_ent).despawn_recursive();
                next_state.set(GameState::StageClear);
                return;
            }
        }

        // Update Top-Center Boss HP UI
        if let Ok(mut fill) = hp_fill_q.get_single_mut() {
            fill.width = Val::Percent((lexie.hp / lexie.max_hp * 100.0).clamp(0.0, 100.0));
        }
        if let Ok(mut lbl) = label_q.get_single_mut() {
            lbl.0 = if stats.tendies < 50 {
                format!(
                    "LEXIE | PICKLED GARLIC QUEEN ({}/50 TENDIES FOR MAX TITAN)",
                    stats.tendies
                )
            } else {
                format!("LEXIE | SKYSCRAPER TITAN SHOWDOWN! HP {:.0}%", lexie.hp / lexie.max_hp * 100.0)
            };
        }
    }

    // Update airborne Pickled Garlic Clove Projectiles & burst into lingering Stench Clouds
    for (g_ent, mut g_tf, mut proj) in &mut garlic_q {
        proj.age += dt;
        proj.velocity.y -= 24.0 * dt;
        g_tf.translation += proj.velocity * dt;
        g_tf.rotate_y(proj.spin * dt);
        g_tf.rotate_x(proj.spin * 0.7 * dt);

        let hit_stan = g_tf.translation.distance(p_tf.translation + Vec3::Y * (0.5 * stats.player_scale))
            < (2.6 + stats.player_scale * 0.25);

        if hit_stan && player.roll_timer <= 0.0 && player.invuln_timer <= 0.0 {
            player.invuln_timer = 1.0;
            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
            } else {
                stats.hearts -= 1;
                play_sfx("hit");
                stats.hint_text = "DIRECT PICKLED GARLIC HIT! Dodge Roll [SHIFT / ROLL]!".to_string();
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }

        if g_tf.translation.y <= 0.6 || hit_stan || proj.age > 5.5 {
            let impact_pos = Vec3::new(g_tf.translation.x, 0.35, g_tf.translation.z);
            commands.entity(g_ent).despawn_recursive();
            let fume_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.68, 0.88, 0.16, 0.26),
                emissive: LinearRgba::rgb(0.42, 0.64, 0.06),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                GarlicStenchCloud {
                    lifetime: 4.8,
                    max_lifetime: 4.8,
                    radius: 5.8,
                    damage_tick: 0.65,
                },
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(fume_mat),
                Transform::from_translation(impact_pos).with_scale(Vec3::new(3.5, 2.2, 3.5)),
            ));
        }
    }

    // Update lingering Pickled Garlic Stench Clouds & intense smell damage
    for (s_ent, mut s_tf, mut cloud) in &mut stench_q {
        cloud.lifetime -= dt;
        if cloud.lifetime <= 0.0 {
            commands.entity(s_ent).despawn_recursive();
            continue;
        }
        let life_frac = (cloud.lifetime / cloud.max_lifetime).clamp(0.0, 1.0);
        let spread = 1.0 + (1.0 - life_frac) * 1.15;
        s_tf.scale = Vec3::new(
            cloud.radius * spread,
            4.2 * (0.65 + (1.0 - life_frac) * 0.5),
            cloud.radius * spread,
        );
        s_tf.rotate_y(1.2 * dt);

        if cloud.damage_tick > 0.0 {
            cloud.damage_tick = (cloud.damage_tick - dt).max(0.0);
        }
        let flat_dist = Vec2::new(
            p_tf.translation.x - s_tf.translation.x,
            p_tf.translation.z - s_tf.translation.z,
        )
        .length();
        if flat_dist < cloud.radius * spread
            && p_tf.translation.y < 6.5
            && player.roll_timer <= 0.0
            && player.invuln_timer <= 0.0
            && cloud.damage_tick <= 0.0
        {
            cloud.damage_tick = 1.45;
            player.invuln_timer = 0.85;
            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
            } else {
                stats.hearts -= 1;
                play_sfx("hit");
                stats.hint_text =
                    "INTENSE PICKLED GARLIC SMELL! Dodge Roll out of green fumes!".to_string();
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }
    }

    // Animate & clean up Empire Citadel demolition debris chunks
    for (d_ent, mut d_tf, mut deb) in &mut debris_q {
        deb.age += dt;
        deb.velocity.y -= 24.0 * dt;
        d_tf.translation += deb.velocity * dt;
        d_tf.rotate_x(deb.spin.x * dt);
        d_tf.rotate_y(deb.spin.y * dt);
        if d_tf.translation.y <= 0.4 || deb.age > 5.0 {
            commands.entity(d_ent).despawn_recursive();
        }
    }
}

pub fn update_city_minimap(
    player_q: Query<&Transform, With<Player>>,
    lexie_q: Query<&Transform, (With<LexieBoss>, Without<Player>)>,
    tendy_q: Query<&Transform, (With<CityTendy>, Without<Player>, Without<LexieBoss>)>,
    mut p_dot_q: Query<&mut Node, With<CityMinimapPlayerDot>>,
    mut b_dot_q: Query<(&mut Node, &mut Visibility), (With<CityMinimapBossDot>, Without<CityMinimapPlayerDot>)>,
    mut t_dot_q: Query<
        (&mut Node, &mut Visibility),
        (
            With<CityMinimapTendyDot>,
            Without<CityMinimapPlayerDot>,
            Without<CityMinimapBossDot>,
        ),
    >,
) {
    let to_pct = |v: f32| -> f32 { ((v + CITY_HALF_EXTENT) / (CITY_HALF_EXTENT * 2.0) * 100.0).clamp(3.0, 97.0) };

    if let (Ok(p_tf), Ok(mut p_node)) = (player_q.get_single(), p_dot_q.get_single_mut()) {
        p_node.left = Val::Percent(to_pct(p_tf.translation.x));
        p_node.top = Val::Percent(to_pct(p_tf.translation.z));
    }

    if let Ok((mut b_node, mut b_vis)) = b_dot_q.get_single_mut() {
        if let Ok(lx_tf) = lexie_q.get_single() {
            *b_vis = Visibility::Visible;
            b_node.left = Val::Percent(to_pct(lx_tf.translation.x));
            b_node.top = Val::Percent(to_pct(lx_tf.translation.z));
        } else {
            *b_vis = Visibility::Hidden;
        }
    }

    let mut tendies_iter = tendy_q.iter();
    for (mut t_node, mut t_vis) in &mut t_dot_q {
        if let Some(t_tf) = tendies_iter.next() {
            *t_vis = Visibility::Visible;
            t_node.left = Val::Percent(to_pct(t_tf.translation.x));
            t_node.top = Val::Percent(to_pct(t_tf.translation.z));
        } else {
            *t_vis = Visibility::Hidden;
        }
    }
}

pub fn animate_city_intro_lexie(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut lexie_q: Query<(&mut LexieBoss, &Children)>,
    mut arm_q: Query<&mut Transform, (With<LexieThrowArm>, Without<CityTowerDebris>)>,
    mut debris_q: Query<(Entity, &mut Transform, &mut CityTowerDebris), Without<LexieThrowArm>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if let Ok((mut lexie, children)) = lexie_q.get_single_mut() {
        for &ch in children {
            if let Ok(mut arm_tf) = arm_q.get_mut(ch) {
                let swing = 0.35 + (t * 6.0).sin() * 0.85;
                arm_tf.rotation = Quat::from_rotation_x(swing) * Quat::from_rotation_z(0.28);
            }
        }
        lexie.debris_timer -= dt;
        if lexie.debris_timer <= 0.0 {
            lexie.debris_timer = 0.42;
            let chunk_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.64, 0.60, 0.56),
                perceptual_roughness: 0.85,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                CityTowerDebris {
                    velocity: Vec3::new(
                        (t * 7.3).sin() * 10.0,
                        4.5 + (t * 4.1).cos().abs() * 6.0,
                        5.0 + (t * 5.7).cos() * 6.0,
                    ),
                    spin: Vec3::new(3.2, 2.4, 4.1),
                    age: 0.0,
                },
                Mesh3d(meshes.add(Cuboid::new(1.8, 1.5, 1.6))),
                MeshMaterial3d(chunk_mat),
                Transform::from_xyz(
                    (t * 3.7).sin() * 5.0,
                    46.0 + (t * 2.1).cos() * 4.0,
                    CITADEL_POS.z + 5.5,
                ),
            ));
        }
    }

    for (d_ent, mut d_tf, mut deb) in &mut debris_q {
        deb.age += dt;
        deb.velocity.y -= 24.0 * dt;
        d_tf.translation += deb.velocity * dt;
        d_tf.rotate_x(deb.spin.x * dt);
        d_tf.rotate_y(deb.spin.y * dt);
        if d_tf.translation.y <= 0.4 || deb.age > 5.0 {
            commands.entity(d_ent).despawn_recursive();
        }
    }
}
