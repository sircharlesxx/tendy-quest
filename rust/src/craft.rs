//! Post-Lexie house-crafting & Black Hole Craft World story arc:
//! 1. `HouseCraftCutscene`: Plays the first time the boys enter Pub 217 after defeating Lexie.
//! 2. `BlackHoleCutscene`: Triggered via Maria's "ORDER 10 BEERS & CRAFT" option -- the boys
//!    argue about *The Odyssey* while 10,000 women overflow the women's restroom and collapse
//!    into a supermassive black hole that swallows the entire city.
//! 3. `CraftPlay`: Empty black-hole Cedar Rapids voxel world where players mine, craft, and build
//!    Pub 217 back up (unlocking Maria's bar, beer drinking, and Casino trips), get Black Hole
//!    Supply Drops, build Stan's new house, and travel back to Cedar Rapids via a mysterious portal.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, set_touch_playing,
    speak_voice, stop_voice,
};
use crate::city::{
    is_in_pub217, CityWorldMarker, PUB217_ANCHOR, PUB217_SCALE,
};
use crate::derek::{make_derek_materials, spawn_derek_rig};
use crate::glenn::{make_glenn_materials, spawn_glenn_rig};
use crate::nate::{make_nate_materials, spawn_nate_rig};
use crate::player::Player;
use crate::pub_portal::{bartender_bar_trigger_pos, spawn_bartender_rig};
use crate::scooter::spawn_mike_rig;
use crate::state::{city_body_scale, CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::tj::spawn_tj_rig;
use crate::transition::{sim_running, SceneTransition};
use crate::ui::{gameplay_running, HudOverlay};

pub const CRAFT_LEVEL_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, -2400.0);
pub const CRAFT_WALK_LIMIT: f32 = 28.0;
pub const CRAFT_SPAWN_POS: Vec3 = Vec3::new(0.0, 0.2, -2386.0);
pub const CRAFT_PORTAL_POS: Vec3 = Vec3::new(0.0, 0.0, -2380.0);

const PUB217_TARGET_BLOCKS: u32 = 18;
const HOUSE_TARGET_BLOCKS: u32 = 30;

// =================================================================================================
// Plugin & Persistent Craft World State
// =================================================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockKind {
    Brick = 0,
    Wood = 1,
    Stone = 2,
    Glass = 3,
    TendyGold = 4,
    Neon217 = 5,
    Leaves = 6,
    TendyOre = 7,
    SupplyCube = 8,
}

const HOTBAR_BLOCKS: [BlockKind; 6] = [
    BlockKind::Brick,
    BlockKind::Wood,
    BlockKind::Stone,
    BlockKind::Glass,
    BlockKind::TendyGold,
    BlockKind::Neon217,
];

impl BlockKind {
    fn short_name(self) -> &'static str {
        match self {
            Self::Brick => "BRICK",
            Self::Wood => "WOOD",
            Self::Stone => "STONE",
            Self::Glass => "GLASS",
            Self::TendyGold => "TENDY",
            Self::Neon217 => "NEON",
            Self::Leaves => "LEAF",
            Self::TendyOre => "ORE",
            Self::SupplyCube => "SUPPLY",
        }
    }
}

#[derive(Resource)]
pub struct CraftWorldData {
    pub initialized: bool,
    pub blocks: HashMap<(i32, i32, i32), BlockKind>,
    pub inventory: [u32; 6],
    pub selected_slot: usize,
    pub pub_blocks_placed: u32,
    pub house_blocks_placed: u32,
    pub house_completed: bool,
    pub supply_timer: f32,
    pub pub217_decor_spawned: bool,
}

impl Default for CraftWorldData {
    fn default() -> Self {
        Self {
            initialized: false,
            blocks: HashMap::new(),
            inventory: [6, 6, 4, 2, 1, 1],
            selected_slot: 0,
            pub_blocks_placed: 0,
            house_blocks_placed: 0,
            house_completed: false,
            supply_timer: 14.0,
            pub217_decor_spawned: false,
        }
    }
}

pub struct CraftPlugin;

impl Plugin for CraftPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HouseCraftState>()
            .init_resource::<BlackHoleState>()
            .init_resource::<CraftWorldData>()
            .init_resource::<CraftInteractPrompt>()
            // City triggers & portal back to Craft World
            .add_systems(
                Update,
                (check_post_lexie_pub217_intro, update_city_craft_portal)
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(gameplay_running),
            )
            // Cutscene 1: Post-Lexie Pub 217 reunion & beer proposal
            .add_systems(OnEnter(GameState::HouseCraftCutscene), enter_house_craft_cutscene)
            .add_systems(OnExit(GameState::HouseCraftCutscene), exit_house_craft_cutscene)
            .add_systems(
                Update,
                update_house_craft_cutscene
                    .run_if(in_state(GameState::HouseCraftCutscene))
                    .run_if(sim_running),
            )
            // Cutscene 2: The Odyssey debate + 10,000 women bathroom Black Hole
            .add_systems(OnEnter(GameState::BlackHoleCutscene), enter_black_hole_cutscene)
            .add_systems(OnExit(GameState::BlackHoleCutscene), exit_black_hole_cutscene)
            .add_systems(
                Update,
                update_black_hole_cutscene
                    .run_if(in_state(GameState::BlackHoleCutscene))
                    .run_if(sim_running),
            )
            // Level 5: Empty Black Hole Cedar Rapids Voxel Craft World
            .add_systems(OnEnter(GameState::CraftPlay), enter_craft_play)
            .add_systems(OnExit(GameState::CraftPlay), exit_craft_play)
            .add_systems(
                Update,
                (
                    update_craft_voxels,
                    update_craft_supply_drops,
                    check_craft_quests,
                    update_craft_npcs_and_portals,
                    handle_craft_modal_input,
                    update_craft_hud,
                )
                    .chain()
                    .run_if(in_state(GameState::CraftPlay))
                    .run_if(gameplay_running),
            );
    }
}

// =================================================================================================
// Section A: Post-Lexie First-Entry Pub 217 Trigger & Bidirectional City Portal
// =================================================================================================

fn city_craft_portal_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(3.8, 0.0, -8.5)
}

#[derive(Component)]
struct CityCraftPortal;

#[derive(Component)]
struct CityCraftPortalRing {
    speed: f32,
}

fn check_post_lexie_pub217_intro(
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    player_q: Query<&Transform, With<Player>>,
) {
    if !stats.in_city
        || !stats.lexie_defeated
        || stats.pub_craft_intro_seen
        || stats.driving_car.is_some()
        || stats.riding_scooter.is_some()
        || stats.riding_wheel
        || stats.bartender_open
        || stats.phone_open
        || stats.char_select_open
    {
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    if is_in_pub217(p_tf.translation, stats.player_scale) {
        stats.pub_craft_intro_seen = true;
        stats.saved_city_pos = bartender_bar_trigger_pos();
        stats.saved_casino_city_scale = stats.player_scale;
        next_state.set(GameState::HouseCraftCutscene);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_city_craft_portal(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    player_q: Query<&Transform, (With<Player>, Without<CityCraftPortal>, Without<CityCraftPortalRing>)>,
    portal_q: Query<Entity, With<CityCraftPortal>>,
    mut ring_q: Query<&mut Transform, (With<CityCraftPortalRing>, Without<Player>)>,
    mut near_cd: Local<f32>,
) {
    let dt = time.delta_secs().min(0.05);
    *near_cd = (*near_cd - dt).max(0.0);

    for mut tf in &mut ring_q {
        tf.rotate_z(2.4 * dt);
        tf.rotate_y(1.1 * dt);
    }

    if !stats.craft_world_unlocked {
        return;
    }

    let portal_pos = city_craft_portal_pos();
    if portal_q.is_empty() {
        let void_mat = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            perceptual_roughness: 0.1,
            ..default()
        });
        let glow_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.68, 0.25, 1.0),
            emissive: LinearRgba::rgb(2.6, 0.85, 4.2),
            unlit: true,
            ..default()
        });
        let gold_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.82, 0.20),
            emissive: LinearRgba::rgb(2.4, 1.7, 0.35),
            unlit: true,
            ..default()
        });
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityCraftPortal,
                Transform::from_translation(portal_pos),
                Visibility::Inherited,
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.95))),
                    MeshMaterial3d(void_mat),
                    Transform::from_xyz(0.0, 1.75, 0.0),
                ));
                p.spawn((
                    CityCraftPortalRing { speed: 2.2 },
                    Mesh3d(meshes.add(Torus::new(0.14, 1.35))),
                    MeshMaterial3d(glow_mat),
                    Transform::from_xyz(0.0, 1.75, 0.0),
                ));
                p.spawn((
                    CityCraftPortalRing { speed: -1.8 },
                    Mesh3d(meshes.add(Torus::new(0.08, 1.65))),
                    MeshMaterial3d(gold_mat.clone()),
                    Transform::from_xyz(0.0, 1.75, 0.0).with_rotation(Quat::from_rotation_x(0.8)),
                ));
                p.spawn((
                    Mesh3d(meshes.add(Cylinder::new(1.5, 0.08))),
                    MeshMaterial3d(gold_mat),
                    Transform::from_xyz(0.0, 0.05, 0.0),
                ));
            });
    }

    if stats.bartender_open || stats.phone_open || stats.char_select_open || stats.driving_car.is_some() {
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let d = Vec2::new(p_tf.translation.x - portal_pos.x, p_tf.translation.z - portal_pos.z).length();
    if d < 3.4 {
        if *near_cd <= 0.0 {
            *near_cd = 2.0;
            stats.hint_text = "MYSTERIOUS BLACK HOLE PORTAL -> CRAFT WORLD! Step inside or press [F/E]".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        if d < 1.55 || (d < 3.0 && (keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE))) {
            play_sfx("portal_warp");
            stats.saved_city_pos = PUB217_ANCHOR + Vec3::new(4.2, 0.0, -4.5);
            stats.saved_casino_city_scale = stats.player_scale;
            next_state.set(GameState::CraftPlay);
        }
    }
}

// =================================================================================================
// Section B: Cutscene 1 -- Post-Lexie Pub 217 Reunion (`GameState::HouseCraftCutscene`)
// =================================================================================================

const HOUSE_CRAFT_STAGE: Vec3 = Vec3::new(0.0, 0.0, -1420.0);

#[derive(Component)]
struct HouseCraftEntity;

#[derive(Component)]
struct HouseCraftOverlay;

#[derive(Component)]
struct HouseCraftSpeakerText;

#[derive(Component)]
struct HouseCraftSubText;

#[derive(Resource, Default)]
pub struct HouseCraftState {
    beat: u8,
    beat_t: f32,
    beat0_spoken: bool,
}

const HOUSE_CRAFT_BEATS: [(&str, &str, &str, f32); 4] = [
    (
        "STAN",
        "\"Well boys, I think we must craft me a new house now that I have defeated my mom!\"",
        "stan",
        4.8,
    ),
    (
        "DEREK",
        "\"Fear not, I have the autism required for this.\"",
        "derek",
        3.8,
    ),
    (
        "TJ, NATE & GLENN",
        "\"Let's have a beer to celebrate our victory first!\"",
        "tj",
        3.8,
    ),
    (
        "MARIA - PUB 217 BARTENDER",
        "\"Step right up to the bar! You can order 10 beers for the Casino, or order 10 beers AND craft!\"",
        "bartender",
        5.0,
    ),
];

