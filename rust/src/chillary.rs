//! Optional Cedar Rapids easter-egg NPC by the River Star Ferris wheel & her "Just Chillin' in
//! Cedar Rapids" viral Snapchat re-enactment + John Carpenter's *They Live* 1-second reptile
//! reveal cutscene (`GameState::ChillaryCutscene`).
//!
//! Why this was added:
//! - Gives exploring the River Star boardwalk plaza a humorous Cedar Rapids homage that opens with
//!   Stan walking in on her filming the selfie video, escalates with a sudden 1.0-second *They Live*
//!   reptilian reveal (`OBEY / CONFORM / STAY ASLEEP`), and ends with her calmly gaslighting Stan
//!   so the gag has full comedic pacing instead of ending abruptly.
//!
//! Gameplay impact:
//! - Completely optional secret interaction: **does NOT affect the Level 2 crew recruitment gate**,
//!   tendy unlocking, boss fight rules, or checkpoint save requirements.
//! - Interaction guards (`!riding_wheel`, no vehicle/phone, `player_scale < 4.5`, `y < 3.0`) and
//!   `city.rs` prompt priority ensure it never blocks boarding the Ferris wheel or Kaiju combat.
//! - First completion awards a modest one-time `+250` score and `+1` heart (clamped to
//!   `max_hearts`, guarded by `stats.chillary_chilled`), returns Stan safely to `saved_city_pos`,
//!   and cleanly clears all overlay/voice state even when skipped via `crate::skip`.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, set_they_live_mode,
    speak_voice, stop_voice,
};
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::state::{city_body_scale, GameState, LevelEntity, PlayerStats};
use crate::ui::HudOverlay;

/// Stands on the River Star Ferris wheel stone plaza (`y = 0.35`), just outside the left entrance
/// arch pillar (local `(-4.8, 0.35, 8.8)` rotated by `3*FRAC_PI_4`).
pub const CHILLARY_POS: Vec3 = Vec3::new(-104.38, 0.35, 111.17);
const CHILLARY_INTERACT_RANGE: f32 = 4.2;

const REPTILE_GLITCH_START: f32 = 0.65;
/// Glitch runs for 1.00s (`0.65..1.65`) before snapping right back to normal smiling Chillary.
const REPTILE_GLITCH_END: f32 = 1.65;

/// Optional boardwalk interaction state bridged into `city::update_city_props`.
#[derive(Resource, Default)]
pub struct ChillaryPrompt {
    pub label: Option<String>,
    pub pressed: bool,
}

/// Beat clock and one-shot SFX edge gate (`reptile_stage`) for `GameState::ChillaryCutscene`.
#[derive(Resource, Default)]
pub struct ChillaryCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
    pub total_t: f32,
    /// `0` = pre-glitch, `1` = 1.0s *They Live* reptile SFX fired, `2` = snap-back SFX fired.
    pub reptile_stage: u8,
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
pub struct ChillaryKitchenLight {
    pub base_color: Color,
    pub base_intensity: f32,
}

#[derive(Component)]
pub struct ChillarySpeakerText;

#[derive(Component)]
pub struct ChillarySubText;

/// Groups UI elements whose `Visibility` toggles between selfie framing and widescreen reaction
/// shots (`update_chillary_cutscene`) under one query to avoid Bevy B0001 mutable-visibility aliasing.
#[derive(Component, Clone, Copy)]
pub enum ChillaryUiSlot {
    SnapChrome,
    SnapCaption,
}

#[derive(Component)]
pub struct ChillarySnapCaptionText;

#[derive(Component)]
pub struct ChillarySnapTimerText;

#[derive(Component)]
pub struct ChillaryAnim {
    pub kind: ChillaryPart,
}

/// Unified animated-part tags queried cleanly without camera/UI alias conflicts; `HumanHead` and
/// `ReptileHead` swap visibility beneath the shared hair/earring `Head` root during Beat 3.
#[derive(Clone, Copy)]
pub enum ChillaryPart {
    BodyRoot,
    Head,
    HumanHead,
    ReptileHead,
    ReptileTongue,
    Brow { side: f32 },
    Mouth,
    SelfieArm,
    LeftArm,
    StanActor,
}

