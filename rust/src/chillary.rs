//! Chillary Clinton NPC by the River Star Ferris wheel & her "Just Chillin' in Cedar Rapids"
//! viral Snapchat re-enactment cutscene (`GameState::ChillaryCutscene`).

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use crate::audio::{is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, speak_voice};
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::state::{city_body_scale, GameState, LevelEntity, PlayerStats};
use crate::ui::HudOverlay;

/// Stands on the River Star Ferris wheel stone plaza (`y = 0.35`), just outside the left entrance
/// arch pillar (local `(-4.8, 0.35, 8.8)` rotated by `3*FRAC_PI_4`).
pub const CHILLARY_POS: Vec3 = Vec3::new(-104.38, 0.35, 111.17);
const CHILLARY_INTERACT_RANGE: f32 = 4.2;

#[derive(Resource, Default)]
pub struct ChillaryPrompt {
    pub label: Option<String>,
    pub pressed: bool,
}

#[derive(Resource, Default)]
pub struct ChillaryCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
    pub total_t: f32,
}

#[derive(Component)]
pub struct CityChillary;

#[derive(Component)]
pub struct ChillaryBeacon;

#[derive(Component)]
pub struct ChillaryCutsceneEntity;

#[derive(Component)]
pub struct ChillaryCutsceneOverlay;

#[derive(Component)]
pub struct ChillarySpeakerText;

#[derive(Component)]
pub struct ChillarySubText;

#[derive(Component)]
pub struct ChillarySnapCaptionBox;

#[derive(Component)]
pub struct ChillarySnapCaptionText;

#[derive(Component)]
pub struct ChillarySnapTimerText;

#[derive(Component)]
pub struct ChillaryAnim {
    pub kind: ChillaryPart,
}

#[derive(Clone, Copy)]
pub enum ChillaryPart {
    BodyRoot,
    Head,
    Brow { side: f32 },
    Mouth,
    SelfieArm,
    LeftArm,
    StanPhotobomb,
}

/// (speaker, line, voice, min_secs, snap_caption)
const CHILLARY_BEATS: [(&str, &str, &str, f32, &str); 3] = [
    (
        "CHILLARY CLINTON (ADJUSTING SNAPCHAT)",
        "Wait... is the little red light blinking? Okay, hold on... let me angle this camera up...",
        "chillary",
        2.8,
        "",
    ),
    (
        "CHILLARY CLINTON (CEDAR RAPIDS, JULY 2015)",
        "I'm just chillin' in Cedar Rapids.",
        "chillary",
        4.6,
        "just chillin in Cedar Rapids",
    ),
    (
        "STAN (PHOTOBOMBING IN THE KITCHEN)",
        "Uh... just chillin' in Cedar Rapids? Cool Snapchat, Chillary. I'll catch you out by the Ferris wheel.",
        "stan",
        4.4,
        "JUST CHILLIN' IN CEDAR RAPIDS  |  #CHILLARY",
    ),
];

pub struct ChillaryMaterials {
    skin: Handle<StandardMaterial>,
    blazer: Handle<StandardMaterial>,
    undershirt: Handle<StandardMaterial>,
    slacks: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    brow: Handle<StandardMaterial>,
    lip: Handle<StandardMaterial>,
    teeth: Handle<StandardMaterial>,
    pearl: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_iris: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
    phone_body: Handle<StandardMaterial>,
    phone_screen: Handle<StandardMaterial>,
    shoe: Handle<StandardMaterial>,
}

pub fn make_chillary_materials(materials: &mut Assets<StandardMaterial>) -> ChillaryMaterials {
    ChillaryMaterials {
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.78, 0.67),
            perceptual_roughness: 0.56,
            ..default()
        }),
        blazer: materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.58, 0.80),
            perceptual_roughness: 0.58,
            ..default()
        }),
        undershirt: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.96, 0.98),
            perceptual_roughness: 0.65,
            ..default()
        }),
        slacks: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.14, 0.24),
            perceptual_roughness: 0.72,
            ..default()
        }),
        hair: materials.add(StandardMaterial {
            base_color: Color::srgb(0.93, 0.81, 0.44),
            perceptual_roughness: 0.52,
            ..default()
        }),
        brow: materials.add(StandardMaterial {
            base_color: Color::srgb(0.68, 0.54, 0.30),
            perceptual_roughness: 0.60,
            ..default()
        }),
        lip: materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.34, 0.42),
            perceptual_roughness: 0.38,
            ..default()
        }),
        teeth: materials.add(StandardMaterial {
            base_color: Color::srgb(0.97, 0.97, 0.95),
            perceptual_roughness: 0.28,
            ..default()
        }),
        pearl: materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.94, 0.90),
            metallic: 0.15,
            perceptual_roughness: 0.22,
            ..default()
        }),
        gold: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.74, 0.26),
            metallic: 0.85,
            perceptual_roughness: 0.28,
            ..default()
        }),
        eye_white: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        }),
        eye_iris: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.46, 0.76),
            ..default()
        }),
        eye_pupil: materials.add(StandardMaterial {
            base_color: Color::BLACK,
            ..default()
        }),
        phone_body: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.93, 0.95),
            perceptual_roughness: 0.25,
            metallic: 0.35,
            ..default()
        }),
        phone_screen: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.92, 0.22),
            emissive: LinearRgba::new(1.1, 0.95, 0.15, 1.0),
            unlit: true,
            ..default()
        }),
        shoe: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.10),
            perceptual_roughness: 0.35,
            ..default()
        }),
    }
}