fn spawn_house_craft_overlay(commands: &mut Commands) {
    commands
        .spawn((
            HouseCraftEntity,
            HouseCraftOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(40),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(52.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new("PUB 217 -- VICTORY TOAST & NEXT QUEST"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.84, 0.30)),
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(118.0),
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.04, 0.90)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    HouseCraftSpeakerText,
                    Text::new(HOUSE_CRAFT_BEATS[0].0),
                    TextFont { font_size: 17.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.84, 0.30)),
                ));
                bar.spawn((
                    HouseCraftSubText,
                    Text::new(HOUSE_CRAFT_BEATS[0].1),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn enter_house_craft_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cs: ResMut<HouseCraftState>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    *cs = HouseCraftState::default();
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    set_touch_playing(false);
    play_cutscene_music();

    commands.insert_resource(ClearColor(Color::srgb(0.07, 0.04, 0.05)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.90, 0.82),
        brightness: 420.0,
    });

    let stage = HOUSE_CRAFT_STAGE;
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.18, 0.11),
        perceptual_roughness: 0.72,
        ..default()
    });
    let brick_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.14, 0.12),
        perceptual_roughness: 0.82,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.10, 0.06),
        perceptual_roughness: 0.36,
        ..default()
    });
    let gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.80, 0.22),
        emissive: LinearRgba::rgb(2.2, 1.4, 0.25),
        unlit: true,
        ..default()
    });

    // Cozy Pub 217 interior stage
    commands.spawn((
        HouseCraftEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)).with_scale(Vec3::new(16.0, 0.2, 16.0)),
    ));
    for (p, s) in [
        (Vec3::new(0.0, 2.8, -6.5), Vec3::new(16.0, 5.6, 0.4)),
        (Vec3::new(-7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 16.0)),
        (Vec3::new(7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 16.0)),
    ] {
        commands.spawn((
            HouseCraftEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick_mat.clone()),
            Transform::from_translation(stage + p).with_scale(s),
        ));
    }
    // Bar counter + beer table + beer tower
    commands.spawn((
        HouseCraftEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(stage + Vec3::new(-4.5, 0.65, -2.5)).with_scale(Vec3::new(1.2, 1.3, 6.2)),
    ));
    commands.spawn((
        HouseCraftEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood_mat),
        Transform::from_translation(stage + Vec3::new(1.5, 0.55, 0.5)).with_scale(Vec3::new(2.8, 1.1, 2.8)),
    ));
    commands.spawn((
        HouseCraftEntity,
        Mesh3d(unit_box),
        MeshMaterial3d(gold_mat),
        Transform::from_translation(stage + Vec3::new(0.0, 3.8, -6.2)).with_scale(Vec3::new(3.2, 0.7, 0.14)),
    ));
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(1.5, 1.1, 0.5)),
            Visibility::Inherited,
        ))
        .with_children(|t| {
            crate::beer_fx::spawn_pub_beer_tower(t, &mut meshes, &mut materials, Vec3::ZERO);
        });
    commands.spawn((
        HouseCraftEntity,
        PointLight {
            color: Color::srgb(1.0, 0.86, 0.65),
            intensity: 95_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 4.6, 0.5)),
    ));

    // Cast: Stan, Derek, TJ, Nate, Glenn, Maria, Mike
    let scale = city_body_scale(1.8);
    commands.spawn((
        HouseCraftEntity,
        SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
        Transform::from_translation(stage + Vec3::new(0.4, 0.0, 2.5))
            .with_rotation(Quat::from_rotation_y(-0.3))
            .with_scale(Vec3::splat(scale)),
        Visibility::Inherited,
    ));

    let derek_mats = make_derek_materials(&mut materials);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(3.2, 0.0, 1.5))
                .with_rotation(Quat::from_rotation_y(-1.1))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_derek_rig(p, &mut meshes, &derek_mats, false));

    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(-0.9, 0.0, -0.6))
                .with_rotation(Quat::from_rotation_y(0.4))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_tj_rig(p, &mut meshes, &mut materials, false, false));

    let nate_mats = make_nate_materials(&mut materials);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(1.8, 0.0, -1.6))
                .with_rotation(Quat::from_rotation_y(-0.2))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_nate_rig(p, &mut meshes, &nate_mats, false));

    let glenn_mats = make_glenn_materials(&mut materials);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(3.5, 0.0, -0.8))
                .with_rotation(Quat::from_rotation_y(-0.9))
                .with_scale(Vec3::splat(scale * 0.89)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_glenn_rig(p, &mut meshes, &glenn_mats));

    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(-5.7, 0.0, -2.5))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                .with_scale(Vec3::splat(PUB217_SCALE)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_bartender_rig(p, &mut meshes, &mut materials));

    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(stage + Vec3::new(-2.8, 0.0, 1.9))
                .with_rotation(Quat::from_rotation_y(0.8))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_mike_rig(p, &mut meshes, &mut materials));

    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = stage + Vec3::new(0.2, 2.4, 6.2);
        cam.look_at(stage + Vec3::new(0.5, 1.5, 0.0), Vec3::Y);
    }

    spawn_house_craft_overlay(&mut commands);
}

fn exit_house_craft_cutscene(
    mut commands: Commands,
    q: Query<Entity, With<HouseCraftEntity>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    stop_voice();
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
    play_viking_chant_music();
}