/// (speaker, line, voice, min_secs, snap_caption) - strictly ASCII for Bevy's default font.
const CHILLARY_BEATS: [(&str, &str, &str, f32, &str); 6] = [
    (
        "STAN (WALKING INTO THE KITCHEN)",
        "Wait a second... is that Hillary Clinton standing alone in a Cedar Rapids kitchen filming herself?",
        "stan",
        4.2,
        "",
    ),
    (
        "CHILLARY CLINTON (FILMING SNAPCHAT)",
        "I'm just chillin' in Cedar Rapids.",
        "chillary",
        4.2,
        "just chillin in Cedar Rapids",
    ),
    (
        "STAN (UTTERLY CONFUSED)",
        "Uh... WTF are you doing?! Why are you filming a selfie two inches from your face in a random kitchen?!",
        "stan",
        4.2,
        "",
    ),
    (
        "CHILLARY CLINTON (STOPS AND TURNS)",
        "(Chillary lowers her phone and locks eyes with Stan...)",
        "",
        2.75,
        "",
    ),
    (
        "STAN (LOSING HIS MIND)",
        "WHOA!! WHAT THE HELL?! YOU JUST TURNED INTO A REPTILE FOR A FULL SECOND!! YOUR EYES WENT SIDEWAYS AND A FORKED TONGUE CAME OUT!!",
        "stan",
        4.6,
        "",
    ),
    (
        "CHILLARY CLINTON (COMPLETELY UNBOTHERED)",
        "What's the problem, Stan? You must have had too many tendies. Relax... we're all just chillin' in Cedar Rapids.",
        "chillary",
        5.2,
        "what's the problem? just chillin in Cedar Rapids",
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
    reptile_scale_dark: Handle<StandardMaterial>,
    reptile_scale_light: Handle<StandardMaterial>,
    reptile_eye_glow: Handle<StandardMaterial>,
    reptile_gum: Handle<StandardMaterial>,
    reptile_tongue: Handle<StandardMaterial>,
    reptile_fang: Handle<StandardMaterial>,
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
        reptile_scale_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.34, 0.12),
            perceptual_roughness: 0.32,
            reflectance: 0.65,
            emissive: LinearRgba::new(0.03, 0.11, 0.02, 1.0),
            ..default()
        }),
        reptile_scale_light: materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.58, 0.24),
            perceptual_roughness: 0.36,
            emissive: LinearRgba::new(0.06, 0.09, 0.02, 1.0),
            ..default()
        }),
        reptile_eye_glow: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.82, 0.08),
            emissive: LinearRgba::new(3.6, 2.4, 0.1, 1.0),
            ..default()
        }),
        reptile_gum: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.05, 0.08),
            perceptual_roughness: 0.20,
            ..default()
        }),
        reptile_tongue: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.10, 0.22),
            emissive: LinearRgba::new(1.2, 0.08, 0.14, 1.0),
            perceptual_roughness: 0.18,
            ..default()
        }),
        reptile_fang: materials.add(StandardMaterial {
            base_color: Color::srgb(0.94, 0.92, 0.78),
            perceptual_roughness: 0.22,
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
        // Hair & pearl stud earrings remain anchored to the skull even during the 1-second
        // They Live reptilian glitch for uncanny pantsuit camouflage contrast.
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::hair_cap(hr, jaw, 0.34, -0.45, 0.14, 0.26))),
            MeshMaterial3d(mats.hair.clone()),
        ));
        hd.spawn((
            Mesh3d(sh::add(
                meshes,
                sh::superellipsoid(Vec3::new(0.17, 0.065, 0.08), 0.7, 0.8, 18, 10)
                    .deformed(|p| Vec3::new(p.x, p.y - 0.8 * p.x * p.x, p.z - 1.1 * p.x * p.x)),
            )),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(0.055, 0.145, 0.225).with_rotation(Quat::from_rotation_z(-0.22)),
        ));
        let bob_side = sh::add(
            meshes,
            sh::limb_profile(0.32, &[0.095, 0.11, 0.125, 0.075], 1.1).deformed(|p| {
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
            // Pearl stud earring
            hd.spawn((
                Mesh3d(meshes.add(Sphere::new(0.016))),
                MeshMaterial3d(mats.pearl.clone()),
                Transform::from_xyz(sx * 0.245, -0.06, 0.01),
            ));
        }
        hd.spawn((
            Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.24, 0.22, 0.16), 0.8, 0.8, 16, 12))),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(0.0, -0.06, -0.18),
        ));

        // Normal human face assembly (hidden during Beat 3's 1.0-second reptile flash)
        let mut human_face = hd.spawn((Transform::default(), Visibility::Inherited));
        if is_cutscene {
            human_face.insert(ChillaryAnim { kind: ChillaryPart::HumanHead });
        }
        human_face.with_children(|hf| {
            hf.spawn((
                Mesh3d(sh::add(meshes, sh::head(hr, jaw))),
                MeshMaterial3d(mats.skin.clone()),
            ));
            for sx in [-1.0_f32, 1.0] {
                hf.spawn((
                    Mesh3d(sh::add(meshes, sh::ear(0.105, sx))),
                    MeshMaterial3d(mats.skin.clone()),
                    Transform::from_xyz(sx * 0.235, -0.01, -0.01),
                ));
            }

            // Nose, eyes & expressive arched eyebrows
            hf.spawn((
                Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.032, 0.048, 0.040), 1.0, 1.0, 12, 8))),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, -0.008, 0.255),
            ));
            let brow_mesh = sh::add(meshes, sh::arc_bar(0.095, &[0.008, 0.015, 0.016, 0.010], 0.014, 0.012, 0.7));
            let eye_w = meshes.add(Sphere::new(0.034));
            let eye_i = meshes.add(Sphere::new(0.021));
            let eye_p = meshes.add(Sphere::new(0.012));
            for sx in [-1.0_f32, 1.0] {
                hf.spawn((
                    Mesh3d(eye_w.clone()),
                    MeshMaterial3d(mats.eye_white.clone()),
                    Transform::from_xyz(sx * 0.088, 0.046, 0.222).with_scale(Vec3::new(1.1, 0.92, 0.65)),
                ));
                hf.spawn((
                    Mesh3d(eye_i.clone()),
                    MeshMaterial3d(mats.eye_iris.clone()),
                    Transform::from_xyz(sx * 0.088, 0.046, 0.236),
                ));
                hf.spawn((
                    Mesh3d(eye_p.clone()),
                    MeshMaterial3d(mats.eye_pupil.clone()),
                    Transform::from_xyz(sx * 0.088, 0.046, 0.248),
                ));
                let mut br = hf.spawn((
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
            let mut mouth = hf.spawn((Transform::from_xyz(0.0, -0.092, 0.232), Visibility::default()));
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

        if is_cutscene {
            spawn_reptile_head(hd, meshes, mats, hr, jaw);
        }
    });
}