/// Builds the smooth Hillary / "Chillary Clinton" rig facing `+Z` in local space:
/// feathered blonde campaign bob + swept bangs, turquoise-blue pantsuit blazer, white undershirt,
/// pearl necklace, expressive animated eyebrows & toothy smile, and raised selfie smartphone.
pub fn spawn_chillary_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &ChillaryMaterials,
    is_cutscene: bool,
) {
    use crate::shapes::{self as sh, Torso};

    // Tailored dark-navy trousers & low-heeled pumps
    let leg_mesh = sh::add(meshes, sh::limb_profile(0.66, &[0.105, 0.095, 0.075], 1.0));
    let shoe_mesh = sh::add(meshes, sh::shoe(0.16, 0.09, 0.24));
    for sx in [-0.125_f32, 0.125] {
        parent.spawn((
            Mesh3d(leg_mesh.clone()),
            MeshMaterial3d(mats.slacks.clone()),
            Transform::from_xyz(sx, 0.34, 0.0),
        ));
        parent.spawn((
            Mesh3d(shoe_mesh.clone()),
            MeshMaterial3d(mats.shoe.clone()),
            Transform::from_xyz(sx, 0.055, 0.03),
        ));
    }

    // Iconic bright turquoise-blue campaign blazer + white undershirt shell + pearl necklace
    let blazer = sh::torso(&Torso {
        height: 0.76,
        hip: Vec2::new(0.255, 0.155),
        waist: Vec2::new(0.235, 0.148),
        chest: Vec2::new(0.275, 0.172),
        shoulder: Vec2::new(0.295, 0.162),
        neck: 0.075,
        sq: 0.78,
        chest_fwd: 0.014,
    });
    let v_undershirt = sh::radial_shell(&blazer, 0.006, |p| {
        let in_v = sh::smoothstep(0.11, 0.08, p.x.abs() + (0.36 - p.y).max(0.0) * 0.28);
        let front = sh::smoothstep(0.0, 0.06, p.z);
        in_v * front * sh::smoothstep(-0.02, 0.10, p.y)
    });
    parent.spawn((
        Mesh3d(sh::add(meshes, blazer)),
        MeshMaterial3d(mats.blazer.clone()),
        Transform::from_xyz(0.0, 1.03, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, v_undershirt)),
        MeshMaterial3d(mats.undershirt.clone()),
        Transform::from_xyz(0.0, 1.03, 0.0),
    ));
    // Blazer notched lapels + gold campaign buttons
    for sx in [-1.0_f32, 1.0] {
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.065, 0.28, 0.024))),
            MeshMaterial3d(mats.blazer.clone()),
            Transform::from_xyz(sx * 0.075, 1.24, 0.165)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.12, sx * 0.15, -sx * 0.24)),
        ));
    }
    let btn_mesh = meshes.add(Cylinder::new(0.016, 0.012));
    for by in [1.02_f32, 0.90, 0.78] {
        parent.spawn((
            Mesh3d(btn_mesh.clone()),
            MeshMaterial3d(mats.gold.clone()),
            Transform::from_xyz(0.0, by, 0.172).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ));
    }

    // Neck & lustrous pearl necklace
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::limb(0.18, 0.068, 0.074))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.46, -0.01),
    ));
    let pearl_mesh = meshes.add(Sphere::new(0.019));
    for i in 0..9 {
        let ang = -1.25 + (i as f32 / 8.0) * 2.50;
        let px = ang.sin() * 0.092;
        let pz = ang.cos() * 0.095 + 0.015;
        let py = 1.405 - ang.cos() * 0.028;
        parent.spawn((
            Mesh3d(pearl_mesh.clone()),
            MeshMaterial3d(mats.pearl.clone()),
            Transform::from_xyz(px, py, pz),
        ));
    }

    // Left arm (relaxed/waves in Beat 2)
    let sleeve_mesh = sh::add(meshes, sh::limb_profile(0.60, &[0.094, 0.084, 0.070], 1.0));
    let mut left_arm = parent.spawn((
        Transform::from_xyz(-0.37, 1.34, 0.02).with_rotation(Quat::from_rotation_z(-0.08)),
        Visibility::default(),
    ));
    if is_cutscene {
        left_arm.insert(ChillaryAnim { kind: ChillaryPart::LeftArm });
    }
    let hand_l = sh::add(meshes, sh::hand(0.078, -1.0, false));
    left_arm.with_children(|a| {
        a.spawn((
            Mesh3d(sleeve_mesh.clone()),
            MeshMaterial3d(mats.blazer.clone()),
            Transform::from_xyz(0.0, -0.24, 0.0),
        ));
        a.spawn((
            Mesh3d(hand_l),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(0.0, -0.57, 0.02),
        ));
    });

    // Right arm raised holding her Snapchat smartphone toward her face
    let selfie_base_rot = Quat::from_euler(EulerRot::XYZ, -1.18, -0.22, -0.18);
    let mut right_arm = parent.spawn((
        Transform::from_xyz(0.37, 1.34, 0.04).with_rotation(selfie_base_rot),
        Visibility::default(),
    ));
    if is_cutscene {
        right_arm.insert(ChillaryAnim { kind: ChillaryPart::SelfieArm });
    }
    let hand_r = sh::add(meshes, sh::hand(0.078, 1.0, true));
    right_arm.with_children(|a| {
        a.spawn((
            Mesh3d(sleeve_mesh.clone()),
            MeshMaterial3d(mats.blazer.clone()),
            Transform::from_xyz(0.0, -0.24, 0.0),
        ));
        a.spawn((
            Mesh3d(hand_r),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(0.0, -0.57, 0.03),
        ));
        // White smartphone + glowing Snapchat yellow screen
        a.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.082, 0.148, 0.014))),
            MeshMaterial3d(mats.phone_body.clone()),
            Transform::from_xyz(-0.03, -0.60, 0.06).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.45, -0.20, 0.0)),
        ))
        .with_children(|ph| {
            ph.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.068, 0.118, 0.004))),
                MeshMaterial3d(mats.phone_screen.clone()),
                Transform::from_xyz(0.0, 0.004, -0.008),
            ));
            ph.spawn((
                Mesh3d(meshes.add(Sphere::new(0.007))),
                MeshMaterial3d(mats.eye_pupil.clone()),
                Transform::from_xyz(0.0, 0.056, 0.008),
            ));
        });
    });

    // Head assembly (+Z front)
    let mut head_root = parent.spawn((Transform::from_xyz(0.0, 1.72, 0.0), Visibility::default()));
    if is_cutscene {
        head_root.insert(ChillaryAnim { kind: ChillaryPart::Head });
    }
    let (hr, jaw) = (0.255_f32, 0.14_f32);
    head_root.with_children(|hd| {
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::head(hr, jaw))),
            MeshMaterial3d(mats.skin.clone()),
        ));
        for sx in [-1.0_f32, 1.0] {
            hd.spawn((
                Mesh3d(sh::add(meshes, sh::ear(0.105, sx))),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(sx * 0.235, -0.01, -0.01),
            ));
            // Pearl stud earring
            hd.spawn((
                Mesh3d(meshes.add(Sphere::new(0.016))),
                MeshMaterial3d(mats.pearl.clone()),
                Transform::from_xyz(sx * 0.245, -0.06, 0.01),
            ));
        }

        // Voluminous feathered blonde campaign bob + swept forehead bangs + side locks
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::hair_cap(hr, jaw, 0.34, -0.45, 0.14, 0.26))),
            MeshMaterial3d(mats.hair.clone()),
        ));
        // Side-swept feathered bangs across top-left forehead
        hd.spawn((
            Mesh3d(sh::add(
                meshes,
                sh::superellipsoid(Vec3::new(0.17, 0.065, 0.08), 0.7, 0.8, 18, 10)
                    .deformed(|p| Vec3::new(p.x, p.y - 0.8 * p.x * p.x, p.z - 1.1 * p.x * p.x)),
            )),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(0.055, 0.145, 0.225).with_rotation(Quat::from_rotation_z(-0.22)),
        ));
        // Chin-length feathered blonde side flips framing both cheeks
        let bob_side = sh::add(
            meshes,
            sh::limb_profile(0.32, &[0.095, 0.11, 0.125, 0.075], 1.1).deformed(|p| {
                // Flare outward softly near the bottom like her classic bob
                let flare = (-p.y * 0.35).max(0.0);
                Vec3::new(p.x * (1.0 + flare), p.y, p.z)
            }),
        );
        for sx in [-1.0_f32, 1.0] {
            hd.spawn((
                Mesh3d(bob_side.clone()),
                MeshMaterial3d(mats.hair.clone()),
                Transform::from_xyz(sx * 0.225, -0.01, -0.05)
                    .with_rotation(Quat::from_rotation_z(sx * 0.16)),
            ));
        }
        // Back bob fullness
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.24, 0.22, 0.16), 0.8, 0.8, 16, 12))),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(0.0, -0.06, -0.18),
        ));

        // Nose, eyes & expressive arched eyebrows
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.032, 0.048, 0.040), 1.0, 1.0, 12, 8))),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(0.0, -0.008, 0.255),
        ));
        let brow_mesh = sh::add(meshes, sh::arc_bar(0.095, &[0.008, 0.015, 0.016, 0.010], 0.014, 0.012, 0.7));
        let eye_w = meshes.add(Sphere::new(0.034));
        let eye_i = meshes.add(Sphere::new(0.021));
        let eye_p = meshes.add(Sphere::new(0.012));
        for sx in [-1.0_f32, 1.0] {
            hd.spawn((
                Mesh3d(eye_w.clone()),
                MeshMaterial3d(mats.eye_white.clone()),
                Transform::from_xyz(sx * 0.088, 0.046, 0.222).with_scale(Vec3::new(1.1, 0.92, 0.65)),
            ));
            hd.spawn((
                Mesh3d(eye_i.clone()),
                MeshMaterial3d(mats.eye_iris.clone()),
                Transform::from_xyz(sx * 0.088, 0.046, 0.236),
            ));
            hd.spawn((
                Mesh3d(eye_p.clone()),
                MeshMaterial3d(mats.eye_pupil.clone()),
                Transform::from_xyz(sx * 0.088, 0.046, 0.248),
            ));
            let mut br = hd.spawn((
                Mesh3d(brow_mesh.clone()),
                MeshMaterial3d(mats.brow.clone()),
                Transform::from_xyz(sx * 0.088, 0.094, 0.234)
                    .with_rotation(Quat::from_rotation_z(-sx * 0.08)),
            ));
            if is_cutscene {
                br.insert(ChillaryAnim { kind: ChillaryPart::Brow { side: sx } });
            }
        }

        // Wide bright campaign smile (pink lips + white teeth bar)
        let mut mouth = hd.spawn((Transform::from_xyz(0.0, -0.092, 0.232), Visibility::default()));
        if is_cutscene {
            mouth.insert(ChillaryAnim { kind: ChillaryPart::Mouth });
        }
        mouth.with_children(|m| {
            m.spawn((
                Mesh3d(sh::add(meshes, sh::arc_bar(0.128, &[0.012, 0.022, 0.024, 0.022, 0.012], -0.012, 0.018, 0.75))),
                MeshMaterial3d(mats.lip.clone()),
            ));
            m.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.084, 0.022, 0.024))),
                MeshMaterial3d(mats.teeth.clone()),
                Transform::from_xyz(0.0, 0.002, 0.006),
            ));
        });
    });
}