#[allow(clippy::type_complexity)]
fn update_house_craft_cutscene(
    time: Res<Time>,
    mut cs: ResMut<HouseCraftState>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<HouseCraftSpeakerText>>,
    mut sub_q: Query<&mut Text, (With<HouseCraftSubText>, Without<HouseCraftSpeakerText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    cs.beat_t += dt;

    if !cs.beat0_spoken && !crate::skip::skipping() {
        cs.beat0_spoken = true;
        speak_voice(HOUSE_CRAFT_BEATS[0].1, HOUSE_CRAFT_BEATS[0].2);
    }

    if crate::skip::skipping() {
        cs.beat = HOUSE_CRAFT_BEATS.len() as u8;
    } else if (cs.beat as usize) < HOUSE_CRAFT_BEATS.len() {
        let min_s = HOUSE_CRAFT_BEATS[cs.beat as usize].3;
        if cs.beat_t >= min_s && !is_voice_speaking() {
            cs.beat += 1;
            cs.beat_t = 0.0;
            if let Some(&(spk, line, voice, _)) = HOUSE_CRAFT_BEATS.get(cs.beat as usize) {
                if let Ok(mut t) = spk_q.get_single_mut() {
                    t.0 = spk.to_string();
                }
                if let Ok(mut t) = sub_q.get_single_mut() {
                    t.0 = line.to_string();
                }
                speak_voice(line, voice);
            }
        }
    }

    let stage = HOUSE_CRAFT_STAGE;
    if let Ok(mut cam) = cam_q.get_single_mut() {
        let (eye, look) = match cs.beat {
            0 => (stage + Vec3::new(0.2, 2.3, 6.2), stage + Vec3::new(0.4, 1.5, 2.2)),
            1 => (stage + Vec3::new(0.6, 2.1, 3.8), stage + Vec3::new(3.2, 1.6, 1.5)),
            2 => (stage + Vec3::new(1.5, 2.6, 4.6), stage + Vec3::new(1.4, 1.4, -0.5)),
            _ => (stage + Vec3::new(-1.5, 2.3, 1.4), stage + Vec3::new(-5.2, 1.8, -2.5)),
        };
        cam.translation = cam.translation.lerp(eye, (dt * 3.8).min(1.0));
        cam.look_at(look, Vec3::Y);
    }

    if (cs.beat as usize) >= HOUSE_CRAFT_BEATS.len() {
        stats.house_craft_return_city = true;
        stats.hint_text = "Talk to Maria [F/E] -- Order 10 Beers (Casino) or Order 10 Beers & Craft!".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        next_state.set(GameState::CityPlay);
    }
}

// =================================================================================================
// Section C: Cutscene 2 -- The Odyssey + 10,000 Women Black Hole (`GameState::BlackHoleCutscene`)
// =================================================================================================

const BLACK_HOLE_STAGE: Vec3 = Vec3::new(0.0, 0.0, -1720.0);

#[derive(Component)]
struct BlackHoleCutsceneEntity;

#[derive(Component)]
struct BlackHoleSpeakerText;

#[derive(Component)]
struct BlackHoleSubText;

#[derive(Component)]
struct BlackHoleCounterText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BlackHoleAnimKind {
    Woman(usize),
    CityChunk(usize),
    Boy(usize),
    Horizon,
    Ring(usize),
}

#[derive(Component)]
struct BlackHoleAnim {
    kind: BlackHoleAnimKind,
    home_pos: Vec3,
    home_scale: Vec3,
    phase: f32,
}

#[derive(Resource, Default)]
pub struct BlackHoleState {
    beat: u8,
    beat_t: f32,
    total_t: f32,
    beat0_spoken: bool,
    boom_played: bool,
}

const BLACK_HOLE_BEATS: [(&str, &str, &str, f32); 7] = [
    (
        "NATE",
        "\"Ten beers down! Dude, The Odyssey was so good though, straight peak cinema!\"",
        "nate",
        4.4,
    ),
    (
        "TJ",
        "\"No, we saw it in the wrong format, so we missed half the context of the movie!\"",
        "tj",
        4.6,
    ),
    (
        "DEREK",
        "\"You're pretentious and just didn't get it, this is the best movie of all time!\"",
        "derek",
        4.5,
    ),
    (
        "STAN",
        "\"I'm gonna break your ankles!\"",
        "stan",
        2.9,
    ),
    (
        "GLENN",
        "\"WAIT A SECOND BOYS!! Look at the women's bathroom -- 10,000 women just walked in and it is COMPLETELY OVERFLOWING!!\"",
        "glenn",
        5.5,
    ),
    (
        "NARRATOR",
        "\"CRITICAL MASS REACHED!! ALL 10,000 WOMEN JUST COLLAPSED INTO A SUPERMASSIVE BLACK HOLE!!\"",
        "tj_upset",
        4.6,
    ),
    (
        "STAN, TJ, DEREK, NATE & GLENN",
        "\"THE ENTIRE CITY IS GETTING SUCKED INTO THE BLACK HOLE!! HOLD ON TO YOUR BEERS!! AAAAAHHHHH!!\"",
        "stan",
        4.6,
    ),
];

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn enter_black_hole_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bh: ResMut<BlackHoleState>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    *bh = BlackHoleState::default();
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    set_touch_playing(false);
    play_cutscene_music();

    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.01, 0.05)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.88, 1.0),
        brightness: 420.0,
    });

    let stage = BLACK_HOLE_STAGE;
    let door_pos = stage + Vec3::new(0.0, 0.0, -6.2);
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.14, 0.09),
        perceptual_roughness: 0.72,
        ..default()
    });
    let brick_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.13, 0.11),
        perceptual_roughness: 0.80,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.09, 0.05),
        perceptual_roughness: 0.36,
        ..default()
    });
    let neon_pink = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.85),
        emissive: LinearRgba::rgb(2.6, 0.5, 2.0),
        unlit: true,
        ..default()
    });
    let void_black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        perceptual_roughness: 0.0,
        ..default()
    });
    let bh_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.22, 1.0),
        emissive: LinearRgba::rgb(3.2, 0.85, 5.0),
        unlit: true,
        ..default()
    });
    let bh_amber = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.65, 0.15),
        emissive: LinearRgba::rgb(4.2, 2.1, 0.3),
        unlit: true,
        ..default()
    });

    // Pub room + beer table + Women's Restroom doorway (all tagged CityChunk so they get sucked in!)
    let mut spawn_chunk = |idx: usize, mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, pos: Vec3, scale: Vec3| {
        commands.spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::CityChunk(idx),
                home_pos: pos,
                home_scale: scale,
                phase: idx as f32 * 0.45,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos).with_scale(scale),
        ));
    };
    spawn_chunk(0, unit_box.clone(), floor_mat, stage + Vec3::new(0.0, -0.1, 0.0), Vec3::new(16.0, 0.2, 16.0));
    spawn_chunk(1, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(-7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 16.0));
    spawn_chunk(2, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 16.0));
    spawn_chunk(3, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(-4.5, 2.8, -6.4), Vec3::new(6.2, 5.6, 0.4));
    spawn_chunk(4, unit_box.clone(), brick_mat, stage + Vec3::new(4.5, 2.8, -6.4), Vec3::new(6.2, 5.6, 0.4));
    spawn_chunk(5, unit_box.clone(), wood_mat.clone(), stage + Vec3::new(0.0, 0.55, 1.5), Vec3::new(3.2, 1.1, 2.4));
    spawn_chunk(6, unit_box.clone(), wood_mat, stage + Vec3::new(-5.2, 0.65, -2.2), Vec3::new(1.1, 1.3, 5.2));
    // "WOMEN'S RESTROOM" neon header above doorway
    spawn_chunk(7, unit_box.clone(), neon_pink, door_pos + Vec3::new(0.0, 3.3, 0.25), Vec3::new(2.8, 0.65, 0.18));

    // Distant Cedar Rapids buildings + cabs around the exterior ready to get sucked into the vortex!
    let tower_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.36, 0.48),
        perceptual_roughness: 0.6,
        ..default()
    });
    let cab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.08),
        perceptual_roughness: 0.3,
        ..default()
    });
    for i in 0..16_usize {
        let a = i as f32 * (TAU / 16.0);
        let r = 15.0 + (i % 3) as f32 * 4.5;
        let h = 10.0 + (i % 4) as f32 * 4.5;
        let is_cab = i % 3 == 0;
        let pos = door_pos + Vec3::new(a.cos() * r, if is_cab { 0.6 } else { h * 0.5 }, a.sin() * r);
        let scale = if is_cab { Vec3::new(2.0, 1.2, 4.0) } else { Vec3::new(4.2, h, 4.2) };
        spawn_chunk(
            10 + i,
            unit_box.clone(),
            if is_cab { cab_mat.clone() } else { tower_mat.clone() },
            pos,
            scale,
        );
    }

    // 44 stylized women visibly pouring into and overflowing out of the Women's Restroom!
    let dress_colors = [
        materials.add(StandardMaterial { base_color: Color::srgb(0.86, 0.12, 0.44), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.14, 0.64, 0.50), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.24, 0.36, 0.88), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.66, 0.16), ..default() }),
    ];
    let skin_w = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.78, 0.68), ..default() });
    let hair_w = materials.add(StandardMaterial { base_color: Color::srgb(0.18, 0.10, 0.08), ..default() });
    let head_sphere = meshes.add(Sphere::new(0.18));
    for wi in 0..44_usize {
        let row = (wi / 6) as f32;
        let col = (wi % 6) as f32 - 2.5;
        let wpos = door_pos + Vec3::new(col * 0.42, (wi % 3) as f32 * 0.22, -0.4 + row * 0.48);
        commands
            .spawn((
                BlackHoleCutsceneEntity,
                BlackHoleAnim {
                    kind: BlackHoleAnimKind::Woman(wi),
                    home_pos: wpos,
                    home_scale: Vec3::ONE,
                    phase: wi as f32 * 0.37,
                },
                Transform::from_translation(wpos),
                Visibility::Inherited,
            ))
            .with_children(|w| {
                w.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(dress_colors[wi % dress_colors.len()].clone()),
                    Transform::from_xyz(0.0, 0.68, 0.0).with_scale(Vec3::new(0.24, 1.32, 0.22)),
                ));
                w.spawn((
                    Mesh3d(head_sphere.clone()),
                    MeshMaterial3d(skin_w.clone()),
                    Transform::from_xyz(0.0, 1.52, 0.0),
                ));
                w.spawn((
                    Mesh3d(head_sphere.clone()),
                    MeshMaterial3d(hair_w.clone()),
                    Transform::from_xyz(0.0, 1.56, -0.06).with_scale(Vec3::new(1.08, 1.25, 1.05)),
                ));
            });
    }

    // Event Horizon + glowing accretion rings inside the bathroom doorway (scale 0.01 until Beat 5!)
    commands.spawn((
        BlackHoleCutsceneEntity,
        BlackHoleAnim {
            kind: BlackHoleAnimKind::Horizon,
            home_pos: door_pos + Vec3::new(0.0, 1.8, -0.2),
            home_scale: Vec3::splat(2.8),
            phase: 0.0,
        },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(void_black),
        Transform::from_translation(door_pos + Vec3::new(0.0, 1.8, -0.2)).with_scale(Vec3::splat(0.01)),
    ));
    for (ri, rmat) in [(0, bh_purple), (1, bh_amber)] {
        commands.spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Ring(ri),
                home_pos: door_pos + Vec3::new(0.0, 1.8, -0.2),
                home_scale: Vec3::splat(3.6 + ri as f32 * 0.9),
                phase: ri as f32,
            },
            Mesh3d(meshes.add(Torus::new(0.08, 1.0))),
            MeshMaterial3d(rmat),
            Transform::from_translation(door_pos + Vec3::new(0.0, 1.8, -0.2)).with_scale(Vec3::splat(0.01)),
        ));
    }

    // Main boys around the table (Stan, Nate, TJ, Derek, Glenn)
    let scale = city_body_scale(1.8);
    let boy_spots = [
        stage + Vec3::new(-1.8, 0.0, 2.2),
        stage + Vec3::new(1.8, 0.0, 2.2),
        stage + Vec3::new(-2.1, 0.0, 0.5),
        stage + Vec3::new(2.1, 0.0, 0.5),
        stage + Vec3::new(0.0, 0.0, 3.2),
    ];
    commands.spawn((
        BlackHoleCutsceneEntity,
        BlackHoleAnim {
            kind: BlackHoleAnimKind::Boy(0),
            home_pos: boy_spots[0],
            home_scale: Vec3::splat(scale),
            phase: 0.0,
        },
        SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
        Transform::from_translation(boy_spots[0]).with_rotation(Quat::from_rotation_y(-1.2)).with_scale(Vec3::splat(scale)),
        Visibility::Inherited,
    ));
    let nate_m = make_nate_materials(&mut materials);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(1),
                home_pos: boy_spots[1],
                home_scale: Vec3::splat(scale * 0.85),
                phase: 1.0,
            },
            Transform::from_translation(boy_spots[1]).with_rotation(Quat::from_rotation_y(-1.8)).with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_nate_rig(p, &mut meshes, &nate_m, false));
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(2),
                home_pos: boy_spots[2],
                home_scale: Vec3::splat(scale * 0.85),
                phase: 2.0,
            },
            Transform::from_translation(boy_spots[2]).with_rotation(Quat::from_rotation_y(FRAC_PI_2)).with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_tj_rig(p, &mut meshes, &mut materials, false, false));
    let derek_m = make_derek_materials(&mut materials);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(3),
                home_pos: boy_spots[3],
                home_scale: Vec3::splat(scale * 0.85),
                phase: 3.0,
            },
            Transform::from_translation(boy_spots[3]).with_rotation(Quat::from_rotation_y(-FRAC_PI_2)).with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_derek_rig(p, &mut meshes, &derek_m, false));
    let glenn_m = make_glenn_materials(&mut materials);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(4),
                home_pos: boy_spots[4],
                home_scale: Vec3::splat(scale * 0.89),
                phase: 4.0,
            },
            Transform::from_translation(boy_spots[4]).with_rotation(Quat::from_rotation_y(PI)).with_scale(Vec3::splat(scale * 0.89)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_glenn_rig(p, &mut meshes, &glenn_m));

    commands.spawn((
        BlackHoleCutsceneEntity,
        PointLight {
            color: Color::srgb(0.95, 0.82, 1.0),
            intensity: 95_000.0,
            range: 34.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 5.2, 0.5)),
    ));

    // Cutscene UI: Top bar + live Women Counter + Bottom subtitle
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(40),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(54.0),
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            ))
            .with_children(|top| {
                top.spawn((
                    Text::new("PUB 217 -- 10 BEERS & THE ODYSSEY"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.82, 0.30)),
                ));
                top.spawn((
                    BlackHoleCounterText,
                    Text::new("WOMEN IN RESTROOM: 18"),
                    TextFont { font_size: 15.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.45, 0.88)),
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(120.0),
                    padding: UiRect::axes(Val::Px(26.0), Val::Px(14.0)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.90)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    BlackHoleSpeakerText,
                    Text::new(BLACK_HOLE_BEATS[0].0),
                    TextFont { font_size: 17.0, ..default() },
                    TextColor(Color::srgb(0.75, 0.55, 1.0)),
                ));
                bar.spawn((
                    BlackHoleSubText,
                    Text::new(BLACK_HOLE_BEATS[0].1),
                    TextFont { font_size: 17.5, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        });

    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = stage + Vec3::new(0.0, 2.7, 7.2);
        cam.look_at(stage + Vec3::new(0.0, 1.5, 0.0), Vec3::Y);
    }
}

