//! Pub 217 attractive bartender (Scarlett), "ORDER 10 BEERS" prompt, and the 10-beers table
//! & Women's Bathroom Magic Portal cutscene (`GameState::PubPortalCutscene`) that warps Stan,
//! TJ, Nate, and Glenn into Level 3 (`GameState::CasinoPlay`).
//!
//! Why: gives the Casino a hilarious, memorable entrance straight out of Pub 217 while keeping
//! the main city skyline uncluttered and giving `CasinoPlay` a dedicated full-room stage.
//!
//! Gameplay impact: talking to Scarlett behind the bar in Pub 217 (during CityPlay or Free Roam)
//! and pressing "ORDER 10 BEERS" plays a voiced, skippable (`crate::skip`) cutscene where the 4
//! guys drink 10 beers, debate TMNT movies, notice crowds vanishing into the women's restroom,
//! and get pulled through a portal straight into Tendy Palace Casino (without affecting story gates).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
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
            .init_resource::<EventHorizonFluidMesh>()
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
// Sculpted 3D Bartender Rig (Scarlett) + Women's Bathroom Door Prop in Pub 217
// =================================================================================================

/// Shared procedural 3D bartender rig (`+Z` local forward).
fn spawn_bartender_rig(
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

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BartenderAction {
    OrderTenBeers,
    Leave,
}

fn bartender_bar_trigger_pos() -> Vec3 {
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

    // Scarlett standing behind the Pub 217 bar facing East (+X) toward customers
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

    // Warm rose-gold bar light illuminating Scarlett
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
    if !stats.in_city || stats.bartender_open || stats.phone_open || stats.riding_wheel || help.open {
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
        prompt.label = Some("TALK TO BARTENDER [F/E]".to_string());
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
            spawn_bartender_ui(&mut commands, &art);
            return;
        }
    }
    prompt.pressed = false;
}

fn spawn_bartender_ui(commands: &mut Commands, art: &CasinoArt) {
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
                            Text::new("SCARLETT - PUB 217 BARTENDER"),
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
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn handle_bartender_modal_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    btn_q: Query<(&Interaction, &BartenderAction), Changed<Interaction>>,
    ui_q: Query<Entity, With<BartenderUiRoot>>,
) {
    if !stats.bartender_open {
        return;
    }

    let mut action_choice = None;
    if keys.just_pressed(KeyCode::Escape) {
        action_choice = Some(BartenderAction::Leave);
    } else if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
        action_choice = Some(BartenderAction::OrderTenBeers);
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
        BartenderAction::OrderTenBeers => {
            stop_voice();
            play_sfx("estus_drink");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = stats.player_scale;
            next_state.set(GameState::PubPortalCutscene);
        }
    }
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
const EVENT_HORIZON_CENTER: Vec3 = Vec3::new(0.0, 1.65, -6.45);
const PATRON_COUNT: usize = 6;
const PATRON_START_TIMES: [f32; PATRON_COUNT] = [0.1, 2.2, 4.6, 7.0, 9.6, 12.2];
const PATRON_WALK_SECS: f32 = 3.8;

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
    CerenkovBlue,
    HellCrimson,
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

#[derive(Resource, Default)]
struct EventHorizonFluidMesh {
    handle: Option<Handle<Mesh>>,
    base_pos: Vec<Vec3>,
}

#[derive(Component, Clone, Copy)]
enum PubPortalAnim {
    /// 0=Stan (-Z model), 1=TJ (+Z), 2=Nate (+Z), 3=Glenn (+Z)
    Guy(usize),
    Chair(usize),
    BeerCan { idx: usize, base: Vec3 },
    DoorPivot,
    /// Rippling dark liquid-mercury Event Horizon surface disc
    HorizonFluid,
    /// Pitch-black Event Horizon singularity sphere & blue/crimson lensing rings
    HorizonCore { ring: bool },
    /// Spiked metallic Event Horizon Gravity Drive gimbal rings (0..3)
    DriveGimbal(usize),
    /// Crackling electric Cerenkov arcs around the doorway & floor (0..8)
    LightningArc(usize),
    /// Attractive women patrons walking into the Women's Bathroom (0..6)
    Patron(usize),
    /// Dark metallic shards & plasma motes sucked into the singularity (0..12)
    FlyingProp { idx: usize, base: Vec3 },
}