/// Spawns Chillary Clinton on the River Star Ferris wheel stone plaza (`CityPlay`).
pub fn spawn_city_chillary(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mats = make_chillary_materials(materials);
    let beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.94, 0.15),
        emissive: LinearRgba::new(2.0, 1.8, 0.2, 1.0),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityChillary,
            Transform::from_translation(CHILLARY_POS)
                .with_rotation(Quat::from_rotation_y(-PI * 0.25))
                .with_scale(Vec3::splat(city_body_scale(1.8))),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_chillary_rig(p, meshes, &mats, false);
        });

    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        ChillaryBeacon,
        Mesh3d(meshes.add(Cuboid::new(0.28, 0.28, 0.28))),
        MeshMaterial3d(beacon_mat),
        Transform::from_translation(CHILLARY_POS + Vec3::Y * 2.85)
            .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.7, 0.7, 0.0)),
    ));
}

/// Updates Chillary Clinton's plaza idle & proximity `[F / E]` prompt (`CityPlay`, before `update_city_props`).
pub fn update_city_chillary(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut c_state: ResMut<ChillaryCutsceneState>,
    mut prompt: ResMut<ChillaryPrompt>,
    mut next_state: crate::transition::SceneTransition,
    player_q: Query<&Transform, With<Player>>,
    mut chillary_q: Query<&mut Transform, (With<CityChillary>, Without<Player>, Without<ChillaryBeacon>)>,
    mut beacon_q: Query<(&mut Transform, &mut Visibility), (With<ChillaryBeacon>, Without<Player>, Without<CityChillary>)>,
) {
    let Ok(p_tf) = player_q.get_single() else {
        prompt.label = None;
        return;
    };
    let t = time.elapsed_secs();
    let dx = p_tf.translation.x - CHILLARY_POS.x;
    let dz = p_tf.translation.z - CHILLARY_POS.z;
    let dist = Vec2::new(dx, dz).length();

    // Turn politely to greet Stan when near the Ferris wheel plaza
    for mut c_tf in &mut chillary_q {
        c_tf.scale = Vec3::splat(city_body_scale(1.8));
        let target_yaw = if dist > 0.2 && dist < 12.0 {
            dx.atan2(dz)
        } else {
            -PI * 0.25
        };
        let target_rot = Quat::from_rotation_y(target_yaw);
        c_tf.rotation = c_tf.rotation.slerp(target_rot, (time.delta_secs() * 6.0).min(1.0));
    }

    for (mut b_tf, mut vis) in &mut beacon_q {
        *vis = if dist < 42.0 && stats.player_scale < 5.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        b_tf.translation = CHILLARY_POS + Vec3::Y * (2.85 + (t * 3.0).sin() * 0.16);
        b_tf.rotate_y(time.delta_secs() * 2.2);
    }

    let can_interact = !stats.riding_wheel
        && stats.driving_car.is_none()
        && stats.riding_scooter.is_none()
        && !stats.phone_open
        && stats.player_scale < 4.5
        && p_tf.translation.y < 3.0
        && dist < CHILLARY_INTERACT_RANGE;

    if can_interact {
        prompt.label = Some("CHILL WITH CHILLARY [F/E]".to_string());
    } else {
        prompt.label = None;
        prompt.pressed = false;
    }

    if can_interact && prompt.pressed {
        prompt.pressed = false;
        prompt.label = None;
        stats.saved_city_pos = p_tf.translation;
        stats.chillary_return_city = true;
        *c_state = ChillaryCutsceneState::default();
        play_sfx("snap_shutter");
        next_state.set(GameState::ChillaryCutscene);
    }
}