fn exit_black_hole_cutscene(
    mut commands: Commands,
    q: Query<Entity, With<BlackHoleCutsceneEntity>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    stop_voice();
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_black_hole_cutscene(
    time: Res<Time>,
    mut bh: ResMut<BlackHoleState>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<BlackHoleAnim>)>,
    mut anim_q: Query<(&mut Transform, &BlackHoleAnim), Without<Camera3d>>,
    mut spk_q: Query<&mut Text, With<BlackHoleSpeakerText>>,
    mut sub_q: Query<&mut Text, (With<BlackHoleSubText>, Without<BlackHoleSpeakerText>)>,
    mut cnt_q: Query<
        &mut Text,
        (
            With<BlackHoleCounterText>,
            Without<BlackHoleSpeakerText>,
            Without<BlackHoleSubText>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    bh.beat_t += dt;
    bh.total_t += dt;

    if !bh.beat0_spoken && !crate::skip::skipping() {
        bh.beat0_spoken = true;
        speak_voice(BLACK_HOLE_BEATS[0].1, BLACK_HOLE_BEATS[0].2);
    }

    if crate::skip::skipping() {
        bh.beat = BLACK_HOLE_BEATS.len() as u8;
    } else if (bh.beat as usize) < BLACK_HOLE_BEATS.len() {
        let min_s = BLACK_HOLE_BEATS[bh.beat as usize].3;
        if bh.beat_t >= min_s && !is_voice_speaking() {
            bh.beat += 1;
            bh.beat_t = 0.0;
            if let Some(&(spk, line, voice, _)) = BLACK_HOLE_BEATS.get(bh.beat as usize) {
                if let Ok(mut t) = spk_q.get_single_mut() {
                    t.0 = spk.to_string();
                }
                if let Ok(mut t) = sub_q.get_single_mut() {
                    t.0 = line.to_string();
                }
                speak_voice(line, voice);
            }
        }
    }

    if bh.beat >= 5 && !bh.boom_played {
        bh.boom_played = true;
        play_sfx("black_hole");
        play_sfx("explosion");
    }

    let stage = BLACK_HOLE_STAGE;
    let bh_center = stage + Vec3::new(0.0, 1.8, -6.4);
    let women_count: u32 = match bh.beat {
        0 => 18 + (bh.beat_t * 25.0) as u32,
        1 => 140 + (bh.beat_t * 160.0) as u32,
        2 => 900 + (bh.beat_t * 600.0) as u32,
        3 => 3600 + (bh.beat_t * 1400.0) as u32,
        4 => (7800 + (bh.beat_t * 500.0) as u32).min(10_000),
        _ => 10_000,
    };
    if let Ok(mut t) = cnt_q.get_single_mut() {
        t.0 = if bh.beat >= 5 {
            "CRITICAL MASS: BLACK HOLE FORMED!!".to_string()
        } else {
            format!("WOMEN IN RESTROOM: {}/10,000", women_count.min(10_000))
        };
    }

    let bh_pull = if bh.beat == 5 {
        (bh.beat_t / 4.2).clamp(0.0, 1.0) * 0.42
    } else if bh.beat >= 6 {
        0.42 + (bh.beat_t / 4.2).clamp(0.0, 1.0) * 0.58
    } else {
        0.0
    };

    for (mut tf, anim) in &mut anim_q {
        match anim.kind {
            BlackHoleAnimKind::Horizon => {
                let s = if bh.beat < 5 { 0.01 } else { 0.4 + bh_pull * 3.8 };
                tf.scale = Vec3::splat(s);
            }
            BlackHoleAnimKind::Ring(idx) => {
                if bh.beat < 5 {
                    tf.scale = Vec3::splat(0.01);
                } else {
                    tf.scale = anim.home_scale * (0.45 + bh_pull * 1.1);
                    tf.rotate_z((3.5 + idx as f32 * 2.2) * dt);
                    tf.rotate_x((1.6 - idx as f32 * 0.8) * dt);
                }
            }
            BlackHoleAnimKind::Woman(idx) => {
                if bh.beat < 5 {
                    let jig = (bh.total_t * 4.5 + anim.phase).sin() * 0.08;
                    let spill = (bh.beat as f32 / 4.0).clamp(0.0, 1.0) * 1.1;
                    tf.translation = anim.home_pos + Vec3::new(jig, 0.0, spill * ((idx % 4) as f32 * 0.4));
                } else {
                    let pull = (bh_pull * 1.45).clamp(0.0, 1.0);
                    let ang = bh.total_t * 7.5 + anim.phase;
                    let orbit = Vec3::new(ang.cos(), (ang * 0.7).sin() * 0.4, ang.sin()) * ((1.0 - pull) * 3.2);
                    tf.translation = anim.home_pos.lerp(bh_center, pull) + orbit * pull;
                    tf.rotate_y(8.0 * dt);
                    tf.scale = Vec3::splat((1.0 - pull * 0.96).max(0.04));
                }
            }
            BlackHoleAnimKind::CityChunk(_) | BlackHoleAnimKind::Boy(_) => {
                if bh.beat >= 6 {
                    let pull = ((bh_pull - 0.42) / 0.58).clamp(0.0, 1.0);
                    let ang = bh.total_t * 5.2 + anim.phase;
                    let swirl = Vec3::new(ang.cos(), 0.35 * ang.sin(), ang.sin()) * (1.0 - pull) * 4.0;
                    tf.translation = anim.home_pos.lerp(bh_center, pull) + swirl * pull;
                    tf.rotate_y(4.5 * dt);
                    tf.rotate_z(2.8 * dt);
                    tf.scale = anim.home_scale * (1.0 - pull * 0.95).max(0.05);
                } else if anim.kind == BlackHoleAnimKind::Boy(0) && bh.beat == 3 {
                    // Stan lunges forward threatening TJ & Derek's ankles
                    tf.translation = anim.home_pos + Vec3::new(0.45 * (bh.beat_t * 8.0).sin().abs(), 0.0, -0.35);
                }
            }
        }
    }

    if let Ok(mut cam) = cam_q.get_single_mut() {
        let (eye, look) = match bh.beat {
            0 | 1 | 2 => (stage + Vec3::new(0.2, 2.5, 6.8), stage + Vec3::new(0.0, 1.4, 0.8)),
            3 => (stage + Vec3::new(-3.2, 2.0, 4.4), stage + Vec3::new(0.0, 1.5, 1.4)),
            4 => (stage + Vec3::new(2.2, 2.8, 3.6), door_pos_cam(stage)),
            5 => (stage + Vec3::new(0.0, 2.6, 5.2), bh_center),
            _ => {
                let swirl_u = (bh.beat_t / 4.2).clamp(0.0, 1.0);
                (
                    stage + Vec3::new((bh.total_t * 2.5).sin() * 3.0, 3.4 - swirl_u * 1.4, 9.0 - swirl_u * 8.0),
                    bh_center,
                )
            }
        };
        cam.translation = cam.translation.lerp(eye, (dt * 4.2).min(1.0));
        cam.look_at(look, Vec3::Y);
    }

    if (bh.beat as usize) >= BLACK_HOLE_BEATS.len() {
        stats.craft_world_unlocked = true;
        next_state.set(GameState::CraftPlay);
    }
}

fn door_pos_cam(stage: Vec3) -> Vec3 {
    stage + Vec3::new(0.0, 1.4, -4.5)
}

// =================================================================================================
// Section D: Level 5 Voxel Craft World (`GameState::CraftPlay`)
// =================================================================================================

#[derive(Component)]
struct CraftLevelEntity;

#[derive(Component)]
struct VoxelBlock {
    coord: (i32, i32, i32),
}

#[derive(Component)]
struct CraftCursorVisual;

#[derive(Component)]
struct CraftDebris {
    vel: Vec3,
    ang_vel: Vec3,
    age: f32,
}

#[derive(Component)]
struct CraftPortalRing;

#[derive(Component)]
struct CraftHouseOutline;

#[derive(Component)]
struct CraftPubDecor;

#[derive(Component)]
struct CraftBeerTowerSpot;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CraftNpcRole {
    Maria,
    Mike,
    Derek,
    Tj,
    Nate,
    Glenn,
}

#[derive(Component)]
struct CraftNpc {
    role: CraftNpcRole,
}

#[derive(Component)]
struct CraftHudRoot;

#[derive(Component)]
struct CraftQuestText;

#[derive(Component)]
struct CraftPromptPill;

#[derive(Component)]
struct CraftPromptText;

#[derive(Component)]
struct CraftCrosshair;

#[derive(Component, Clone, Copy)]
struct CraftHotbarBtn(usize);

#[derive(Component)]
struct CraftHotbarText(usize);

#[derive(Component)]
struct CraftModalRoot;

#[derive(Component, Clone, Copy)]
enum CraftModalAction {
    Close,
    CraftBrick,
    CraftGlass,
    CraftNeon,
    FreeScrapKit,
    DerekHyperfocusPub217,
    DrinkBeer,
    CasinoFromPub,
}

#[derive(Resource)]
struct CraftBlockAssets {
    cube_mesh: Handle<Mesh>,
    mats: [Handle<StandardMaterial>; 9],
}

#[derive(Resource, Default)]
struct CraftInteractPrompt {
    target: Option<CraftPromptTarget>,
    tapped: bool,
    modal_is_derek: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CraftPromptTarget {
    Npc(CraftNpcRole),
    BeerTower,
    Portal,
}

fn grid_to_world(coord: (i32, i32, i32)) -> Vec3 {
    CRAFT_LEVEL_ORIGIN + Vec3::new(coord.0 as f32, coord.1 as f32 + 0.5, coord.2 as f32)
}

fn world_to_grid(p: Vec3) -> (i32, i32, i32) {
    let rel = p - CRAFT_LEVEL_ORIGIN;
    (rel.x.round() as i32, rel.y.floor() as i32, rel.z.round() as i32)
}

fn in_pub217_zone(c: (i32, i32, i32)) -> bool {
    (-9..=-2).contains(&c.0) && (0..=5).contains(&c.1) && (-6..=1).contains(&c.2)
}

fn in_house_zone(c: (i32, i32, i32)) -> bool {
    (3..=11).contains(&c.0) && (0..=6).contains(&c.1) && (-6..=2).contains(&c.2)
}

fn recount_quest_blocks(data: &mut CraftWorldData) {
    let mut pub_c = 0_u32;
    let mut house_c = 0_u32;
    for &c in data.blocks.keys() {
        if in_pub217_zone(c) {
            pub_c += 1;
        } else if in_house_zone(c) {
            house_c += 1;
        }
    }
    data.pub_blocks_placed = pub_c;
    data.house_blocks_placed = house_c;
}

fn populate_empty_city_world(data: &mut CraftWorldData) {
    if data.initialized {
        recount_quest_blocks(data);
        return;
    }
    data.initialized = true;
    data.inventory = [6, 6, 4, 2, 1, 1];
    data.selected_slot = 0;
    data.supply_timer = 14.0;

    // 8 chunky Minecraft trees around the edges
    for (tx, tz) in [
        (-18, -14),
        (-20, 6),
        (-16, 16),
        (18, -14),
        (20, 8),
        (16, 16),
        (-6, -18),
        (6, -18),
    ] {
        for y in 0..=2 {
            data.blocks.insert((tx, y, tz), BlockKind::Wood);
        }
        for dx in -1_i32..=1 {
            for dz in -1_i32..=1 {
                for dy in 3_i32..=4 {
                    if dx == 0 && dz == 0 && dy == 3 {
                        data.blocks.insert((tx, dy, tz), BlockKind::Wood);
                    } else if !(dy == 4 && dx.abs() == 1 && dz.abs() == 1) {
                        data.blocks.insert((tx + dx, dy, tz + dz), BlockKind::Leaves);
                    }
                }
            }
        }
    }

    // 5 shattered Brick/Stone rubble piles from old Cedar Rapids
    for (rx, rz) in [(-13, -10), (-12, 9), (14, -10), (14, 10), (0, -14)] {
        data.blocks.insert((rx, 0, rz), BlockKind::Brick);
        data.blocks.insert((rx + 1, 0, rz), BlockKind::Stone);
        data.blocks.insert((rx, 0, rz + 1), BlockKind::Stone);
        data.blocks.insert((rx, 1, rz), BlockKind::Brick);
    }

    // 4 Glass clusters & 4 Glowing Tendy Ore boulders
    for (gx, gz) in [(-14, -1), (15, 2), (-4, 12), (5, 12)] {
        data.blocks.insert((gx, 0, gz), BlockKind::Glass);
        data.blocks.insert((gx + 1, 0, gz), BlockKind::Glass);
        data.blocks.insert((gx, 1, gz), BlockKind::Glass);
    }
    for (ox, oz) in [(-21, -2), (21, -2), (-2, -20), (12, -18)] {
        data.blocks.insert((ox, 0, oz), BlockKind::TendyOre);
        data.blocks.insert((ox + 1, 0, oz), BlockKind::Stone);
        data.blocks.insert((ox, 1, oz), BlockKind::TendyOre);
    }

    recount_quest_blocks(data);
}

fn spawn_voxel_entity(
    commands: &mut Commands,
    assets: &CraftBlockAssets,
    coord: (i32, i32, i32),
    kind: BlockKind,
) -> Entity {
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            VoxelBlock { coord },
            Mesh3d(assets.cube_mesh.clone()),
            MeshMaterial3d(assets.mats[kind as usize].clone()),
            Transform::from_translation(grid_to_world(coord)),
            PlatformBox {
                half_extents: Vec3::splat(0.5),
            },
        ))
        .id()
}