/// Spawns the hidden 1-second *They Live* reptilian humanoid skull underneath Chillary's blonde bob:
/// scaly green cranium, protruding snout, heavy brow crests, glowing amber vertical-slit eyes,
/// fanged gaping maw, throat frills, and a flicking forked crimson tongue.
fn spawn_reptile_head(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &ChillaryMaterials,
    hr: f32,
    jaw: f32,
) {
    use crate::shapes as sh;

    parent
        .spawn((
            ChillaryAnim { kind: ChillaryPart::ReptileHead },
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|rh| {
            // Scaly deformed cranium with sunken cheekbones & sinusoidal dermal bumps
            let scaly_skull = sh::head(hr * 0.96, jaw * 1.16).deformed(|p| {
                let cheek = sh::smoothstep(-0.14, 0.04, p.y)
                    * sh::smoothstep(0.08, -0.06, p.y)
                    * sh::smoothstep(0.06, 0.20, p.x.abs());
                let bumps = ((p.x * 48.0).sin() * (p.y * 44.0).cos() * (p.z * 42.0).sin()) * 0.008;
                let brow_shelf = if p.z > 0.14 && p.y > 0.05 && p.y < 0.16 { 0.026 } else { 0.0 };
                Vec3::new(p.x * (1.0 - 0.12 * cheek), p.y, p.z + brow_shelf) + p.normalize_or_zero() * bumps
            });
            rh.spawn((
                Mesh3d(sh::add(meshes, scaly_skull)),
                MeshMaterial3d(mats.reptile_scale_dark.clone()),
            ));

            // Protruding scaly lizard snout + nasal ridge
            let snout = sh::superellipsoid(Vec3::new(0.115, 0.072, 0.145), 0.75, 0.85, 18, 12).deformed(|p| {
                let scale = ((p.x * 54.0).sin() * (p.z * 50.0).cos()) * 0.006;
                let taper = (1.0 - p.z.max(0.0) * 1.8).clamp(0.68, 1.0);
                Vec3::new(p.x * taper, p.y, p.z) + p.normalize_or_zero() * scale
            });
            rh.spawn((
                Mesh3d(sh::add(meshes, snout)),
                MeshMaterial3d(mats.reptile_scale_light.clone()),
                Transform::from_xyz(0.0, -0.032, 0.245),
            ));

            // Flared reptilian nostrils + 3 forehead/snout dorsal spines
            let nostril_mesh = meshes.add(Sphere::new(0.016));
            for sx in [-1.0_f32, 1.0] {
                rh.spawn((
                    Mesh3d(nostril_mesh.clone()),
                    MeshMaterial3d(mats.eye_pupil.clone()),
                    Transform::from_xyz(sx * 0.044, 0.004, 0.355).with_scale(Vec3::new(1.0, 0.75, 1.3)),
                ));
            }
            let crest_mesh = meshes.add(Cone::new(0.018, 0.055));
            for (cy, cz, pitch) in [(0.195_f32, 0.165_f32, 0.35_f32), (0.125, 0.235, 0.58), (0.048, 0.295, 0.82)] {
                rh.spawn((
                    Mesh3d(crest_mesh.clone()),
                    MeshMaterial3d(mats.reptile_scale_light.clone()),
                    Transform::from_xyz(0.0, cy, cz).with_rotation(Quat::from_rotation_x(pitch)),
                ));
            }

            // Heavy menacing brow crests, glowing amber eyeballs, razor vertical slit pupils & eyelids
            let crest_bar = sh::add(meshes, sh::arc_bar(0.14, &[0.016, 0.028, 0.024, 0.014], 0.018, 0.024, 0.9));
            let eye_glow = meshes.add(Sphere::new(0.042));
            let slit_pupil = meshes.add(Cuboid::new(0.011, 0.068, 0.024));
            let lid_mesh = meshes.add(Cuboid::new(0.095, 0.016, 0.040));
            for sx in [-1.0_f32, 1.0] {
                rh.spawn((
                    Mesh3d(crest_bar.clone()),
                    MeshMaterial3d(mats.reptile_scale_light.clone()),
                    Transform::from_xyz(sx * 0.098, 0.102, 0.242)
                        .with_rotation(Quat::from_rotation_z(-sx * 0.34)),
                ));
                rh.spawn((
                    Mesh3d(eye_glow.clone()),
                    MeshMaterial3d(mats.reptile_eye_glow.clone()),
                    Transform::from_xyz(sx * 0.095, 0.052, 0.228).with_scale(Vec3::new(1.08, 0.95, 0.80)),
                ));
                rh.spawn((
                    Mesh3d(slit_pupil.clone()),
                    MeshMaterial3d(mats.eye_pupil.clone()),
                    Transform::from_xyz(sx * 0.095, 0.052, 0.258),
                ));
                for sy in [-0.036_f32, 0.038] {
                    rh.spawn((
                        Mesh3d(lid_mesh.clone()),
                        MeshMaterial3d(mats.reptile_scale_dark.clone()),
                        Transform::from_xyz(sx * 0.095, 0.052 + sy, 0.242)
                            .with_rotation(Quat::from_rotation_z(-sx * 0.16)),
                    ));
                }
                // Cheek / jaw frill plates flare out laterally
                rh.spawn((
                    Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.055, 0.095, 0.045), 0.7, 0.7, 12, 8))),
                    MeshMaterial3d(mats.reptile_scale_dark.clone()),
                    Transform::from_xyz(sx * 0.165, -0.075, 0.11)
                        .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.0, sx * 0.45, sx * 0.24)),
                ));
            }

            // Gaping red reptile maw + hinged lower mandible + 10 conical fangs
            rh.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.175, 0.095, 0.12))),
                MeshMaterial3d(mats.reptile_gum.clone()),
                Transform::from_xyz(0.0, -0.095, 0.240),
            ));
            rh.spawn((
                Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.105, 0.045, 0.135), 0.8, 0.8, 16, 10))),
                MeshMaterial3d(mats.reptile_scale_light.clone()),
                Transform::from_xyz(0.0, -0.145, 0.242).with_rotation(Quat::from_rotation_x(0.32)),
            ));
            let fang_small = meshes.add(Cone::new(0.009, 0.038));
            let fang_viper = meshes.add(Cone::new(0.012, 0.052));
            for (fx, fz, is_viper) in [
                (-0.072_f32, 0.305_f32, false),
                (-0.052, 0.338, true),
                (-0.020, 0.352, false),
                (0.020, 0.352, false),
                (0.052, 0.338, true),
                (0.072, 0.305, false),
            ] {
                rh.spawn((
                    Mesh3d(if is_viper { fang_viper.clone() } else { fang_small.clone() }),
                    MeshMaterial3d(mats.reptile_fang.clone()),
                    Transform::from_xyz(fx, -0.078, fz).with_rotation(Quat::from_rotation_x(PI)),
                ));
            }
            for (fx, fz) in [(-0.058_f32, 0.322_f32), (-0.022, 0.338), (0.022, 0.338), (0.058, 0.322)] {
                rh.spawn((
                    Mesh3d(fang_small.clone()),
                    MeshMaterial3d(mats.reptile_fang.clone()),
                    Transform::from_xyz(fx, -0.118, fz),
                ));
            }

            // Scaly neck collar visible just above her pearl necklace & lapels
            rh.spawn((
                Mesh3d(sh::add(meshes, sh::limb(0.16, 0.088, 0.095))),
                MeshMaterial3d(mats.reptile_scale_dark.clone()),
                Transform::from_xyz(0.0, -0.22, 0.02),
            ));

            // Flicking forked crimson reptile tongue
            let mut tongue_mesh = sh::rod(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.165), 0.020);
            tongue_mesh = tongue_mesh
                .merged(sh::rod(Vec3::new(0.0, 0.0, 0.160), Vec3::new(-0.038, 0.008, 0.255), 0.011))
                .merged(sh::rod(Vec3::new(0.0, 0.0, 0.160), Vec3::new(0.038, 0.008, 0.255), 0.011));
            rh.spawn((
                ChillaryAnim { kind: ChillaryPart::ReptileTongue },
                Mesh3d(sh::add(meshes, tongue_mesh)),
                MeshMaterial3d(mats.reptile_tongue.clone()),
                Transform::from_xyz(0.0, -0.102, 0.265),
                Visibility::Inherited,
            ));
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

    // Only offer the boardwalk easter egg while Stan is standing on foot at normal scale so it
    // never fires during Kaiju rampages, gondola rides, driving, or aerial stunts.
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