/// Enters the bright Cedar Rapids kitchen stage to re-enact "just chillin' in Cedar Rapids".
pub fn enter_chillary_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut c_state: ResMut<ChillaryCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut clear_col: ResMut<ClearColor>,
) {
    *c_state = ChillaryCutsceneState::default();
    for mut hv in &mut hud_q {
        *hv = Visibility::Hidden;
    }
    // Bright daytime window glow outside the Cedar Rapids home kitchen
    clear_col.0 = Color::srgb(0.76, 0.86, 0.96);
    let stage = Vec3::new(0.0, 0.0, -760.0);

    // Bright domestic kitchen lighting + soft front selfie-cam fill
    commands.spawn((
        ChillaryCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            intensity: 620_000.0,
            range: 26.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 3.2, 1.4)),
    ));
    commands.spawn((
        ChillaryCutsceneEntity,
        PointLight {
            color: Color::srgb(0.95, 0.98, 1.0),
            intensity: 340_000.0,
            range: 22.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(1.8, 2.5, -1.2)),
    ));

    // Sunlit Cedar Rapids home kitchen set (off-white plaster, cabinets, speckled counter, fridge, window blinds)
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.90, 0.84),
        perceptual_roughness: 0.86,
        ..default()
    });
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.56, 0.42),
        perceptual_roughness: 0.68,
        ..default()
    });
    let cab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.93),
        perceptual_roughness: 0.48,
        ..default()
    });
    let counter_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.72, 0.66),
        perceptual_roughness: 0.35,
        ..default()
    });
    let fridge_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.77, 0.80),
        metallic: 0.75,
        perceptual_roughness: 0.30,
        ..default()
    });
    let tile_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.88, 0.89),
        perceptual_roughness: 0.32,
        ..default()
    });
    let steel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.58, 0.62),
        metallic: 0.85,
        perceptual_roughness: 0.28,
        ..default()
    });
    let window_sun = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.98, 1.0),
        emissive: LinearRgba::new(1.6, 1.7, 1.8, 1.0),
        unlit: true,
        ..default()
    });
    let mag_red = materials.add(StandardMaterial { base_color: Color::srgb(0.88, 0.16, 0.16), ..default() });
    let mag_blue = materials.add(StandardMaterial { base_color: Color::srgb(0.14, 0.36, 0.86), ..default() });

    // Room shell (floor, ceiling, back/side walls)
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(10.0, 0.2, 10.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(10.0, 0.2, 10.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 3.7, 0.0)),
    ));
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(10.0, 3.8, 0.25))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 1.9, -2.85)),
    ));
    for sx in [-4.5_f32, 4.5] {
        commands.spawn((
            ChillaryCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::new(0.25, 3.8, 10.0))),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(stage + Vec3::new(sx, 1.9, 0.0)),
        ));
    }

    // Back counter + tile backsplash + upper shaker cabinets + handles
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(6.8, 0.95, 1.05))),
        MeshMaterial3d(cab_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.5, 0.475, -2.25)),
    ));
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(6.9, 0.08, 1.12))),
        MeshMaterial3d(counter_mat),
        Transform::from_translation(stage + Vec3::new(0.5, 0.98, -2.25)),
    ));
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(6.8, 0.85, 0.06))),
        MeshMaterial3d(tile_mat),
        Transform::from_translation(stage + Vec3::new(0.5, 1.44, -2.71)),
    ));
    // Upper cabinets flanking kitchen window
    for (cx, w) in [(-1.85_f32, 1.85_f32), (2.75, 2.2)] {
        commands.spawn((
            ChillaryCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::new(w, 1.15, 0.55))),
            MeshMaterial3d(cab_mat.clone()),
            Transform::from_translation(stage + Vec3::new(cx, 2.52, -2.48)),
        ));
        commands.spawn((
            ChillaryCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::new(0.03, 0.24, 0.04))),
            MeshMaterial3d(steel_mat.clone()),
            Transform::from_translation(stage + Vec3::new(cx + 0.15, 2.22, -2.18)),
        ));
    }
    // Bright kitchen window with crisp horizontal venetian blinds
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(2.1, 1.25, 0.06))),
        MeshMaterial3d(window_sun),
        Transform::from_translation(stage + Vec3::new(0.35, 2.48, -2.70)),
    ));
    let slat_mesh = meshes.add(Cuboid::new(2.12, 0.035, 0.05));
    for bi in 0..7 {
        commands.spawn((
            ChillaryCutsceneEntity,
            Mesh3d(slat_mesh.clone()),
            MeshMaterial3d(cab_mat.clone()),
            Transform::from_translation(stage + Vec3::new(0.35, 1.98 + bi as f32 * 0.16, -2.64)),
        ));
    }

    // Stainless refrigerator on left side with campaign magnets + mug on counter
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(1.25, 2.55, 1.15))),
        MeshMaterial3d(fridge_mat),
        Transform::from_translation(stage + Vec3::new(-3.55, 1.275, -2.20)),
    ));
    for (mx, my, mat) in [(-3.25_f32, 1.92_f32, mag_red), (-3.55, 1.72, mag_blue)] {
        commands.spawn((
            ChillaryCutsceneEntity,
            Mesh3d(meshes.add(Cuboid::new(0.14, 0.10, 0.03))),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(mx, my, -1.61)),
        ));
    }
    commands.spawn((
        ChillaryCutsceneEntity,
        Mesh3d(meshes.add(Cylinder::new(0.09, 0.18))),
        MeshMaterial3d(cab_mat),
        Transform::from_translation(stage + Vec3::new(1.15, 1.11, -2.10)),
    ));

    // Chillary Clinton front and center holding her selfie phone
    let mats = make_chillary_materials(&mut materials);
    commands
        .spawn((
            ChillaryCutsceneEntity,
            ChillaryAnim { kind: ChillaryPart::BodyRoot },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 0.0)).with_scale(Vec3::splat(1.12)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_chillary_rig(p, &mut meshes, &mats, true);
        });

    // Real Stan (`plaid_hero.glb` faces -Z at IDENTITY, so rotate PI to face +Z toward Chillary's selfie cam!)
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        ChillaryCutsceneEntity,
        ChillaryAnim { kind: ChillaryPart::StanPhotobomb },
        SceneRoot(hero_scene),
        Transform::from_translation(stage + Vec3::new(-0.92, 0.0, -1.35))
            .with_rotation(Quat::from_rotation_y(PI - 0.18))
            .with_scale(Vec3::splat(1.06)),
        Visibility::Hidden,
    ));

    // Initial low-angle selfie camera
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.18, 1.42, 0.92);
        c_tf.look_at(stage + Vec3::new(0.0, 1.92, 0.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.70;
        }
    }

    // Authentic 2015 vertical Snapchat phone frame UI (strictly ASCII)
    let (spk, line, voice, _, cap) = CHILLARY_BEATS[0];
    commands
        .spawn((
            ChillaryCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            // Left & right dark sidebars creating a vertical phone-story silhouette
            for is_left in [true, false] {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        left: if is_left { Val::Px(0.0) } else { Val::Auto },
                        right: if is_left { Val::Auto } else { Val::Px(0.0) },
                        width: Val::Percent(22.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.72)),
                ));
            }

            // Top row: SNAP STORY pill on left + classic Snapchat countdown circle on right
            root.spawn((
                Node {
                    width: Val::Percent(52.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(18.0)),
                    ..default()
                },
            ))
            .with_children(|top| {
                top.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 0.90, 0.12, 0.92)),
                    BorderColor(Color::WHITE),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|pill| {
                    pill.spawn((
                        Text::new("SNAP STORY  |  CEDAR RAPIDS, IA"),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(Color::srgb(0.08, 0.08, 0.10)),
                    ));
                });

                top.spawn((
                    Node {
                        width: Val::Px(38.0),
                        height: Val::Px(38.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.06, 0.08, 0.68)),
                    BorderColor(Color::WHITE),
                    BorderRadius::all(Val::Px(19.0)),
                ))
                .with_children(|badge| {
                    badge.spawn((
                        ChillarySnapTimerText,
                        Text::new("10"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });

            // Center-lower Snapchat translucent dark horizontal caption strip + bottom dialogue card
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ))
            .with_children(|bot| {
                bot.spawn((
                    ChillarySnapCaptionBox,
                    Node {
                        width: Val::Percent(56.0),
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.68)),
                    Visibility::Hidden,
                ))
                .with_children(|cb| {
                    cb.spawn((
                        ChillarySnapCaptionText,
                        Text::new(cap),
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });

                bot.spawn((
                    Node {
                        width: Val::Percent(88.0),
                        max_width: Val::Px(800.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(13.0)),
                        row_gap: Val::Px(5.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.10, 0.16, 0.90)),
                    BorderColor(Color::srgb(0.20, 0.72, 0.95)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|card| {
                    card.spawn((
                        ChillarySpeakerText,
                        Text::new(spk),
                        TextFont { font_size: 13.5, ..default() },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));
                    card.spawn((
                        ChillarySubText,
                        Text::new(format!("\"{}\"", line)),
                        TextFont { font_size: 15.5, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });

    play_cutscene_music();
    play_sfx("snap_pop");
    speak_voice(line, voice);
}

#[allow(clippy::type_complexity)]
pub fn update_chillary_cutscene(
    time: Res<Time>,
    mut c_state: ResMut<ChillaryCutsceneState>,
    mut next_state: crate::transition::SceneTransition,
    mut anim_q: Query<(&mut Transform, &mut Visibility, &ChillaryAnim), Without<Camera3d>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<ChillaryAnim>)>,
    mut spk_q: Query<
        &mut Text,
        (
            With<ChillarySpeakerText>,
            Without<ChillarySubText>,
            Without<ChillarySnapCaptionText>,
            Without<ChillarySnapTimerText>,
        ),
    >,
    mut sub_q: Query<
        &mut Text,
        (
            With<ChillarySubText>,
            Without<ChillarySpeakerText>,
            Without<ChillarySnapCaptionText>,
            Without<ChillarySnapTimerText>,
        ),
    >,
    mut cap_box_q: Query<&mut Visibility, (With<ChillarySnapCaptionBox>, Without<ChillaryAnim>)>,
    mut cap_txt_q: Query<
        &mut Text,
        (
            With<ChillarySnapCaptionText>,
            Without<ChillarySpeakerText>,
            Without<ChillarySubText>,
            Without<ChillarySnapTimerText>,
        ),
    >,
    mut timer_q: Query<
        &mut Text,
        (
            With<ChillarySnapTimerText>,
            Without<ChillarySpeakerText>,
            Without<ChillarySubText>,
            Without<ChillarySnapCaptionText>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    c_state.beat_t += dt;
    c_state.total_t += dt;

    let skipping = crate::skip::skipping();
    if skipping {
        c_state.beat = (CHILLARY_BEATS.len() as u8).saturating_sub(1);
        c_state.beat_t = c_state.beat_t.max(4.4);
    }

    let idx = (c_state.beat as usize).min(CHILLARY_BEATS.len() - 1);
    let min_t = CHILLARY_BEATS[idx].3;
    if c_state.beat_t >= min_t && (skipping || !is_voice_speaking() || c_state.beat_t >= 16.0) {
        if c_state.beat + 1 >= CHILLARY_BEATS.len() as u8 {
            next_state.set(GameState::CityPlay);
            return;
        }
        c_state.beat += 1;
        c_state.beat_t = 0.0;
        let (spk, line, voice, _, cap) = CHILLARY_BEATS[c_state.beat as usize];
        if c_state.beat == 1 {
            play_sfx("snap_shutter");
        } else {
            play_sfx("snap_pop");
        }
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
        if let Ok(mut cb_vis) = cap_box_q.get_single_mut() {
            *cb_vis = if cap.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
        }
        if let Ok(mut ct) = cap_txt_q.get_single_mut() {
            ct.0 = cap.to_string();
        }
    }

    // Update top-right Snapchat story countdown badge (10 -> 1)
    let rem = (11.8 - c_state.total_t).ceil().clamp(1.0, 10.0) as u32;
    if let Ok(mut tm) = timer_q.get_single_mut() {
        tm.0 = rem.to_string();
    }

    let beat = c_state.beat;
    let bt = c_state.beat_t;
    let t = time.elapsed_secs();
    let stage = Vec3::new(0.0, 0.0, -760.0);

    for (mut tf, mut vis, anim) in &mut anim_q {
        match anim.kind {
            ChillaryPart::BodyRoot => {
                let sway = match beat {
                    0 => (t * 2.4).sin() * 0.03,
                    1 => 0.04,
                    _ => (t * 4.5).sin() * 0.04,
                };
                tf.translation = stage;
                tf.rotation = Quat::from_rotation_y(sway);
            }
            ChillaryPart::Head => {
                // Beat 1 mimics the iconic bright awkward side tilt & frozen campaign smile after speaking
                let tilt_z = match beat {
                    0 => (t * 3.1).sin() * 0.05,
                    1 => -0.09 + (bt * 1.4).sin().max(0.0) * 0.02,
                    _ => (t * 3.5).sin() * 0.06,
                };
                let nod_x = match beat {
                    0 => 0.10 - (bt / 2.6).min(1.0) * 0.14,
                    1 => -0.04,
                    _ => (t * 6.0).sin() * 0.05,
                };
                tf.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, nod_x, tilt_z);
            }
            ChillaryPart::Brow { side } => {
                let lift = match beat {
                    0 => 0.0,
                    1 => 0.015,
                    _ => 0.010 + (t * 5.0).sin() * 0.004,
                };
                tf.translation = Vec3::new(side * 0.088, 0.094 + lift, 0.234);
                tf.rotation = Quat::from_rotation_z(-side * (0.08 + lift * 2.0));
            }
            ChillaryPart::Mouth => {
                let speaking = beat == 1 && bt < 2.2 || beat == 0 && bt < 2.1;
                let open_y = if speaking { 1.0 + (t * 16.0).sin().abs() * 0.45 } else if beat == 1 { 1.15 } else { 1.0 };
                tf.scale = Vec3::new(if beat == 1 { 1.10 } else { 1.0 }, open_y, 1.0);
            }
            ChillaryPart::SelfieArm => {
                let base_x = match beat {
                    0 => -0.95 - (bt / 2.5).min(1.0) * 0.25,
                    1 => -1.20 + (t * 2.2).sin() * 0.025,
                    _ => -1.12,
                };
                tf.rotation = Quat::from_euler(EulerRot::XYZ, base_x, -0.22, -0.18);
            }
            ChillaryPart::LeftArm => {
                if beat == 2 {
                    let wave = (t * 9.0).sin() * 0.25;
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, -0.65, 0.0, -0.55 - wave);
                } else {
                    tf.rotation = Quat::from_rotation_z(-0.08);
                }
            }
            ChillaryPart::StanPhotobomb => {
                *vis = if beat == 2 { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }

    // Authentic handheld vertical smartphone selfie camera angles
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 4.2).min(1.0);
        match beat {
            0 => {
                // Shaky low "boomer angle" rising onto her face
                let rise = (bt / 2.6).clamp(0.0, 1.0);
                let wobble = Vec3::new((t * 7.0).sin() * 0.025, (t * 9.0).cos() * 0.025, 0.0);
                c_tf.translation = stage + Vec3::new(0.18 - rise * 0.14, 1.42 + rise * 0.44, 0.94 - rise * 0.18) + wobble;
                c_tf.look_at(stage + Vec3::new(0.0, 1.92, 0.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.68 - rise * 0.18;
                }
            }
            1 => {
                // Ultra-close-up dead-on Snapchat selfie frame: "I'm just chillin' in Cedar Rapids."
                let wrist_x = (t * 2.1).sin() * 0.015;
                let wrist_y = (t * 1.7).cos() * 0.012;
                c_tf.translation = stage + Vec3::new(0.02 + wrist_x, 1.88 + wrist_y, 0.72 - push * 0.07);
                c_tf.look_at(stage + Vec3::new(0.0, 1.91, 0.0), Vec3::Y);
                c_tf.rotate_local_z((t * 1.3).sin() * 0.02);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.46 - push * 0.02;
                }
            }
            _ => {
                // Slightly pulled-back kitchen shot revealing bewildered Stan photobombing behind her!
                c_tf.translation = stage + Vec3::new(-0.08, 1.74, 1.48 - push * 0.10);
                c_tf.look_at(stage + Vec3::new(-0.28, 1.62, -0.35), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.64;
                }
            }
        }
    }
}

pub fn exit_chillary_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<ChillaryCutsceneEntity>>,
    ov_q: Query<Entity, With<ChillaryCutsceneOverlay>>,
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
    if !stats.chillary_chilled {
        stats.chillary_chilled = true;
        stats.score += 250;
        stats.hearts = (stats.hearts + 1).min(stats.max_hearts);
    }
    play_viking_chant_music();
}