fn spawn_pub217_rebuilt_interior(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let pub_center = CRAFT_LEVEL_ORIGIN + Vec3::new(-5.5, 0.0, -2.5);
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.12, 0.06),
        perceptual_roughness: 0.35,
        ..default()
    });
    let gold_neon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.80, 0.22),
        emissive: LinearRgba::rgb(2.6, 1.8, 0.3),
        unlit: true,
        ..default()
    });
    // Mahogany bar counter inside rebuilt Pub 217
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftPubDecor,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood.clone()),
        Transform::from_translation(pub_center + Vec3::new(-1.0, 0.55, 0.2)).with_scale(Vec3::new(0.9, 1.1, 3.8)),
        PlatformBox { half_extents: Vec3::new(0.45, 0.55, 1.9) },
    ));
    // Glowing 217 neon pub sign above front entrance
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftPubDecor,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_neon),
        Transform::from_translation(pub_center + Vec3::new(3.7, 3.8, 0.5)).with_scale(Vec3::new(0.2, 0.75, 2.2)),
    ));
    // Interactive Beer Tower Table inside rebuilt Pub 217
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            CraftPubDecor,
            CraftBeerTowerSpot,
            Transform::from_translation(pub_center + Vec3::new(1.5, 0.0, 1.8)),
            Visibility::Inherited,
        ))
        .with_children(|t| {
            t.spawn((
                Mesh3d(unit_box),
                MeshMaterial3d(wood),
                Transform::from_xyz(0.0, 0.45, 0.0).with_scale(Vec3::new(1.4, 0.9, 1.4)),
            ));
            crate::beer_fx::spawn_pub_beer_tower(t, meshes, materials, Vec3::new(0.0, 0.9, 0.0));
        });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftPubDecor,
        PointLight {
            color: Color::srgb(1.0, 0.82, 0.48),
            intensity: 45_000.0,
            range: 14.0,
            ..default()
        },
        Transform::from_translation(pub_center + Vec3::new(0.0, 3.4, 0.0)),
    ));
}

fn npc_position(role: CraftNpcRole, rebuilt: bool) -> (Vec3, f32) {
    let base = CRAFT_LEVEL_ORIGIN;
    if rebuilt {
        match role {
            CraftNpcRole::Maria => (base + Vec3::new(-7.6, 0.0, -2.2), FRAC_PI_2),
            CraftNpcRole::Mike => (base + Vec3::new(-4.8, 0.0, 0.5), 0.2),
            CraftNpcRole::Derek => (base + Vec3::new(-1.8, 0.0, 3.5), -0.4),
            CraftNpcRole::Tj => (base + Vec3::new(-5.5, 0.0, -4.0), 0.0),
            CraftNpcRole::Nate => (base + Vec3::new(-3.2, 0.0, -2.2), -FRAC_PI_2),
            CraftNpcRole::Glenn => (base + Vec3::new(-5.5, 0.0, -0.5), PI),
        }
    } else {
        match role {
            CraftNpcRole::Maria => (base + Vec3::new(-1.2, 0.0, 4.2), 0.2),
            CraftNpcRole::Mike => (base + Vec3::new(-0.8, 0.0, -1.0), 0.3),
            CraftNpcRole::Derek => (base + Vec3::new(-3.2, 0.0, 4.2), 0.0),
            CraftNpcRole::Tj => (base + Vec3::new(1.4, 0.0, 3.6), -0.3),
            CraftNpcRole::Nate => (base + Vec3::new(2.8, 0.0, 4.5), -0.5),
            CraftNpcRole::Glenn => (base + Vec3::new(3.8, 0.0, 3.2), -0.8),
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines, clippy::type_complexity)]
fn enter_craft_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut data: ResMut<CraftWorldData>,
    mut orbit: ResMut<CameraOrbit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old_q: Query<Entity, With<CraftLevelEntity>>,
    mut hud_q: Query<(&mut Node, &mut Visibility), With<HudOverlay>>,
    mut player_q: Query<
        (&mut Transform, &mut Player, &mut Visibility),
        (With<Player>, Without<HudOverlay>, Without<Camera3d>),
    >,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<HudOverlay>)>,
) {
    for e in &old_q {
        commands.entity(e).despawn_recursive();
    }

    stats.in_city = false;
    stats.in_casino_level = false;
    stats.in_club_level = false;
    stats.in_craft_level = true;
    stats.boss_mode = false;
    stats.enraged = false;
    stats.player_scale = 1.8;
    stats.bartender_open = false;
    stats.craft_modal_open = false;
    stats.driving_car = None;
    stats.riding_scooter = None;
    stats.riding_wheel = false;
    stats.phone_open = false;
    data.pub217_decor_spawned = stats.craft_pub217_rebuilt;

    crate::audio::set_touch_playing(true);
    crate::audio::set_touch_craft_mode(true);
    play_viking_chant_music();

    commands.insert_resource(ClearColor(Color::srgb(0.11, 0.14, 0.24)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.88, 0.92, 1.0),
        brightness: 450.0,
    });

    for (mut node, mut vis) in &mut hud_q {
        node.display = Display::Flex;
        *vis = Visibility::Inherited;
    }

    let assets = CraftBlockAssets {
        cube_mesh: meshes.add(Cuboid::from_size(Vec3::ONE)),
        mats: [
            materials.add(StandardMaterial { base_color: Color::srgb(0.56, 0.20, 0.16), perceptual_roughness: 0.78, ..default() }),
            materials.add(StandardMaterial { base_color: Color::srgb(0.46, 0.28, 0.14), perceptual_roughness: 0.72, ..default() }),
            materials.add(StandardMaterial { base_color: Color::srgb(0.48, 0.50, 0.54), perceptual_roughness: 0.85, ..default() }),
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.46, 0.82, 1.0, 0.62),
                metallic: 0.4,
                perceptual_roughness: 0.14,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.96, 0.74, 0.18),
                emissive: LinearRgba::rgb(0.28, 0.16, 0.02),
                perceptual_roughness: 0.42,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.82, 0.22),
                emissive: LinearRgba::rgb(2.2, 1.5, 0.28),
                unlit: true,
                ..default()
            }),
            materials.add(StandardMaterial { base_color: Color::srgb(0.18, 0.52, 0.20), perceptual_roughness: 0.88, ..default() }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.64, 0.16),
                emissive: LinearRgba::rgb(0.6, 0.35, 0.05),
                metallic: 0.45,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.68, 0.24, 1.0),
                emissive: LinearRgba::rgb(1.4, 0.45, 2.5),
                unlit: true,
                ..default()
            }),
        ],
    };

    populate_empty_city_world(&mut data);

    // Flat empty Cedar Rapids ground + faint ghostly road lines showing what used to be here
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.21, 0.24),
        perceptual_roughness: 0.90,
        ..default()
    });
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.13, 0.15),
        perceptual_roughness: 0.94,
        ..default()
    });
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(ground_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(0.0, -0.5, 0.0))
            .with_scale(Vec3::new(66.0, 1.0, 66.0)),
    ));
    for z in [-8.0_f32, 8.0] {
        commands.spawn((
            LevelEntity,
            CraftLevelEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(road_mat.clone()),
            Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(0.0, 0.015, z))
                .with_scale(Vec3::new(62.0, 0.02, 5.2)),
        ));
    }

    // Glowing Blueprint Zone 1: Pub 217 Footprint (-9..=-2, -6..=1)
    let pub_pad_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.72, 0.18, 0.34),
        emissive: LinearRgba::rgb(1.1, 0.7, 0.1),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(pub_pad_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(-5.5, 0.02, -2.5))
            .with_scale(Vec3::new(8.2, 0.03, 8.2)),
    ));

    // Glowing Blueprint Zone 2: Stan's New House Footprint (3..=11, -6..=2)
    let house_pad_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.24, 0.92, 1.0, 0.32),
        emissive: LinearRgba::rgb(0.25, 1.1, 1.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftHouseOutline,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(house_pad_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(7.0, 0.02, -2.0))
            .with_scale(Vec3::new(9.2, 0.03, 9.2)),
        if stats.craft_pub217_rebuilt { Visibility::Inherited } else { Visibility::Hidden },
    ));

    // Derek's Blueprint Crafting Workbench at (-3.2, 0, 3.0)
    let bench_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.24, 0.12),
        perceptual_roughness: 0.65,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(bench_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(-3.2, 0.5, 3.0))
            .with_scale(Vec3::new(1.2, 1.0, 1.2)),
        PlatformBox { half_extents: Vec3::new(0.6, 0.5, 0.6) },
    ));

    // Spawn all voxel blocks from persistent CraftWorldData
    let existing_blocks: Vec<((i32, i32, i32), BlockKind)> =
        data.blocks.iter().map(|(&c, &k)| (c, k)).collect();
    for (coord, kind) in existing_blocks {
        spawn_voxel_entity(&mut commands, &assets, coord, kind);
    }
    if stats.craft_pub217_rebuilt {
        spawn_pub217_rebuilt_interior(&mut commands, &mut meshes, &mut materials);
    }
    commands.insert_resource(assets);

    // Mysterious Black Hole Return Portal in South Plaza
    let portal_void = materials.add(StandardMaterial { base_color: Color::BLACK, ..default() });
    let portal_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.70, 0.26, 1.0),
        emissive: LinearRgba::rgb(2.8, 0.85, 4.4),
        unlit: true,
        ..default()
    });
    let portal_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.22),
        emissive: LinearRgba::rgb(2.6, 1.7, 0.35),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            Transform::from_translation(CRAFT_PORTAL_POS),
            Visibility::Inherited,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(1.15))),
                MeshMaterial3d(portal_void),
                Transform::from_xyz(0.0, 1.9, 0.0),
            ));
            p.spawn((
                CraftPortalRing,
                Mesh3d(meshes.add(Torus::new(0.16, 1.55))),
                MeshMaterial3d(portal_purple),
                Transform::from_xyz(0.0, 1.9, 0.0),
            ));
            p.spawn((
                CraftPortalRing,
                Mesh3d(meshes.add(Torus::new(0.09, 1.95))),
                MeshMaterial3d(portal_gold.clone()),
                Transform::from_xyz(0.0, 1.9, 0.0).with_rotation(Quat::from_rotation_x(0.75)),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.75, 0.08))),
                MeshMaterial3d(portal_gold),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
        });

    // Translucent Highlight Box showing mined/placed voxel cell
    let cursor_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.94, 0.35, 0.32),
        emissive: LinearRgba::rgb(0.6, 0.5, 0.08),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftCursorVisual,
        Mesh3d(unit_box),
        MeshMaterial3d(cursor_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN).with_scale(Vec3::splat(1.05)),
        Visibility::Hidden,
    ));

    // Craft NPCs: Maria, Mike, Derek, TJ, Nate, Glenn
    let scale = city_body_scale(1.8);
    let derek_m = make_derek_materials(&mut materials);
    let nate_m = make_nate_materials(&mut materials);
    let glenn_m = make_glenn_materials(&mut materials);
    for role in [
        CraftNpcRole::Maria,
        CraftNpcRole::Mike,
        CraftNpcRole::Derek,
        CraftNpcRole::Tj,
        CraftNpcRole::Nate,
        CraftNpcRole::Glenn,
    ] {
        let (pos, yaw) = npc_position(role, stats.craft_pub217_rebuilt);
        let rig_scale = match role {
            CraftNpcRole::Maria => PUB217_SCALE,
            CraftNpcRole::Glenn => scale * 0.89,
            _ => scale * 0.85,
        };
        commands
            .spawn((
                LevelEntity,
                CraftLevelEntity,
                CraftNpc { role },
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(rig_scale)),
                Visibility::Inherited,
            ))
            .with_children(|p| match role {
                CraftNpcRole::Maria => spawn_bartender_rig(p, &mut meshes, &mut materials),
                CraftNpcRole::Mike => spawn_mike_rig(p, &mut meshes, &mut materials),
                CraftNpcRole::Derek => spawn_derek_rig(p, &mut meshes, &derek_m, false),
                CraftNpcRole::Tj => spawn_tj_rig(p, &mut meshes, &mut materials, false, false),
                CraftNpcRole::Nate => spawn_nate_rig(p, &mut meshes, &nate_m, false),
                CraftNpcRole::Glenn => spawn_glenn_rig(p, &mut meshes, &glenn_m),
            });
    }

    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        PointLight {
            color: Color::srgb(0.92, 0.88, 1.0),
            intensity: 200_000.0,
            range: 90.0,
            ..default()
        },
        Transform::from_translation(CRAFT_LEVEL_ORIGIN + Vec3::new(0.0, 22.0, 0.0)),
    ));

    // Spawn player at south plaza facing north toward Pub 217 footprint
    if let Ok((mut p_tf, mut player, mut p_vis)) = player_q.get_single_mut() {
        p_tf.translation = CRAFT_SPAWN_POS;
        player.facing_yaw = 0.0;
        p_tf.rotation = Quat::IDENTITY;
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.grounded = true;
        *p_vis = Visibility::Visible;
    }
    orbit.yaw = 0.0;
    orbit.pitch = 0.34;
    orbit.distance = 8.2;
    orbit.current_dist = 8.2;
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        cam_tf.translation = CRAFT_SPAWN_POS + Vec3::new(0.0, 3.8, 7.6);
        cam_tf.look_at(CRAFT_SPAWN_POS + Vec3::new(0.0, 1.6, 0.0), Vec3::Y);
    }

    spawn_craft_hud(&mut commands);
    stats.hint_text = if stats.craft_pub217_rebuilt {
        "Pub 217 Rebuilt! Drink beer, visit Casino, build Stan's New House, or enter South Portal!"
            .to_string()
    } else {
        "EMPTY CEDAR RAPIDS! Mine trees/rubble [L-Click] & Place 18 blocks [R-Click] in Pub 217 plot (or Talk to Derek [F/E])!"
            .to_string()
    };
    stats.hint_bump = stats.hint_bump.wrapping_add(1);
}

