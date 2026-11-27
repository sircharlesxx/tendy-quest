//! Pub 217 attractive bartender (Maria), "ORDER 10 BEERS" prompt, and the 10-beers table
//! & Women's Bathroom Magic Portal cutscene (`GameState::PubPortalCutscene`) that warps Stan,
//! TJ, Nate, and Glenn into Level 3 (`GameState::CasinoPlay`).
//!
//! Why: gives the Casino a hilarious, memorable entrance straight out of Pub 217 while keeping
//! the main city skyline uncluttered and giving `CasinoPlay` a dedicated full-room stage.
//!
//! Gameplay impact: talking to Maria behind the bar in Pub 217 (during CityPlay or Free Roam)
//! and pressing "ORDER 10 BEERS" plays a voiced, skippable (`crate::skip`) cutscene where the 4
//! guys drink 10 beers, debate TMNT movies, notice crowds vanishing into the women's restroom,
//! and get pulled through a portal straight into Tendy Palace Casino (without affecting story gates).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, set_touch_playing,
    speak_voice, stop_voice,
};
use crate::casino::CasinoArt;
use crate::city::{CityWorldMarker, PUB217_ANCHOR, PUB217_SCALE};
use crate::player::Player;
use crate::shapes as sh;
use crate::state::{GameState, LevelEntity, PlayerStats};
use crate::transition::{SceneTransition, sim_running};
use crate::ui::{HelpPause, HudOverlay};

// =================================================================================================
// Plugin & Public Prompt Resource
// =================================================================================================

/// Prompt handoff shared with `city::update_city_props`.
#[derive(Resource, Default)]
pub struct BartenderPrompt {
    pub label: Option<String>,
    pub pressed: bool,
}

pub struct PubPortalPlugin;

impl Plugin for PubPortalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BartenderPrompt>()
            .init_resource::<PubPortalCutsceneState>()
            .add_systems(OnExit(GameState::CityPlay), cleanup_bartender_on_exit)
            .add_systems(
                Update,
                (
                    spawn_pub_bartender_if_needed,
                    handle_bartender_modal_input.run_if(sim_running),
                )
                    .run_if(in_state(GameState::CityPlay)),
            )
            .add_systems(OnEnter(GameState::PubPortalCutscene), enter_pub_portal_cutscene)
            .add_systems(OnExit(GameState::PubPortalCutscene), exit_pub_portal_cutscene)
            .add_systems(
                Update,
                update_pub_portal_cutscene
                    .run_if(in_state(GameState::PubPortalCutscene))
                    .run_if(sim_running),
            );
    }
}

// =================================================================================================
// Sculpted 3D Bartender Rig (Maria) + Women's Bathroom Door Prop in Pub 217
// =================================================================================================

/// Shared procedural 3D bartender rig (`+Z` local forward).
pub(crate) fn spawn_bartender_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.78, 0.68),
        perceptual_roughness: 0.56,
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.14, 0.09),
        perceptual_roughness: 0.45,
        ..default()
    });
    let hair_hi = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.24, 0.14),
        perceptual_roughness: 0.38,
        ..default()
    });
    let dress = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.07, 0.20),
        perceptual_roughness: 0.35,
        metallic: 0.08,
        ..default()
    });
    let skirt = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.07, 0.10),
        perceptual_roughness: 0.48,
        ..default()
    });
    let apron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.94, 0.96),
        perceptual_roughness: 0.72,
        ..default()
    });
    let lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.08, 0.22),
        perceptual_roughness: 0.2,
        ..default()
    });
    let iris = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.74, 0.44),
        emissive: LinearRgba::rgb(0.04, 0.18, 0.10),
        perceptual_roughness: 0.2,
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.98, 1.0),
        perceptual_roughness: 0.25,
        ..default()
    });
    let lash = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.04, 0.06),
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.28),
        metallic: 0.82,
        perceptual_roughness: 0.2,
        ..default()
    });
    let chrome = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.90, 0.96),
        metallic: 0.90,
        perceptual_roughness: 0.16,
        ..default()
    });
    let blush = materials.add(StandardMaterial {
        base_color: Color::srgba(0.98, 0.42, 0.52, 0.65),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    // Slender legs + heels
    let thigh_mesh = sh::add(meshes, sh::limb_profile(0.42, &[0.095, 0.082, 0.066], 1.0));
    let calf_mesh = sh::add(meshes, sh::limb_profile(0.40, &[0.064, 0.056, 0.040], 1.0));
    let heel_mesh = sh::add(meshes, sh::shoe(0.12, 0.08, 0.22));
    for sx in [-0.11_f32, 0.11] {
        parent.spawn((
            Mesh3d(thigh_mesh.clone()),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(sx, 0.62, 0.0),
        ));
        parent.spawn((
            Mesh3d(calf_mesh.clone()),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(sx, 0.24, 0.0),
        ));
        parent.spawn((
            Mesh3d(heel_mesh.clone()),
            MeshMaterial3d(dress.clone()),
            Transform::from_xyz(sx, 0.05, 0.04),
        ));
        parent.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.014, 0.08))),
            MeshMaterial3d(gold.clone()),
            Transform::from_xyz(sx, 0.04, -0.05),
        ));
    }

    // Mini skirt + crisp white bartender apron + hourglass fitted velvet bodice
    let skirt_mesh = sh::add(
        meshes,
        sh::loft(
            &[
                sh::ring(0.69, 0.0, 0.0),
                sh::ring(0.70, 0.23, 0.17),
                sh::ring(0.84, 0.19, 0.15),
                sh::ring(0.96, 0.15, 0.12),
                sh::ring(0.97, 0.0, 0.0),
            ],
            2,
            20,
            1.0,
        ),
    );
    parent.spawn((
        Mesh3d(skirt_mesh),
        MeshMaterial3d(skirt),
        Transform::IDENTITY,
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::plate(0.28, 0.26, 0.04, 0.35))),
        MeshMaterial3d(apron),
        Transform::from_xyz(0.0, 0.83, 0.165),
    ));

    let torso_mesh = sh::add(
        meshes,
        sh::loft(
            &[
                sh::ring(0.93, 0.0, 0.0),
                sh::ring(0.94, 0.145, 0.11),
                sh::ring(1.07, 0.130, 0.10).dz(0.005),
                sh::ring(1.24, 0.185, 0.148).dz(0.025),
                sh::ring(1.38, 0.175, 0.115),
                sh::ring(1.44, 0.12, 0.085),
                sh::ring(1.45, 0.0, 0.0),
            ],
            2,
            22,
            1.0,
        ),
    );
    parent.spawn((
        Mesh3d(torso_mesh),
        MeshMaterial3d(dress),
        Transform::IDENTITY,
    ));

    // Neck, collarbone & golden pendant necklace
    parent.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.055, 0.16))),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.0, 1.49, 0.01),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Torus::new(0.012, 0.068))),
        MeshMaterial3d(gold.clone()),
        Transform::from_xyz(0.0, 1.43, 0.02).with_rotation(Quat::from_rotation_x(0.25)),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Sphere::new(0.022))),
        MeshMaterial3d(gold.clone()),
        Transform::from_xyz(0.0, 1.40, 0.092),
    ));

    // Sculpted arms (left arm resting warmly, right arm holding a silver cocktail shaker)
    let arm_mesh = sh::add(meshes, sh::limb_profile(0.46, &[0.052, 0.044, 0.034], 1.0));
    parent.spawn((
        Mesh3d(arm_mesh.clone()),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(-0.22, 1.20, 0.04).with_rotation(Quat::from_rotation_z(-0.20)),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::hand(0.08, -1.0, false))),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(-0.26, 0.96, 0.06),
    ));
    parent.spawn((
        Mesh3d(arm_mesh),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.22, 1.26, 0.09)
            .with_rotation(Quat::from_rotation_x(-0.62) * Quat::from_rotation_z(0.16)),
    ));
    // Chrome cocktail shaker in right hand
    parent
        .spawn((
            BartenderShakerPivot,
            Transform::from_xyz(0.25, 1.28, 0.24).with_rotation(Quat::from_rotation_x(-0.35)),
            Visibility::default(),
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.055, 0.22))),
                MeshMaterial3d(chrome.clone()),
                Transform::IDENTITY,
            ));
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.028, 0.05))),
                MeshMaterial3d(chrome),
                Transform::from_xyz(0.0, 0.13, 0.0),
            ));
        });

    // Head + expressive face + lush wavy auburn hair
    parent
        .spawn((
            BartenderHeadPivot,
            Transform::from_xyz(0.0, 1.56, 0.02),
            Visibility::default(),
        ))
        .with_children(|h| {
            h.spawn((
                Mesh3d(sh::add(meshes, sh::head(0.185, 0.125))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 0.14, 0.0),
            ));
            // Rosy cheeks
            for sx in [-1.0_f32, 1.0_f32] {
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.032))),
                    MeshMaterial3d(blush.clone()),
                    Transform::from_xyz(sx * 0.095, 0.105, 0.145).with_scale(Vec3::new(1.3, 0.75, 0.45)),
                ));
                // Emerald eyes, lashes, catchlight & gold hoop earring
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.030))),
                    MeshMaterial3d(eye_white.clone()),
                    Transform::from_xyz(sx * 0.068, 0.165, 0.162).with_scale(Vec3::new(1.15, 0.92, 0.48)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.021))),
                    MeshMaterial3d(iris.clone()),
                    Transform::from_xyz(sx * 0.068, 0.164, 0.174).with_scale(Vec3::new(1.0, 1.0, 0.42)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.011))),
                    MeshMaterial3d(lash.clone()),
                    Transform::from_xyz(sx * 0.068, 0.164, 0.181),
                ));
                h.spawn((
                    Mesh3d(sh::add(meshes, sh::plate(0.068, 0.014, 0.012, 0.3))),
                    MeshMaterial3d(lash.clone()),
                    Transform::from_xyz(sx * 0.068, 0.194, 0.168)
                        .with_rotation(Quat::from_rotation_z(-sx * 0.14)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Torus::new(0.007, 0.034))),
                    MeshMaterial3d(gold.clone()),
                    Transform::from_xyz(sx * 0.185, 0.08, 0.01)
                        .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
                ));
            }
            // Glossy ruby lips
            h.spawn((
                Mesh3d(meshes.add(Sphere::new(0.028))),
                MeshMaterial3d(lip),
                Transform::from_xyz(0.0, 0.068, 0.172).with_scale(Vec3::new(1.45, 0.46, 0.45)),
            ));
            // Hair crown + wavy side & back locks
            h.spawn((
                Mesh3d(sh::add(meshes, sh::hair_cap(0.198, 0.135, 0.14, 0.25, 0.055, 0.045))),
                MeshMaterial3d(hair.clone()),
                Transform::from_xyz(0.0, 0.15, -0.01),
            ));
            h.spawn((
                Mesh3d(meshes.add(Sphere::new(0.21))),
                MeshMaterial3d(hair.clone()),
                Transform::from_xyz(0.0, 0.12, -0.10).with_scale(Vec3::new(1.05, 1.35, 0.92)),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.115))),
                    MeshMaterial3d(hair_hi.clone()),
                    Transform::from_xyz(sx * 0.16, 0.04, -0.02).with_scale(Vec3::new(0.75, 1.85, 0.85)),
                ));
            }
        });
}