#[derive(Resource, Default)]
pub struct PubPortalCutsceneState {
    beat: u8,
    beat_t: f32,
    total_t: f32,
    slam_played: bool,
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
        "Easy -- the 1990 Teenage Mutant Ninja Turtles movie! Practical rubber suits, Shredder on the rooftop, pure cinema!",
        "tj",
        4.8,
    ),
    (
        "NATE",
        "Facts! Teenage Mutant Ninja Turtles 2: Secret of the Ooze goes crazy too -- wait... hold on guys. Why have like twenty people just walked into the women's bathroom?!",
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
        "WHOA!! THE DOOR JUST BLEW OPEN!! IT'S A MAGIC PORTAL!! HOLD ON -- AAAAAHHHH!!",
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
/// Positioned strictly north of `TABLE_CENTER` (north rim is `z = -0.72`) so when the guys stand
/// and look at the Women's Bathroom door, the camera sits north of the beer tower with zero occlusion.
const STAND_SPOTS: [Vec3; 4] = [
    Vec3::new(-2.35, 0.0, -1.25),
    Vec3::new(-1.25, 0.0, -1.65),
    Vec3::new(1.25, 0.0, -1.65),
    Vec3::new(2.35, 0.0, -1.25),
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

/// Builds a deform-ready polar liquid-mercury disc (`MAIN_WORLD | RENDER_WORLD`) facing `+Z`.
fn build_fluid_horizon_disc(radius: f32) -> (Mesh, Vec<Vec3>) {
    const RINGS: usize = 20;
    const SEGS: usize = 32;
    let mut base_pos = Vec::with_capacity(1 + RINGS * SEGS);
    let mut uvs = Vec::with_capacity(1 + RINGS * SEGS);
    base_pos.push(Vec3::ZERO);
    uvs.push([0.5, 0.5]);
    for r in 1..=RINGS {
        let rad = radius * (r as f32 / RINGS as f32);
        for s in 0..SEGS {
            let a = (s as f32 / SEGS as f32) * TAU;
            let x = a.cos() * rad;
            let y = a.sin() * rad;
            base_pos.push(Vec3::new(x, y, 0.0));
            uvs.push([0.5 + x / (2.0 * radius), 0.5 - y / (2.0 * radius)]);
        }
    }
    let idx_at = |r: usize, s: usize| -> u32 { (1 + r * SEGS + (s % SEGS)) as u32 };
    let mut indices = Vec::with_capacity(SEGS * 3 + (RINGS - 1) * SEGS * 6);
    for s in 0..SEGS {
        let sn = (s + 1) % SEGS;
        indices.extend_from_slice(&[0, idx_at(0, s), idx_at(0, sn)]);
    }
    for r in 0..(RINGS - 1) {
        for s in 0..SEGS {
            let sn = (s + 1) % SEGS;
            let a = idx_at(r, s);
            let b = idx_at(r + 1, s);
            let c = idx_at(r + 1, sn);
            let d = idx_at(r, sn);
            indices.extend_from_slice(&[a, b, d, b, c, d]);
        }
    }
    let pos_arr: Vec<[f32; 3]> = base_pos.iter().map(|v| v.to_array()).collect();
    let norm_arr = vec![[0.0_f32, 0.0, 1.0]; base_pos.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos_arr);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm_arr);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, base_pos)
}