fn spawn_craft_hud(commands: &mut Commands) {
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            CraftHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(12.0)),
                ..default()
            },
            GlobalZIndex(18),
        ))
        .with_children(|root| {
            // Top quest tracker
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.04, 0.12, 0.86)),
                BorderColor(Color::srgb(0.72, 0.42, 1.0)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|q| {
                q.spawn((
                    CraftQuestText,
                    Text::new("QUEST 1: REBUILD PUB 217 (0/18)"),
                    TextFont { font_size: 13.5, ..default() },
                    TextColor(Color::srgb(1.0, 0.88, 0.35)),
                ));
            });

            // Center tiny crosshair
            root.spawn((
                CraftCrosshair,
                Text::new("+"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.72)),
            ));

            // Bottom column: interact pill + 6-slot Minecraft hotbar
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|bot| {
                bot.spawn((
                    CraftPromptPill,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.06, 0.20, 0.94)),
                    BorderColor(Color::srgb(0.45, 0.95, 0.65)),
                    BorderRadius::all(Val::Px(999.0)),
                ))
                .with_children(|p| {
                    p.spawn((
                        CraftPromptText,
                        Text::new("INTERACT [F/E]"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });

                bot.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.12, 0.88)),
                    BorderColor(Color::srgba(0.95, 0.78, 0.28, 0.8)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|bar| {
                    for (idx, kind) in HOTBAR_BLOCKS.iter().enumerate() {
                        bar.spawn((
                            CraftHotbarBtn(idx),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.12, 0.14, 0.24, 0.92)),
                            BorderColor(Color::srgba(0.6, 0.7, 0.88, 0.45)),
                            BorderRadius::all(Val::Px(6.0)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                CraftHotbarText(idx),
                                Text::new(format!("{}:{} x0", idx + 1, kind.short_name())),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });
            });
        });
}