/// Enters the bright Cedar Rapids kitchen stage to re-enact "just chillin' in Cedar Rapids"
/// with a 1-second *They Live* reptilian reveal.
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
    // Defensive cleanup: guarantees *They Live* contrast grading and Beat 3 silence never linger
    // across restarts or mid-glitch skips (`crate::skip`) back into `CityPlay`.
    set_they_live_mode(false);
    for mut hv in &mut hud_q {
        *hv = Visibility::Hidden;
    }
    // Bright daytime window glow outside the Cedar Rapids home kitchen
    clear_col.0 = Color::srgb(0.76, 0.86, 0.96);
    let stage = Vec3::new(0.0, 0.0, -760.0);

    // Bright domestic kitchen lighting + soft front fill (snaps to stark horror glare on glitch)
    let key_color = Color::srgb(1.0, 0.96, 0.88);
    let key_intensity = 620_000.0;
    commands.spawn((
        ChillaryCutsceneEntity,
        ChillaryKitchenLight {
            base_color: key_color,
            base_intensity: key_intensity,
        },
        PointLight {
            color: key_color,
            intensity: key_intensity,
            range: 26.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 3.2, 1.4)),
    ));
    let fill_color = Color::srgb(0.95, 0.98, 1.0);
    let fill_intensity = 340_000.0;
    commands.spawn((
        ChillaryCutsceneEntity,
        ChillaryKitchenLight {
            base_color: fill_color,
            base_intensity: fill_intensity,
        },
        PointLight {
            color: fill_color,
            intensity: fill_intensity,
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

    // Real Stan (`plaid_hero.glb` faces -Z at IDENTITY, so negate delta to face Chillary at origin)
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    let stan_start = Vec3::new(-2.25, 0.0, 1.60);
    let stan_yaw = stan_start.x.atan2(stan_start.z); // (-(-dx)).atan2(-(-dz)) = x.atan2(z) toward origin
    commands.spawn((
        ChillaryCutsceneEntity,
        ChillaryAnim { kind: ChillaryPart::StanActor },
        SceneRoot(hero_scene),
        Transform::from_translation(stage + stan_start)
            .with_rotation(Quat::from_rotation_y(stan_yaw))
            .with_scale(Vec3::splat(1.08)),
        Visibility::Inherited,
    ));

    // Beat 0 wide kitchen establishing shot as Stan walks in and spots Chillary filming
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(-0.65, 1.94, 3.45);
        c_tf.look_at(stage + Vec3::new(-0.45, 1.50, 0.20), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.72;
        }
    }

    // Authentic 2015 vertical Snapchat chrome (visible on filming beat) + widescreen dialogue card
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
            // Left & right dark sidebars creating a vertical phone-story silhouette in Beat 1
            for is_left in [true, false] {
                root.spawn((
                    ChillaryUiSlot::SnapChrome,
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
                    Visibility::Hidden,
                ));
            }

            // Top row: SNAP STORY pill on left + classic Snapchat countdown circle on right
            root.spawn((
                ChillaryUiSlot::SnapChrome,
                Node {
                    width: Val::Percent(52.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(18.0)),
                    ..default()
                },
                Visibility::Hidden,
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
                    ChillaryUiSlot::SnapCaption,
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
                        Text::new(format_sub_line(line, voice)),
                        TextFont { font_size: 15.5, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });

    play_cutscene_music();
    play_sfx("snap_pop");
    if !voice.is_empty() {
        speak_voice(line, voice);
    }
}

fn format_sub_line(line: &str, voice: &str) -> String {
    if voice.is_empty() {
        line.to_string()
    } else {
        format!("\"{}\"", line)
    }
}

#[allow(clippy::type_complexity)]
pub fn update_chillary_cutscene(
    time: Res<Time>,
    mut c_state: ResMut<ChillaryCutsceneState>,
    mut next_state: crate::transition::SceneTransition,
    mut anim_q: Query<(&mut Transform, &mut Visibility, &ChillaryAnim), (Without<Camera3d>, Without<ChillaryUiSlot>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<ChillaryAnim>)>,
    mut light_q: Query<(&mut PointLight, &ChillaryKitchenLight)>,
    mut ui_vis_q: Query<(&mut Visibility, &ChillaryUiSlot), (Without<ChillaryAnim>, Without<Camera3d>)>,
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
        c_state.beat_t = c_state.beat_t.max(6.0);
    }

    let idx = (c_state.beat as usize).min(CHILLARY_BEATS.len() - 1);
    let min_t = CHILLARY_BEATS[idx].3;
    if c_state.beat_t >= min_t && (skipping || !is_voice_speaking() || c_state.beat_t >= 16.0) {
        if c_state.beat + 1 >= CHILLARY_BEATS.len() as u8 {
            set_they_live_mode(false);
            next_state.set(GameState::CityPlay);
            return;
        }
        c_state.beat += 1;
        c_state.beat_t = 0.0;
        c_state.reptile_stage = 0;
        set_they_live_mode(false);

        let (spk, line, voice, _, cap) = CHILLARY_BEATS[c_state.beat as usize];
        match c_state.beat {
            1 => play_sfx("snap_shutter"),
            3 => stop_voice(),
            4 => play_sfx("hit"),
            _ => play_sfx("snap_pop"),
        }
        if !voice.is_empty() {
            speak_voice(line, voice);
        }
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format_sub_line(line, voice);
        }
        if let Ok(mut ct) = cap_txt_q.get_single_mut() {
            ct.0 = cap.to_string();
        }
    }

    let beat = c_state.beat;
    let bt = c_state.beat_t;
    let t = time.elapsed_secs();
    let stage = Vec3::new(0.0, 0.0, -760.0);

    // Exact 1.0-second *They Live* reptile glitch trigger & snap-back inside Beat 3
    let is_reptile = beat == 3 && (REPTILE_GLITCH_START..REPTILE_GLITCH_END).contains(&bt);
    if beat == 3 && bt >= REPTILE_GLITCH_START && c_state.reptile_stage == 0 {
        c_state.reptile_stage = 1;
        set_they_live_mode(true);
        play_sfx("they_live_glitch");
        play_sfx("reptile_hiss");
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = "[SIGNAL GLITCH - OBEY / CONFORM / STAY ASLEEP]".to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = "[HISSING REPTILIAN STATIC]".to_string();
        }
    } else if beat == 3 && bt >= REPTILE_GLITCH_END && c_state.reptile_stage == 1 {
        c_state.reptile_stage = 2;
        set_they_live_mode(false);
        play_sfx("they_live_snapback");
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = "CHILLARY CLINTON".to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = "(Blinks pleasantly and smiles as if nothing happened.)".to_string();
        }
    }

    // Kitchen light snaps to cold flickering green-white glare during the 1-second reveal
    for (mut pl, kl) in &mut light_q {
        if is_reptile {
            pl.color = Color::srgb(0.80, 1.0, 0.84);
            pl.intensity = kl.base_intensity * (1.55 + (t * 47.0).sin() * 0.25);
        } else {
            pl.color = kl.base_color;
            pl.intensity = kl.base_intensity;
        }
    }

    // Snapchat frame chrome only on the selfie take (Beat 1); caption pill on Beats 1 & 5
    let cur_cap = CHILLARY_BEATS[(beat as usize).min(CHILLARY_BEATS.len() - 1)].4;
    for (mut vis, slot) in &mut ui_vis_q {
        *vis = match slot {
            ChillaryUiSlot::SnapChrome => {
                if beat == 1 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            }
            ChillaryUiSlot::SnapCaption => {
                if !cur_cap.is_empty() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            }
        };
    }

    if beat == 1 {
        let rem = (10.0 - bt * 2.0).ceil().clamp(1.0, 10.0) as u32;
        if let Ok(mut tm) = timer_q.get_single_mut() {
            tm.0 = rem.to_string();
        }
    }

    // Stan walks in during Beat 0, stands in Beats 1..3, stumbles back in panic in Beat 4
    let stan_idle = Vec3::new(-1.10, 0.0, 0.85);
    let stan_back = Vec3::new(-1.62, 0.0, 1.25);
    let stan_pos = match beat {
        0 => {
            let walk_u = (bt / 2.7).clamp(0.0, 1.0);
            let smooth_u = walk_u * walk_u * (3.0 - 2.0 * walk_u);
            let xz = Vec3::new(-2.25, 0.0, 1.60).lerp(stan_idle, smooth_u);
            let stride_bob = if walk_u < 0.98 { (bt * 9.5).sin().abs() * 0.07 } else { 0.0 };
            xz + Vec3::Y * stride_bob
        }
        1..=3 => stan_idle,
        4 => {
            let recoil = (bt / 0.65).clamp(0.0, 1.0);
            let smooth_r = 1.0 - (1.0 - recoil) * (1.0 - recoil);
            let jump_y = (recoil * PI).sin() * 0.09;
            stan_idle.lerp(stan_back, smooth_r) + Vec3::Y * jump_y
        }
        _ => stan_back,
    };

    for (mut tf, mut vis, anim) in &mut anim_q {
        match anim.kind {
            ChillaryPart::BodyRoot => {
                let yaw = match beat {
                    0 => (t * 1.8).sin() * 0.03,
                    1 => 0.04,
                    2 => {
                        let turn = (bt / 0.8).clamp(0.0, 1.0);
                        0.04 - turn * 0.59
                    }
                    3 => -0.58 + if is_reptile { (t * 44.0).sin() * 0.04 } else { 0.0 },
                    4 => -0.55,
                    _ => -0.30 + (t * 1.9).sin() * 0.04,
                };
                let lunge = if is_reptile {
                    Vec3::new(-0.06, 0.0, 0.05)
                } else {
                    Vec3::ZERO
                };
                tf.translation = stage + lunge;
                tf.rotation = Quat::from_rotation_y(yaw);
            }
            ChillaryPart::Head => {
                let (yaw_y, nod_x, tilt_z) = match beat {
                    0 => (0.0, 0.06 - (bt / 2.6).min(1.0) * 0.10, (t * 2.5).sin() * 0.04),
                    1 => (0.0, -0.04, -0.09 + (bt * 1.4).sin().max(0.0) * 0.02),
                    2 => {
                        let turn = (bt / 0.7).clamp(0.0, 1.0);
                        (-turn * 0.30, (t * 3.0).sin() * 0.03, turn * 0.05)
                    }
                    3 => {
                        if is_reptile {
                            (
                                -0.33 + (t * 51.0).sin() * 0.04,
                                -0.06 + (t * 43.0).cos() * 0.04,
                                -0.14 + (t * 37.0).sin() * 0.06,
                            )
                        } else if bt >= REPTILE_GLITCH_END {
                            (-0.33, -0.02, 0.07)
                        } else {
                            (-0.33, 0.02, 0.03)
                        }
                    }
                    4 => (-0.30, 0.0, 0.07),
                    _ => (-0.18 + (t * 2.2).sin() * 0.06, -0.03, -0.06 + (t * 2.0).sin() * 0.03),
                };
                tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw_y, nod_x, tilt_z);
            }
            ChillaryPart::HumanHead => {
                *vis = if is_reptile { Visibility::Hidden } else { Visibility::Inherited };
            }
            ChillaryPart::ReptileHead => {
                *vis = if is_reptile { Visibility::Inherited } else { Visibility::Hidden };
            }
            ChillaryPart::ReptileTongue => {
                if is_reptile {
                    let flick = ((bt - REPTILE_GLITCH_START) * 26.0).sin().abs();
                    tf.scale = Vec3::new(1.0, 1.0, 0.45 + flick * 0.95);
                    tf.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        (t * 36.0).sin() * 0.22,
                        (t * 29.0).cos() * 0.28,
                        0.0,
                    );
                } else {
                    tf.scale = Vec3::ZERO;
                }
            }
            ChillaryPart::Brow { side } => {
                let lift = match beat {
                    0 => 0.0,
                    1 => 0.015,
                    3 if bt >= REPTILE_GLITCH_END => 0.014,
                    5 => 0.016 + ((t * 3.5).sin().max(0.0)) * 0.008,
                    _ => 0.006,
                };
                tf.translation = Vec3::new(side * 0.088, 0.094 + lift, 0.234);
                tf.rotation = Quat::from_rotation_z(-side * (0.08 + lift * 2.0));
            }
            ChillaryPart::Mouth => {
                let speaking = (beat == 1 && bt < 2.2) || (beat == 5 && bt < 4.6);
                let wide_smile = beat == 1 || (beat == 3 && bt >= REPTILE_GLITCH_END) || beat == 5;
                let open_y = if speaking {
                    1.0 + (t * 16.0).sin().abs() * 0.45
                } else if wide_smile {
                    1.15
                } else {
                    0.85
                };
                tf.scale = Vec3::new(if wide_smile { 1.10 } else { 0.96 }, open_y, 1.0);
            }
            ChillaryPart::SelfieArm => {
                let base_x = match beat {
                    0 => -1.16 + (t * 2.0).sin() * 0.04,
                    1 => -1.20 + (t * 2.2).sin() * 0.025,
                    2 => {
                        let drop = (bt / 0.7).clamp(0.0, 1.0);
                        -1.18 + drop * 0.98
                    }
                    3 | 4 => -0.18,
                    _ => {
                        let raise = (bt / 0.7).clamp(0.0, 1.0);
                        -0.18 - raise * 0.94 + (t * 2.0).sin() * 0.02
                    }
                };
                tf.rotation = Quat::from_euler(EulerRot::XYZ, base_x, -0.22, -0.18);
            }
            ChillaryPart::LeftArm => {
                if beat == 5 {
                    let wave = (t * 6.5).sin() * 0.16;
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, -0.45, 0.0, -0.35 - wave);
                } else {
                    tf.rotation = Quat::from_rotation_z(-0.08);
                }
            }
            ChillaryPart::StanActor => {
                *vis = Visibility::Inherited;
                tf.translation = stage + stan_pos;
                let base_yaw = stan_pos.x.atan2(stan_pos.z);
                let (pitch, roll) = match beat {
                    0 => {
                        let w = if bt < 2.65 { 1.0 } else { 0.0 };
                        (-0.05 * w, (bt * 9.5).cos() * 0.06 * w)
                    }
                    2 => ((t * 7.5).sin().abs() * 0.06, (t * 4.5).sin() * 0.03),
                    4 => (0.11 + (t * 26.0).sin() * 0.04, (t * 22.0).cos() * 0.05),
                    _ => (0.0, 0.0),
                };
                tf.rotation = Quat::from_euler(EulerRot::YXZ, base_yaw, pitch, roll);
            }
        }
    }

    // Dynamic shot blocking: establishing walk-in, Snapchat selfie, Stan reverse shots, and *They Live* glitch
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 4.2).min(1.0);
        match beat {
            0 => {
                // Wide kitchen establishing shot as Stan enters from the left and spots Chillary
                c_tf.translation = stage + Vec3::new(-0.65 + push * 0.14, 1.94, 3.45 - push * 0.25);
                c_tf.look_at(stage + Vec3::new(-0.45, 1.50, 0.20), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72 - push * 0.04;
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
            2 => {
                // Unobstructed front 3/4 reverse shot on bewildered Stan: "WTF are you doing?!"
                c_tf.translation = stage + Vec3::new(0.14 - push * 0.08, 1.92, 0.62);
                c_tf.look_at(stage + stan_idle + Vec3::Y * 1.92, Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.56;
                }
            }
            3 => {
                // Stan's POV onto Chillary -> sudden 1.0-second *They Live* reptile snap-in!
                if is_reptile {
                    let jitter = Vec3::new((t * 61.0).sin() * 0.012, (t * 73.0).cos() * 0.012, 0.0);
                    c_tf.translation = stage + Vec3::new(-0.66, 1.90, 0.50) + jitter;
                    c_tf.look_at(stage + Vec3::new(0.0, 1.91, 0.0), Vec3::Y);
                    c_tf.rotate_local_z(0.075);
                    if let Projection::Perspective(ref mut p) = *proj {
                        p.fov = 0.54;
                    }
                } else {
                    c_tf.translation = stage + Vec3::new(-0.76, 1.91, 0.58);
                    c_tf.look_at(stage + Vec3::new(0.0, 1.93, 0.0), Vec3::Y);
                    if let Projection::Perspective(ref mut p) = *proj {
                        p.fov = 0.56;
                    }
                }
            }
            4 => {
                // Unobstructed reverse shot on Stan stumbling backward in total disbelief
                let panic_wobble = Vec3::new((t * 28.0).sin() * 0.014, (t * 34.0).cos() * 0.014, 0.0);
                c_tf.translation = stage + Vec3::new(-0.18, 1.92, 0.55) + panic_wobble;
                c_tf.look_at(stage + stan_pos + Vec3::Y * 1.90, Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54;
                }
            }
            _ => {
                // Calm, gaslighting close-up on normal smiling Chillary raising her phone again
                c_tf.translation = stage + Vec3::new(-0.34, 1.88, 0.96 - push * 0.06);
                c_tf.look_at(stage + Vec3::new(0.0, 1.91, 0.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.50 - push * 0.02;
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
    // Defensive cleanup: guarantees *They Live* contrast grading and Beat 3 silence never linger
    // across restarts or mid-glitch skips (`crate::skip`) back into `CityPlay`.
    set_they_live_mode(false);
    stop_voice();
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
    // Modest one-time exploration bonus (`+250` score, `+1` heart); leaves crew gate and tendies untouched.
    if !stats.chillary_chilled {
        stats.chillary_chilled = true;
        stats.score += 250;
        stats.hearts = (stats.hearts + 1).min(stats.max_hearts);
    }
    play_viking_chant_music();
}