/// Builds a jagged crackling plasma lightning bolt chaining 6 `sh::rod` segments.
fn build_lightning_bolt_geo(seed: u32, len: f32) -> sh::Geo {
    let mut g = sh::Geo::default();
    let mut prev = Vec3::ZERO;
    let mut h = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    for seg in 1..=6 {
        let frac = seg as f32 / 6.0;
        h = h.wrapping_mul(277_803_737).wrapping_add(1);
        let jx = ((((h >> 8) & 0xFF) as f32) / 127.5 - 1.0) * 0.16;
        let jy = ((((h >> 16) & 0xFF) as f32) / 127.5 - 1.0) * 0.14;
        let taper = if seg == 6 { 0.0 } else { 1.0 };
        let next = Vec3::new(jx * taper, jy * taper, frac * len);
        g = g.merged(sh::rod(prev, next, 0.022));
        prev = next;
    }
    g
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
    mut fluid: ResMut<EventHorizonFluidMesh>,
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

    clear_col.0 = Color::srgb(0.03, 0.02, 0.05);
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.90, 0.96),
        brightness: 440.0,
    });

    // Pub lights + Event Horizon Gravity Drive Cerenkov Blue & Hell Crimson core lights
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
        PubPortalLight { role: PortalLightRole::CerenkovBlue },
        PointLight {
            color: Color::srgb(0.42, 0.78, 1.0),
            intensity: 0.0,
            range: 36.0,
            ..default()
        },
        Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 2.1, -5.0)),
    ));
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalLight { role: PortalLightRole::HellCrimson },
        PointLight {
            color: Color::srgb(1.0, 0.14, 0.05),
            intensity: 0.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER + Vec3::Z * 0.6),
    ));

    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));

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
    let void_black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        ..default()
    });
    let liquid_mercury = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.022, 0.045),
        metallic: 0.96,
        perceptual_roughness: 0.06,
        ..default()
    });
    let drive_brass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.36, 0.22),
        metallic: 0.88,
        perceptual_roughness: 0.24,
        ..default()
    });
    let drive_steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.25, 0.31),
        metallic: 0.90,
        perceptual_roughness: 0.22,
        ..default()
    });
    let cerenkov_blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.54, 0.90, 1.0),
        emissive: LinearRgba::rgb(1.4, 3.0, 6.0),
        unlit: true,
        ..default()
    });
    let hell_crimson = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.22, 0.08),
        emissive: LinearRgba::rgb(3.8, 0.55, 0.16),
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

    // Dark cosmic void chamber behind the Women's Bathroom doorway + riveted steel jamb collar
    for (vx, vy, vz, sx, sy, sz) in [
        (0.0_f32, 2.3_f32, -9.0_f32, 5.6_f32, 5.2_f32, 0.3_f32),
        (-2.7, 2.3, -7.4, 0.3, 5.2, 3.4),
        (2.7, 2.3, -7.4, 0.3, 5.2, 3.4),
        (0.0, -0.1, -7.4, 5.6, 0.2, 3.4),
        (0.0, 4.8, -7.4, 5.6, 0.2, 3.4),
    ] {
        commands.spawn((
            PubPortalCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(void_black.clone()),
            Transform::from_translation(PORTAL_STAGE + Vec3::new(vx, vy, vz))
                .with_scale(Vec3::new(sx, sy, sz)),
        ));
    }
    for (cx, cy, sx, sy) in [
        (-1.02_f32, 1.40_f32, 0.14_f32, 2.86_f32),
        (1.02, 1.40, 0.14, 2.86),
        (0.0, 2.84, 2.18, 0.14),
    ] {
        commands.spawn((
            PubPortalCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(drive_steel.clone()),
            Transform::from_translation(PORTAL_STAGE + Vec3::new(cx, cy, -5.66))
                .with_scale(Vec3::new(sx, sy, 0.22)),
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
            Transform::from_translation(PORTAL_STAGE + Vec3::new(0.0, 3.26, -5.50))
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

    // *Event Horizon* Gravity Drive Singularity behind the Women's Bathroom door:
    // 1. Rippling Obsidian Liquid-Mercury Event Horizon surface
    let (disc_mesh, disc_base) = build_fluid_horizon_disc(1.25);
    let disc_handle = meshes.add(disc_mesh);
    fluid.handle = Some(disc_handle.clone());
    fluid.base_pos = disc_base;
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::HorizonFluid,
        Mesh3d(disc_handle),
        MeshMaterial3d(liquid_mercury),
        Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER).with_scale(Vec3::splat(0.001)),
    ));

    // 2. Pitch-black Singularity sphere + Cerenkov blue lensing ring + Crimson rift ring
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::HorizonCore { ring: false },
        Mesh3d(unit_sphere.clone()),
        MeshMaterial3d(void_black),
        Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER - Vec3::Z * 0.12)
            .with_scale(Vec3::splat(0.001)),
    ));
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::HorizonCore { ring: true },
        Mesh3d(meshes.add(Torus::new(0.055, 1.22))),
        MeshMaterial3d(cerenkov_blue.clone()),
        Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(0.001)),
    ));
    commands.spawn((
        PubPortalCutsceneEntity,
        PubPortalAnim::HorizonCore { ring: true },
        Mesh3d(meshes.add(Torus::new(0.032, 0.92))),
        MeshMaterial3d(hell_crimson),
        Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER + Vec3::Z * 0.05)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::splat(0.001)),
    ));

    // 3. 3 Interlocking Spiked Gravity Drive Gimbal Rings
    let spike_mesh = meshes.add(Cone::new(0.048, 0.22));
    let pip_mesh = meshes.add(Sphere::new(0.034));
    for (idx, (radius, metal)) in [
        (0.84_f32, drive_brass.clone()),
        (1.08, drive_steel.clone()),
        (1.32, drive_brass),
    ]
    .into_iter()
    .enumerate()
    {
        let ring_mesh = meshes.add(Torus::new(0.068, radius));
        commands
            .spawn((
                PubPortalCutsceneEntity,
                PubPortalAnim::DriveGimbal(idx),
                Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER)
                    .with_scale(Vec3::splat(0.001)),
                Visibility::default(),
            ))
            .with_children(|g| {
                crate::casino::spawn_gravity_drive_ring(
                    g,
                    ring_mesh,
                    spike_mesh.clone(),
                    pip_mesh.clone(),
                    metal,
                    cerenkov_blue.clone(),
                    radius,
                );
            });
    }

    // 4. 8 Crackling Electric Cerenkov Plasma Arcs around the doorway threshold
    for idx in 0..8 {
        let bolt_mesh = sh::add(&mut meshes, build_lightning_bolt_geo(idx as u32 + 11, 1.8));
        commands.spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::LightningArc(idx),
            Mesh3d(bolt_mesh),
            MeshMaterial3d(cerenkov_blue.clone()),
            Transform::from_translation(PORTAL_STAGE + EVENT_HORIZON_CENTER),
            Visibility::Hidden,
        ));
    }

    // Background Pub Bar & Scarlett watching warmly from left wall
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

    // Dark metallic gravity shards & streaking Cerenkov plasma motes whipped toward the core
    for idx in 0..12 {
        let base = Vec3::new(
            ((idx as f32) - 5.5) * 0.72,
            0.45 + (idx % 4) as f32 * 0.55,
            -2.1 - (idx % 3) as f32 * 0.65,
        );
        let is_plasma = idx % 2 == 1;
        commands.spawn((
            PubPortalCutsceneEntity,
            PubPortalAnim::FlyingProp { idx, base },
            Mesh3d(if is_plasma { unit_sphere.clone() } else { unit_box.clone() }),
            MeshMaterial3d(if is_plasma {
                cerenkov_blue.clone()
            } else {
                drive_steel.clone()
            }),
            Transform::from_translation(PORTAL_STAGE + base).with_scale(if is_plasma {
                Vec3::new(0.06, 0.06, 0.24)
            } else {
                Vec3::new(0.18, 0.04, 0.14)
            }),
            Visibility::Hidden,
        ));
    }
    let _ = gold_mat;

    // Initial Beat 0 camera establishing shot
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = PORTAL_STAGE + Vec3::new(0.0, 3.0, 6.2);
        c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.3, 0.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.68;
        }
    }

    // Widescreen letterbox & dialogue subtitles UI
    let (spk, line, voice, _) = PUB_PORTAL_BEATS[0];
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
    speak_voice(line, voice);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines)]