fn exit_craft_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    q: Query<Entity, With<CraftLevelEntity>>,
) {
    crate::audio::set_touch_craft_mode(false);
    stats.in_craft_level = false;
    stats.craft_modal_open = false;
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct VoxelRayQueries<'w, 's> {
    player_q: Query<'w, 's, &'static Transform, (With<Player>, Without<VoxelBlock>, Without<CraftCursorVisual>)>,
    cam_q: Query<'w, 's, &'static Transform, (With<Camera3d>, Without<Player>, Without<VoxelBlock>, Without<CraftCursorVisual>)>,
    block_q: Query<'w, 's, (Entity, &'static VoxelBlock), (Without<Player>, Without<CraftCursorVisual>, Without<CraftDebris>)>,
    cursor_q: Query<'w, 's, (&'static mut Transform, &'static mut Visibility), (With<CraftCursorVisual>, Without<Player>, Without<VoxelBlock>)>,
    debris_q: Query<'w, 's, (Entity, &'static mut Transform, &'static mut CraftDebris), (Without<Player>, Without<VoxelBlock>, Without<CraftCursorVisual>)>,
    hotbar_btn_q: Query<'w, 's, (&'static Interaction, &'static CraftHotbarBtn), Changed<Interaction>>,
}

#[allow(clippy::too_many_arguments)]
fn update_craft_voxels(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
    mut data: ResMut<CraftWorldData>,
    assets: Option<Res<CraftBlockAssets>>,
    mut rq: VoxelRayQueries,
    mut prev_block_btn: Local<bool>,
    mut act_cd: Local<f32>,
) {
    let dt = time.delta_secs().min(0.05);
    *act_cd = (*act_cd - dt).max(0.0);

    // Update voxel break particles
    for (e, mut tf, mut d) in &mut rq.debris_q {
        d.age += dt;
        if d.age >= 0.65 {
            commands.entity(e).despawn();
            continue;
        }
        d.vel.y -= 22.0 * dt;
        tf.translation += d.vel * dt;
        tf.rotate_x(d.ang_vel.x * dt);
        tf.rotate_y(d.ang_vel.y * dt);
        if tf.translation.y < 0.1 {
            tf.translation.y = 0.1;
            d.vel.y = -d.vel.y * 0.3;
        }
    }

    if stats.craft_modal_open {
        for (_, mut vis) in &mut rq.cursor_q {
            *vis = Visibility::Hidden;
        }
        return;
    }

    // Slot selection via keys [1..6], [C], hotbar clicks, or touch SLOT button
    for (i, code) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ]
    .iter()
    .enumerate()
    {
        if keys.just_pressed(*code) {
            data.selected_slot = i;
        }
    }
    if keys.just_pressed(KeyCode::KeyC) || crate::audio::take_touch_weapon_cycle() {
        data.selected_slot = (data.selected_slot + 1) % 6;
    }
    for (inter, btn) in &rq.hotbar_btn_q {
        if *inter == Interaction::Pressed {
            data.selected_slot = btn.0.min(5);
        }
    }

    let Ok(p_tf) = rq.player_q.get_single() else {
        return;
    };
    let Ok(cam_tf) = rq.cam_q.get_single() else {
        return;
    };

    // Raymarch from head along camera view
    let origin = p_tf.translation + Vec3::Y * 1.45;
    let dir = *cam_tf.forward();
    let mut hit_cell: Option<(i32, i32, i32)> = None;
    let mut place_cell: Option<(i32, i32, i32)> = None;
    let mut last_empty: Option<(i32, i32, i32)> = None;
    let mut dist = 1.1_f32;
    while dist <= 8.5 {
        let sample = origin + dir * dist;
        if sample.y <= 0.02 {
            let g = world_to_grid(Vec3::new(sample.x, 0.2, sample.z));
            let ground_c = (g.0, 0, g.2);
            if !data.blocks.contains_key(&ground_c) {
                place_cell = Some(ground_c);
            }
            break;
        }
        let g = world_to_grid(sample);
        if data.blocks.contains_key(&g) {
            hit_cell = Some(g);
            place_cell = last_empty;
            break;
        }
        last_empty = Some(g);
        dist += 0.22;
    }

    // Highlight targeted block or ground placement spot
    if let Ok((mut c_tf, mut c_vis)) = rq.cursor_q.get_single_mut() {
        if let Some(c) = hit_cell.or(place_cell) {
            c_tf.translation = grid_to_world(c);
            *c_vis = Visibility::Inherited;
        } else {
            *c_vis = Visibility::Hidden;
        }
    }

    let want_mine = mouse.just_pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::KeyJ)
        || keys.just_pressed(KeyCode::KeyX)
        || crate::audio::take_touch_attack();
    let touch_blk_now = crate::audio::touch_block_held();
    let touch_blk_tap = touch_blk_now && !*prev_block_btn;
    *prev_block_btn = touch_blk_now;
    let want_place = mouse.just_pressed(MouseButton::Right)
        || keys.just_pressed(KeyCode::KeyR)
        || keys.just_pressed(KeyCode::KeyB)
        || touch_blk_tap;

    let Some(assets) = assets else {
        return;
    };

    if want_mine && *act_cd <= 0.0 {
        if let Some(c) = hit_cell {
            if let Some(kind) = data.blocks.remove(&c) {
                *act_cd = 0.14;
                for (e, vb) in &rq.block_q {
                    if vb.coord == c {
                        commands.entity(e).despawn_recursive();
                        break;
                    }
                }
                match kind {
                    BlockKind::Leaves => {
                        data.inventory[BlockKind::Wood as usize] += 1;
                    }
                    BlockKind::TendyOre => {
                        data.inventory[BlockKind::TendyGold as usize] += 2;
                        data.inventory[BlockKind::Neon217 as usize] += 1;
                    }
                    BlockKind::SupplyCube => {
                        for n in &mut data.inventory {
                            *n += 2;
                        }
                        stats.score += 125;
                        stats.hint_text = "BLACK HOLE SUPPLY CUBE! +2 OF EVERY BLOCK!".to_string();
                        stats.hint_bump = stats.hint_bump.wrapping_add(1);
                    }
                    k if (k as usize) < 6 => {
                        data.inventory[k as usize] += 1;
                    }
                    _ => {}
                }
                for i in 0..6_usize {
                    let a = i as f32 * 1.047 + c.0 as f32;
                    commands.spawn((
                        LevelEntity,
                        CraftLevelEntity,
                        CraftDebris {
                            vel: Vec3::new(a.cos() * 3.8, 4.2 + (i % 2) as f32 * 1.5, a.sin() * 3.8),
                            ang_vel: Vec3::new(7.0, 9.0, -6.0),
                            age: 0.0,
                        },
                        Mesh3d(assets.cube_mesh.clone()),
                        MeshMaterial3d(assets.mats[kind as usize].clone()),
                        Transform::from_translation(grid_to_world(c)).with_scale(Vec3::splat(0.24)),
                    ));
                }
                play_sfx("block_break");
                stats.score += 25;
                recount_quest_blocks(&mut data);
            }
        }
    } else if want_place && *act_cd <= 0.0 {
        if let Some(c) = place_cell {
            let slot = data.selected_slot.min(5);
            let wp = grid_to_world(c);
            let flat_p = Vec2::new(wp.x - p_tf.translation.x, wp.z - p_tf.translation.z).length();
            let overlaps_player = flat_p < 0.72 && (wp.y - (p_tf.translation.y + 0.9)).abs() < 1.1;
            if (-24..=24).contains(&c.0)
                && (0..=10).contains(&c.1)
                && (-24..=24).contains(&c.2)
                && !data.blocks.contains_key(&c)
                && !overlaps_player
                && data.inventory[slot] > 0
            {
                *act_cd = 0.14;
                data.inventory[slot] -= 1;
                let kind = HOTBAR_BLOCKS[slot];
                data.blocks.insert(c, kind);
                spawn_voxel_entity(&mut commands, &assets, c, kind);
                play_sfx("block_place");
                stats.score += 35;
                recount_quest_blocks(&mut data);
            } else if data.inventory[slot] == 0 {
                play_sfx("hack_deny");
                stats.hint_text = "Out of that block -- mine trees/rubble or Talk to Derek to craft!"
                    .to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
        }
    }
}

fn update_craft_supply_drops(
    mut commands: Commands,
    time: Res<Time>,
    mut data: ResMut<CraftWorldData>,
    mut stats: ResMut<PlayerStats>,
    assets: Option<Res<CraftBlockAssets>>,
) {
    let dt = time.delta_secs().min(0.05);
    data.supply_timer -= dt;
    if data.supply_timer > 0.0 {
        return;
    }
    data.supply_timer = 18.0;
    let Some(assets) = assets else {
        return;
    };
    let t_i = (time.elapsed_secs() * 7.0) as i32;
    let gx = ((t_i * 5) % 15) - 7;
    let gz = 8 + ((t_i * 3) % 7);
    let c = (gx, 0, gz);
    if !data.blocks.contains_key(&c) {
        data.blocks.insert(c, BlockKind::SupplyCube);
        spawn_voxel_entity(&mut commands, &assets, c, BlockKind::SupplyCube);
        play_sfx("portal_warp");
        stats.hint_text = "BLACK HOLE RIFT DROPPED A TENDY SUPPLY CUBE IN THE PLAZA!".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn check_craft_quests(
    mut commands: Commands,
    mut data: ResMut<CraftWorldData>,
    mut stats: ResMut<PlayerStats>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut npc_q: Query<(&mut Transform, &CraftNpc)>,
    mut house_outline_q: Query<&mut Visibility, With<CraftHouseOutline>>,
) {
    if stats.craft_pub217_rebuilt && !data.pub217_decor_spawned {
        data.pub217_decor_spawned = true;
        spawn_pub217_rebuilt_interior(&mut commands, &mut meshes, &mut materials);
        for (mut tf, npc) in &mut npc_q {
            let (pos, yaw) = npc_position(npc.role, true);
            tf.translation = pos;
            tf.rotation = Quat::from_rotation_y(yaw);
        }
        for mut v in &mut house_outline_q {
            *v = Visibility::Inherited;
        }
    }

    if !stats.craft_pub217_rebuilt && data.pub_blocks_placed >= PUB217_TARGET_BLOCKS {
        stats.craft_pub217_rebuilt = true;
        stats.score += 2500;
        stats.add_tendy_wendies(25);
        play_sfx("hack_success");
        play_sfx("powerup");
        speak_voice(
            "Pub 217 is back baby! Come grab a cold beer and let's craft Stan a mansion next!",
            "bartender",
        );
        stats.hint_text =
            "PUB 217 REBUILT! (+2500 | +25 TW) Beer & Casino Open! Next: Build Stan's House!"
                .to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }

    if stats.craft_pub217_rebuilt && !data.house_completed && data.house_blocks_placed >= HOUSE_TARGET_BLOCKS {
        data.house_completed = true;
        stats.score += 5000;
        stats.add_tendy_wendies(50);
        play_sfx("powerup");
        speak_voice(
            "Whoa!! Stan and the boys have a new crib in the black hole! Legend!",
            "stan",
        );
        stats.hint_text =
            "STAN & THE BOYS' NEW HOUSE COMPLETE! (+5000 | +50 TW) Keep building or use the South Portal!"
                .to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_craft_npcs_and_portals(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    data: Res<CraftWorldData>,
    mut prompt: ResMut<CraftInteractPrompt>,
    mut next_state: SceneTransition,
    player_q: Query<&Transform, (With<Player>, Without<CraftPortalRing>, Without<CraftNpc>, Without<CraftBeerTowerSpot>)>,
    mut ring_q: Query<&mut Transform, (With<CraftPortalRing>, Without<Player>)>,
    npc_q: Query<(&Transform, &CraftNpc), (Without<Player>, Without<CraftPortalRing>)>,
    tower_q: Query<&Transform, (With<CraftBeerTowerSpot>, Without<Player>, Without<CraftPortalRing>, Without<CraftNpc>)>,
    prompt_btn_q: Query<&Interaction, (With<CraftPromptPill>, Changed<Interaction>)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let dt = time.delta_secs().min(0.05);
    for mut tf in &mut ring_q {
        tf.rotate_z(2.4 * dt);
        tf.rotate_y(1.2 * dt);
    }

    if stats.craft_modal_open {
        prompt.target = None;
        prompt.tapped = false;
        return;
    }

    let Ok(p_tf) = player_q.get_single() else {
        prompt.target = None;
        return;
    };
    let p = p_tf.translation;

    // Portal collision / prompt
    let d_portal = Vec2::new(p.x - CRAFT_PORTAL_POS.x, p.z - CRAFT_PORTAL_POS.z).length();
    let mut best: Option<(CraftPromptTarget, f32)> = None;
    if d_portal < 3.4 {
        best = Some((CraftPromptTarget::Portal, d_portal));
    }
    if let Ok(t_tf) = tower_q.get_single() {
        let d_tower = Vec2::new(p.x - t_tf.translation.x, p.z - t_tf.translation.z).length();
        if d_tower < 2.8 && best.map_or(true, |(_, bd)| d_tower < bd) {
            best = Some((CraftPromptTarget::BeerTower, d_tower));
        }
    }
    for (n_tf, npc) in &npc_q {
        let d_npc = Vec2::new(p.x - n_tf.translation.x, p.z - n_tf.translation.z).length();
        if d_npc < 3.0 && best.map_or(true, |(_, bd)| d_npc < bd) {
            best = Some((CraftPromptTarget::Npc(npc.role), d_npc));
        }
    }
    prompt.target = best.map(|(t, _)| t);

    let tapped_pill = prompt_btn_q.iter().any(|i| *i == Interaction::Pressed);
    let interact = keys.just_pressed(KeyCode::KeyF)
        || keys.just_pressed(KeyCode::KeyE)
        || tapped_pill
        || std::mem::take(&mut prompt.tapped);

    if d_portal < 1.75 || (prompt.target == Some(CraftPromptTarget::Portal) && interact) {
        play_sfx("portal_warp");
        stats.craft_return_city = true;
        stats.saved_city_pos = PUB217_ANCHOR + Vec3::new(4.2, 0.0, -4.5);
        next_state.set(GameState::CityPlay);
        return;
    }
    if !interact {
        return;
    }

    match prompt.target {
        Some(CraftPromptTarget::BeerTower) => {
            play_sfx("estus_drink");
            stats.drunk_timer = 14.0;
            stats.hearts = stats.max_hearts;
            stats.score += 150;
            stats.hint_text = "CRACKED A COLD ONE IN REBUILT PUB 217! FULL HEARTS!".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        Some(CraftPromptTarget::Npc(CraftNpcRole::Mike)) => {
            play_sfx("smoke_puff");
            stats.high_timer = 14.0;
            stats.hearts = stats.max_hearts;
            speak_voice("Here bro, hit this black-hole kush while we rebuild 217!", "mike");
            stats.hint_text = "SMOKED WITH MIKE! Full Hearts & Groovy Vibes!".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        Some(CraftPromptTarget::Npc(CraftNpcRole::Tj)) => {
            speak_voice("Still saying we watched The Odyssey in the wrong format, bro.", "tj");
            stats.hint_text = "TJ: \"Mine glass & stone to finish Pub 217, Stan!\"".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        Some(CraftPromptTarget::Npc(CraftNpcRole::Nate)) => {
            speak_voice("Wait till we rebuild Pub 217 so Maria can pour us ten more beers!", "nate");
            stats.hint_text = "NATE: \"Build inside the glowing amber Pub 217 outline!\"".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        Some(CraftPromptTarget::Npc(CraftNpcRole::Glenn)) => {
            speak_voice("Bro, 10,000 women fit in one restroom and launched us here!", "glenn");
            stats.hint_text = "GLENN: \"Purple supply cubes drop every 18s in the plaza!\"".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        Some(CraftPromptTarget::Npc(role @ (CraftNpcRole::Derek | CraftNpcRole::Maria))) => {
            stats.craft_modal_open = true;
            set_touch_playing(false);
            if let Ok(mut win) = window_q.get_single_mut() {
                win.cursor_options.grab_mode = CursorGrabMode::None;
                win.cursor_options.visible = true;
            }
            if role == CraftNpcRole::Derek {
                prompt.modal_is_derek = true;
                speak_voice("Pick a craft recipe or engage my hyperfocus blueprint!", "derek");
                spawn_derek_craft_modal(&mut commands, &stats, &data);
            } else {
                prompt.modal_is_derek = false;
                if stats.craft_pub217_rebuilt {
                    speak_voice("Welcome back to Pub 217! Cold beer and Casino trips are back!", "bartender");
                } else {
                    speak_voice("Place 18 blocks in the Pub 217 outline, or talk to Derek!", "bartender");
                }
                spawn_maria_craft_modal(&mut commands, &stats);
            }
        }
        _ => {}
    }
}

fn spawn_derek_craft_modal(commands: &mut Commands, stats: &PlayerStats, data: &CraftWorldData) {
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            CraftModalRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.72)),
            GlobalZIndex(850),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(520.0),
                    max_width: Val::Percent(92.0),
                    padding: UiRect::all(Val::Px(20.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.06, 0.14, 0.96)),
                BorderColor(Color::srgb(0.45, 0.95, 0.65)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("DEREK'S CRAFTING & HYPERFOCUS TABLE"),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::srgb(0.48, 1.0, 0.72)),
                ));
                card.spawn((
                    Text::new(format!(
                        "Inventory -- Brick:{} Wood:{} Stone:{} Glass:{} Tendy:{} Neon:{}",
                        data.inventory[0], data.inventory[1], data.inventory[2],
                        data.inventory[3], data.inventory[4], data.inventory[5]
                    )),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.92, 0.94, 0.98)),
                ));
                for (act, label, bg) in [
                    (CraftModalAction::CraftBrick, "CRAFT +2 BRICK (Costs 2 STONE)", Color::srgb(0.42, 0.16, 0.16)),
                    (CraftModalAction::CraftGlass, "CRAFT +2 GLASS (Costs 1 WOOD)", Color::srgb(0.14, 0.38, 0.52)),
                    (CraftModalAction::CraftNeon, "CRAFT +2 NEON217 (Costs 1 TENDY GOLD)", Color::srgb(0.55, 0.36, 0.08)),
                    (CraftModalAction::FreeScrapKit, "SCAVENGE VOID SCRAP (+3 BRICK, +3 WOOD)", Color::srgb(0.24, 0.24, 0.38)),
                ] {
                    card.spawn((
                        act,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor(Color::srgba(1.0, 0.9, 0.4, 0.55)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(label), TextFont { font_size: 13.5, ..default() }, TextColor(Color::WHITE)));
                    });
                }
                if !stats.craft_pub217_rebuilt {
                    card.spawn((
                        CraftModalAction::DerekHyperfocusPub217,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(11.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.38, 0.12, 0.68)),
                        BorderColor(Color::srgb(1.0, 0.84, 0.30)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("DEREK HYPERFOCUS: AUTO-REBUILD PUB 217 NOW!"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(1.0, 0.94, 0.45)),
                        ));
                    });
                }
                card.spawn((
                    CraftModalAction::Close,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.22, 0.14, 0.22)),
                    BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
                    BorderRadius::all(Val::Px(8.0)),
                ))
                .with_children(|b| {
                    b.spawn((Text::new("CLOSE [ESC]"), TextFont { font_size: 13.0, ..default() }, TextColor(Color::WHITE)));
                });
            });
        });
}