#[derive(Component)]
struct PubBartenderRoot;

#[derive(Component)]
pub struct BartenderShakerPivot;

#[derive(Component)]
pub struct BartenderHeadPivot;

#[derive(Component)]
struct BartenderUiRoot;

#[derive(Component)]
struct BartenderTeleportPrompt;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BartenderAction {
    OrderTenBeers,
    OrderTenBeersAndCraft,
    BooberEats,
    TeleportEveryone,
    GoAlone,
    Leave,
}

pub(crate) fn bartender_bar_trigger_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * PUB217_SCALE
}

fn womens_door_world_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(-3.75, 0.0, -6.40) * PUB217_SCALE
}

fn spawn_pub_bartender_if_needed(
    mut commands: Commands,
    stats: Res<PlayerStats>,
    art: Option<Res<CasinoArt>>,
    existing_q: Query<Entity, With<PubBartenderRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !stats.in_city || !existing_q.is_empty() {
        return;
    }
    let Some(art) = art else {
        return;
    };

    // Maria standing behind the Pub 217 bar facing East (+X) toward customers
    let bar_pos = PUB217_ANCHOR + Vec3::new(-9.15, 0.0, -0.8) * PUB217_SCALE;
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            PubBartenderRoot,
            Transform::from_translation(bar_pos)
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                .with_scale(Vec3::splat(PUB217_SCALE * 0.96)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_bartender_rig(p, &mut meshes, &mut materials);
        });

    // Warm rose-gold bar light illuminating Maria
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        PubBartenderRoot,
        PointLight {
            color: Color::srgb(1.0, 0.84, 0.72),
            intensity: 26_000.0,
            range: 9.5,
            ..default()
        },
        Transform::from_translation(bar_pos + Vec3::new(0.8, 2.75 * PUB217_SCALE, 0.0)),
    ));

    // Pub 217 Women's Bathroom Door mounted flush on the North interior wall (facing South +Z)
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let frame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.07, 0.05),
        perceptual_roughness: 0.45,
        ..default()
    });
    let door_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.13, 0.09),
        perceptual_roughness: 0.40,
        ..default()
    });
    let brass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.25),
        metallic: 0.8,
        perceptual_roughness: 0.22,
        ..default()
    });
    let portal_seep_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.38, 0.85),
        emissive: LinearRgba::rgb(2.4, 0.65, 1.85),
        unlit: true,
        ..default()
    });
    let sign_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(art.womens_sign.clone()),
        emissive: LinearRgba::rgb(1.3, 1.0, 1.25),
        emissive_texture: Some(art.womens_sign.clone()),
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            PubBartenderRoot,
            Transform::from_translation(womens_door_world_pos()).with_scale(Vec3::splat(PUB217_SCALE)),
            Visibility::default(),
        ))
        .with_children(|d| {
            d.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(frame_mat),
                Transform::from_xyz(0.0, 1.25, 0.0).with_scale(Vec3::new(1.56, 2.50, 0.16)),
            ));
            d.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(door_mat),
                Transform::from_xyz(0.0, 1.22, 0.04).with_scale(Vec3::new(1.36, 2.38, 0.12)),
            ));
            d.spawn((
                Mesh3d(meshes.add(Sphere::new(0.07))),
                MeshMaterial3d(brass_mat),
                Transform::from_xyz(0.50, 1.05, 0.14),
            ));
            d.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(portal_seep_mat),
                Transform::from_xyz(0.0, 0.04, 0.12).with_scale(Vec3::new(1.32, 0.05, 0.06)),
            ));
            d.spawn((
                Mesh3d(unit_box),
                MeshMaterial3d(sign_mat),
                Transform::from_xyz(0.0, 2.58, 0.12).with_scale(Vec3::new(1.15, 0.56, 0.06)),
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_pub_bartender(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<CasinoArt>>,
    mut stats: ResMut<PlayerStats>,
    help: Res<HelpPause>,
    boober: Res<crate::city_events::BooberEatsState>,
    mut prompt: ResMut<BartenderPrompt>,
    player_q: Query<&Transform, (With<Player>, Without<BartenderHeadPivot>, Without<BartenderShakerPivot>)>,
    mut head_q: Query<&mut Transform, (With<BartenderHeadPivot>, Without<BartenderShakerPivot>)>,
    mut shaker_q: Query<&mut Transform, (With<BartenderShakerPivot>, Without<BartenderHeadPivot>)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let t = time.elapsed_secs();
    for mut tf in &mut shaker_q {
        tf.rotation = Quat::from_rotation_x(-0.35 + (t * 6.5).sin() * 0.18)
            * Quat::from_rotation_z((t * 3.2).cos() * 0.08);
    }
    for mut tf in &mut head_q {
        tf.rotation = Quat::from_rotation_y((t * 0.9).sin() * 0.12);
    }

    prompt.label = None;
    let Some(art) = art else {
        prompt.pressed = false;
        return;
    };
    if !stats.in_city || stats.bartender_open || stats.phone_open || stats.riding_wheel || stats.char_select_open || help.open {
        prompt.pressed = false;
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        prompt.pressed = false;
        return;
    };
    let on_foot = stats.driving_car.is_none()
        && stats.riding_scooter.is_none()
        && p_tf.translation.y < 3.5
        && stats.player_scale <= 3.2;
    if !on_foot {
        prompt.pressed = false;
        return;
    }

    let spot = bartender_bar_trigger_pos();
    let dist = Vec2::new(p_tf.translation.x - spot.x, p_tf.translation.z - spot.z).length();
    if dist < 3.6 {
        prompt.label = Some("TALK TO MARIA [F/E]".to_string());
        if prompt.pressed {
            prompt.pressed = false;
            stats.bartender_open = true;
            set_touch_playing(false);
            if let Ok(mut win) = window_q.get_single_mut() {
                win.cursor_options.grab_mode = CursorGrabMode::None;
                win.cursor_options.visible = true;
            }
            play_sfx("chip_bet");
            speak_voice(
                "Hey handsome! Ready to order ten beers for you and the boys?",
                "bartender",
            );
            spawn_bartender_ui(&mut commands, &art, boober.on_shift, stats.lexie_defeated || stats.free_roam);
            return;
        }
    }
    prompt.pressed = false;
}

fn spawn_bartender_ui(
    commands: &mut Commands,
    art: &CasinoArt,
    boober_on_shift: bool,
    craft_unlocked: bool,
) {
    commands
        .spawn((
            BartenderUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.03, 0.68)),
            GlobalZIndex(900),
        ))
        .with_children(|bg| {
            bg.spawn((
                Node {
                    width: Val::Percent(94.0),
                    max_width: Val::Px(540.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.11, 0.04, 0.09, 0.96)),
                BorderColor(Color::srgb(0.98, 0.78, 0.30)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(14.0),
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        Node {
                            padding: UiRect::all(Val::Px(3.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor(Color::srgb(1.0, 0.72, 0.84)),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|frame| {
                        frame.spawn((
                            ImageNode::new(art.bartender_portrait.clone()),
                            Node {
                                width: Val::Px(112.0),
                                height: Val::Px(112.0),
                                ..default()
                            },
                        ));
                    });

                    top.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        flex_shrink: 1.0,
                        ..default()
                    })
                    .with_children(|info| {
                        info.spawn((
                            Text::new("MARIA - PUB 217 BARTENDER"),
                            TextFont { font_size: 17.0, ..default() },
                            TextColor(Color::srgb(1.0, 0.84, 0.30)),
                        ));
                        info.spawn((
                            Text::new("\"Hey handsome! Pull up a table with TJ, Nate, and Glenn -- ready for 10 ice-cold beers?\""),
                            TextFont { font_size: 14.5, ..default() },
                            TextColor(Color::srgb(0.98, 0.94, 0.98)),
                        ));
                        info.spawn((
                            Text::new("Orders 10 beers on the house & starts the Pub 217 hangout!"),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::srgb(0.84, 0.76, 0.88)),
                        ));
                    });
                });

                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        BartenderAction::Leave,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.32, 0.14, 0.20)),
                        BorderColor(Color::srgba(1.0, 0.82, 0.42, 0.7)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("MAYBE LATER [ESC]"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    row.spawn((
                        BartenderAction::BooberEats,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.44, 0.56)),
                        BorderColor(Color::srgb(0.28, 0.94, 1.0)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(if boober_on_shift { "END BOOBEREATS" } else { "BOOBEREATS JOB" }),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    row.spawn((
                        BartenderAction::OrderTenBeers,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.76, 0.44, 0.08)),
                        BorderColor(Color::srgb(1.0, 0.88, 0.34)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("ORDER 10 BEERS [SPACE]"),
                            TextFont { font_size: 15.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    if craft_unlocked {
                        row.spawn((
                            BartenderAction::OrderTenBeersAndCraft,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.32, 0.14, 0.62)),
                            BorderColor(Color::srgb(0.42, 0.96, 0.68)),
                            BorderRadius::all(Val::Px(8.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("ORDER 10 BEERS & CRAFT [C]"),
                                TextFont { font_size: 14.5, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn handle_bartender_modal_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut boober: ResMut<crate::city_events::BooberEatsState>,
    mut next_state: SceneTransition,
    btn_q: Query<(&Interaction, &BartenderAction), Changed<Interaction>>,
    ui_q: Query<Entity, With<BartenderUiRoot>>,
    teleport_prompt_q: Query<(), With<BartenderTeleportPrompt>>,
) {
    if !stats.bartender_open {
        return;
    }

    let mut action_choice = None;
    if keys.just_pressed(KeyCode::Escape) {
        action_choice = Some(BartenderAction::Leave);
    } else if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
        if !teleport_prompt_q.is_empty() {
            action_choice = Some(BartenderAction::TeleportEveryone);
        } else {
            action_choice = Some(BartenderAction::OrderTenBeers);
        }
    } else if keys.just_pressed(KeyCode::KeyC)
        && teleport_prompt_q.is_empty()
        && (stats.lexie_defeated || stats.free_roam)
    {
        action_choice = Some(BartenderAction::OrderTenBeersAndCraft);
    } else {
        for (inter, act) in &btn_q {
            if *inter == Interaction::Pressed {
                action_choice = Some(*act);
                break;
            }
        }
    }

    let Some(act) = action_choice else {
        return;
    };

    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    stats.bartender_open = false;

    match act {
        BartenderAction::Leave => {
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            stop_voice();
        }
        BartenderAction::BooberEats => {
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            stop_voice();
            if boober.on_shift {
                boober.want_end_shift = true;
            } else {
                boober.want_start_order = true;
            }
        }
        BartenderAction::OrderTenBeers => {
            let sq = crate::audio::net_squad_info();
            let total_squad = sq[0];
            let in_city_peers = sq[3];
            stats.bartender_open = true;
            spawn_casino_teleport_prompt(&mut commands, in_city_peers, total_squad);
        }
        BartenderAction::OrderTenBeersAndCraft => {
            stop_voice();
            play_sfx("estus_drink");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = stats.player_scale;
            next_state.set(GameState::BlackHoleCutscene);
        }
        BartenderAction::TeleportEveryone => {
            crate::audio::net_event(crate::net::EVENT_CASINO_WARP_ALL, 1);
            stop_voice();
            play_sfx("estus_drink");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = stats.player_scale;
            next_state.set(GameState::PubPortalCutscene);
        }
        BartenderAction::GoAlone => {
            stop_voice();
            play_sfx("estus_drink");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = stats.player_scale;
            next_state.set(GameState::PubPortalCutscene);
        }
    }
}

fn spawn_casino_teleport_prompt(commands: &mut Commands, in_city_peers: u32, total_squad: u32) {
    let body_msg = if in_city_peers > 0 {
        format!(
            "{} other player(s) are still in Cedar Rapids. Bring everyone through the Pub 217 portal with you, or go alone?",
            in_city_peers
        )
    } else if total_squad > 1 {
        "Other squad players are online. Send the Casino teleport to anyone in Cedar Rapids, or go alone?".to_string()
    } else {
        "Ready for Tendy Palace Casino! Choose TELEPORT EVERYONE to pull any players in Cedar Rapids with you, or GO ALONE.".to_string()
    };
    commands
        .spawn((
            BartenderUiRoot,
            BartenderTeleportPrompt,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.62)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(500.0),
                    max_width: Val::Percent(92.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.04, 0.14, 0.96)),
                BorderColor(Color::srgb(0.95, 0.76, 0.25)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("TELEPORT EVERYONE TO TENDY PALACE CASINO?"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.35)),
                ));
                card.spawn((
                    Text::new(body_msg),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.92, 0.98)),
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        BartenderAction::Leave,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.24, 0.14, 0.22)),
                        BorderColor(Color::srgba(1.0, 0.82, 0.42, 0.6)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("CANCEL [ESC]"),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                    row.spawn((
                        BartenderAction::GoAlone,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.32, 0.56)),
                        BorderColor(Color::srgb(0.42, 0.78, 1.0)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("GO ALONE"),
                            TextFont { font_size: 13.5, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                    row.spawn((
                        BartenderAction::TeleportEveryone,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.76, 0.44, 0.08)),
                        BorderColor(Color::srgb(1.0, 0.88, 0.34)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("TELEPORT EVERYONE [SPACE]"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                });
            });
        });
}

fn cleanup_bartender_on_exit(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut prompt: ResMut<BartenderPrompt>,
    ui_q: Query<Entity, With<BartenderUiRoot>>,
) {
    stats.bartender_open = false;
    prompt.label = None;
    prompt.pressed = false;
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
}

// =================================================================================================
// Pub 217 10-Beers & Women's Bathroom Magic Portal Cutscene (`GameState::PubPortalCutscene`)
// =================================================================================================

const PORTAL_STAGE: Vec3 = Vec3::new(0.0, 0.0, -1080.0);
const PATRON_COUNT: usize = 6;
const PATRON_START_TIMES: [f32; PATRON_COUNT] = [0.5, 3.2, 6.4, 9.2, 12.0, 14.8];
const PATRON_WALK_SECS: f32 = 4.2;

#[derive(Component)]
struct PubPortalCutsceneEntity;

#[derive(Component)]
struct PubPortalCutsceneOverlay;

#[derive(Component)]
struct PubPortalSpeakerText;

#[derive(Component)]
struct PubPortalSubText;

#[derive(Component, Clone, Copy)]
enum PortalLightRole {
    PubWarm,
    WalkwayFill,
    PortalGlow,
}

#[derive(Component)]
struct PubPortalLight {
    role: PortalLightRole,
}

#[derive(Component)]
struct PatronLeg {
    patron_idx: usize,
    is_left: bool,
}

#[derive(Component, Clone, Copy)]
enum PubPortalAnim {
    /// 0=Stan (-Z model), 1=TJ (+Z), 2=Nate (+Z), 3=Glenn (+Z)
    Guy(usize),
    Chair(usize),
    BeerCan { idx: usize, base: Vec3 },
    DoorPivot,
    PortalCore,
    PortalRing(usize),
    /// Attractive women patrons walking into the Women's Bathroom (0..6)
    Patron(usize),
    /// Glowing flying poker chips whipped toward the neon portal (0..10)
    FlyingProp { idx: usize, base: Vec3 },
}

#[derive(Resource, Default)]
pub struct PubPortalCutsceneState {
    beat: u8,
    beat_t: f32,
    total_t: f32,
    slam_played: bool,
    beat0_spoken: bool,
}

/// (speaker, dialogue, voice_profile, min_seconds)
const PUB_PORTAL_BEATS: [(&str, &str, &str, f32); 6] = [
    (
        "STAN",
        "Ten ice-cold beers and a full beer tower! Alright boys, settle this right now: what is the greatest movie of all time?",
        "stan",
        4.4,
    ),
    (
        "TJ",
        "Easy, the 1990 Teenage Mutant Ninja Turtles movie! Practical rubber suits, Shredder on the rooftop, pure cinema!",
        "tj",
        4.8,
    ),
    (
        "NATE",
        "Facts! Teenage Mutant Ninja Turtles 2: Secret of the Ooze goes crazy too... wait, hold on guys. Why have like twenty people just walked into the women's bathroom?!",
        "nate",
        5.2,
    ),
    (
        "GLENN",
        "Bro, nobody is coming back out!! How are there so many people fitting in there?! Is there some kind of magic portal in the women's bathroom?!",
        "glenn",
        5.0,
    ),
    (
        "STAN",
        "Wait a second... let's check this out.",
        "stan",
        4.0,
    ),
    (
        "TJ, NATE, GLENN & STAN",
        "WHOA!! THE DOOR JUST BLEW OPEN!! IT'S A MAGIC PORTAL!! HOLD ON, AAAAAHHHH!!",
        "tj_upset",
        5.0,
    ),
];

const TABLE_CENTER: Vec3 = Vec3::new(0.0, 0.0, 0.6);
const DOOR_POS: Vec3 = Vec3::new(0.0, 0.0, -5.8);
const SEATED_SPOTS: [Vec3; 4] = [
    Vec3::new(-1.65, 0.0, 1.35),
    Vec3::new(-1.55, 0.0, -0.35),
    Vec3::new(1.55, 0.0, -0.35),
    Vec3::new(1.65, 0.0, 1.35),
];
const STAND_SPOTS: [Vec3; 4] = [
    Vec3::new(-1.85, 0.0, 1.20),
    Vec3::new(-1.35, 0.0, -0.15),
    Vec3::new(1.35, 0.0, -0.15),
    Vec3::new(1.85, 0.0, 1.20),
];

fn yaw_toward(from: Vec3, target: Vec3, model_neg_z: bool) -> f32 {
    let dx = target.x - from.x;
    let dz = target.z - from.z;
    if model_neg_z {
        (-dx).atan2(-dz)
    } else {
        dx.atan2(dz)
    }
}

/// Evaluates the curved catwalk path of attractive woman `pi` toward the Women's Bathroom door.
fn patron_walk_pose(pi: usize, total_t: f32) -> Option<(Vec3, f32, f32)> {
    let start_t = PATRON_START_TIMES[pi];
    let local_t = total_t - start_t;
    if local_t < 0.0 || local_t > PATRON_WALK_SECS {
        return None;
    }
    let u = (local_t / PATRON_WALK_SECS).clamp(0.0, 1.0);
    let side = if pi % 2 == 0 { -1.0_f32 } else { 1.0_f32 };
    let z_entry = -2.85 - (pi % 3) as f32 * 0.25;
    let p0 = Vec3::new(side * 5.2, 0.0, z_entry);
    let p1 = Vec3::new(side * 1.4, 0.0, -4.15);
    let p2 = DOOR_POS + Vec3::new(0.0, 0.0, -0.55);
    let eval = |s: f32| {
        let inv = 1.0 - s;
        p0 * (inv * inv) + p1 * (2.0 * inv * s) + p2 * (s * s)
    };
    let pos = eval(u);
    let ahead = eval((u + 0.02).min(1.0));
    let yaw = yaw_toward(pos, ahead, false);
    Some((pos, yaw, u))
}

/// Builds a sculpted, glamorous woman NPC (`+Z` forward) in one of 4 cocktail-dress styles with
/// articulated hip pivots (`PatronLeg`) for graceful walking strides into the Women's Bathroom.
#[allow(clippy::too_many_lines)]
fn spawn_attractive_woman_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    style_idx: usize,
    patron_idx: usize,
) {
    let (hair_col, hair_hi_col, dress_col, heel_col, eye_col, skin_col, metal_col) = match style_idx % 4 {
        0 => (
            // Platinum blonde waves + emerald cocktail dress
            Color::srgb(0.95, 0.84, 0.48),
            Color::srgb(1.0, 0.94, 0.72),
            Color::srgb(0.06, 0.60, 0.36),
            Color::srgb(0.92, 0.76, 0.22),
            Color::srgb(0.18, 0.56, 0.98),
            Color::srgb(0.97, 0.80, 0.70),
            Color::srgb(1.0, 0.82, 0.28),
        ),
        1 => (
            // Espresso brunette + crimson velvet mini dress
            Color::srgb(0.24, 0.12, 0.08),
            Color::srgb(0.44, 0.22, 0.14),
            Color::srgb(0.76, 0.07, 0.20),
            Color::srgb(0.08, 0.08, 0.10),
            Color::srgb(0.56, 0.40, 0.14),
            Color::srgb(0.94, 0.74, 0.62),
            Color::srgb(1.0, 0.82, 0.28),
        ),
        2 => (
            // Jet black glossy waves + magenta satin dress
            Color::srgb(0.06, 0.06, 0.09),
            Color::srgb(0.22, 0.24, 0.36),
            Color::srgb(0.84, 0.14, 0.56),
            Color::srgb(0.90, 0.92, 0.96),
            Color::srgb(0.32, 0.82, 1.0),
            Color::srgb(0.95, 0.76, 0.66),
            Color::srgb(0.90, 0.92, 0.98),
        ),
        _ => (
            // Copper-auburn curls + little black dress + red stilettos
            Color::srgb(0.62, 0.18, 0.10),
            Color::srgb(0.86, 0.34, 0.16),
            Color::srgb(0.08, 0.07, 0.11),
            Color::srgb(0.86, 0.08, 0.18),
            Color::srgb(0.14, 0.74, 0.42),
            Color::srgb(0.97, 0.81, 0.72),
            Color::srgb(0.98, 0.96, 0.94),
        ),
    };

    let skin = materials.add(StandardMaterial {
        base_color: skin_col,
        perceptual_roughness: 0.52,
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: hair_col,
        perceptual_roughness: 0.38,
        ..default()
    });
    let hair_hi = materials.add(StandardMaterial {
        base_color: hair_hi_col,
        perceptual_roughness: 0.30,
        ..default()
    });
    let dress = materials.add(StandardMaterial {
        base_color: dress_col,
        perceptual_roughness: 0.30,
        metallic: 0.14,
        ..default()
    });
    let heel = materials.add(StandardMaterial {
        base_color: heel_col,
        perceptual_roughness: 0.24,
        metallic: 0.28,
        ..default()
    });
    let iris = materials.add(StandardMaterial {
        base_color: eye_col,
        emissive: eye_col.to_linear() * 0.22,
        perceptual_roughness: 0.18,
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.98, 1.0),
        perceptual_roughness: 0.25,
        ..default()
    });
    let lash = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.04, 0.06),
        ..default()
    });
    let lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.10, 0.26),
        perceptual_roughness: 0.18,
        ..default()
    });
    let jewelry = materials.add(StandardMaterial {
        base_color: metal_col,
        metallic: 0.85,
        perceptual_roughness: 0.18,
        ..default()
    });
    let blush = materials.add(StandardMaterial {
        base_color: Color::srgba(0.98, 0.44, 0.54, 0.62),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    // Articulated slender legs under hip pivots (y = 0.72) for graceful catwalk strides
    let thigh_mesh = sh::add(meshes, sh::limb_profile(0.38, &[0.092, 0.080, 0.064], 1.0));
    let calf_mesh = sh::add(meshes, sh::limb_profile(0.38, &[0.062, 0.055, 0.038], 1.0));
    let shoe_mesh = sh::add(meshes, sh::shoe(0.11, 0.075, 0.21));
    let spike_mesh = meshes.add(Cylinder::new(0.013, 0.085));
    for (idx, sx) in [-0.10_f32, 0.10].iter().enumerate() {
        parent
            .spawn((
                PatronLeg {
                    patron_idx,
                    is_left: idx == 0,
                },
                Transform::from_xyz(*sx, 0.72, 0.0),
                Visibility::default(),
            ))
            .with_children(|leg| {
                leg.spawn((
                    Mesh3d(thigh_mesh.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, -0.17, 0.0),
                ));
                leg.spawn((
                    Mesh3d(calf_mesh.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, -0.49, 0.0),
                ));
                leg.spawn((
                    Mesh3d(shoe_mesh.clone()),
                    MeshMaterial3d(heel.clone()),
                    Transform::from_xyz(0.0, -0.67, 0.04),
                ));
                leg.spawn((
                    Mesh3d(spike_mesh.clone()),
                    MeshMaterial3d(jewelry.clone()),
                    Transform::from_xyz(0.0, -0.68, -0.045),
                ));
            });
    }

    // Hourglass fitted cocktail dress (skirt flare -> cinched waist -> sculpted bust -> collar)
    let dress_mesh = sh::add(
        meshes,
        sh::loft(
            &[
                sh::ring(0.69, 0.0, 0.0),
                sh::ring(0.70, 0.215, 0.165),
                sh::ring(0.86, 0.195, 0.152),
                sh::ring(1.06, 0.125, 0.098).dz(0.005),
                sh::ring(1.24, 0.188, 0.148).dz(0.028),
                sh::ring(1.37, 0.168, 0.110),
                sh::ring(1.42, 0.115, 0.082),
                sh::ring(1.43, 0.0, 0.0),
            ],
            2,
            22,
            1.0,
        ),
    );
    parent.spawn((
        Mesh3d(dress_mesh),
        MeshMaterial3d(dress),
        Transform::IDENTITY,
    ));

    // Neck & necklace
    parent.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.052, 0.16))),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.0, 1.48, 0.01),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Torus::new(0.011, 0.065))),
        MeshMaterial3d(jewelry.clone()),
        Transform::from_xyz(0.0, 1.42, 0.02).with_rotation(Quat::from_rotation_x(0.24)),
    ));

    // Slender arms + gleaming clutch purse in right hand
    let arm_mesh = sh::add(meshes, sh::limb_profile(0.45, &[0.048, 0.040, 0.032], 1.0));
    parent.spawn((
        Mesh3d(arm_mesh.clone()),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(-0.21, 1.19, 0.02).with_rotation(Quat::from_rotation_z(-0.18)),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::hand(0.075, -1.0, false))),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(-0.25, 0.95, 0.04),
    ));
    parent.spawn((
        Mesh3d(arm_mesh),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.21, 1.21, 0.05)
            .with_rotation(Quat::from_rotation_x(-0.28) * Quat::from_rotation_z(0.16)),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::plate(0.10, 0.14, 0.22, 0.3))),
        MeshMaterial3d(heel),
        Transform::from_xyz(0.27, 0.95, 0.12),
    ));

    // Sculpted face, sparkling eyes with lashes, rosy cheeks, glossy lips, earrings & flowing hair
    parent
        .spawn((Transform::from_xyz(0.0, 1.55, 0.02), Visibility::default()))
        .with_children(|h| {
            h.spawn((
                Mesh3d(sh::add(meshes, sh::head(0.180, 0.120))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 0.14, 0.0),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.030))),
                    MeshMaterial3d(blush.clone()),
                    Transform::from_xyz(sx * 0.092, 0.105, 0.142)
                        .with_scale(Vec3::new(1.3, 0.75, 0.45)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.028))),
                    MeshMaterial3d(eye_white.clone()),
                    Transform::from_xyz(sx * 0.065, 0.164, 0.158)
                        .with_scale(Vec3::new(1.15, 0.92, 0.48)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.020))),
                    MeshMaterial3d(iris.clone()),
                    Transform::from_xyz(sx * 0.065, 0.163, 0.170)
                        .with_scale(Vec3::new(1.0, 1.0, 0.42)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.010))),
                    MeshMaterial3d(lash.clone()),
                    Transform::from_xyz(sx * 0.065, 0.163, 0.176),
                ));
                h.spawn((
                    Mesh3d(sh::add(meshes, sh::plate(0.064, 0.013, 0.011, 0.3))),
                    MeshMaterial3d(lash.clone()),
                    Transform::from_xyz(sx * 0.065, 0.192, 0.164)
                        .with_rotation(Quat::from_rotation_z(-sx * 0.14)),
                ));
                h.spawn((
                    Mesh3d(meshes.add(Torus::new(0.007, 0.032))),
                    MeshMaterial3d(jewelry.clone()),
                    Transform::from_xyz(sx * 0.180, 0.08, 0.01)
                        .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
                ));
            }
            h.spawn((
                Mesh3d(meshes.add(Sphere::new(0.027))),
                MeshMaterial3d(lip),
                Transform::from_xyz(0.0, 0.068, 0.168).with_scale(Vec3::new(1.42, 0.45, 0.45)),
            ));
            // Hair crown + cascading shoulder-length waves
            h.spawn((
                Mesh3d(sh::add(meshes, sh::hair_cap(0.192, 0.130, 0.13, 0.25, 0.052, 0.044))),
                MeshMaterial3d(hair.clone()),
                Transform::from_xyz(0.0, 0.15, -0.01),
            ));
            h.spawn((
                Mesh3d(meshes.add(Sphere::new(0.205))),
                MeshMaterial3d(hair),
                Transform::from_xyz(0.0, 0.06, -0.10).with_scale(Vec3::new(1.08, 1.48, 0.90)),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                h.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.112))),
                    MeshMaterial3d(hair_hi.clone()),
                    Transform::from_xyz(sx * 0.155, 0.02, -0.01)
                        .with_scale(Vec3::new(0.75, 1.90, 0.85)),
                ));
            }
        });
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn enter_pub_portal_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    art: Option<Res<CasinoArt>>,
    mut p_state: ResMut<PubPortalCutsceneState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut clear_col: ResMut<ClearColor>,
) {
    *p_state = PubPortalCutsceneState::default();
    for mut vis in &mut hud_q {
        *vis = Visibility::Hidden;
    }

    clear_col.0 = Color::srgb(0.05, 0.03, 0.07);
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.88, 0.92),
        brightness: 460.0,
    });

    // Warm pub chandelier light + attractive-patron walkway fill + neon Women's Bathroom Portal light
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalLight { role: PortalLightRole::PubWarm },
        PointLight {
            color: Color::srgb(1.0, 0.84, 0.62),
            intensity: 240_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 4.8, 0.8)),
    ));
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalLight { role: PortalLightRole::WalkwayFill },
        PointLight {
            color: Color::srgb(1.0, 0.92, 0.88),
            intensity: 150_000.0,
            range: 18.0,
            ..default()
        },
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 4.4, -3.5)),
    ));
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalLight { role: PortalLightRole::PortalGlow },
        PointLight {
            color: Color::srgb(0.95, 0.35, 1.0),
            intensity: 0.0,
            range: 34.0,
            ..default()
        },
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 2.2, -5.2)),
    ));

    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let ring_mesh = meshes.add(Torus::new(0.16, 1.65));

    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.16, 0.10),
        perceptual_roughness: 0.72,
        ..default()
    });
    let brick_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.13, 0.10),
        perceptual_roughness: 0.78,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.11, 0.07),
        perceptual_roughness: 0.36,
        ..default()
    });
    let door_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.14, 0.09),
        perceptual_roughness: 0.40,
        ..default()
    });
    let gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.24),
        metallic: 0.72,
        perceptual_roughness: 0.24,
        ..default()
    });
    let can_blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.42, 0.92),
        metallic: 0.55,
        perceptual_roughness: 0.28,
        ..default()
    });
    let foam_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.97, 0.92),
        perceptual_roughness: 0.7,
        ..default()
    });
    let neon_pink = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.34, 0.86),
        emissive: LinearRgba::rgb(2.8, 0.75, 2.2),
        unlit: true,
        ..default()
    });
    let neon_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.94, 1.0),
        emissive: LinearRgba::rgb(0.65, 2.4, 3.0),
        unlit: true,
        ..default()
    });
    let portal_core = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.42, 1.0, 0.88),
        emissive: LinearRgba::rgb(2.8, 1.1, 3.8),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // Floor, side walls, dark ceiling
    commands.spawn((
        PubPortalCutsceneEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, -0.2, 0.0))
            .with_scale(Vec3::new(20.0, 0.4, 18.0)),
    ));
    for (wx, wy, wz, sx, sy, sz) in [
        (-9.5_f32, 3.2_f32, 0.0_f32, 0.6_f32, 6.4_f32, 18.0_f32),
        (9.5, 3.2, 0.0, 0.6, 6.4, 18.0),
        (0.0, 6.5, 0.0, 20.0, 0.4, 18.0),
        // North wall split around Women's Bathroom doorway (opening: x -1.0..1.0, y 0..2.8)
        (-5.25, 3.2, -5.8, 8.5, 6.4, 0.5),
        (5.25, 3.2, -5.8, 8.5, 6.4, 0.5),
        (0.0, 4.65, -5.8, 2.2, 3.7, 0.5),
    ] {
        commands.spawn((
            PubPortalCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick_mat.clone()),
            Transform::from_translation(PORTAL_STAGE + Vec3::new(wx, wy, wz))
                .with_scale(Vec3::new(sx, sy, sz)),
        ));
    }

    // Glowing WOMEN'S Bathroom plaque mounted right above the North doorway
    if let Some(ref art) = art {
        let sign_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.womens_sign.clone()),
            emissive: LinearRgba::rgb(1.4, 1.1, 1.35),
            emissive_texture: Some(art.womens_sign.clone()),
            unlit: true,
            ..default()
        });
        commands.spawn((
            PubPortalCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(sign_mat),
            Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 3.22, -5.50))
                .with_scale(Vec3::new(1.65, 0.80, 0.08)),
        ));
    }

    // Hinged Women's Bathroom Door (pivoted at x = -0.95 so positive yaw flings violently into room!)
    commands
        .spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::DoorPivot,
            Transform::from_translation(PORTAL_STAGE + DOOR_POS + Vec3::new(-0.95, 0.0, 0.0)),
            Visibility::default(),
        ))
        .with_children(|d| {
            d.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(door_mat),
                Transform::from_xyz(0.95, 1.40, 0.0).with_scale(Vec3::new(1.90, 2.80, 0.14)),
            ));
            d.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(gold_mat.clone()),
                Transform::from_xyz(1.70, 1.25, 0.12).with_scale(Vec3::splat(0.11)),
            ));
        });

    // Interdimensional Casino Portal Vortex behind the Women's Bathroom door
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::PortalCore,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(portal_core),
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 1.65, -6.2))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(0.001)),
    ));
    for (idx, mat) in [neon_pink.clone(), neon_cyan, gold_mat.clone()]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::PortalRing(idx),
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 1.65, -6.05 + idx as f32 * 0.08))
                .with_scale(Vec3::splat(0.001)),
        ));
    }

    // Background Pub Bar & Maria watching warmly from left wall
    commands.spawn((
        PubPortalCutsceneEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(PORTAL_STAGE + Vec3::new(-6.0, 0.62, -2.5))
            .with_scale(Vec3::new(1.1, 1.24, 5.2)),
    ));
    commands
        .spawn((
            PubPortalCutsceneEntity,
            Transform::from_translation(PORTAL_STAGE + Vec3::new(-7.4, 0.0, -2.5))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                .with_scale(Vec3::splat(1.12)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_bartender_rig(p, &mut meshes, &mut materials);
        });

    // Center Table + Beer Tower Pyramid + 10 Ice-Cold Beers!
    commands
        .spawn((
            PubPortalCutsceneEntity,
            Transform::from_translation(PORTAL_STAGE + TABLE_CENTER),
            Visibility::default(),
        ))
        .with_children(|tbl| {
            tbl.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.45, 0.0).with_scale(Vec3::new(0.32, 0.90, 0.32)),
            ));
            tbl.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.92, 0.0).with_scale(Vec3::new(1.32, 0.12, 1.32)),
            ));
            crate::beer_fx::spawn_pub_beer_tower(tbl, &mut meshes, &mut materials, Vec3::new(0.0, 0.98, 0.0));
        });

    for idx in 0..10 {
        let a = (idx as f32 / 10.0) * TAU + 0.18;
        let base = TABLE_CENTER + Vec3::new(a.cos() * 0.94, 1.07, a.sin() * 0.94);
        commands
            .spawn((
                PubPortalCutsceneEntity,
                PubPortalAnim::BeerCan { idx, base },
                Transform::from_translation(PORTAL_STAGE + base),
                Visibility::default(),
            ))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(can_blue.clone()),
                    Transform::IDENTITY.with_scale(Vec3::new(0.055, 0.19, 0.055)),
                ));
                c.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(foam_mat.clone()),
                    Transform::from_xyz(0.0, 0.105, 0.0).with_scale(Vec3::new(0.058, 0.035, 0.058)),
                ));
            });
    }

    // 4 Pub Chairs around the table
    for (idx, pos) in SEATED_SPOTS.iter().enumerate() {
        let cyaw = yaw_toward(*pos, TABLE_CENTER, false);
        commands
            .spawn((
                PubPortalCutsceneEntity,
                PubPortalAnim::Chair(idx),
                Transform::from_translation(PORTAL_STAGE + *pos)
                    .with_rotation(Quat::from_rotation_y(cyaw)),
                Visibility::default(),
            ))
            .with_children(|ch| {
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.50, 0.0).with_scale(Vec3::new(0.56, 0.08, 0.54)),
                ));
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.90, -0.24).with_scale(Vec3::new(0.54, 0.76, 0.08)),
                ));
                for (lx, lz) in [(-0.22, -0.22), (0.22, -0.22), (-0.22, 0.22), (0.22, 0.22)] {
                    ch.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(wood_mat.clone()),
                        Transform::from_xyz(lx, 0.25, lz).with_scale(Vec3::new(0.07, 0.50, 0.07)),
                    ));
                }
            });
    }

    // All 4 Guys around the table: 0 Stan (-Z GLB), 1 TJ (+Z), 2 Nate (+Z), 3 Glenn (+Z)
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::Guy(0),
        SceneRoot(hero_scene),
        Transform::from_translation(PORTAL_STAGE + SEATED_SPOTS[0] + Vec3::Y * -0.28)
            .with_rotation(Quat::from_rotation_y(yaw_toward(SEATED_SPOTS[0], TABLE_CENTER, true)))
            .with_scale(Vec3::splat(1.18)),
        Visibility::Inherited,
    ));

    commands
        .spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::Guy(1),
            Transform::from_translation(PORTAL_STAGE + SEATED_SPOTS[1] + Vec3::Y * -0.28)
                .with_rotation(Quat::from_rotation_y(yaw_toward(SEATED_SPOTS[1], TABLE_CENTER, false)))
                .with_scale(Vec3::splat(0.95)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::tj::spawn_tj_rig(p, &mut meshes, &mut materials, false, false);
        });

    let nate_mats = crate::nate::make_nate_materials(&mut materials);
    commands
        .spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::Guy(2),
            Transform::from_translation(PORTAL_STAGE + SEATED_SPOTS[2] + Vec3::Y * -0.28)
                .with_rotation(Quat::from_rotation_y(yaw_toward(SEATED_SPOTS[2], TABLE_CENTER, false)))
                .with_scale(Vec3::splat(0.95)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::nate::spawn_nate_rig(p, &mut meshes, &nate_mats, false);
        });

    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    commands
        .spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::Guy(3),
            Transform::from_translation(PORTAL_STAGE + SEATED_SPOTS[3] + Vec3::Y * -0.28)
                .with_rotation(Quat::from_rotation_y(yaw_toward(SEATED_SPOTS[3], TABLE_CENTER, false)))
                .with_scale(Vec3::splat(0.90)),
            Visibility::default(),
        ))
        .with_children(|p| {
            crate::glenn::spawn_glenn_rig(p, &mut meshes, &glenn_mats);
        });

    // Staggered parade of extremely attractive women walking gracefully into the Women's Bathroom
    for idx in 0..PATRON_COUNT {
        commands
            .spawn((
                PubPortalCutsceneEntity,
                PubPortalAnim::Patron(idx),
                Transform::from_translation(PORTAL_STAGE + Vec3::new(-5.2, 0.0, -3.2)),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                spawn_attractive_woman_rig(p, &mut meshes, &mut materials, idx, idx);
            });
    }

    // Glowing flying poker chips / sparks whipped out when the portal erupts
    for idx in 0..10 {
        let base = Vec3::new(((idx as f32) - 4.5) * 0.85, 0.6 + (idx % 3) as f32 * 0.5, -2.5);
        commands.spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::FlyingProp { idx, base },
            Mesh3d(unit_cyl.clone()),
            MeshMaterial3d(if idx % 2 == 0 { gold_mat.clone() } else { neon_pink.clone() }),
            Transform::from_translation(PORTAL_STAGE + base).with_scale(Vec3::new(0.22, 0.05, 0.22)),
            Visibility::Hidden,
        ));
    }

    // Initial Beat 0 camera establishing shot
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = PORTAL_STAGE + Vec3::new(0.0, 3.0, 6.2);
        c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.3, 0.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.68;
        }
    }

    // Widescreen letterbox & dialogue subtitles UI
    let (spk, line, _, _) = PUB_PORTAL_BEATS[0];
    commands
        .spawn((
            PubPortalCutsceneOverlay,
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
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(7.5),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
            ));

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::bottom(Val::Px(22.0)),
                    ..default()
                },
            ))
            .with_children(|bot| {
                bot.spawn((
                    Node {
                        width: Val::Percent(88.0),
                        max_width: Val::Px(820.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(14.0)),
                        row_gap: Val::Px(5.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.04, 0.09, 0.90)),
                    BorderColor(Color::srgb(0.98, 0.78, 0.28)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|card| {
                    card.spawn((
                        PubPortalSpeakerText,
                        Text::new(spk),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.84, 0.30)),
                    ));
                    card.spawn((
                        PubPortalSubText,
                        Text::new(format!("\"{}\"", line)),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });

    play_cutscene_music();
}

#[allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines)]
fn update_pub_portal_cutscene(
    time: Res<Time>,
    mut p_state: ResMut<PubPortalCutsceneState>,
    mut next_state: SceneTransition,
    mut anim_q: Query<(&mut Transform, &mut Visibility, &PubPortalAnim), (Without<Camera3d>, Without<PatronLeg>)>,
    mut leg_q: Query<(&PatronLeg, &mut Transform), (Without<PubPortalAnim>, Without<Camera3d>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<PubPortalAnim>, Without<PatronLeg>)>,
    mut light_q: Query<(&mut PointLight, &PubPortalLight)>,
    mut spk_q: Query<&mut Text, (With<PubPortalSpeakerText>, Without<PubPortalSubText>)>,
    mut sub_q: Query<&mut Text, (With<PubPortalSubText>, Without<PubPortalSpeakerText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    p_state.beat_t += dt;
    p_state.total_t += dt;

    if !p_state.beat0_spoken && p_state.total_t >= 0.12 {
        p_state.beat0_spoken = true;
        let (_, line, voice, _) = PUB_PORTAL_BEATS[0];
        speak_voice(line, voice);
    }

    let skipping = crate::skip::skipping();
    if skipping {
        p_state.beat = (PUB_PORTAL_BEATS.len() as u8).saturating_sub(1);
        p_state.beat_t = p_state.beat_t.max(5.5);
    }

    let idx = (p_state.beat as usize).min(PUB_PORTAL_BEATS.len() - 1);
    let min_t = PUB_PORTAL_BEATS[idx].3;
    if p_state.beat_t >= min_t && (skipping || !is_voice_speaking() || p_state.beat_t >= 14.0) {
        if p_state.beat + 1 >= PUB_PORTAL_BEATS.len() as u8 {
            next_state.set(GameState::CasinoPlay);
            return;
        }
        p_state.beat += 1;
        p_state.beat_t = 0.0;

        let (spk, line, voice, _) = PUB_PORTAL_BEATS[p_state.beat as usize];
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
    }

    let beat = p_state.beat;
    let bt = p_state.beat_t;
    let t = p_state.total_t;

    if beat == 5 && !p_state.slam_played {
        p_state.slam_played = true;
        play_sfx("door_slam");
        play_sfx("portal_suck");
    }

    // Stand-up factor during Beat 4 (0 -> 1 over first 1.2s), full stand in Beat 5 before suction
    let stand_u = if beat < 4 {
        0.0
    } else if beat == 4 {
        (bt / 1.2).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let stand_smooth = stand_u * stand_u * (3.0 - 2.0 * stand_u);

    // Suction factor in Beat 5 (door blows open in 0..0.3s, guys & props yanked in 0.35..4.6s)
    let suck_u = if beat == 5 {
        ((bt - 0.35) / 4.25).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Door angle: swings open smoothly as any attractive woman enters in Beats 0..=3, rattles in Beat 4, slams open in Beat 5
    let mut min_patron_door_d = f32::MAX;
    if beat < 4 {
        for pi in 0..PATRON_COUNT {
            if let Some((pos, _, _)) = patron_walk_pose(pi, t) {
                let d = Vec2::new(pos.x - DOOR_POS.x, pos.z - DOOR_POS.z).length();
                min_patron_door_d = min_patron_door_d.min(d);
            }
        }
    }
    let door_slam = if beat == 5 {
        let k = (bt / 0.28).clamp(0.0, 1.0);
        2.15 * (1.0 - (1.0 - k).powi(3)) + ((t * 28.0).sin() * 0.06 * (1.0 - k))
    } else if beat == 4 {
        (t * 42.0).sin() * 0.05
    } else {
        let proximity = (1.0 - (min_patron_door_d / 1.45).clamp(0.0, 1.0)).powf(0.8);
        0.04 + proximity * 0.92
    };

    for (mut pl, lcomp) in &mut light_q {
        match lcomp.role {
            PortalLightRole::PubWarm => {
                pl.intensity = if beat == 5 { 90_000.0 } else { 240_000.0 };
            }
            PortalLightRole::WalkwayFill => {
                pl.intensity = if beat == 5 { 60_000.0 } else { 150_000.0 };
            }
            PortalLightRole::PortalGlow => {
                pl.intensity = match beat {
                    0..=2 => 4_000.0,
                    3 => 35_000.0 + (t * 8.0).sin() * 12_000.0,
                    4 => 110_000.0 + (t * 14.0).sin() * 35_000.0,
                    _ => 650_000.0 + (t * 20.0).sin() * 140_000.0,
                };
            }
        }
    }

    for (leg, mut tf) in &mut leg_q {
        let phase = t * 7.4 + leg.patron_idx as f32 * 0.9;
        let swing = phase.sin() * 0.42 * if leg.is_left { 1.0 } else { -1.0 };
        tf.rotation = Quat::from_rotation_x(swing);
    }

    let portal_target = PORTAL_STAGE + Vec3::new(0.0, 1.65, -6.05);
    for (mut tf, mut vis, anim) in &mut anim_q {
        match *anim {
            PubPortalAnim::DoorPivot => {
                tf.rotation = Quat::from_rotation_y(door_slam);
            }
            PubPortalAnim::PortalCore => {
                let s = match beat {
                    0..=2 => 0.001,
                    3 => 0.35 + (t * 6.0).sin() * 0.06,
                    4 => 0.90 + (t * 9.0).sin() * 0.10,
                    _ => 1.75 + (t * 12.0).sin() * 0.14,
                };
                tf.scale = Vec3::new(s, 0.16, s * 1.35);
                tf.rotate_y(4.2 * dt);
            }
            PubPortalAnim::PortalRing(ri) => {
                let s = match beat {
                    0..=3 => 0.001,
                    4 => 0.45 + (ri as f32) * 0.14,
                    _ => (1.05 + (ri as f32) * 0.24) * (1.0 + (t * 9.0 + ri as f32).sin() * 0.08),
                };
                tf.scale = Vec3::splat(s);
                let dir = if ri % 2 == 0 { 1.0 } else { -1.0 };
                tf.rotate_z(dir * (3.2 + ri as f32 * 1.4) * dt);
            }
            PubPortalAnim::Chair(ci) => {
                let base = SEATED_SPOTS[ci];
                let push_back = if stand_smooth > 0.0 {
                    let away = (base - TABLE_CENTER).normalize_or_zero();
                    away * (stand_smooth * 0.45)
                } else {
                    Vec3::ZERO
                };
                if suck_u > 0.0 {
                    let pull = (suck_u * 0.72).powf(1.3);
                    let pos = (PORTAL_STAGE + base + push_back).lerp(portal_target, pull);
                    tf.translation = pos + Vec3::Y * (pull * 1.2 + (t * 7.0 + ci as f32).sin() * 0.25);
                    tf.rotation = Quat::from_euler(
                        EulerRot::YXZ,
                        t * 4.5 + ci as f32,
                        t * 3.2,
                        t * 2.8,
                    );
                } else {
                    tf.translation = PORTAL_STAGE + base + push_back;
                    tf.rotation = Quat::from_rotation_y(yaw_toward(base, TABLE_CENTER, false));
                }
            }
            PubPortalAnim::BeerCan { idx, base } => {
                if suck_u > 0.0 {
                    let phase = (suck_u * 1.4 - idx as f32 * 0.04).clamp(0.0, 1.0);
                    let swirl = t * 6.0 + idx as f32 * 0.7;
                    let orbit_r = (1.0 - phase) * 0.55;
                    let p = (PORTAL_STAGE + base).lerp(portal_target, phase.powf(1.2));
                    tf.translation = p + Vec3::new(swirl.cos() * orbit_r, (phase * PI).sin() * 1.1, swirl.sin() * orbit_r);
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, swirl, swirl * 1.3, swirl * 0.8);
                } else {
                    tf.translation = PORTAL_STAGE + base;
                    tf.rotation = Quat::IDENTITY;
                }
            }
            PubPortalAnim::FlyingProp { idx, base } => {
                if beat == 5 {
                    *vis = Visibility::Inherited;
                    let phase = ((t * 1.4 + idx as f32 * 0.19) % 1.0).clamp(0.0, 1.0);
                    let p = (PORTAL_STAGE + base).lerp(portal_target, phase);
                    let ang = t * 9.0 + idx as f32 * 0.9;
                    tf.translation = p + Vec3::new(ang.cos() * (1.0 - phase) * 1.2, ang.sin() * 0.4, 0.0);
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, ang, ang * 1.5, ang * 0.7);
                } else {
                    *vis = Visibility::Hidden;
                }
            }
            PubPortalAnim::Patron(pi) => {
                if beat < 4 {
                    if let Some((pos, yaw, u)) = patron_walk_pose(pi, t) {
                        *vis = if u > 0.94 { Visibility::Hidden } else { Visibility::Inherited };
                        tf.translation = PORTAL_STAGE + pos;
                        let hip_sway = (t * 7.4 + pi as f32 * 0.9).sin() * 0.045;
                        tf.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(hip_sway);
                    } else {
                        *vis = Visibility::Hidden;
                    }
                } else {
                    *vis = Visibility::Hidden;
                }
            }
            PubPortalAnim::Guy(gi) => {
                let model_neg_z = gi == 0;
                let base_scale = match gi {
                    0 => 1.18,
                    1 | 2 => 0.95,
                    _ => 0.90,
                };
                if beat == 5 && suck_u > 0.0 {
                    // Violent portal suction: spin and spiral straight into the neon vortex!
                    let delay = (gi as f32) * 0.06;
                    let p = ((suck_u - delay) / (1.0 - delay)).clamp(0.0, 1.0);
                    let start = PORTAL_STAGE + STAND_SPOTS[gi];
                    let curve = p.powf(1.45);
                    let spiral_r = (1.0 - curve) * 0.85 * (curve * 4.5).min(1.0);
                    let ang = t * 8.5 + gi as f32 * FRAC_PI_2;
                    tf.translation = start.lerp(portal_target, curve)
                        + Vec3::new(ang.cos() * spiral_r, (p * PI).sin() * 0.85 + curve * 0.5, ang.sin() * spiral_r);
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, ang * 1.4, curve * 1.1, (t * 6.0).sin() * 0.45);
                    tf.scale = Vec3::splat(base_scale * (1.0 - curve * 0.88).max(0.08));
                } else if beat >= 4 {
                    let seat_p = SEATED_SPOTS[gi] + Vec3::Y * -0.28;
                    let stand_p = STAND_SPOTS[gi];
                    let pos = seat_p.lerp(stand_p, stand_smooth);
                    let y_table = yaw_toward(pos, TABLE_CENTER, model_neg_z);
                    let y_door = yaw_toward(pos, DOOR_POS, model_neg_z);
                    let mut diff = (y_door - y_table + PI).rem_euclid(TAU) - PI;
                    diff *= stand_smooth;
                    let shiver = if beat == 5 { (t * 36.0 + gi as f32).sin() * 0.05 } else { 0.0 };
                    // Subtle peer-around-the-beer-tower lean (left pair leans left, right pair leans right)
                    let peer_side = if gi < 2 { -1.0 } else { 1.0 };
                    let peer_lean = if beat == 4 { 0.10 * stand_smooth * peer_side } else { 0.0 };
                    tf.translation = PORTAL_STAGE + pos + Vec3::new(shiver, 0.0, 0.0);
                    tf.rotation = Quat::from_rotation_y(y_table + diff) * Quat::from_rotation_z(peer_lean);
                    tf.scale = Vec3::splat(base_scale);
                } else {
                    let bob = (t * 2.4 + gi as f32 * 1.3).sin() * 0.025;
                    let seat_p = SEATED_SPOTS[gi] + Vec3::new(0.0, -0.28 + bob, 0.0);
                    let y_table = yaw_toward(SEATED_SPOTS[gi], TABLE_CENTER, model_neg_z);
                    let turn = if beat == 2 && gi == 2 {
                        -0.65
                    } else if beat == 3 {
                        let peer_dir = if gi < 2 { -0.55 } else { 0.55 };
                        peer_dir + (t * 2.6 + gi as f32).sin() * 0.16
                    } else {
                        0.0
                    };
                    tf.translation = PORTAL_STAGE + seat_p;
                    tf.rotation = Quat::from_rotation_y(y_table + turn);
                    tf.scale = Vec3::splat(base_scale);
                }
            }
        }
    }

    // Dynamic cinematic camera per Beat: keeps focus on the guys at the table with girls casually
    // passing in the background, then frames all 4 peering around the beer tower toward the door.
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        match beat {
            0 => {
                // Close warm orbit around the 4 guys sitting at the 10-beer table + tower
                let a = bt * 0.18 - 0.30;
                c_tf.translation = PORTAL_STAGE + Vec3::new(a.sin() * 4.3, 2.35, 0.6 + a.cos() * 4.3);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.28, 0.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62;
                }
            }
            1 => {
                // Tighter table shot on TJ & the crew debating TMNT while girls pass casually behind
                let k = (bt / 4.8).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(2.75 - k * 0.45, 2.05, 2.75);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(-0.55, 1.28, -0.15), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.56;
                }
            }
            2 => {
                // Focus on Nate at the table glancing back as girls casually enter the Women's room
                let k = (bt / 5.2).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(-2.85, 2.10, 2.65);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.55, 1.32, -0.45 - k * 1.4), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.58;
                }
            }
            3 => {
                // Elevated 3/4 table view: all 4 guys turning around the tower toward the glowing door
                c_tf.translation = PORTAL_STAGE + Vec3::new(2.85, 2.25, 3.15);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(-0.25, 1.38, -1.35), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62;
                }
            }
            4 => {
                // Boys stand & peer around both sides of the beer tower to investigate the door
                let k = (bt / 4.0).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(-1.35 + k * 0.45, 2.15 - k * 0.15, 3.85 - k * 0.85);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.52, -3.2 - k * 1.6), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62 - k * 0.04;
                }
            }
            _ => {
                // Door slams open & camera sweeps around the beer tower straight into the portal!
                let pull = (suck_u * 1.1).clamp(0.0, 1.0);
                let shake = Vec3::new((t * 53.0).sin() * 0.08, (t * 67.0).cos() * 0.08, 0.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(-0.85 * (1.0 - pull), 2.00 - pull * 0.10, 2.70 - pull * 4.30) + shake;
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.65, -5.8), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.68 + pull * 0.28;
                }
            }
        }
    }
}

fn exit_pub_portal_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<PubPortalCutsceneEntity>>,
    ov_q: Query<Entity, With<PubPortalCutsceneOverlay>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    stop_voice();
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    for mut vis in &mut hud_q {
        *vis = Visibility::Inherited;
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
    // Keeps soundscape snappy if returning cleanly after skip or jump.
    let _ = play_viking_chant_music;
}