fn update_pub_portal_cutscene(
    time: Res<Time>,
    mut p_state: ResMut<PubPortalCutsceneState>,
    fluid: Res<EventHorizonFluidMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
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

    // Ripple & recompute smooth normals on the black liquid-mercury Event Horizon surface
    if beat >= 3 {
        if let Some(ref handle) = fluid.handle {
            if let Some(mesh) = meshes.get_mut(handle) {
                let amp = match beat {
                    3 => 0.035,
                    4 => 0.075,
                    _ => 0.14,
                };
                let tris: Vec<[usize; 3]> = match mesh.indices() {
                    Some(Indices::U32(ids)) => ids
                        .chunks_exact(3)
                        .map(|c| [c[0] as usize, c[1] as usize, c[2] as usize])
                        .collect(),
                    _ => Vec::new(),
                };
                let mut pos_vec: Vec<Vec3> = fluid
                    .base_pos
                    .iter()
                    .map(|&bp| {
                        let r = (bp.x * bp.x + bp.y * bp.y).sqrt();
                        let ang = bp.y.atan2(bp.x);
                        let wave = (r * 13.0 - t * 9.5).sin() * 0.72
                            + (ang * 5.0 + t * 5.4).cos() * 0.28;
                        let falloff = (1.0 - (r / 1.25) * 0.35).clamp(0.15, 1.0);
                        Vec3::new(bp.x, bp.y, wave * amp * falloff)
                    })
                    .collect();
                let mut norms = vec![Vec3::ZERO; pos_vec.len()];
                for [i0, i1, i2] in tris {
                    let e1 = pos_vec[i1] - pos_vec[i0];
                    let e2 = pos_vec[i2] - pos_vec[i0];
                    let fnm = e1.cross(e2);
                    norms[i0] += fnm;
                    norms[i1] += fnm;
                    norms[i2] += fnm;
                }
                let pos_arr: Vec<[f32; 3]> = pos_vec.drain(..).map(|v| v.to_array()).collect();
                let norm_arr: Vec<[f32; 3]> = norms
                    .into_iter()
                    .map(|n| {
                        let nn = if n.length_squared() > 1e-8 {
                            n.normalize()
                        } else {
                            Vec3::Z
                        };
                        nn.to_array()
                    })
                    .collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos_arr);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm_arr);
            }
        }
    }

    // Stand-up factor during Beat 4 (0 -> 1 over first 1.5s), full stand in Beat 5 before suction
    let stand_u = if beat < 4 {
        0.0
    } else if beat == 4 {
        (bt / 1.5).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let stand_smooth = stand_u * stand_u * (3.0 - 2.0 * stand_u);

    // Suction factor in Beat 5 (door blows open in 0..0.28s, guys & props yanked in 0.32..4.6s)
    let suck_u = if beat == 5 {
        ((bt - 0.32) / 4.28).clamp(0.0, 1.0)
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
        2.22 * (1.0 - (1.0 - k).powi(3)) + ((t * 32.0).sin() * 0.06 * (1.0 - k))
    } else if beat == 4 {
        (t * 44.0).sin() * 0.06
    } else {
        let proximity = (1.0 - (min_patron_door_d / 1.45).clamp(0.0, 1.0)).powf(0.8);
        0.04 + proximity * 0.92
    };

    for (mut pl, lcomp) in &mut light_q {
        match lcomp.role {
            PortalLightRole::PubWarm => {
                pl.intensity = if beat == 5 { 36_000.0 } else if beat == 4 { 145_000.0 } else { 240_000.0 };
            }
            PortalLightRole::WalkwayFill => {
                pl.intensity = if beat == 5 { 20_000.0 } else { 150_000.0 };
            }
            PortalLightRole::CerenkovBlue => {
                let strobe = 0.78 + 0.22 * (t * 24.0).sin();
                pl.intensity = match beat {
                    0..=2 => 6_000.0,
                    3 => 55_000.0 + (t * 8.0).sin() * 18_000.0,
                    4 => 180_000.0 * strobe,
                    _ => 760_000.0 * strobe,
                };
            }
            PortalLightRole::HellCrimson => {
                let flicker = (0.4 + 0.6 * (t * 17.0).sin().abs()).powf(1.5);
                pl.intensity = match beat {
                    0..=3 => 0.0,
                    4 => 65_000.0 * flicker,
                    _ => 290_000.0 * flicker,
                };
            }
        }
    }

    for (leg, mut tf) in &mut leg_q {
        let phase = t * 7.4 + leg.patron_idx as f32 * 0.9;
        let swing = phase.sin() * 0.42 * if leg.is_left { 1.0 } else { -1.0 };
        tf.rotation = Quat::from_rotation_x(swing);
    }

    let portal_target = PORTAL_STAGE + EVENT_HORIZON_CENTER;
    let gimbal_axes = [
        Vec3::new(1.0, 0.7, 1.2),
        Vec3::new(-0.9, 1.4, -0.6),
        Vec3::new(0.6, -1.2, 1.6),
    ];

    for (mut tf, mut vis, anim) in &mut anim_q {
        match *anim {
            PubPortalAnim::DoorPivot => {
                tf.rotation = Quat::from_rotation_y(door_slam);
            }
            PubPortalAnim::HorizonFluid => {
                let s = match beat {
                    0..=2 => 0.001,
                    3 => 0.55,
                    4 => 0.92,
                    _ => 1.38 + (t * 10.0).sin() * 0.05,
                };
                tf.scale = Vec3::splat(s);
            }
            PubPortalAnim::HorizonCore { ring } => {
                let s = match beat {
                    0..=2 => 0.001,
                    3 => 0.45,
                    4 => 0.90 + (t * 11.0).sin() * 0.04,
                    _ => 1.34 + (t * 14.0).sin() * 0.07,
                };
                if ring {
                    tf.scale = Vec3::splat(s);
                    tf.rotate_z(2.8 * dt);
                } else {
                    tf.scale = Vec3::new(0.75 * s, 0.75 * s, 0.35 * s);
                }
            }
            PubPortalAnim::DriveGimbal(ri) => {
                let s = match beat {
                    0..=2 => 0.001,
                    3 => 0.35,
                    4 => 0.78,
                    _ => 1.15 + (t * 9.0 + ri as f32).sin() * 0.04,
                };
                tf.scale = Vec3::splat(s);
                let rate = if beat == 5 { 3.4 } else if beat == 4 { 1.4 } else { 0.5 };
                let av = gimbal_axes[ri % 3] * (rate * dt);
                tf.rotate_x(av.x);
                tf.rotate_y(av.y);
                tf.rotate_z(av.z);
            }
            PubPortalAnim::LightningArc(li) => {
                let active = (beat == 4 && bt > 1.5 && ((t * 19.0 + li as f32 * 1.7).sin() > 0.45))
                    || (beat == 5 && ((t * 27.0 + li as f32 * 2.3).sin() > 0.05));
                *vis = if active { Visibility::Inherited } else { Visibility::Hidden };
                if active {
                    let ang = (li as f32 / 8.0) * TAU + t * 3.1;
                    tf.translation = PORTAL_STAGE + EVENT_HORIZON_CENTER
                        + Vec3::new(ang.cos() * 0.45, ang.sin() * 0.45, 0.25);
                    tf.rotation = Quat::from_euler(
                        EulerRot::ZXY,
                        ang,
                        (t * 23.0 + li as f32).sin() * 0.65,
                        (t * 17.0).cos() * 0.45,
                    );
                    tf.scale = Vec3::splat(if beat == 5 { 1.25 } else { 0.85 });
                }
            }
            PubPortalAnim::Chair(ci) => {
                let base = SEATED_SPOTS[ci];
                let push_back = if stand_smooth > 0.0 {
                    let away = (base - TABLE_CENTER).normalize_or_zero();
                    away * (stand_smooth * 0.55)
                } else {
                    Vec3::ZERO
                };
                if suck_u > 0.0 {
                    let pull = (suck_u * 0.68).powf(1.3);
                    let pos = (PORTAL_STAGE + base + push_back).lerp(portal_target, pull);
                    tf.translation = pos + Vec3::Y * (pull * 1.1 + (t * 7.0 + ci as f32).sin() * 0.22);
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
                    let phase = ((t * 1.65 + idx as f32 * 0.17) % 1.0).clamp(0.0, 1.0);
                    let p = (PORTAL_STAGE + base).lerp(portal_target, phase.powf(1.35));
                    let ang = t * 11.0 + idx as f32 * 0.85;
                    tf.translation = p + Vec3::new(ang.cos() * (1.0 - phase) * 0.9, ang.sin() * 0.35, 0.0);
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, ang, ang * 1.4, ang * 0.7);
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
                    // Torn forward from north of the table straight into the Event Horizon singularity
                    let delay = (gi as f32) * 0.06;
                    let p = ((suck_u - delay) / (1.0 - delay)).clamp(0.0, 1.0);
                    let start = PORTAL_STAGE + STAND_SPOTS[gi];
                    let curve = p.powf(1.45);
                    let spiral_r = (1.0 - curve) * 0.75 * (curve * 4.5).min(1.0);
                    let ang = t * 8.5 + gi as f32 * FRAC_PI_2;
                    tf.translation = start.lerp(portal_target, curve)
                        + Vec3::new(ang.cos() * spiral_r, (p * PI).sin() * 0.75 + curve * 0.35, ang.sin() * spiral_r);
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, ang * 1.4, curve * 1.1, (t * 6.0).sin() * 0.45);
                    tf.scale = Vec3::splat(base_scale * (1.0 - curve * 0.88).max(0.08));
                } else if beat >= 4 {
                    let seat_p = SEATED_SPOTS[gi] + Vec3::Y * -0.28;
                    let stand_p = STAND_SPOTS[gi];
                    let mut pos = seat_p.lerp(stand_p, stand_smooth);
                    // Stan & Glenn walk smoothly around the left/right edges of the table in Beat 4
                    let bow_x = if gi == 0 {
                        -0.65 * (stand_smooth * PI).sin()
                    } else if gi == 3 {
                        0.65 * (stand_smooth * PI).sin()
                    } else {
                        0.0
                    };
                    pos.x += bow_x;
                    let y_table = yaw_toward(pos, TABLE_CENTER, model_neg_z);
                    let y_door = yaw_toward(pos, DOOR_POS, model_neg_z);
                    let mut diff = (y_door - y_table + PI).rem_euclid(TAU) - PI;
                    diff *= stand_smooth;
                    let shiver = if beat == 5 { (t * 36.0 + gi as f32).sin() * 0.05 } else { 0.0 };
                    tf.translation = PORTAL_STAGE + pos + Vec3::new(shiver, 0.0, 0.0);
                    tf.rotation = Quat::from_rotation_y(y_table + diff);
                    tf.scale = Vec3::splat(base_scale);
                } else {
                    let bob = (t * 2.4 + gi as f32 * 1.3).sin() * 0.025;
                    let seat_p = SEATED_SPOTS[gi] + Vec3::new(0.0, -0.28 + bob, 0.0);
                    let y_table = yaw_toward(SEATED_SPOTS[gi], TABLE_CENTER, model_neg_z);
                    let turn = if beat == 2 && gi == 2 {
                        -0.45
                    } else if beat == 3 {
                        (t * 3.0 + gi as f32).sin() * 0.25
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

    // Dynamic unobstructed camera per Beat (Beats 2..=5 stay strictly north of the beer tower!)
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        match beat {
            0 => {
                // Warm orbit around all 4 guys and the 10-beer table + tower
                let a = bt * 0.22 - 0.35;
                c_tf.translation = PORTAL_STAGE + Vec3::new(a.sin() * 6.2, 2.95, 0.6 + a.cos() * 6.2);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.35, 0.4), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
            1 => {
                // Frame TJ & Nate talking about the 1990 TMNT movie while attractive women pass behind
                let k = (bt / 4.8).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(2.2 - k * 0.7, 2.35, 3.6);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(-0.1, 1.40, -1.2), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62;
                }
            }
            2 => {
                // Clean northwest aisle view north of table framing attractive women entering doorway
                let k = (bt / 5.2).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(-2.9 + k * 0.6, 2.05, -0.95 - k * 0.45);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.1, 1.42, -4.6), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
            3 => {
                // Clean diagonal view north of beer tower showing doorway Cerenkov glow & women
                c_tf.translation = PORTAL_STAGE + Vec3::new(2.9, 2.10, -0.90);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(-0.1, 1.48, -4.5), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
            4 => {
                // Strictly north of the beer tower looking down center aisle past the 4 standing guys
                let k = (bt / 4.0).clamp(0.0, 1.0);
                c_tf.translation = PORTAL_STAGE + Vec3::new(0.0, 2.02 - k * 0.14, -0.95 - k * 0.85);
                c_tf.look_at(PORTAL_STAGE + Vec3::new(0.0, 1.68, -5.8), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.68 - k * 0.06;
                }
            }
            _ => {
                // Strictly north of beer tower (z = -1.20 -> -4.15): pure unobstructed Event Horizon view!
                let pull = (suck_u * 1.05).clamp(0.0, 1.0);
                let shake = Vec3::new((t * 59.0).sin() * 0.085, (t * 71.0).cos() * 0.085, 0.0);
                c_tf.translation =
                    PORTAL_STAGE + Vec3::new(1.25 * (1.0 - pull), 1.88, -1.20 - pull * 2.95) + shake;
                c_tf.look_at(PORTAL_STAGE + EVENT_HORIZON_CENTER, Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72 + pull * 0.30;
                }
            }
        }
    }
}

fn exit_pub_portal_cutscene(
    mut commands: Commands,
    mut fluid: ResMut<EventHorizonFluidMesh>,
    ent_q: Query<Entity, With<PubPortalCutsceneEntity>>,
    ov_q: Query<Entity, With<PubPortalCutsceneOverlay>>,
    mut hud_q: Query<&mut Visibility, With<HudOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    stop_voice();
    fluid.handle = None;
    fluid.base_pos.clear();
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