fn spawn_maria_craft_modal(commands: &mut Commands, stats: &PlayerStats) {
    commands
        .spawn((
            LevelEntity,
            CraftLevelEntity,
            CraftModalRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.72)),
            GlobalZIndex(850),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(490.0),
                    max_width: Val::Percent(92.0),
                    padding: UiRect::all(Val::Px(20.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.11, 0.04, 0.10, 0.96)),
                BorderColor(Color::srgb(0.98, 0.78, 0.30)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(if stats.craft_pub217_rebuilt {
                        "MARIA -- PUB 217 IS OPEN!"
                    } else {
                        "MARIA -- PUB 217 IN RUINS"
                    }),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.84, 0.30)),
                ));
                card.spawn((
                    Text::new(if stats.craft_pub217_rebuilt {
                        "\"You boys rebuilt 217! Beer taps and the Casino warp are back online. Now build Stan's new house across the street!\""
                    } else {
                        "\"The black hole wiped 217 off the map! Place 18 blocks in the glowing amber Pub 217 outline -- or talk to Derek right next to me!\""
                    }),
                    TextFont { font_size: 14.0, ..default() },
                    TextColor(Color::WHITE),
                ));
                if stats.craft_pub217_rebuilt {
                    card.spawn((
                        CraftModalAction::DrinkBeer,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)), border: UiRect::all(Val::Px(1.5)), ..default() },
                        BackgroundColor(Color::srgb(0.68, 0.38, 0.08)),
                        BorderColor(Color::srgb(1.0, 0.86, 0.32)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("DRINK ICE-COLD BEER (Full Hearts +TW!)"), TextFont { font_size: 14.0, ..default() }, TextColor(Color::WHITE)));
                    });
                    card.spawn((
                        CraftModalAction::CasinoFromPub,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)), border: UiRect::all(Val::Px(1.5)), ..default() },
                        BackgroundColor(Color::srgb(0.34, 0.12, 0.62)),
                        BorderColor(Color::srgb(0.84, 0.45, 1.0)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("ORDER 10 BEERS -> TENDY PALACE CASINO"), TextFont { font_size: 14.0, ..default() }, TextColor(Color::WHITE)));
                    });
                    card.spawn((
                        CraftModalAction::FreeScrapKit,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)), border: UiRect::all(Val::Px(1.5)), ..default() },
                        BackgroundColor(Color::srgb(0.10, 0.40, 0.28)),
                        BorderColor(Color::srgb(0.42, 0.95, 0.65)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("BAR BUILDER SUPPLIES (+3 BRICK, +3 WOOD)"), TextFont { font_size: 13.5, ..default() }, TextColor(Color::WHITE)));
                    });
                }
                card.spawn((
                    CraftModalAction::Close,
                    Button,
                    Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)), border: UiRect::all(Val::Px(1.5)), ..default() },
                    BackgroundColor(Color::srgb(0.24, 0.14, 0.20)),
                    BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
                    BorderRadius::all(Val::Px(8.0)),
                ))
                .with_children(|b| {
                    b.spawn((Text::new("CLOSE [ESC]"), TextFont { font_size: 13.0, ..default() }, TextColor(Color::WHITE)));
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn handle_craft_modal_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut stats: ResMut<PlayerStats>,
    mut data: ResMut<CraftWorldData>,
    prompt: Res<CraftInteractPrompt>,
    assets: Option<Res<CraftBlockAssets>>,
    mut next_state: SceneTransition,
    btn_q: Query<(&Interaction, &CraftModalAction), Changed<Interaction>>,
    modal_q: Query<Entity, With<CraftModalRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !stats.craft_modal_open {
        return;
    }

    let mut chosen = None;
    if keys.just_pressed(KeyCode::Escape) {
        chosen = Some(CraftModalAction::Close);
    } else {
        for (inter, act) in &btn_q {
            if *inter == Interaction::Pressed {
                chosen = Some(*act);
                break;
            }
        }
    }
    let Some(act) = chosen else {
        return;
    };

    match act {
        CraftModalAction::CraftBrick => {
            if data.inventory[BlockKind::Stone as usize] >= 2 {
                data.inventory[BlockKind::Stone as usize] -= 2;
                data.inventory[BlockKind::Brick as usize] += 2;
                play_sfx("block_place");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::CraftGlass => {
            if data.inventory[BlockKind::Wood as usize] >= 1 {
                data.inventory[BlockKind::Wood as usize] -= 1;
                data.inventory[BlockKind::Glass as usize] += 2;
                play_sfx("block_place");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::CraftNeon => {
            if data.inventory[BlockKind::TendyGold as usize] >= 1 {
                data.inventory[BlockKind::TendyGold as usize] -= 1;
                data.inventory[BlockKind::Neon217 as usize] += 2;
                play_sfx("block_place");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::FreeScrapKit => {
            data.inventory[BlockKind::Brick as usize] += 3;
            data.inventory[BlockKind::Wood as usize] += 3;
            play_sfx("powerup");
        }
        CraftModalAction::DerekHyperfocusPub217 => {
            if let Some(assets) = assets {
                // Auto-build cozy Brick/Wood/Glass/Neon217 walls around Pub 217 foundation!
                for gx in -9..=-2 {
                    for gz in -6..=1 {
                        let edge = gx == -9 || gx == -2 || gz == -6 || gz == 1;
                        let door = gx == -2 && (gz == -2 || gz == -1);
                        if edge && !door {
                            for gy in 0..=2 {
                                let k = if gy == 1 && (gx == -5 || gz == -3) {
                                    BlockKind::Glass
                                } else if gy == 2 && gx == -2 {
                                    BlockKind::Neon217
                                } else {
                                    BlockKind::Brick
                                };
                                let c = (gx, gy, gz);
                                if !data.blocks.contains_key(&c) {
                                    data.blocks.insert(c, k);
                                    spawn_voxel_entity(&mut commands, &assets, c, k);
                                }
                            }
                        }
                    }
                }
                recount_quest_blocks(&mut data);
            }
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            return;
        }
        CraftModalAction::DrinkBeer => {
            play_sfx("estus_drink");
            stats.drunk_timer = 14.0;
            stats.hearts = stats.max_hearts;
            stats.add_tendy_wendies(5);
            stats.hint_text = "CRACKED A COLD ONE WITH MARIA! Full Hearts +5 TW!".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        CraftModalAction::CasinoFromPub => {
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stop_voice();
            play_sfx("estus_drink");
            next_state.set(GameState::CasinoPlay);
            return;
        }
        CraftModalAction::Close => {}
    }

    // Re-open or close cleanly
    if matches!(act, CraftModalAction::Close) {
        for e in &modal_q {
            commands.entity(e).despawn_recursive();
        }
        stats.craft_modal_open = false;
        stats.casino_esc_grace = true;
        set_touch_playing(true);
        stop_voice();
        if touches.iter().next().is_none() && !crate::audio::skip_touch_ui() {
            if let Ok(mut win) = window_q.get_single_mut() {
                win.cursor_options.grab_mode = CursorGrabMode::Locked;
                win.cursor_options.visible = false;
            }
        }
    } else if matches!(
        act,
        CraftModalAction::CraftBrick
            | CraftModalAction::CraftGlass
            | CraftModalAction::CraftNeon
            | CraftModalAction::FreeScrapKit
    ) {
        for e in &modal_q {
            commands.entity(e).despawn_recursive();
        }
        if prompt.modal_is_derek {
            spawn_derek_craft_modal(&mut commands, &stats, &data);
        } else {
            spawn_maria_craft_modal(&mut commands, &stats);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_craft_hud(
    stats: Res<PlayerStats>,
    data: Res<CraftWorldData>,
    prompt: Res<CraftInteractPrompt>,
    mut quest_q: Query<&mut Text, With<CraftQuestText>>,
    mut cross_q: Query<&mut Visibility, With<CraftCrosshair>>,
    mut pill_q: Query<(&mut Node, &mut Visibility), (With<CraftPromptPill>, Without<CraftCrosshair>)>,
    mut ptext_q: Query<&mut Text, (With<CraftPromptText>, Without<CraftQuestText>)>,
    mut slot_q: Query<(&CraftHotbarBtn, &mut BorderColor, &mut BackgroundColor)>,
    mut slot_txt_q: Query<
        (&CraftHotbarText, &mut Text),
        (Without<CraftQuestText>, Without<CraftPromptText>),
    >,
) {
    if let Ok(mut qt) = quest_q.get_single_mut() {
        let next_msg = if !stats.craft_pub217_rebuilt {
            format!(
                "QUEST 1: REBUILD PUB 217 ({}/{} blocks) -- Mine trees/rubble [L-Click] & Place [R-Click] or Talk to Derek [F/E]",
                data.pub_blocks_placed.min(PUB217_TARGET_BLOCKS),
                PUB217_TARGET_BLOCKS
            )
        } else if !data.house_completed {
            format!(
                "PUB 217 REBUILT! | QUEST 2: BUILD STAN'S NEW HOUSE ({}/{} blocks) -- Cyan Foundation East",
                data.house_blocks_placed.min(HOUSE_TARGET_BLOCKS),
                HOUSE_TARGET_BLOCKS
            )
        } else {
            "PUB 217 & STAN'S HOUSE COMPLETE! Party at 217 or enter South Portal -> Cedar Rapids!"
                .to_string()
        };
        if qt.0 != next_msg {
            qt.0 = next_msg;
        }
    }

    if let Ok(mut vis) = cross_q.get_single_mut() {
        *vis = if stats.craft_modal_open {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }

    if let Ok((mut node, mut vis)) = pill_q.get_single_mut() {
        match prompt.target {
            Some(tgt) if !stats.craft_modal_open => {
                node.display = Display::Flex;
                *vis = Visibility::Inherited;
                let msg = match tgt {
                    CraftPromptTarget::Portal => "MYSTERIOUS PORTAL -> CEDAR RAPIDS [F/E]",
                    CraftPromptTarget::BeerTower => "POUR COLD BEER AT PUB 217 [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Maria) => "TALK TO MARIA [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Derek) => "CRAFT / BLUEPRINT TABLE WITH DEREK [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Mike) => "SMOKE BREAK WITH MIKE [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Tj) => "TALK TO TJ [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Nate) => "TALK TO NATE [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Glenn) => "TALK TO GLENN [F/E]",
                };
                if let Ok(mut pt) = ptext_q.get_single_mut() {
                    if pt.0 != msg {
                        pt.0 = msg.to_string();
                    }
                }
            }
            _ => {
                node.display = Display::None;
                *vis = Visibility::Hidden;
            }
        }
    }

    for (btn, mut border, mut bg) in &mut slot_q {
        let sel = btn.0 == data.selected_slot;
        border.0 = if sel {
            Color::srgb(1.0, 0.90, 0.30)
        } else {
            Color::srgba(0.55, 0.65, 0.85, 0.4)
        };
        bg.0 = if sel {
            Color::srgb(0.45, 0.24, 0.08)
        } else {
            Color::srgba(0.10, 0.12, 0.22, 0.90)
        };
    }

    for (st, mut txt) in &mut slot_txt_q {
        let i = st.0.min(5);
        let label = format!("{}:{} x{}", i + 1, HOTBAR_BLOCKS[i].short_name(), data.inventory[i]);
        if txt.0 != label {
            txt.0 = label;
        }
    }
}
