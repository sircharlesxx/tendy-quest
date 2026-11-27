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

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::audio::{
    broadcast_craft_world_sync, close_craft_sync_modal, has_craft_sync_offer,
    is_craft_sync_modal_open, is_derek_tutorial_open, is_voice_speaking, load_craft_world,
    net_event, open_craft_sync_modal, open_derek_tutorial_video, play_cutscene_music, play_sfx,
    play_start_menu_music, play_viking_chant_music, save_craft_world, set_touch_craft_mode,
    set_touch_craft_shape, set_touch_playing, speak_voice, stop_voice, take_craft_sync_accepted,
    take_craft_sync_closed, take_derek_tutorial_closed, take_touch_craft_bench,
    take_touch_craft_shape, take_touch_craft_sync, take_touch_craft_view,
};
use crate::net::{pack_craft_voxel, CRAFT_VOXEL_REMOVE, EVENT_CRAFT_VOXEL};
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
use crate::ui::{gameplay_running, HudOverlay, TutorialShowcaseText};

pub const CRAFT_LEVEL_ORIGIN: Vec3 = Vec3::new(0.0, 0.0, -2400.0);
pub const CRAFT_WALK_LIMIT: f32 = 28.0;
pub const CRAFT_SPAWN_POS: Vec3 = Vec3::new(0.0, 0.05, -2386.0);
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

    fn full_name(self) -> &'static str {
        match self {
            Self::Brick => "RED BRICK",
            Self::Wood => "WOOD LOG",
            Self::Stone => "STONE BLOCK",
            Self::Glass => "GLASS PANE",
            Self::TendyGold => "TENDY GOLD",
            Self::Neon217 => "NEON 217",
            Self::Leaves => "TREE LEAVES",
            Self::TendyOre => "TENDY ORE",
            Self::SupplyCube => "VOID SUPPLY CUBE",
        }
    }

    fn ui_color(self) -> Color {
        match self {
            Self::Brick => Color::srgb(0.68, 0.25, 0.20),
            Self::Wood => Color::srgb(0.56, 0.35, 0.18),
            Self::Stone => Color::srgb(0.54, 0.56, 0.62),
            Self::Glass => Color::srgb(0.42, 0.84, 1.0),
            Self::TendyGold => Color::srgb(0.98, 0.76, 0.18),
            Self::Neon217 => Color::srgb(1.0, 0.86, 0.24),
            Self::Leaves => Color::srgb(0.20, 0.62, 0.24),
            Self::TendyOre => Color::srgb(0.95, 0.64, 0.16),
            Self::SupplyCube => Color::srgb(0.72, 0.30, 1.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BuildShape {
    #[default]
    Single,
    Wall3x3,
    Floor3x3,
    Window3x3,
}

impl BuildShape {
    fn label(self) -> &'static str {
        match self {
            Self::Single => "1x1 BLOCK",
            Self::Wall3x3 => "3x3 WALL",
            Self::Floor3x3 => "3x3 FLOOR",
            Self::Window3x3 => "3x3 WINDOW",
        }
    }

    fn short_tag(self) -> &'static str {
        match self {
            Self::Single => "1x1",
            Self::Wall3x3 => "3x3 W",
            Self::Floor3x3 => "3x3 F",
            Self::Window3x3 => "WIN",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Single => Self::Wall3x3,
            Self::Wall3x3 => Self::Floor3x3,
            Self::Floor3x3 => Self::Window3x3,
            Self::Window3x3 => Self::Single,
        }
    }

    fn offsets(self, yaw: f32) -> Vec<((i32, i32, i32), bool)> {
        let fx = -yaw.sin();
        let fz = -yaw.cos();
        let along_x = fz.abs() >= fx.abs();
        match self {
            Self::Single => vec![((0, 0, 0), false)],
            Self::Floor3x3 => {
                let mut v = Vec::with_capacity(9);
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        v.push(((dx, 0, dz), false));
                    }
                }
                v
            }
            Self::Wall3x3 | Self::Window3x3 => {
                let is_win = matches!(self, Self::Window3x3);
                let mut v = Vec::with_capacity(9);
                for u in -1..=1 {
                    for dy in 0..=2 {
                        let c = if along_x { (u, dy, 0) } else { (0, dy, u) };
                        let glass_center = is_win && u == 0 && dy == 1;
                        v.push((c, glass_center));
                    }
                }
                v
            }
        }
    }

    fn ghost_scale_and_offset(self, yaw: f32) -> (Vec3, Vec3) {
        let fx = -yaw.sin();
        let fz = -yaw.cos();
        let along_x = fz.abs() >= fx.abs();
        match self {
            Self::Single => (Vec3::splat(1.04), Vec3::ZERO),
            Self::Floor3x3 => (Vec3::new(3.04, 1.04, 3.04), Vec3::ZERO),
            Self::Wall3x3 | Self::Window3x3 => {
                if along_x {
                    (Vec3::new(3.04, 3.04, 1.04), Vec3::new(0.0, 1.0, 0.0))
                } else {
                    (Vec3::new(1.04, 3.04, 3.04), Vec3::new(0.0, 1.0, 0.0))
                }
            }
        }
    }
}

#[derive(Resource)]
pub struct CraftWorldData {
    pub initialized: bool,
    pub blocks: HashMap<(i32, i32, i32), BlockKind>,
    pub inventory: [u32; 6],
    pub selected_slot: usize,
    pub build_shape: BuildShape,
    pub tutorial_video_shown: bool,
    pub tutorial_auto_delay: f32,
    pub pub_blocks_placed: u32,
    pub house_blocks_placed: u32,
    pub house_completed: bool,
    pub supply_timer: f32,
    pub pub217_decor_spawned: bool,
    pub target_label: String,
    pub target_has_hit: bool,
    pub target_can_place: bool,
    pub target_out_of_stock: bool,
    pub feedback_msg: String,
    pub feedback_timer: f32,
    pub needs_world_rebuild: bool,
    pub online_sync_hint_shown: bool,
}

impl Default for CraftWorldData {
    fn default() -> Self {
        Self {
            initialized: false,
            blocks: HashMap::new(),
            inventory: [12, 12, 8, 6, 4, 4],
            selected_slot: 0,
            build_shape: BuildShape::Single,
            tutorial_video_shown: false,
            tutorial_auto_delay: 0.40,
            pub_blocks_placed: 0,
            house_blocks_placed: 0,
            house_completed: false,
            supply_timer: 14.0,
            pub217_decor_spawned: false,
            target_label: String::new(),
            target_has_hit: false,
            target_can_place: false,
            target_out_of_stock: false,
            feedback_msg: String::new(),
            feedback_timer: 0.0,
            needs_world_rebuild: false,
            online_sync_hint_shown: false,
        }
    }
}

pub struct CraftPlugin;

impl Plugin for CraftPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuDioramaState>()
            .init_resource::<HouseCraftState>()
            .init_resource::<BlackHoleState>()
            .init_resource::<CraftWorldData>()
            .init_resource::<CraftInteractPrompt>()
            // Start Menu: Pub 217 3D character tease showcase
            .add_systems(OnEnter(GameState::Tutorial), enter_tutorial_menu_diorama)
            .add_systems(OnExit(GameState::Tutorial), exit_tutorial_menu_diorama)
            .add_systems(
                Update,
                update_tutorial_menu_diorama
                    .run_if(in_state(GameState::Tutorial))
                    .run_if(sim_running),
            )
            // City triggers & portal back to Craft World
            .add_systems(
                Update,
                (check_post_lexie_pub217_intro, update_city_craft_portal)
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(gameplay_running),
            )
            // Cutscene 1: Post-Lexie Pub 217 reunion & Maria's interactive 10-beer choices
            .add_systems(OnEnter(GameState::HouseCraftCutscene), enter_house_craft_cutscene)
            .add_systems(OnExit(GameState::HouseCraftCutscene), exit_house_craft_cutscene)
            .add_systems(
                Update,
                (update_house_craft_cutscene, handle_house_craft_choice_input)
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
            // Level 5: Empty Black Hole Cedar Rapids Voxel Craft World & Cross-State P2P World Sync
            .add_systems(
                Update,
                poll_craft_sync_bridge
                    .run_if(
                        in_state(GameState::Playing)
                            .or(in_state(GameState::BossFight))
                            .or(in_state(GameState::CityPlay))
                            .or(in_state(GameState::CasinoPlay))
                            .or(in_state(GameState::ClubPlay))
                            .or(in_state(GameState::CraftPlay)),
                    )
                    .run_if(gameplay_running),
            )
            .add_systems(OnEnter(GameState::CraftPlay), enter_craft_play)
            .add_systems(OnExit(GameState::CraftPlay), exit_craft_play)
            .add_systems(
                Update,
                (
                    apply_craft_net_sync,
                    update_craft_voxels,
                    update_craft_supply_drops,
                    check_craft_quests,
                    update_craft_npcs_and_portals,
                    handle_craft_modal_input,
                    update_craft_hud,
                )
                    .chain()
                    .after(poll_craft_sync_bridge)
                    .run_if(in_state(GameState::CraftPlay))
                    .run_if(gameplay_running),
            );
    }
}

// =================================================================================================
// Start Menu Diorama: Pub 217 3D Character Close-Up Tease (`GameState::Tutorial`)
// =================================================================================================

const MENU_PUB_STAGE: Vec3 = Vec3::new(0.0, 0.0, -1180.0);
const MENU_TABLE_REL: Vec3 = Vec3::new(0.0, 0.0, 0.4);
const MENU_SEAT_REL: [Vec3; 5] = [
    Vec3::new(-1.85, 0.0, 1.35),  // 0: Stan (-Z GLB)
    Vec3::new(-2.05, 0.0, -0.55), // 1: TJ (+Z)
    Vec3::new(0.0, 0.0, -1.95),   // 2: Derek (+Z)
    Vec3::new(2.05, 0.0, -0.55),  // 3: Nate (+Z)
    Vec3::new(1.85, 0.0, 1.35),   // 4: Glenn (+Z)
];
const MENU_SHOT_SECS: f32 = 4.2;

#[derive(Component)]
struct MenuDioramaEntity;

#[derive(Component)]
struct MenuDioramaBob {
    base_y: f32,
    phase: f32,
}

#[derive(Resource, Default)]
struct MenuDioramaState {
    t: f32,
    cam_look: Vec3,
    last_shot: usize,
    snapped: bool,
}

#[allow(clippy::too_many_lines)]
fn enter_tutorial_menu_diorama(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ms: ResMut<MenuDioramaState>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
    old_q: Query<Entity, With<MenuDioramaEntity>>,
) {
    for e in &old_q {
        commands.entity(e).despawn_recursive();
    }
    *ms = MenuDioramaState {
        t: 0.0,
        cam_look: Vec3::ZERO,
        last_shot: usize::MAX,
        snapped: false,
    };

    commands.insert_resource(ClearColor(Color::srgb(0.06, 0.04, 0.04)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.90, 0.80),
        brightness: 500.0,
    });
    play_start_menu_music();

    let stage = MENU_PUB_STAGE;
    let table_pos = stage + MENU_TABLE_REL;
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.14, 0.09),
        perceptual_roughness: 0.72,
        ..default()
    });
    let brick_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.14, 0.12),
        perceptual_roughness: 0.80,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.11, 0.06),
        perceptual_roughness: 0.34,
        ..default()
    });
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.06, 0.05),
        perceptual_roughness: 0.85,
        ..default()
    });
    let booth_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.10, 0.14),
        perceptual_roughness: 0.55,
        ..default()
    });
    let gold_neon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.24),
        emissive: LinearRgba::rgb(2.4, 1.55, 0.28),
        unlit: true,
        ..default()
    });
    let amber_neon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.18),
        emissive: LinearRgba::rgb(2.2, 0.85, 0.18),
        unlit: true,
        ..default()
    });

    // Warm Pub 217 interior room, brick walls, ceiling, neon 217 sign, bar & stools
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 1.0)).with_scale(Vec3::new(18.0, 0.2, 20.0)),
    ));
    for (p, s) in [
        (Vec3::new(0.0, 2.8, -6.5), Vec3::new(18.0, 5.6, 0.4)),
        (Vec3::new(-8.2, 2.8, 1.2), Vec3::new(0.4, 5.6, 20.0)),
        (Vec3::new(8.2, 2.8, 1.2), Vec3::new(0.4, 5.6, 20.0)),
        (Vec3::new(0.0, 2.8, 9.2), Vec3::new(18.0, 5.6, 0.4)),
    ] {
        commands.spawn((
            MenuDioramaEntity,
            bevy::pbr::NotShadowCaster,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick_mat.clone()),
            Transform::from_translation(stage + p).with_scale(s),
        ));
    }
    commands.spawn((
        MenuDioramaEntity,
        bevy::pbr::NotShadowCaster,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(ceiling_mat),
        Transform::from_translation(stage + Vec3::new(0.0, 5.5, 0.0)).with_scale(Vec3::new(18.0, 0.3, 19.0)),
    ));
    // Mahogany bar counter, neon Pub 217 wall signs, and cozy side booths
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(stage + Vec3::new(-5.4, 0.62, -1.6)).with_scale(Vec3::new(1.1, 1.24, 5.6)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(gold_neon),
        Transform::from_translation(stage + Vec3::new(0.0, 3.85, -6.22)).with_scale(Vec3::new(3.4, 0.72, 0.12)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(amber_neon),
        Transform::from_translation(stage + Vec3::new(-7.9, 3.1, -1.6)).with_scale(Vec3::new(0.12, 0.55, 2.8)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(booth_red),
        Transform::from_translation(stage + Vec3::new(6.8, 0.55, -2.5)).with_scale(Vec3::new(1.8, 1.1, 4.8)),
    ));

    // Round center hangout table + 7-tier Beer Tower + 8 Bash Lyte cans
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_cyl.clone()),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(table_pos + Vec3::new(0.0, 0.47, 0.0)).with_scale(Vec3::new(0.28, 0.94, 0.28)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        Mesh3d(unit_cyl),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(table_pos + Vec3::new(0.0, 0.94, 0.0)).with_scale(Vec3::new(1.42, 0.12, 1.42)),
    ));
    commands
        .spawn((
            MenuDioramaEntity,
            Transform::from_translation(table_pos + Vec3::new(0.0, 1.00, 0.0)).with_scale(Vec3::splat(0.30)),
            Visibility::Inherited,
        ))
        .with_children(|t| {
            crate::beer_fx::spawn_pub_beer_tower(t, &mut meshes, &mut materials, Vec3::ZERO);
        });

    let can_m = meshes.add(crate::beer_fx::can_mesh());
    let can_mats = crate::beer_fx::can_materials(&mut materials);
    let can_s = Vec3::splat(0.42);
    for ci in 0..8_usize {
        let a = (ci as f32 / 8.0) * TAU + 0.2;
        let cpos = table_pos + Vec3::new(a.cos() * 1.02, 1.00 + crate::beer_fx::CAN_HALF_H * 0.42, a.sin() * 1.02);
        commands.spawn((
            MenuDioramaEntity,
            Mesh3d(can_m.clone()),
            MeshMaterial3d(can_mats[ci % can_mats.len()].clone()),
            Transform::from_translation(cpos)
                .with_rotation(Quat::from_rotation_y(-a + 0.5))
                .with_scale(can_s),
        ));
    }

    // 5 Wooden Pub Chairs + all 5 Playable Characters seated around the table in Pub 217
    for rel in MENU_SEAT_REL {
        let spot = table_pos + rel;
        let cyaw = yaw_toward(spot, table_pos, false);
        commands
            .spawn((
                MenuDioramaEntity,
                Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(cyaw)),
                Visibility::Inherited,
            ))
            .with_children(|ch| {
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.48, 0.0).with_scale(Vec3::new(0.54, 0.08, 0.52)),
                ));
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.88, -0.23).with_scale(Vec3::new(0.52, 0.74, 0.08)),
                ));
                for (lx, lz) in [(-0.21, -0.21), (0.21, -0.21), (-0.21, 0.21), (0.21, 0.21)] {
                    ch.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(wood_mat.clone()),
                        Transform::from_xyz(lx, 0.24, lz).with_scale(Vec3::new(0.07, 0.48, 0.07)),
                    ));
                }
            });
    }

    let seat_positions = MENU_SEAT_REL.map(|r| table_pos + r + Vec3::Y * -0.26);
    commands.spawn((
        MenuDioramaEntity,
        MenuDioramaBob { base_y: seat_positions[0].y, phase: 0.0 },
        SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
        Transform::from_translation(seat_positions[0])
            .with_rotation(Quat::from_rotation_y(yaw_toward(seat_positions[0], table_pos, true)))
            .with_scale(Vec3::splat(0.96)),
        Visibility::Inherited,
    ));
    commands
        .spawn((
            MenuDioramaEntity,
            MenuDioramaBob { base_y: seat_positions[1].y, phase: 1.2 },
            Transform::from_translation(seat_positions[1])
                .with_rotation(Quat::from_rotation_y(yaw_toward(seat_positions[1], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_tj_rig(p, &mut meshes, &mut materials, false, false));

    let derek_m = make_derek_materials(&mut materials);
    commands
        .spawn((
            MenuDioramaEntity,
            MenuDioramaBob { base_y: seat_positions[2].y, phase: 2.4 },
            Transform::from_translation(seat_positions[2])
                .with_rotation(Quat::from_rotation_y(yaw_toward(seat_positions[2], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_derek_rig(p, &mut meshes, &derek_m, false));

    let nate_m = make_nate_materials(&mut materials);
    commands
        .spawn((
            MenuDioramaEntity,
            MenuDioramaBob { base_y: seat_positions[3].y, phase: 3.6 },
            Transform::from_translation(seat_positions[3])
                .with_rotation(Quat::from_rotation_y(yaw_toward(seat_positions[3], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_nate_rig(p, &mut meshes, &nate_m, false));

    let glenn_m = make_glenn_materials(&mut materials);
    commands
        .spawn((
            MenuDioramaEntity,
            MenuDioramaBob { base_y: seat_positions[4].y, phase: 4.8 },
            Transform::from_translation(seat_positions[4])
                .with_rotation(Quat::from_rotation_y(yaw_toward(seat_positions[4], table_pos, false)))
                .with_scale(Vec3::splat(0.94)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_glenn_rig(p, &mut meshes, &glenn_m));

    // Maria tending bar & Mike chilling beside the bar
    commands
        .spawn((
            MenuDioramaEntity,
            Transform::from_translation(stage + Vec3::new(-6.6, 0.0, -1.6))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                .with_scale(Vec3::splat(PUB217_SCALE)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_bartender_rig(p, &mut meshes, &mut materials));
    let scale = city_body_scale(1.8);
    commands
        .spawn((
            MenuDioramaEntity,
            Transform::from_translation(stage + Vec3::new(-4.0, 0.0, 2.4))
                .with_rotation(Quat::from_rotation_y(yaw_toward(stage + Vec3::new(-4.0, 0.0, 2.4), table_pos, false)))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_mike_rig(p, &mut meshes, &mut materials));

    // Warm pub chandelier + table rim fill lights so close-up faces pop crisp and inviting
    commands.spawn((
        MenuDioramaEntity,
        PointLight {
            color: Color::srgb(1.0, 0.88, 0.70),
            intensity: 190_000.0,
            range: 30.0,
            ..default()
        },
        Transform::from_translation(table_pos + Vec3::new(0.0, 4.4, 0.0)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        PointLight {
            color: Color::srgb(1.0, 0.94, 0.88),
            intensity: 130_000.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(table_pos + Vec3::new(0.0, 2.3, 2.6)),
    ));
    commands.spawn((
        MenuDioramaEntity,
        PointLight {
            color: Color::srgb(1.0, 0.86, 0.72),
            intensity: 110_000.0,
            range: 22.0,
            ..default()
        },
        Transform::from_translation(table_pos + Vec3::new(-2.8, 2.5, -2.8)),
    ));

    // Start frame 1 cleanly framed on Stan's face in Pub 217
    let stan_spot = table_pos + MENU_SEAT_REL[0];
    let stan_fwd = Vec3::new(table_pos.x - stan_spot.x, 0.0, table_pos.z - stan_spot.z).normalize_or_zero();
    let stan_look = stan_spot + Vec3::Y * 1.42;
    ms.cam_look = stan_look;
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = stan_spot + Vec3::Y * 1.54 + stan_fwd * 1.52;
        cam.look_at(stan_look, Vec3::Y);
    }
}

fn exit_tutorial_menu_diorama(
    mut commands: Commands,
    q: Query<Entity, With<MenuDioramaEntity>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
    // Restore exact Level 1 apartment lighting so START QUEST opens in the bright warm kitchen
    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 620.0,
    });
}

fn update_tutorial_menu_diorama(
    mut commands: Commands,
    time: Res<Time>,
    mut ms: ResMut<MenuDioramaState>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<MenuDioramaBob>)>,
    mut bob_q: Query<(&mut Transform, &MenuDioramaBob), Without<Camera3d>>,
    mut txt_q: Query<&mut Text, With<TutorialShowcaseText>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    for mut node in &mut hud_q {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }

    let dt = time.delta_secs().min(0.05);
    ms.t += dt;

    for (mut tf, bob) in &mut bob_q {
        tf.translation.y = bob.base_y + (ms.t * 2.2 + bob.phase).sin() * 0.022;
    }

    let table_pos = MENU_PUB_STAGE + MENU_TABLE_REL;
    let total_cycle = MENU_SHOT_SECS * 6.0;
    let cyc = ms.t % total_cycle;
    let shot = ((cyc / MENU_SHOT_SECS).floor() as usize).min(5);
    let shot_u = ((cyc % MENU_SHOT_SECS) / MENU_SHOT_SECS).clamp(0.0, 1.0);

    let (target_eye, target_look, badge_text) = if shot < 5 {
        let spot = table_pos + MENU_SEAT_REL[shot];
        let to_tbl = Vec3::new(table_pos.x - spot.x, 0.0, table_pos.z - spot.z).normalize_or_zero();
        let right = Vec3::Y.cross(to_tbl).normalize_or_zero();
        let face_y = match shot {
            0 => 1.50, // Stan
            1 => 1.51, // TJ
            2 => 1.43, // Derek
            3 => 1.41, // Nate
            _ => 1.45, // Glenn
        };
        let pan = (shot_u - 0.5) * 0.36;
        let eye = spot + Vec3::Y * (face_y + 0.04) + to_tbl * 1.52 + right * pan;
        // Aim slightly below face_y so the character's head & shoulders sit cleanly above the bottom menu card
        let look = spot + Vec3::Y * (face_y - 0.08);
        let label = match shot {
            0 => "PUB 217 -- PLAY AS STAN (THE TENDY HERO)",
            1 => "PUB 217 -- PLAY AS TJ (FORMAT CRITIC)",
            2 => "PUB 217 -- PLAY AS DEREK (MASTER ARCHITECT)",
            3 => "PUB 217 -- PLAY AS NATE (CEDAR RAPIDS LEGEND)",
            _ => "PUB 217 -- PLAY AS GLENN (RESTROOM WATCHER)",
        };
        (eye, look, label)
    } else {
        let ang = -0.35 + shot_u * 1.15;
        let eye = table_pos + Vec3::new(ang.sin() * 4.4, 2.30, ang.cos() * 4.4);
        let look = table_pos + Vec3::new(0.0, 1.10, 0.0);
        (eye, look, "PUB 217 -- PLAY ANY OF THE BOYS IN FREE ROAM!")
    };

    if shot != ms.last_shot {
        ms.last_shot = shot;
        for mut t in &mut txt_q {
            if t.0 != badge_text {
                t.0 = badge_text.to_string();
            }
        }
    }

    if !ms.snapped {
        ms.snapped = true;
        play_start_menu_music();
        commands.insert_resource(ClearColor(Color::srgb(0.06, 0.04, 0.04)));
        commands.insert_resource(AmbientLight {
            color: Color::srgb(1.0, 0.90, 0.80),
            brightness: 500.0,
        });
        ms.cam_look = target_look;
        if let Ok(mut cam) = cam_q.get_single_mut() {
            cam.translation = target_eye;
            cam.look_at(ms.cam_look, Vec3::Y);
        }
        return;
    }

    if let Ok(mut cam) = cam_q.get_single_mut() {
        if cam.translation.distance(target_eye) > 20.0 {
            ms.cam_look = target_look;
            cam.translation = target_eye;
        } else {
            let lerp_f = (dt * 3.2).min(1.0);
            ms.cam_look = ms.cam_look.lerp(target_look, lerp_f);
            cam.translation = cam.translation.lerp(target_eye, lerp_f);
        }
        cam.look_at(ms.cam_look, Vec3::Y);
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

#[derive(Component, Clone, Copy)]
enum HouseCraftChoice {
    TenBeersCasino,
    TenBeersCraft,
    FreeRoamCity,
}

#[derive(Resource, Default)]
pub struct HouseCraftState {
    beat: u8,
    beat_t: f32,
    beat0_spoken: bool,
    choice_shown: bool,
    cam_look: Vec3,
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

fn spawn_house_craft_choice_modal(commands: &mut Commands, art: Option<&crate::casino::CasinoArt>) {
    commands
        .spawn((
            HouseCraftEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.04, 0.56)),
            GlobalZIndex(80),
        ))
        .with_children(|bg| {
            bg.spawn((
                Node {
                    width: Val::Percent(94.0),
                    max_width: Val::Px(560.0),
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
                    if let Some(a) = art {
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
                                ImageNode::new(a.bartender_portrait.clone()),
                                Node {
                                    width: Val::Px(92.0),
                                    height: Val::Px(92.0),
                                    ..default()
                                },
                            ));
                        });
                    }
                    top.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        flex_shrink: 1.0,
                        ..default()
                    })
                    .with_children(|info| {
                        info.spawn((
                            Text::new("MARIA -- PUB 217 BARTENDER"),
                            TextFont { font_size: 16.5, ..default() },
                            TextColor(Color::srgb(1.0, 0.84, 0.30)),
                        ));
                        info.spawn((
                            Text::new("\"What'll it be, boys? Order 10 beers for the Casino, or order 10 beers AND craft?\""),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                });

                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        HouseCraftChoice::TenBeersCasino,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(11.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.76, 0.44, 0.08)),
                        BorderColor(Color::srgb(1.0, 0.88, 0.34)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("ORDER 10 BEERS -> CASINO [1]"),
                            TextFont { font_size: 15.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    col.spawn((
                        HouseCraftChoice::TenBeersCraft,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(11.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.38, 0.14, 0.68)),
                        BorderColor(Color::srgb(0.45, 0.96, 0.68)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("ORDER 10 BEERS & CRAFT [2]"),
                            TextFont { font_size: 15.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    col.spawn((
                        HouseCraftChoice::FreeRoamCity,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.36, 0.46)),
                        BorderColor(Color::srgba(0.62, 0.92, 1.0, 0.7)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("EXPLORE CEDAR RAPIDS FREE ROAM [ESC / 3]"),
                            TextFont { font_size: 13.5, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
                });
            });
        });
}

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
        brightness: 460.0,
    });

    let stage = HOUSE_CRAFT_STAGE;
    let table_c = stage + Vec3::new(1.5, 0.0, 0.5);
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
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.06, 0.05),
        perceptual_roughness: 0.85,
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
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)).with_scale(Vec3::new(18.0, 0.2, 18.0)),
    ));
    for (p, s) in [
        (Vec3::new(0.0, 2.8, -6.5), Vec3::new(18.0, 5.6, 0.4)),
        (Vec3::new(-7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 18.0)),
        (Vec3::new(7.5, 2.8, 0.0), Vec3::new(0.4, 5.6, 18.0)),
        (Vec3::new(0.0, 2.8, 8.0), Vec3::new(18.0, 5.6, 0.4)),
    ] {
        commands.spawn((
            HouseCraftEntity,
            bevy::pbr::NotShadowCaster,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick_mat.clone()),
            Transform::from_translation(stage + p).with_scale(s),
        ));
    }
    commands.spawn((
        HouseCraftEntity,
        bevy::pbr::NotShadowCaster,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(ceiling_mat),
        Transform::from_translation(stage + Vec3::new(0.0, 5.5, 0.0)).with_scale(Vec3::new(18.0, 0.3, 18.0)),
    ));
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
        Transform::from_translation(table_c + Vec3::new(0.0, 0.55, 0.0)).with_scale(Vec3::new(2.4, 1.1, 2.4)),
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
            Transform::from_translation(table_c + Vec3::new(0.0, 1.1, 0.0)).with_scale(Vec3::splat(0.32)),
            Visibility::Inherited,
        ))
        .with_children(|t| {
            crate::beer_fx::spawn_pub_beer_tower(t, &mut meshes, &mut materials, Vec3::ZERO);
        });
    commands.spawn((
        HouseCraftEntity,
        PointLight {
            color: Color::srgb(1.0, 0.88, 0.72),
            intensity: 165_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.5, 4.4, 0.5)),
    ));
    commands.spawn((
        HouseCraftEntity,
        PointLight {
            color: Color::srgb(1.0, 0.90, 0.80),
            intensity: 110_000.0,
            range: 20.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(-3.2, 3.2, -1.8)),
    ));

    // Cast: Stan, Derek, TJ, Nate, Glenn, Maria, Mike
    let scale = city_body_scale(1.8);
    let stan_p = stage + Vec3::new(0.4, 0.0, 2.5);
    commands.spawn((
        HouseCraftEntity,
        SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
        Transform::from_translation(stan_p)
            .with_rotation(Quat::from_rotation_y(yaw_toward(stan_p, table_c, true)))
            .with_scale(Vec3::splat(0.96)),
        Visibility::Inherited,
    ));

    let derek_mats = make_derek_materials(&mut materials);
    let derek_p = stage + Vec3::new(3.2, 0.0, 1.5);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(derek_p)
                .with_rotation(Quat::from_rotation_y(yaw_toward(derek_p, table_c, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_derek_rig(p, &mut meshes, &derek_mats, false));

    let tj_p = stage + Vec3::new(-0.9, 0.0, -0.6);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(tj_p)
                .with_rotation(Quat::from_rotation_y(yaw_toward(tj_p, table_c, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_tj_rig(p, &mut meshes, &mut materials, false, false));

    let nate_mats = make_nate_materials(&mut materials);
    let nate_p = stage + Vec3::new(1.8, 0.0, -1.6);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(nate_p)
                .with_rotation(Quat::from_rotation_y(yaw_toward(nate_p, table_c, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_nate_rig(p, &mut meshes, &nate_mats, false));

    let glenn_mats = make_glenn_materials(&mut materials);
    let glenn_p = stage + Vec3::new(3.5, 0.0, -0.8);
    commands
        .spawn((
            HouseCraftEntity,
            Transform::from_translation(glenn_p)
                .with_rotation(Quat::from_rotation_y(yaw_toward(glenn_p, table_c, false)))
                .with_scale(Vec3::splat(0.94)),
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

    let init_look = stan_p + Vec3::Y * 1.56;
    cs.cam_look = init_look;
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = stage + Vec3::new(1.2, 1.68, 1.1);
        cam.look_at(init_look, Vec3::Y);
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
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_house_craft_cutscene(
    mut commands: Commands,
    time: Res<Time>,
    art: Option<Res<crate::casino::CasinoArt>>,
    mut cs: ResMut<HouseCraftState>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<HouseCraftSpeakerText>>,
    mut sub_q: Query<&mut Text, (With<HouseCraftSubText>, Without<HouseCraftSpeakerText>)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let dt = time.delta_secs().min(0.05);
    cs.beat_t += dt;

    if !cs.beat0_spoken && !crate::skip::skipping() {
        cs.beat0_spoken = true;
        speak_voice(HOUSE_CRAFT_BEATS[0].1, HOUSE_CRAFT_BEATS[0].2);
    }

    if !cs.choice_shown {
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

        if (cs.beat as usize) >= HOUSE_CRAFT_BEATS.len() {
            cs.choice_shown = true;
            if let Ok(mut win) = window_q.get_single_mut() {
                win.cursor_options.grab_mode = CursorGrabMode::None;
                win.cursor_options.visible = true;
            }
            spawn_house_craft_choice_modal(&mut commands, art.as_deref());
        }
    }

    let stage = HOUSE_CRAFT_STAGE;
    let (target_eye, target_look) = match cs.beat {
        0 => (stage + Vec3::new(1.25, 1.72, 1.05), stage + Vec3::new(0.4, 1.56, 2.5)),
        1 => (stage + Vec3::new(1.65, 1.68, 0.90), stage + Vec3::new(3.2, 1.56, 1.5)),
        2 => (stage + Vec3::new(1.5, 2.35, 3.8), stage + Vec3::new(1.5, 1.45, 0.0)),
        _ => (stage + Vec3::new(-2.4, 1.78, -1.55), stage + Vec3::new(-5.7, 1.56, -2.5)),
    };
    cs.cam_look = cs.cam_look.lerp(target_look, (dt * 4.2).min(1.0));
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = cam.translation.lerp(target_eye, (dt * 4.2).min(1.0));
        cam.look_at(cs.cam_look, Vec3::Y);
    }
}

fn handle_house_craft_choice_input(
    keys: Res<ButtonInput<KeyCode>>,
    cs: Res<HouseCraftState>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    btn_q: Query<(&Interaction, &HouseCraftChoice), Changed<Interaction>>,
) {
    if !cs.choice_shown {
        return;
    }

    let mut chosen: Option<HouseCraftChoice> = None;
    if keys.just_pressed(KeyCode::Digit1) {
        chosen = Some(HouseCraftChoice::TenBeersCasino);
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::KeyC) {
        chosen = Some(HouseCraftChoice::TenBeersCraft);
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Escape) {
        chosen = Some(HouseCraftChoice::FreeRoamCity);
    }
    for (inter, c) in &btn_q {
        if *inter == Interaction::Pressed {
            chosen = Some(*c);
        }
    }

    let Some(c) = chosen else {
        return;
    };
    match c {
        HouseCraftChoice::TenBeersCasino => {
            play_sfx("chip_bet");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = 1.8;
            next_state.set(GameState::PubPortalCutscene);
        }
        HouseCraftChoice::TenBeersCraft => {
            play_sfx("portal_warp");
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = 1.8;
            next_state.set(GameState::BlackHoleCutscene);
        }
        HouseCraftChoice::FreeRoamCity => {
            stats.house_craft_return_city = true;
            stats.saved_city_pos = bartender_bar_trigger_pos();
            stats.saved_casino_city_scale = 1.8;
            next_state.set(GameState::CityPlay);
        }
    }
}

fn yaw_toward(from: Vec3, target: Vec3, model_neg_z: bool) -> f32 {
    let dx = target.x - from.x;
    let dz = target.z - from.z;
    if model_neg_z {
        (-dx).atan2(-dz)
    } else {
        dx.atan2(dz)
    }
}

// =================================================================================================
// Section C: Cutscene 2 -- The Odyssey + 10,000 Women Black Hole (`GameState::BlackHoleCutscene`)
// =================================================================================================

const BLACK_HOLE_STAGE: Vec3 = Vec3::new(0.0, 0.0, -1720.0);
const BH_SEAT_OFFSET: f32 = -0.26;
const BH_BOY_SEAT_REL: [Vec3; 5] = [
    Vec3::new(-1.85, 0.0, 1.35),  // 0: Stan (-Z GLB)
    Vec3::new(1.85, 0.0, 1.35),   // 1: Nate (+Z)
    Vec3::new(-2.05, 0.0, -0.45), // 2: TJ (+Z)
    Vec3::new(2.05, 0.0, -0.45),  // 3: Derek (+Z)
    Vec3::new(0.0, 0.0, 2.35),    // 4: Glenn (+Z)
];

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
    cam_look: Vec3,
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
    art: Option<Res<crate::casino::CasinoArt>>,
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

    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.02, 0.05)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.92, 0.98),
        brightness: 480.0,
    });

    let stage = BLACK_HOLE_STAGE;
    let table_pos = stage + Vec3::new(0.0, 0.0, 1.4);
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
        base_color: Color::srgb(0.22, 0.10, 0.06),
        perceptual_roughness: 0.34,
        ..default()
    });
    let gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.24),
        emissive: LinearRgba::rgb(1.4, 0.9, 0.18),
        metallic: 0.65,
        perceptual_roughness: 0.25,
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

    // Pub room + round beer table + Women's Restroom doorway (all tagged CityChunk so they get sucked in!)
    let mut spawn_chunk =
        |idx: usize, mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, pos: Vec3, scale: Vec3| {
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
    spawn_chunk(0, unit_box.clone(), floor_mat, stage + Vec3::new(0.0, -0.1, 0.0), Vec3::new(18.0, 0.2, 18.0));
    spawn_chunk(1, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(-8.2, 2.8, 0.0), Vec3::new(0.4, 5.6, 18.0));
    spawn_chunk(2, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(8.2, 2.8, 0.0), Vec3::new(0.4, 5.6, 18.0));
    spawn_chunk(3, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(-4.8, 2.8, -6.4), Vec3::new(6.8, 5.6, 0.4));
    spawn_chunk(4, unit_box.clone(), brick_mat.clone(), stage + Vec3::new(4.8, 2.8, -6.4), Vec3::new(6.8, 5.6, 0.4));
    spawn_chunk(5, unit_box.clone(), brick_mat, stage + Vec3::new(0.0, 4.4, -6.4), Vec3::new(2.8, 2.4, 0.4));
    // Round Pub Table + left Bar counter
    spawn_chunk(6, unit_cyl.clone(), wood_mat.clone(), table_pos + Vec3::new(0.0, 0.47, 0.0), Vec3::new(0.28, 0.94, 0.28));
    spawn_chunk(7, unit_cyl.clone(), wood_mat.clone(), table_pos + Vec3::new(0.0, 0.94, 0.0), Vec3::new(1.42, 0.12, 1.42));
    spawn_chunk(8, unit_box.clone(), wood_mat.clone(), stage + Vec3::new(-6.0, 0.62, -2.2), Vec3::new(1.1, 1.24, 5.4));
    spawn_chunk(9, unit_box.clone(), gold_mat, stage + Vec3::new(-6.0, 3.8, -6.1), Vec3::new(2.8, 0.65, 0.14));

    // Glowing "WOMEN'S RESTROOM" plaque & pink trim above open doorway
    if let Some(ref a) = art {
        let sign_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(a.womens_sign.clone()),
            emissive: LinearRgba::rgb(1.4, 1.1, 1.35),
            emissive_texture: Some(a.womens_sign.clone()),
            unlit: true,
            ..default()
        });
        spawn_chunk(
            42,
            unit_box.clone(),
            sign_mat,
            door_pos + Vec3::new(0.0, 3.55, 0.22),
            Vec3::new(1.75, 0.85, 0.10),
        );
    }
    spawn_chunk(43, unit_box.clone(), neon_pink, door_pos + Vec3::new(0.0, 3.12, 0.20), Vec3::new(2.75, 0.14, 0.12));

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
        let s = if is_cab { Vec3::new(2.0, 1.2, 4.0) } else { Vec3::new(4.2, h, 4.2) };
        spawn_chunk(
            10 + i,
            unit_box.clone(),
            if is_cab { cab_mat.clone() } else { tower_mat.clone() },
            pos,
            s,
        );
    }

    // Glowing 7-tier Beer Tower centered atop the pub table!
    let tower_pos = table_pos + Vec3::new(0.0, 1.00, 0.0);
    let tower_scale = Vec3::splat(0.32);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::CityChunk(44),
                home_pos: tower_pos,
                home_scale: tower_scale,
                phase: 0.7,
            },
            Transform::from_translation(tower_pos).with_scale(tower_scale),
            Visibility::Inherited,
        ))
        .with_children(|t| {
            crate::beer_fx::spawn_pub_beer_tower(t, &mut meshes, &mut materials, Vec3::ZERO);
        });

    // 10 Tall-boy Bash Lyte cans around the table rim
    let single_can = meshes.add(crate::beer_fx::can_mesh());
    let can_mats = crate::beer_fx::can_materials(&mut materials);
    let can_scale = Vec3::splat(0.45);
    for ci in 0..10_usize {
        let a = (ci as f32 / 10.0) * TAU + 0.15;
        let cpos = table_pos + Vec3::new(a.cos() * 1.05, 1.00 + crate::beer_fx::CAN_HALF_H * 0.45, a.sin() * 1.05);
        commands.spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::CityChunk(30 + ci),
                home_pos: cpos,
                home_scale: can_scale,
                phase: ci as f32 * 0.55,
            },
            Mesh3d(single_can.clone()),
            MeshMaterial3d(can_mats[ci % can_mats.len()].clone()),
            Transform::from_translation(cpos)
                .with_rotation(Quat::from_rotation_y(-a + 0.4))
                .with_scale(can_scale),
        ));
    }

    // Maria behind the bar & Mike nearby
    let maria_pos = stage + Vec3::new(-7.1, 0.0, -2.2);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::CityChunk(40),
                home_pos: maria_pos,
                home_scale: Vec3::splat(PUB217_SCALE),
                phase: 2.1,
            },
            Transform::from_translation(maria_pos)
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                .with_scale(Vec3::splat(PUB217_SCALE)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_bartender_rig(p, &mut meshes, &mut materials));

    let mike_pos = stage + Vec3::new(-4.1, 0.0, 2.2);
    let scale = city_body_scale(1.8);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::CityChunk(41),
                home_pos: mike_pos,
                home_scale: Vec3::splat(scale * 0.85),
                phase: 2.8,
            },
            Transform::from_translation(mike_pos)
                .with_rotation(Quat::from_rotation_y(yaw_toward(mike_pos, table_pos, false)))
                .with_scale(Vec3::splat(scale * 0.85)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_mike_rig(p, &mut meshes, &mut materials));

    // 20 full sculpted glamorous women pouring out of & overflowing the Women's Restroom!
    let woman_scale = Vec3::splat(0.95);
    for wi in 0..20_usize {
        let row = (wi / 5) as f32;
        let col = (wi % 5) as f32 - 2.0;
        let stagger = if (wi / 5) % 2 == 1 { 0.18 } else { -0.10 };
        let x_spread = col * 0.58 + stagger;
        let z_spill = 1.75 - row * 0.72 + ((wi % 3) as f32 - 1.0) * 0.12;
        let wpos = door_pos + Vec3::new(x_spread, 0.0, z_spill);
        let wyaw = col * -0.18 + ((wi % 2) as f32 - 0.5) * 0.22;
        commands
            .spawn((
                BlackHoleCutsceneEntity,
                BlackHoleAnim {
                    kind: BlackHoleAnimKind::Woman(wi),
                    home_pos: wpos,
                    home_scale: woman_scale,
                    phase: wi as f32 * 0.43,
                },
                Transform::from_translation(wpos)
                    .with_rotation(Quat::from_rotation_y(wyaw))
                    .with_scale(woman_scale),
                Visibility::Inherited,
            ))
            .with_children(|w| {
                crate::pub_portal::spawn_attractive_woman_rig(w, &mut meshes, &mut materials, wi, wi);
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

    // 5 Wooden Pub Chairs + Main Boys seated around the Beer Tower table (Stan, Nate, TJ, Derek, Glenn)
    for (ci, rel) in BH_BOY_SEAT_REL.iter().enumerate() {
        let spot = table_pos + *rel;
        let cyaw = yaw_toward(spot, table_pos, false);
        commands
            .spawn((
                BlackHoleCutsceneEntity,
                BlackHoleAnim {
                    kind: BlackHoleAnimKind::CityChunk(50 + ci),
                    home_pos: spot,
                    home_scale: Vec3::ONE,
                    phase: 0.9 + ci as f32 * 0.4,
                },
                Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(cyaw)),
                Visibility::Inherited,
            ))
            .with_children(|ch| {
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.48, 0.0).with_scale(Vec3::new(0.54, 0.08, 0.52)),
                ));
                ch.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(wood_mat.clone()),
                    Transform::from_xyz(0.0, 0.88, -0.23).with_scale(Vec3::new(0.52, 0.74, 0.08)),
                ));
                for (lx, lz) in [(-0.21, -0.21), (0.21, -0.21), (-0.21, 0.21), (0.21, 0.21)] {
                    ch.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(wood_mat.clone()),
                        Transform::from_xyz(lx, 0.24, lz).with_scale(Vec3::new(0.07, 0.48, 0.07)),
                    ));
                }
            });
    }

    let boy_spots: [Vec3; 5] = BH_BOY_SEAT_REL.map(|r| table_pos + r + Vec3::Y * BH_SEAT_OFFSET);
    commands.spawn((
        BlackHoleCutsceneEntity,
        BlackHoleAnim {
            kind: BlackHoleAnimKind::Boy(0),
            home_pos: boy_spots[0],
            home_scale: Vec3::splat(0.96),
            phase: 0.0,
        },
        SceneRoot(asset_server.load("models/plaid_hero.glb#Scene0")),
        Transform::from_translation(boy_spots[0])
            .with_rotation(Quat::from_rotation_y(yaw_toward(boy_spots[0], table_pos, true)))
            .with_scale(Vec3::splat(0.96)),
        Visibility::Inherited,
    ));
    let nate_m = make_nate_materials(&mut materials);
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(1),
                home_pos: boy_spots[1],
                home_scale: Vec3::splat(0.96),
                phase: 1.0,
            },
            Transform::from_translation(boy_spots[1])
                .with_rotation(Quat::from_rotation_y(yaw_toward(boy_spots[1], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_nate_rig(p, &mut meshes, &nate_m, false));
    commands
        .spawn((
            BlackHoleCutsceneEntity,
            BlackHoleAnim {
                kind: BlackHoleAnimKind::Boy(2),
                home_pos: boy_spots[2],
                home_scale: Vec3::splat(0.96),
                phase: 2.0,
            },
            Transform::from_translation(boy_spots[2])
                .with_rotation(Quat::from_rotation_y(yaw_toward(boy_spots[2], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
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
                home_scale: Vec3::splat(0.96),
                phase: 3.0,
            },
            Transform::from_translation(boy_spots[3])
                .with_rotation(Quat::from_rotation_y(yaw_toward(boy_spots[3], table_pos, false)))
                .with_scale(Vec3::splat(0.96)),
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
                home_scale: Vec3::splat(0.94),
                phase: 4.0,
            },
            Transform::from_translation(boy_spots[4])
                .with_rotation(Quat::from_rotation_y(yaw_toward(boy_spots[4], table_pos, false)))
                .with_scale(Vec3::splat(0.94)),
            Visibility::Inherited,
        ))
        .with_children(|p| spawn_glenn_rig(p, &mut meshes, &glenn_m));

    // Warm pub chandelier over the Beer Tower + front face fill + bright Women's Restroom spotlight
    commands.spawn((
        BlackHoleCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.88, 0.72),
            intensity: 200_000.0,
            range: 30.0,
            ..default()
        },
        Transform::from_translation(table_pos + Vec3::new(0.0, 4.2, 0.0)),
    ));
    commands.spawn((
        BlackHoleCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.94, 0.88),
            intensity: 120_000.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(table_pos + Vec3::new(0.0, 2.5, 3.8)),
    ));
    commands.spawn((
        BlackHoleCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.94, 0.92),
            intensity: 195_000.0,
            range: 20.0,
            ..default()
        },
        Transform::from_translation(door_pos + Vec3::new(0.0, 3.0, 2.8)),
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

    let init_look = table_pos + Vec3::new(0.0, 1.48, 0.0);
    bh.cam_look = init_look;
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = table_pos + Vec3::new(-0.55, 1.72, 2.25);
        cam.look_at(init_look, Vec3::Y);
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
    let table_pos = stage + Vec3::new(0.0, 0.0, 1.4);
    let door_pos = stage + Vec3::new(0.0, 0.0, -6.2);
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
                    let sway_x = (bh.total_t * 3.8 + anim.phase).sin() * 0.07;
                    let bob_y = ((bh.total_t * 6.0 + anim.phase).sin().abs()) * 0.04;
                    let spill = (bh.beat as f32 / 4.0).clamp(0.0, 1.0) * 0.85;
                    tf.translation = anim.home_pos
                        + Vec3::new(sway_x, bob_y, spill * ((idx % 4) as f32 * 0.28));
                } else {
                    let pull = (bh_pull * 1.45).clamp(0.0, 1.0);
                    let ang = bh.total_t * 7.5 + anim.phase;
                    let orbit = Vec3::new(ang.cos(), (ang * 0.7).sin() * 0.4, ang.sin())
                        * ((1.0 - pull) * 3.2);
                    tf.translation = anim.home_pos.lerp(bh_center, pull) + orbit * pull;
                    tf.rotate_y(8.0 * dt);
                    tf.scale = anim.home_scale * (1.0 - pull * 0.95).max(0.04);
                }
            }
            BlackHoleAnimKind::CityChunk(_) => {
                if bh.beat >= 6 {
                    let pull = ((bh_pull - 0.42) / 0.58).clamp(0.0, 1.0);
                    let ang = bh.total_t * 5.2 + anim.phase;
                    let swirl = Vec3::new(ang.cos(), 0.35 * ang.sin(), ang.sin()) * (1.0 - pull) * 4.0;
                    tf.translation = anim.home_pos.lerp(bh_center, pull) + swirl * pull;
                    tf.rotate_y(4.5 * dt);
                    tf.rotate_z(2.8 * dt);
                    tf.scale = anim.home_scale * (1.0 - pull * 0.95).max(0.05);
                }
            }
            BlackHoleAnimKind::Boy(bi) => {
                if bh.beat >= 6 {
                    let pull = ((bh_pull - 0.42) / 0.58).clamp(0.0, 1.0);
                    let ang = bh.total_t * 5.2 + anim.phase;
                    let swirl = Vec3::new(ang.cos(), 0.35 * ang.sin(), ang.sin()) * (1.0 - pull) * 4.0;
                    tf.translation = anim.home_pos.lerp(bh_center, pull) + swirl * pull;
                    tf.rotate_y(4.5 * dt);
                    tf.rotate_z(2.8 * dt);
                    tf.scale = anim.home_scale * (1.0 - pull * 0.95).max(0.05);
                } else {
                    let speaker_bi: Option<usize> = match bh.beat {
                        0 => Some(1),
                        1 => Some(2),
                        2 => Some(3),
                        3 => Some(0),
                        4 => Some(4),
                        _ => None,
                    };
                    let active = speaker_bi == Some(bi);
                    let bob = if active {
                        (bh.total_t * 6.5 + anim.phase).sin().abs() * 0.045
                    } else {
                        (bh.total_t * 2.2 + anim.phase).sin() * 0.015
                    };
                    if bi == 0 && bh.beat == 3 {
                        // Stan rises & lunges forward threatening TJ & Derek's ankles
                        let to_tbl = Vec3::new(table_pos.x - anim.home_pos.x, 0.0, table_pos.z - anim.home_pos.z)
                            .normalize_or_zero();
                        let lunge = (bh.beat_t * 2.2).clamp(0.0, 0.35);
                        tf.translation = anim.home_pos + to_tbl * lunge + Vec3::Y * (lunge * 0.65);
                    } else if bi == 4 && bh.beat >= 4 {
                        // Glenn jumps out of his chair pointing toward the overflowing Women's Restroom
                        let rise = ((bh.beat_t - 1.2).max(0.0) * 1.4).clamp(0.0, 1.0);
                        tf.translation = anim.home_pos + Vec3::Y * (rise * 0.24 + bob);
                        let y_tbl = yaw_toward(anim.home_pos, table_pos, false);
                        let y_door = yaw_toward(anim.home_pos, door_pos, false);
                        let mut diff = (y_door - y_tbl + PI).rem_euclid(TAU) - PI;
                        diff *= rise;
                        tf.rotation = Quat::from_rotation_y(y_tbl + diff);
                    } else {
                        tf.translation = anim.home_pos + Vec3::Y * bob;
                    }
                }
            }
        }
    }

    let face_closeup = |bi: usize, dist: f32, pan: f32, elev: f32| -> (Vec3, Vec3) {
        let spot = table_pos + BH_BOY_SEAT_REL[bi];
        let to_tbl = Vec3::new(table_pos.x - spot.x, 0.0, table_pos.z - spot.z).normalize_or_zero();
        let right = Vec3::Y.cross(to_tbl).normalize_or_zero();
        let face_y = match bi {
            0 => 1.50, // Stan
            1 => 1.41, // Nate
            2 => 1.51, // TJ
            3 => 1.43, // Derek
            _ => 1.45, // Glenn
        };
        let look = spot + Vec3::Y * face_y;
        let eye = look + to_tbl * dist + right * pan + Vec3::Y * elev;
        (eye, look)
    };

    let (target_eye, target_look) = match bh.beat {
        // Beat 0: Sweeping hero reveal of the 7-tier Beer Tower & 10 cans -> pan straight into Nate's face!
        0 => {
            if bh.beat_t < 1.55 {
                let u = bh.beat_t / 1.55;
                (
                    table_pos + Vec3::new(-0.55 + u * 0.75, 1.72, 2.25 - u * 0.20),
                    table_pos + Vec3::new(0.0, 1.48, 0.0),
                )
            } else {
                let pan = 0.22 - ((bh.beat_t - 1.55) * 0.14).clamp(0.0, 0.32);
                face_closeup(1, 1.48, pan, 0.05)
            }
        }
        // Beat 1: Tight panning close-up on TJ's glasses & bearded face!
        1 => face_closeup(2, 1.48, -0.22 + (bh.beat_t * 0.10).clamp(0.0, 0.40), 0.04),
        // Beat 2: Tight panning close-up on Derek's face!
        2 => face_closeup(3, 1.50, 0.24 - (bh.beat_t * 0.10).clamp(0.0, 0.40), 0.04),
        // Beat 3: Dramatic low-angle close-up on Stan's face as he lunges forward!
        3 => face_closeup(0, 1.46, -0.14 + (bh.beat_t * 0.08).clamp(0.0, 0.24), -0.04),
        // Beat 4: Glenn's shocked face close-up -> whip-pan across the sculpted women overflowing out of the Restroom!
        4 => {
            if bh.beat_t < 1.9 {
                face_closeup(4, 1.50, -0.18 + bh.beat_t * 0.15, 0.05)
            } else {
                let pan = ((bh.beat_t - 1.9) * 0.34).clamp(0.0, 1.15);
                (
                    door_pos + Vec3::new(-0.55 + pan, 1.56, 3.85),
                    door_pos + Vec3::new(0.0, 1.44, 0.85),
                )
            }
        }
        // Beat 5: Tense push-in watching the sculpted women spiral into the black hole event horizon
        5 => (
            door_pos + Vec3::new(0.35, 2.15, 5.2 - (bh.beat_t * 0.24).clamp(0.0, 1.0)),
            bh_center,
        ),
        // Beat 6: Wide sweeping vortex pulling Beer Tower, 10 cans, boys, cabs & city towers in!
        _ => {
            let swirl_u = (bh.beat_t / 4.2).clamp(0.0, 1.0);
            (
                stage + Vec3::new((bh.total_t * 2.2).sin() * 3.4, 3.4 - swirl_u * 1.3, 8.8 - swirl_u * 7.6),
                bh_center,
            )
        }
    };

    bh.cam_look = bh.cam_look.lerp(target_look, (dt * 4.8).min(1.0));
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation = cam.translation.lerp(target_eye, (dt * 4.8).min(1.0));
        cam.look_at(bh.cam_look, Vec3::Y);
    }

    if (bh.beat as usize) >= BLACK_HOLE_BEATS.len() {
        stats.craft_world_unlocked = true;
        next_state.set(GameState::CraftPlay);
    }
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
struct CraftMineCursor;

#[derive(Component)]
struct CraftPlaceGhost;

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

#[derive(Component)]
struct CraftTargetPill;

#[derive(Component)]
struct CraftTargetText;

#[derive(Component)]
struct CraftFeedbackPill;

#[derive(Component)]
struct CraftFeedbackText;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CraftQuickAction {
    OpenBench,
    CycleShape,
    ToggleView,
    OpenSync,
    OpenTutorial,
}

#[derive(Component, Clone, Copy)]
struct CraftQuickBtn(CraftQuickAction);

#[derive(Component)]
struct CraftShapeBtnText;

#[derive(Component)]
struct CraftViewBtnText;

#[derive(Component)]
struct CraftSyncBtnText;

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
    CraftStone,
    CraftGlass,
    CraftNeon,
    FreeScrapKit,
    SelectShape(BuildShape),
    DerekHyperfocusPub217,
    DerekHyperfocusHouse,
    OpenSyncModal,
    WatchTutorial,
    DrinkBeer,
    CasinoFromPub,
}

#[derive(Resource)]
struct CraftBlockAssets {
    cube_mesh: Handle<Mesh>,
    mats: [Handle<StandardMaterial>; 9],
    ghost_ok_mat: Handle<StandardMaterial>,
    ghost_bad_mat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct CraftInteractPrompt {
    target: Option<CraftPromptTarget>,
    tapped: bool,
    modal_is_derek: bool,
    modal_just_opened: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CraftPromptTarget {
    Npc(CraftNpcRole),
    BeerTower,
    Portal,
}

fn block_kind_from_u8(v: u8) -> Option<BlockKind> {
    match v {
        0 => Some(BlockKind::Brick),
        1 => Some(BlockKind::Wood),
        2 => Some(BlockKind::Stone),
        3 => Some(BlockKind::Glass),
        4 => Some(BlockKind::TendyGold),
        5 => Some(BlockKind::Neon217),
        6 => Some(BlockKind::Leaves),
        7 => Some(BlockKind::TendyOre),
        8 => Some(BlockKind::SupplyCube),
        _ => None,
    }
}

fn encode_craft_world_save(data: &CraftWorldData, pub_rebuilt: bool) -> String {
    let mut entries: Vec<((i32, i32, i32), BlockKind)> =
        data.blocks.iter().map(|(&c, &k)| (c, k)).collect();
    entries.sort_by_key(|(c, _)| *c);
    let mut out = format!(
        "TQCW1|{}|{}|{},{},{},{},{},{}|",
        u8::from(pub_rebuilt),
        u8::from(data.house_completed),
        data.inventory[0].min(999),
        data.inventory[1].min(999),
        data.inventory[2].min(999),
        data.inventory[3].min(999),
        data.inventory[4].min(999),
        data.inventory[5].min(999),
    );
    for (idx, ((x, y, z), k)) in entries.into_iter().take(6000).enumerate() {
        if idx > 0 {
            out.push(';');
        }
        out.push_str(&format!("{},{},{},{}", x, y, z, k as u8));
    }
    out
}

fn apply_craft_world_save(
    raw: &str,
    data: &mut CraftWorldData,
    stats: &mut PlayerStats,
    replace_inventory: bool,
) -> bool {
    let parts: Vec<&str> = raw.trim().splitn(5, '|').collect();
    if parts.len() != 5 || parts[0] != "TQCW1" {
        return false;
    }
    let pub_rebuilt = parts[1] == "1";
    let house_done = parts[2] == "1";

    let inv_nums: Vec<u32> = parts[3]
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();
    if inv_nums.len() != 6 {
        return false;
    }

    let mut next_blocks: HashMap<(i32, i32, i32), BlockKind> = HashMap::new();
    for item in parts[4].split(';').filter(|s| !s.is_empty()).take(6000) {
        let mut nums = item.split(',');
        let (Some(xs), Some(ys), Some(zs), Some(ks)) =
            (nums.next(), nums.next(), nums.next(), nums.next())
        else {
            continue;
        };
        let (Ok(x), Ok(y), Ok(z), Ok(kv)) = (
            xs.parse::<i32>(),
            ys.parse::<i32>(),
            zs.parse::<i32>(),
            ks.parse::<u8>(),
        ) else {
            continue;
        };
        if (-24..=24).contains(&x) && (0..=10).contains(&y) && (-24..=24).contains(&z) {
            if let Some(kind) = block_kind_from_u8(kv) {
                next_blocks.insert((x, y, z), kind);
            }
        }
    }

    data.blocks = next_blocks;
    data.initialized = true;
    data.house_completed = house_done;
    stats.craft_pub217_rebuilt = pub_rebuilt;
    if pub_rebuilt || !data.blocks.is_empty() {
        stats.craft_world_unlocked = true;
    }
    for (i, &val) in inv_nums.iter().enumerate() {
        if replace_inventory {
            data.inventory[i] = val.min(999);
        } else {
            data.inventory[i] = data.inventory[i].max(val).min(999);
        }
    }
    recount_quest_blocks(data);
    true
}

fn persist_craft_world(data: &CraftWorldData, pub_rebuilt: bool) {
    save_craft_world(&encode_craft_world_save(data, pub_rebuilt));
}

fn grid_to_world(coord: (i32, i32, i32)) -> Vec3 {
    CRAFT_LEVEL_ORIGIN + Vec3::new(coord.0 as f32, coord.1 as f32 + 0.5, coord.2 as f32)
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
    data.inventory = [12, 12, 8, 6, 4, 4];
    data.selected_slot = 0;
    data.build_shape = BuildShape::Single;
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
    stats.craft_first_person = false;
    stats.driving_car = None;
    stats.riding_scooter = None;
    stats.riding_wheel = false;
    stats.phone_open = false;
    data.pub217_decor_spawned = stats.craft_pub217_rebuilt;
    data.feedback_timer = 0.0;
    data.target_label.clear();
    data.tutorial_auto_delay = if data.tutorial_video_shown { 0.0 } else { 0.45 };

    set_touch_playing(true);
    set_touch_craft_mode(true);
    set_touch_craft_shape(data.build_shape.short_tag());
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
        ghost_ok_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.24, 0.92, 1.0, 0.36),
            emissive: LinearRgba::rgb(0.22, 0.85, 1.1),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        ghost_bad_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.24, 0.24, 0.42),
            emissive: LinearRgba::rgb(0.95, 0.16, 0.16),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };

    if !data.initialized {
        let saved = load_craft_world();
        if !saved.is_empty() && apply_craft_world_save(&saved, &mut data, &mut stats, true) {
            recount_quest_blocks(&mut data);
        } else {
            populate_empty_city_world(&mut data);
            persist_craft_world(&data, stats.craft_pub217_rebuilt);
        }
    } else {
        recount_quest_blocks(&mut data);
    }

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
        PlatformBox {
            half_extents: Vec3::new(33.0, 0.5, 33.0),
        },
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
    let ghost_ok_mat = assets.ghost_ok_mat.clone();
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

    // Red/orange Mine Target Box + Valheim-style Translucent Cyan Place Ghost Preview
    let mine_cursor_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.34, 0.18, 0.34),
        emissive: LinearRgba::rgb(1.1, 0.25, 0.08),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftMineCursor,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(mine_cursor_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN).with_scale(Vec3::splat(1.05)),
        Visibility::Hidden,
    ));
    commands.spawn((
        LevelEntity,
        CraftLevelEntity,
        CraftPlaceGhost,
        Mesh3d(unit_box),
        MeshMaterial3d(ghost_ok_mat),
        Transform::from_translation(CRAFT_LEVEL_ORIGIN).with_scale(Vec3::splat(1.04)),
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
    stats.hearts = stats.max_hearts;
    if let Ok((mut p_tf, mut player, mut p_vis)) = player_q.get_single_mut() {
        p_tf.translation = CRAFT_SPAWN_POS;
        player.facing_yaw = 0.0;
        p_tf.rotation = Quat::IDENTITY;
        p_tf.scale = Vec3::splat(city_body_scale(1.8));
        player.velocity = Vec3::ZERO;
        player.knockback = Vec3::ZERO;
        player.grounded = true;
        *p_vis = Visibility::Visible;
    }
    orbit.yaw = 0.0;
    orbit.pitch = 0.28;
    orbit.distance = 4.6;
    orbit.current_dist = 4.6;
    orbit.snap_follow = true;
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        cam_tf.translation = CRAFT_SPAWN_POS + Vec3::new(0.90, 3.25, 4.4);
        cam_tf.look_at(CRAFT_SPAWN_POS + Vec3::new(0.90, 1.85, -9.5), Vec3::Y);
    }

    spawn_craft_hud(&mut commands);
    stats.hint_text.clear();
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
                padding: UiRect::vertical(Val::Px(10.0)),
                ..default()
            },
            GlobalZIndex(18),
        ))
        .with_children(|root| {
            // Top quest tracker (offset below MIC: OFF and compact so corners stay unobstructed)
            root.spawn((
                Node {
                    margin: UiRect::top(Val::Px(34.0)),
                    max_width: Val::Px(560.0),
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.14, 0.88)),
                BorderColor(Color::srgb(0.72, 0.42, 1.0)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|q| {
                q.spawn((
                    CraftQuestText,
                    Text::new("QUEST 1: REBUILD PUB 217 (0/18)"),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.88, 0.35)),
                ));
            });

            // Center crosshair + live Valheim/Minecraft action readout directly beneath +
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|mid| {
                mid.spawn((
                    CraftCrosshair,
                    Text::new("+"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.88)),
                ));
                mid.spawn((
                    CraftTargetPill,
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.14, 0.80)),
                    BorderColor(Color::srgba(0.30, 0.90, 1.0, 0.72)),
                    BorderRadius::all(Val::Px(999.0)),
                ))
                .with_children(|tp| {
                    tp.spawn((
                        CraftTargetText,
                        Text::new("[R-CLICK] Place BRICK"),
                        TextFont { font_size: 11.5, ..default() },
                        TextColor(Color::srgb(0.92, 0.97, 1.0)),
                    ));
                });
            });

            // Bottom stack: floating toast, NPC prompt, quick actions, color-swatched hotbar
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|bot| {
                bot.spawn((
                    CraftFeedbackPill,
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.22, 0.14, 0.92)),
                    BorderColor(Color::srgb(0.35, 0.95, 0.62)),
                    BorderRadius::all(Val::Px(999.0)),
                ))
                .with_children(|fb| {
                    fb.spawn((
                        CraftFeedbackText,
                        Text::new("+2 WOOD"),
                        TextFont { font_size: 12.5, ..default() },
                        TextColor(Color::srgb(0.55, 1.0, 0.75)),
                    ));
                });

                bot.spawn((
                    CraftPromptPill,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
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
                        TextFont { font_size: 13.5, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });

                // Clickable quick-action bar above the hotbar
                bot.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|tools| {
                    tools
                        .spawn((
                            CraftQuickBtn(CraftQuickAction::OpenBench),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.28, 0.18, 0.92)),
                            BorderColor(Color::srgb(0.38, 0.95, 0.62)),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("[B] CRAFT BENCH"),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    tools
                        .spawn((
                            CraftQuickBtn(CraftQuickAction::CycleShape),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.24, 0.36, 0.92)),
                            BorderColor(Color::srgb(0.32, 0.90, 1.0)),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                CraftShapeBtnText,
                                Text::new("[G] 1x1 BLOCK"),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    tools
                        .spawn((
                            CraftQuickBtn(CraftQuickAction::ToggleView),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.14, 0.20, 0.42, 0.92)),
                            BorderColor(Color::srgb(0.58, 0.78, 1.0)),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                CraftViewBtnText,
                                Text::new("[V] 3RD PERSON"),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    tools
                        .spawn((
                            CraftQuickBtn(CraftQuickAction::OpenSync),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.16, 0.14, 0.44, 0.94)),
                            BorderColor(Color::srgb(0.45, 0.90, 1.0)),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                CraftSyncBtnText,
                                Text::new("[Y] SYNC WORLD"),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::srgb(0.68, 0.96, 1.0)),
                            ));
                        });
                    tools
                        .spawn((
                            CraftQuickBtn(CraftQuickAction::OpenTutorial),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.42, 0.26, 0.06, 0.94)),
                            BorderColor(Color::srgb(1.0, 0.84, 0.28)),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("[T] DEREK VIDEO"),
                                TextFont { font_size: 11.5, ..default() },
                                TextColor(Color::srgb(1.0, 0.92, 0.42)),
                            ));
                        });
                });

                // 6-slot color-swatched Minecraft hotbar
                bot.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.12, 0.90)),
                    BorderColor(Color::srgba(0.95, 0.78, 0.28, 0.85)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|bar| {
                    for (idx, kind) in HOTBAR_BLOCKS.iter().enumerate() {
                        bar.spawn((
                            CraftHotbarBtn(idx),
                            Button,
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.12, 0.14, 0.24, 0.92)),
                            BorderColor(Color::srgba(0.6, 0.7, 0.88, 0.45)),
                            BorderRadius::all(Val::Px(6.0)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Node {
                                    width: Val::Px(13.0),
                                    height: Val::Px(13.0),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(kind.ui_color()),
                                BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
                                BorderRadius::all(Val::Px(3.0)),
                            ));
                            btn.spawn((
                                CraftHotbarText(idx),
                                Text::new(format!("[{}] {} x12", idx + 1, kind.short_name())),
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
    close_craft_sync_modal();
    crate::audio::close_derek_tutorial_video();
    set_touch_craft_mode(false);
    stats.in_craft_level = false;
    stats.craft_modal_open = false;
    stats.craft_first_person = false;
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

fn ray_voxel_aabb(origin: Vec3, dir: Vec3, bmin: Vec3, bmax: Vec3) -> Option<(f32, (i32, i32, i32))> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    let mut hit_axis = 0_usize;
    let mut hit_sign = 1_i32;

    for axis in 0..3_usize {
        let d = dir[axis];
        let o = origin[axis];
        if d.abs() < 1e-6 {
            if o < bmin[axis] || o > bmax[axis] {
                return None;
            }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (bmin[axis] - o) * inv;
            let mut t2 = (bmax[axis] - o) * inv;
            let mut sign = -1_i32;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
                sign = 1;
            }
            if t1 > t_min {
                t_min = t1;
                hit_axis = axis;
                hit_sign = sign;
            }
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }
    if t_max <= 0.25 || t_min <= 0.25 {
        return None;
    }
    let normal = match hit_axis {
        0 => (hit_sign, 0, 0),
        1 => (0, hit_sign, 0),
        _ => (0, 0, hit_sign),
    };
    Some((t_min, normal))
}

#[derive(bevy::ecs::system::SystemParam)]
struct VoxelRayQueries<'w, 's> {
    player_q: Query<
        'w,
        's,
        &'static Transform,
        (
            With<Player>,
            Without<Camera3d>,
            Without<VoxelBlock>,
            Without<CraftMineCursor>,
            Without<CraftPlaceGhost>,
            Without<CraftDebris>,
        ),
    >,
    cam_q: Query<
        'w,
        's,
        &'static Transform,
        (
            With<Camera3d>,
            Without<Player>,
            Without<VoxelBlock>,
            Without<CraftMineCursor>,
            Without<CraftPlaceGhost>,
            Without<CraftDebris>,
        ),
    >,
    block_q: Query<
        'w,
        's,
        (Entity, &'static VoxelBlock),
        (
            Without<Player>,
            Without<Camera3d>,
            Without<CraftMineCursor>,
            Without<CraftPlaceGhost>,
            Without<CraftDebris>,
        ),
    >,
    mine_cursor_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility),
        (
            With<CraftMineCursor>,
            Without<CraftPlaceGhost>,
            Without<Player>,
            Without<Camera3d>,
            Without<VoxelBlock>,
            Without<CraftDebris>,
        ),
    >,
    ghost_q: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static mut Visibility,
            &'static mut MeshMaterial3d<StandardMaterial>,
        ),
        (
            With<CraftPlaceGhost>,
            Without<CraftMineCursor>,
            Without<Player>,
            Without<Camera3d>,
            Without<VoxelBlock>,
            Without<CraftDebris>,
        ),
    >,
    debris_q: Query<
        'w,
        's,
        (Entity, &'static mut Transform, &'static mut CraftDebris),
        (
            Without<Player>,
            Without<Camera3d>,
            Without<VoxelBlock>,
            Without<CraftMineCursor>,
            Without<CraftPlaceGhost>,
        ),
    >,
    ui_inter_q: Query<
        'w,
        's,
        (
            &'static Interaction,
            Option<&'static CraftHotbarBtn>,
            Option<&'static CraftQuickBtn>,
            Option<&'static CraftPromptPill>,
        ),
        Or<(With<CraftHotbarBtn>, With<CraftQuickBtn>, With<CraftPromptPill>)>,
    >,
    window_q: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
}

#[allow(clippy::too_many_arguments)]
fn poll_craft_sync_bridge(
    state: Res<State<GameState>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut data: ResMut<CraftWorldData>,
    mut next_state: SceneTransition,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if take_craft_sync_closed() {
        stats.casino_esc_grace = true;
    }

    let in_craft = *state.get() == GameState::CraftPlay;
    let trigger_sync = (keys.just_pressed(KeyCode::KeyY) || (in_craft && take_touch_craft_sync()))
        && !stats.craft_modal_open
        && !stats.casino_open
        && !stats.arcade_open
        && !stats.bartender_open
        && !stats.char_select_open
        && !stats.club_modal_open
        && !is_derek_tutorial_open()
        && !is_craft_sync_modal_open()
        && (in_craft || has_craft_sync_offer());

    if trigger_sync {
        if !data.initialized {
            let saved = load_craft_world();
            if saved.is_empty() || !apply_craft_world_save(&saved, &mut data, &mut stats, true) {
                populate_empty_city_world(&mut data);
            }
        }
        if let Ok(mut win) = window_q.get_single_mut() {
            win.cursor_options.grab_mode = CursorGrabMode::None;
            win.cursor_options.visible = true;
        }
        let encoded = encode_craft_world_save(&data, stats.craft_pub217_rebuilt);
        open_craft_sync_modal(
            &encoded,
            data.blocks.len() as u32,
            stats.craft_pub217_rebuilt,
            data.house_completed,
            in_craft,
        );
    }

    let accepted = take_craft_sync_accepted();
    if accepted.is_empty() {
        return;
    }
    let parts: Vec<&str> = accepted.splitn(3, '|').collect();
    if parts.len() != 3 {
        return;
    }
    let join_craft = parts[0] == "JOIN";
    let sender = parts[1];
    let raw_save = parts[2];
    if !apply_craft_world_save(raw_save, &mut data, &mut stats, false) {
        play_sfx("hack_deny");
        return;
    }

    let block_n = data.blocks.len();
    persist_craft_world(&data, stats.craft_pub217_rebuilt);
    data.needs_world_rebuild = true;
    data.online_sync_hint_shown = true;
    let msg = format!("SYNCED CRAFT WORLD FROM {} ({} BLOCKS)!", sender, block_n);
    push_craft_toast(&mut data, msg.clone());
    if !in_craft && !join_craft {
        stats.hint_text = msg;
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
    play_sfx("portal_warp");
    play_sfx("powerup");

    if join_craft && !in_craft {
        stats.casino_open = false;
        stats.arcade_open = false;
        stats.bartender_open = false;
        stats.phone_open = false;
        stats.club_modal_open = false;
        stats.driving_car = None;
        stats.riding_scooter = None;
        stats.riding_wheel = false;
        next_state.set(GameState::CraftPlay);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_craft_net_sync(
    mut commands: Commands,
    mut data: ResMut<CraftWorldData>,
    stats: Res<PlayerStats>,
    net: Res<crate::net::NetState>,
    mut coop: ResMut<crate::net::NetCoopEvents>,
    assets: Option<Res<CraftBlockAssets>>,
    block_q: Query<
        (Entity, &VoxelBlock),
        (
            Without<CraftPubDecor>,
            Without<CraftNpc>,
            Without<CraftHouseOutline>,
        ),
    >,
    decor_q: Query<
        Entity,
        (
            With<CraftPubDecor>,
            Without<VoxelBlock>,
            Without<CraftNpc>,
            Without<CraftHouseOutline>,
        ),
    >,
    mut npc_q: Query<
        (&mut Transform, &CraftNpc),
        (
            Without<VoxelBlock>,
            Without<CraftPubDecor>,
            Without<CraftHouseOutline>,
        ),
    >,
    mut house_q: Query<
        &mut Visibility,
        (
            With<CraftHouseOutline>,
            Without<VoxelBlock>,
            Without<CraftPubDecor>,
            Without<CraftNpc>,
        ),
    >,
) {
    let Some(assets) = assets else {
        return;
    };

    if !data.online_sync_hint_shown
        && data.feedback_timer <= 0.0
        && net.peer_count() > 0
        && (data.pub_blocks_placed > 0 || stats.craft_pub217_rebuilt)
    {
        data.online_sync_hint_shown = true;
        push_craft_toast(&mut data, "SQUAD ONLINE! Press [Y] to Share & Sync Craft Save!");
    }

    if data.needs_world_rebuild {
        data.needs_world_rebuild = false;
        for (e, _) in &block_q {
            commands.entity(e).despawn_recursive();
        }
        for e in &decor_q {
            commands.entity(e).despawn_recursive();
        }
        let blocks_copy: Vec<((i32, i32, i32), BlockKind)> =
            data.blocks.iter().map(|(&c, &k)| (c, k)).collect();
        for (coord, kind) in blocks_copy {
            spawn_voxel_entity(&mut commands, &assets, coord, kind);
        }
        data.pub217_decor_spawned = false;
        if !stats.craft_pub217_rebuilt {
            for (mut tf, npc) in &mut npc_q {
                let (pos, yaw) = npc_position(npc.role, false);
                tf.translation = pos;
                tf.rotation = Quat::from_rotation_y(yaw);
            }
            for mut v in &mut house_q {
                *v = Visibility::Hidden;
            }
        }
        coop.craft_voxels.clear();
        recount_quest_blocks(&mut data);
        return;
    }

    if coop.craft_voxels.is_empty() {
        return;
    }
    let ops = std::mem::take(&mut coop.craft_voxels);
    let mut changed = false;
    for (gx, gy, gz, op_kind) in ops {
        if !(-24..=24).contains(&gx) || !(0..=10).contains(&gy) || !(-24..=24).contains(&gz) {
            continue;
        }
        let coord = (gx, gy, gz);
        for (e, vb) in &block_q {
            if vb.coord == coord {
                commands.entity(e).despawn_recursive();
            }
        }
        if op_kind == CRAFT_VOXEL_REMOVE {
            if data.blocks.remove(&coord).is_some() {
                play_sfx("block_break");
                changed = true;
            }
        } else if let Some(kind) = block_kind_from_u8(op_kind) {
            data.blocks.insert(coord, kind);
            spawn_voxel_entity(&mut commands, &assets, coord, kind);
            play_sfx("block_place");
            changed = true;
        }
    }
    if changed {
        recount_quest_blocks(&mut data);
        persist_craft_world(&data, stats.craft_pub217_rebuilt);
    }
}

fn push_craft_toast(data: &mut CraftWorldData, msg: impl Into<String>) {
    data.feedback_msg = msg.into();
    data.feedback_timer = 2.4;
}

fn open_derek_bench_modal(
    commands: &mut Commands,
    stats: &mut PlayerStats,
    data: &CraftWorldData,
    prompt: &mut CraftInteractPrompt,
    window_q: &mut Query<&mut Window, With<PrimaryWindow>>,
) {
    stats.craft_modal_open = true;
    prompt.modal_is_derek = true;
    prompt.modal_just_opened = true;
    set_touch_playing(false);
    if let Ok(mut win) = window_q.get_single_mut() {
        win.cursor_options.grab_mode = CursorGrabMode::None;
        win.cursor_options.visible = true;
    }
    speak_voice("Pick a craft recipe, build stamp, or engage hyperfocus!", "derek");
    spawn_derek_craft_modal(commands, stats, data);
}

#[allow(clippy::too_many_arguments)]
fn update_craft_voxels(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    orbit: Res<CameraOrbit>,
    mut stats: ResMut<PlayerStats>,
    mut data: ResMut<CraftWorldData>,
    mut prompt: ResMut<CraftInteractPrompt>,
    assets: Option<Res<CraftBlockAssets>>,
    mut rq: VoxelRayQueries,
    mut timers: Local<(f32, f32, f32, bool)>,
) {
    let dt = time.delta_secs().min(0.05);
    let (mine_cd, place_cd, scroll_acc, prev_block_btn) = &mut *timers;
    *mine_cd = (*mine_cd - dt).max(0.0);
    *place_cd = (*place_cd - dt).max(0.0);
    if data.feedback_timer > 0.0 {
        data.feedback_timer = (data.feedback_timer - dt).max(0.0);
    }

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

    // Auto-open Derek's animated tutorial video once on first entry into Craft World
    if data.tutorial_auto_delay > 0.0 && !stats.craft_modal_open {
        data.tutorial_auto_delay = (data.tutorial_auto_delay - dt).max(0.0);
        if data.tutorial_auto_delay <= 0.0
            && !data.tutorial_video_shown
            && !crate::audio::debug_autoskip_enabled()
        {
            data.tutorial_video_shown = true;
            open_derek_tutorial_video();
        }
    }

    if take_derek_tutorial_closed() {
        stats.casino_esc_grace = true;
    }

    if stats.craft_modal_open || is_derek_tutorial_open() || is_craft_sync_modal_open() {
        for (_, mut vis) in &mut rq.mine_cursor_q {
            *vis = Visibility::Hidden;
        }
        for (_, mut vis, _) in &mut rq.ghost_q {
            *vis = Visibility::Hidden;
        }
        data.target_label.clear();
        return;
    }

    // Check HUD UI interactions first so clicking HUD pills doesn't mine/place behind them
    let mut ui_pointer_busy = false;
    let mut hud_open_bench = false;
    let mut hud_cycle_shape = false;
    let mut hud_toggle_view = false;
    let mut hud_open_sync = false;
    let mut hud_open_tutorial = false;
    for (inter, hotbar, quick, _prompt_pill) in &rq.ui_inter_q {
        if matches!(*inter, Interaction::Pressed | Interaction::Hovered) {
            ui_pointer_busy = true;
        }
        if *inter == Interaction::Pressed {
            if let Some(h) = hotbar {
                data.selected_slot = h.0.min(5);
            }
            if let Some(q) = quick {
                match q.0 {
                    CraftQuickAction::OpenBench => hud_open_bench = true,
                    CraftQuickAction::CycleShape => hud_cycle_shape = true,
                    CraftQuickAction::ToggleView => hud_toggle_view = true,
                    CraftQuickAction::OpenSync => hud_open_sync = true,
                    CraftQuickAction::OpenTutorial => hud_open_tutorial = true,
                }
            }
        }
    }

    if hud_open_sync {
        if let Ok(mut win) = rq.window_q.get_single_mut() {
            win.cursor_options.grab_mode = CursorGrabMode::None;
            win.cursor_options.visible = true;
        }
        let encoded = encode_craft_world_save(&data, stats.craft_pub217_rebuilt);
        open_craft_sync_modal(
            &encoded,
            data.blocks.len() as u32,
            stats.craft_pub217_rebuilt,
            data.house_completed,
            true,
        );
        return;
    }

    if keys.just_pressed(KeyCode::KeyT) || hud_open_tutorial {
        data.tutorial_video_shown = true;
        if let Ok(mut win) = rq.window_q.get_single_mut() {
            win.cursor_options.grab_mode = CursorGrabMode::None;
            win.cursor_options.visible = true;
        }
        open_derek_tutorial_video();
        return;
    }

    if keys.just_pressed(KeyCode::KeyB)
        || keys.just_pressed(KeyCode::Tab)
        || take_touch_craft_bench()
        || hud_open_bench
    {
        open_derek_bench_modal(
            &mut commands,
            &mut stats,
            &data,
            &mut prompt,
            &mut rq.window_q,
        );
        return;
    }

    if keys.just_pressed(KeyCode::KeyG) || take_touch_craft_shape() || hud_cycle_shape {
        data.build_shape = data.build_shape.next();
        set_touch_craft_shape(data.build_shape.short_tag());
        play_sfx("esxi_click");
        let msg = format!("BUILD SHAPE: {}", data.build_shape.label());
        push_craft_toast(&mut data, msg);
    }

    if keys.just_pressed(KeyCode::KeyV) || take_touch_craft_view() || hud_toggle_view {
        stats.craft_first_person = !stats.craft_first_person;
        play_sfx("esxi_click");
        push_craft_toast(
            &mut data,
            if stats.craft_first_person {
                "VIEW: MINECRAFT 1ST-PERSON"
            } else {
                "VIEW: VALHEIM OVER-SHOULDER"
            },
        );
    }

    // Slot selection via [1..6], [C], mouse wheel, hotbar clicks, or touch NEXT BLK button
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
    if scroll.delta.y.abs() > 0.0 {
        let delta_steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.025,
        };
        *scroll_acc += delta_steps;
        if *scroll_acc >= 0.45 {
            data.selected_slot = (data.selected_slot + 5) % 6;
            *scroll_acc = 0.0;
        } else if *scroll_acc <= -0.45 {
            data.selected_slot = (data.selected_slot + 1) % 6;
            *scroll_acc = 0.0;
        }
    } else {
        *scroll_acc *= 0.85;
    }

    let Ok(p_tf) = rq.player_q.get_single() else {
        return;
    };
    let Ok(cam_tf) = rq.cam_q.get_single() else {
        return;
    };

    // Exact camera-crosshair raycast (matches + crosshair pixel-for-pixel!)
    let origin = cam_tf.translation;
    let dir = *cam_tf.forward();
    let player_eye = p_tf.translation + Vec3::Y * 1.45;
    let reach = 12.0_f32;

    let mut best_t = f32::INFINITY;
    let mut hit_cell: Option<((i32, i32, i32), BlockKind, (i32, i32, i32))> = None;
    for (&coord, &kind) in &data.blocks {
        let center = grid_to_world(coord);
        if center.distance(player_eye) > reach {
            continue;
        }
        let bmin = center - Vec3::splat(0.5);
        let bmax = center + Vec3::splat(0.5);
        if let Some((t_hit, normal)) = ray_voxel_aabb(origin, dir, bmin, bmax) {
            if t_hit < best_t {
                best_t = t_hit;
                hit_cell = Some((coord, kind, normal));
            }
        }
    }

    // Check ground plane y = 0.0 if closer than any hit block
    let mut place_cell: Option<(i32, i32, i32)> = None;
    let mut ground_hit = false;
    if dir.y < -0.004 {
        let t_g = -origin.y / dir.y;
        if t_g > 0.25 && t_g < best_t {
            let gp = origin + dir * t_g;
            if gp.distance(player_eye) <= reach {
                let rel = gp - CRAFT_LEVEL_ORIGIN;
                let gx = rel.x.round() as i32;
                let gz = rel.z.round() as i32;
                if (-24..=24).contains(&gx) && (-24..=24).contains(&gz) {
                    let gcell = (gx, 0, gz);
                    if !data.blocks.contains_key(&gcell) {
                        place_cell = Some(gcell);
                        ground_hit = true;
                        hit_cell = None;
                    }
                }
            }
        }
    }

    if !ground_hit {
        if let Some((coord, _, normal)) = hit_cell {
            let adj = (coord.0 + normal.0, coord.1 + normal.1, coord.2 + normal.2);
            if (-24..=24).contains(&adj.0)
                && (0..=10).contains(&adj.1)
                && (-24..=24).contains(&adj.2)
                && !data.blocks.contains_key(&adj)
            {
                place_cell = Some(adj);
            }
        }
    }

    let slot = data.selected_slot.min(5);
    let sel_kind = HOTBAR_BLOCKS[slot];
    let has_stock = data.inventory[slot] > 0;
    let pulse = 1.0 + (time.elapsed_secs() * 7.0).sin() * 0.018;

    // 1. Update Red Mine Target Box around exact hit block
    if let Ok((mut m_tf, mut m_vis)) = rq.mine_cursor_q.get_single_mut() {
        if let Some((c, _, _)) = hit_cell {
            m_tf.translation = grid_to_world(c);
            m_tf.scale = Vec3::splat(1.05 * pulse);
            *m_vis = Visibility::Inherited;
        } else {
            *m_vis = Visibility::Hidden;
        }
    }

    let Some(assets) = assets else {
        return;
    };

    // 2. Update Cyan / Red Valheim Ghost Preview (scaled to 1x1 or 3x3 stamp!)
    if let Ok((mut g_tf, mut g_vis, mut g_mat)) = rq.ghost_q.get_single_mut() {
        if let Some(pc) = place_cell {
            let (sc, off) = data.build_shape.ghost_scale_and_offset(orbit.yaw);
            g_tf.translation = grid_to_world(pc) + off;
            g_tf.scale = sc;
            g_mat.0 = if has_stock {
                assets.ghost_ok_mat.clone()
            } else {
                assets.ghost_bad_mat.clone()
            };
            *g_vis = Visibility::Inherited;
        } else {
            *g_vis = Visibility::Hidden;
        }
    }

    // Live crosshair target readout
    data.target_has_hit = hit_cell.is_some();
    data.target_can_place = place_cell.is_some();
    data.target_out_of_stock = !has_stock;
    data.target_label = if !has_stock && place_cell.is_some() && hit_cell.is_none() {
        format!(
            "OUT OF {}! Scroll/[1-6] or press [B] Craft Bench",
            sel_kind.short_name()
        )
    } else if let Some((_, kind, _)) = hit_cell {
        if place_cell.is_some() && has_stock {
            format!(
                "{}  |  [L-CLICK] Mine   [R-CLICK] Place {} ({})",
                kind.full_name(),
                sel_kind.short_name(),
                data.build_shape.short_tag()
            )
        } else {
            format!("{}  |  [L-CLICK] Mine Block", kind.full_name())
        }
    } else if let Some(pc) = place_cell {
        let zone = if in_pub217_zone(pc) {
            format!(
                "PUB 217 PLOT ({}/{})",
                data.pub_blocks_placed.min(PUB217_TARGET_BLOCKS),
                PUB217_TARGET_BLOCKS
            )
        } else if stats.craft_pub217_rebuilt && in_house_zone(pc) {
            format!(
                "STAN'S HOUSE ({}/{})",
                data.house_blocks_placed.min(HOUSE_TARGET_BLOCKS),
                HOUSE_TARGET_BLOCKS
            )
        } else {
            "BUILD GRID".to_string()
        };
        format!(
            "{}  |  [R-CLICK] Place {} ({})",
            zone,
            sel_kind.short_name(),
            data.build_shape.label()
        )
    } else {
        "Aim at Tree, Rubble, or Ground  |  [B] Craft Bench   [T] Derek Tutorial".to_string()
    };

    // Continuous hold-to-mine & hold-to-place (also catches single-frame rapid clicks)
    let touch_atk_tap = crate::audio::take_touch_attack();
    let touch_atk_held = crate::audio::touch_attack_held();
    let want_mine = !ui_pointer_busy
        && (mouse.pressed(MouseButton::Left)
            || mouse.just_pressed(MouseButton::Left)
            || keys.pressed(KeyCode::KeyJ)
            || keys.just_pressed(KeyCode::KeyJ)
            || keys.pressed(KeyCode::KeyX)
            || keys.just_pressed(KeyCode::KeyX)
            || touch_atk_held
            || touch_atk_tap);
    let touch_blk_now = crate::audio::touch_block_held();
    let touch_blk_tap = touch_blk_now && !*prev_block_btn;
    *prev_block_btn = touch_blk_now;
    let want_place = !ui_pointer_busy
        && (mouse.pressed(MouseButton::Right)
            || mouse.just_pressed(MouseButton::Right)
            || keys.pressed(KeyCode::KeyR)
            || keys.just_pressed(KeyCode::KeyR)
            || touch_blk_now
            || touch_blk_tap);

    if want_mine && *mine_cd <= 0.0 {
        if let Some((c, _, _)) = hit_cell {
            if let Some(kind) = data.blocks.remove(&c) {
                *mine_cd = 0.22;
                for (e, vb) in &rq.block_q {
                    if vb.coord == c {
                        commands.entity(e).despawn_recursive();
                        break;
                    }
                }
                let toast = match kind {
                    BlockKind::Leaves => {
                        data.inventory[BlockKind::Wood as usize] += 1;
                        "+1 WOOD".to_string()
                    }
                    BlockKind::TendyOre => {
                        data.inventory[BlockKind::TendyGold as usize] += 2;
                        data.inventory[BlockKind::Neon217 as usize] += 1;
                        "+2 TENDY GOLD  +1 NEON".to_string()
                    }
                    BlockKind::SupplyCube => {
                        for n in &mut data.inventory {
                            *n += 4;
                        }
                        stats.score += 150;
                        stats.hint_text = "BLACK HOLE SUPPLY CUBE! +4 OF EVERY BLOCK!".to_string();
                        stats.hint_bump = stats.hint_bump.wrapping_add(1);
                        "SUPPLY CUBE! +4 ALL BLOCKS".to_string()
                    }
                    k if (k as usize) < 6 => {
                        data.inventory[k as usize] += 2;
                        format!("+2 {}", k.short_name())
                    }
                    _ => "+1 BLOCK".to_string(),
                };
                push_craft_toast(&mut data, toast);
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
                net_event(EVENT_CRAFT_VOXEL, pack_craft_voxel(c.0, c.1, c.2, CRAFT_VOXEL_REMOVE));
                persist_craft_world(&data, stats.craft_pub217_rebuilt);
            }
        }
    } else if want_place && *place_cd <= 0.0 {
        if let Some(base_c) = place_cell {
            if !has_stock {
                *place_cd = 0.28;
                play_sfx("hack_deny");
                push_craft_toast(
                    &mut data,
                    format!("OUT OF {}! Press [B] to Craft!", sel_kind.short_name()),
                );
            } else {
                let offsets = data.build_shape.offsets(orbit.yaw);
                let mut placed_n = 0_u32;
                for (off, want_glass) in offsets {
                    let c = (base_c.0 + off.0, base_c.1 + off.1, base_c.2 + off.2);
                    if !(-24..=24).contains(&c.0)
                        || !(0..=10).contains(&c.1)
                        || !(-24..=24).contains(&c.2)
                        || data.blocks.contains_key(&c)
                    {
                        continue;
                    }
                    let wp = grid_to_world(c);
                    let flat_p = Vec2::new(wp.x - p_tf.translation.x, wp.z - p_tf.translation.z).length();
                    let overlaps_player =
                        flat_p < 0.72 && (wp.y - (p_tf.translation.y + 0.9)).abs() < 1.1;
                    if overlaps_player {
                        continue;
                    }
                    let glass_idx = BlockKind::Glass as usize;
                    let (use_slot, use_kind) = if want_glass && data.inventory[glass_idx] > 0 {
                        (glass_idx, BlockKind::Glass)
                    } else {
                        (slot, sel_kind)
                    };
                    if data.inventory[use_slot] == 0 {
                        continue;
                    }
                    data.inventory[use_slot] -= 1;
                    data.blocks.insert(c, use_kind);
                    spawn_voxel_entity(&mut commands, &assets, c, use_kind);
                    net_event(EVENT_CRAFT_VOXEL, pack_craft_voxel(c.0, c.1, c.2, use_kind as u8));
                    placed_n += 1;
                }
                if placed_n > 0 {
                    *place_cd = if matches!(data.build_shape, BuildShape::Single) {
                        0.16
                    } else {
                        0.32
                    };
                    play_sfx("block_place");
                    stats.score += 35 * placed_n;
                    recount_quest_blocks(&mut data);
                    persist_craft_world(&data, stats.craft_pub217_rebuilt);
                    if placed_n > 1 {
                        let msg = format!("PLACED {} ({} BLOCKS)!", data.build_shape.label(), placed_n);
                        push_craft_toast(&mut data, msg);
                    }
                }
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
        net_event(
            EVENT_CRAFT_VOXEL,
            pack_craft_voxel(c.0, c.1, c.2, BlockKind::SupplyCube as u8),
        );
        persist_craft_world(&data, stats.craft_pub217_rebuilt);
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
        persist_craft_world(&data, stats.craft_pub217_rebuilt);
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
        persist_craft_world(&data, stats.craft_pub217_rebuilt);
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

    if stats.craft_modal_open || is_derek_tutorial_open() || is_craft_sync_modal_open() {
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
                prompt.modal_just_opened = true;
                speak_voice("Pick a craft recipe, build stamp, or engage hyperfocus!", "derek");
                spawn_derek_craft_modal(&mut commands, &stats, &data);
            } else {
                prompt.modal_is_derek = false;
                prompt.modal_just_opened = true;
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
            BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.74)),
            GlobalZIndex(850),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(580.0),
                    max_width: Val::Percent(94.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(9.0),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.06, 0.14, 0.97)),
                BorderColor(Color::srgb(0.45, 0.95, 0.65)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("DEREK'S CRAFTING & BLUEPRINT WORKBENCH [B / TAB]"),
                    TextFont { font_size: 17.5, ..default() },
                    TextColor(Color::srgb(0.48, 1.0, 0.72)),
                ));

                // Live color-swatched inventory strip
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|inv| {
                    for (idx, kind) in HOTBAR_BLOCKS.iter().enumerate() {
                        inv.spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(5.0),
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.12, 0.14, 0.24, 0.9)),
                            BorderColor(Color::srgba(0.55, 0.65, 0.85, 0.4)),
                            BorderRadius::all(Val::Px(6.0)),
                        ))
                        .with_children(|chip| {
                            chip.spawn((
                                Node {
                                    width: Val::Px(11.0),
                                    height: Val::Px(11.0),
                                    ..default()
                                },
                                BackgroundColor(kind.ui_color()),
                                BorderRadius::all(Val::Px(2.0)),
                            ));
                            chip.spawn((
                                Text::new(format!("{}: {}", kind.short_name(), data.inventory[idx])),
                                TextFont { font_size: 12.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                let brick_ready = data.inventory[BlockKind::Stone as usize] >= 2;
                let stone_ready = data.inventory[BlockKind::Wood as usize] >= 2;
                let glass_ready = data.inventory[BlockKind::Wood as usize] >= 1;
                let neon_ready = data.inventory[BlockKind::TendyGold as usize] >= 1;

                let recipes = [
                    (
                        CraftModalAction::CraftBrick,
                        format!(
                            "CRAFT +4 BRICK   (Need 2 STONE, Have {})  {}",
                            data.inventory[BlockKind::Stone as usize],
                            if brick_ready { "-- READY" } else { "-- NEED STONE" }
                        ),
                        if brick_ready { Color::srgb(0.46, 0.18, 0.18) } else { Color::srgb(0.18, 0.14, 0.18) },
                        brick_ready,
                    ),
                    (
                        CraftModalAction::CraftStone,
                        format!(
                            "CRAFT +4 STONE   (Need 2 WOOD, Have {})   {}",
                            data.inventory[BlockKind::Wood as usize],
                            if stone_ready { "-- READY" } else { "-- NEED WOOD" }
                        ),
                        if stone_ready { Color::srgb(0.24, 0.28, 0.36) } else { Color::srgb(0.15, 0.16, 0.22) },
                        stone_ready,
                    ),
                    (
                        CraftModalAction::CraftGlass,
                        format!(
                            "CRAFT +3 GLASS   (Need 1 WOOD, Have {})   {}",
                            data.inventory[BlockKind::Wood as usize],
                            if glass_ready { "-- READY" } else { "-- NEED WOOD" }
                        ),
                        if glass_ready { Color::srgb(0.14, 0.40, 0.56) } else { Color::srgb(0.14, 0.20, 0.26) },
                        glass_ready,
                    ),
                    (
                        CraftModalAction::CraftNeon,
                        format!(
                            "CRAFT +3 NEON217 (Need 1 TENDY, Have {})  {}",
                            data.inventory[BlockKind::TendyGold as usize],
                            if neon_ready { "-- READY" } else { "-- NEED TENDY" }
                        ),
                        if neon_ready { Color::srgb(0.56, 0.36, 0.08) } else { Color::srgb(0.22, 0.18, 0.14) },
                        neon_ready,
                    ),
                    (
                        CraftModalAction::FreeScrapKit,
                        "SCAVENGE VOID SCRAP (+6 BRICK, +6 WOOD, +4 STONE)".to_string(),
                        Color::srgb(0.20, 0.24, 0.42),
                        true,
                    ),
                ];

                for (act, label, bg, ready) in recipes {
                    card.spawn((
                        act,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(13.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor(if ready {
                            Color::srgb(0.45, 0.96, 0.62)
                        } else {
                            Color::srgba(0.95, 0.40, 0.40, 0.55)
                        }),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(label), TextFont { font_size: 13.0, ..default() }, TextColor(Color::WHITE)));
                    });
                }

                // Valheim Quick-Build Stamps row
                card.spawn((
                    Text::new("VALHEIM QUICK-BUILD STAMPS (Right-Click Places Entire Shape!):"),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.42, 0.92, 1.0)),
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    for sh in [
                        BuildShape::Single,
                        BuildShape::Wall3x3,
                        BuildShape::Floor3x3,
                        BuildShape::Window3x3,
                    ] {
                        let active = data.build_shape == sh;
                        row.spawn((
                            CraftModalAction::SelectShape(sh),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(11.0), Val::Px(7.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(if active {
                                Color::srgb(0.14, 0.46, 0.58)
                            } else {
                                Color::srgb(0.14, 0.18, 0.30)
                            }),
                            BorderColor(if active {
                                Color::srgb(1.0, 0.88, 0.32)
                            } else {
                                Color::srgba(0.42, 0.90, 1.0, 0.55)
                            }),
                            BorderRadius::all(Val::Px(7.0)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new(sh.label()),
                                TextFont { font_size: 12.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                if !stats.craft_pub217_rebuilt {
                    card.spawn((
                        CraftModalAction::DerekHyperfocusPub217,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
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
                            TextFont { font_size: 13.5, ..default() },
                            TextColor(Color::srgb(1.0, 0.94, 0.45)),
                        ));
                    });
                } else if !data.house_completed {
                    card.spawn((
                        CraftModalAction::DerekHyperfocusHouse,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.10, 0.44, 0.64)),
                        BorderColor(Color::srgb(0.42, 0.95, 1.0)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("DEREK HYPERFOCUS: AUTO-BUILD STAN'S NEW HOUSE!"),
                            TextFont { font_size: 13.5, ..default() },
                            TextColor(Color::srgb(1.0, 0.95, 0.55)),
                        ));
                    });
                }

                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|foot| {
                    foot.spawn((
                        CraftModalAction::OpenSyncModal,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.16, 0.44)),
                        BorderColor(Color::srgb(0.45, 0.90, 1.0)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("SYNC / SHARE SAVE [Y]"),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::srgb(0.68, 0.96, 1.0)),
                        ));
                    });

                    foot.spawn((
                        CraftModalAction::WatchTutorial,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(13.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.42, 0.26, 0.06)),
                        BorderColor(Color::srgb(1.0, 0.84, 0.28)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("WATCH DEREK TUTORIAL VIDEO [T]"),
                            TextFont { font_size: 12.5, ..default() },
                            TextColor(Color::srgb(1.0, 0.92, 0.42)),
                        ));
                    });

                    foot.spawn((
                        CraftModalAction::Close,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.22, 0.14, 0.22)),
                        BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.45)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("BACK TO BUILD [ESC / B]"), TextFont { font_size: 12.5, ..default() }, TextColor(Color::WHITE)));
                    });
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
    mut prompt: ResMut<CraftInteractPrompt>,
    assets: Option<Res<CraftBlockAssets>>,
    mut next_state: SceneTransition,
    btn_q: Query<(&Interaction, &CraftModalAction), Changed<Interaction>>,
    modal_q: Query<Entity, With<CraftModalRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !stats.craft_modal_open {
        return;
    }

    let just_opened = std::mem::take(&mut prompt.modal_just_opened);
    let mut chosen = None;
    if keys.just_pressed(KeyCode::Escape)
        || (!just_opened && (keys.just_pressed(KeyCode::KeyB) || keys.just_pressed(KeyCode::Tab)))
    {
        chosen = Some(CraftModalAction::Close);
    } else if !just_opened && keys.just_pressed(KeyCode::KeyT) {
        chosen = Some(CraftModalAction::WatchTutorial);
    } else if !just_opened && keys.just_pressed(KeyCode::KeyY) {
        chosen = Some(CraftModalAction::OpenSyncModal);
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
                data.inventory[BlockKind::Brick as usize] += 4;
                play_sfx("block_place");
                push_craft_toast(&mut data, "+4 BRICK CRAFTED!");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::CraftStone => {
            if data.inventory[BlockKind::Wood as usize] >= 2 {
                data.inventory[BlockKind::Wood as usize] -= 2;
                data.inventory[BlockKind::Stone as usize] += 4;
                play_sfx("block_place");
                push_craft_toast(&mut data, "+4 STONE CRAFTED!");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::CraftGlass => {
            if data.inventory[BlockKind::Wood as usize] >= 1 {
                data.inventory[BlockKind::Wood as usize] -= 1;
                data.inventory[BlockKind::Glass as usize] += 3;
                play_sfx("block_place");
                push_craft_toast(&mut data, "+3 GLASS CRAFTED!");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::CraftNeon => {
            if data.inventory[BlockKind::TendyGold as usize] >= 1 {
                data.inventory[BlockKind::TendyGold as usize] -= 1;
                data.inventory[BlockKind::Neon217 as usize] += 3;
                play_sfx("block_place");
                push_craft_toast(&mut data, "+3 NEON 217 CRAFTED!");
            } else {
                play_sfx("hack_deny");
            }
        }
        CraftModalAction::FreeScrapKit => {
            data.inventory[BlockKind::Brick as usize] += 6;
            data.inventory[BlockKind::Wood as usize] += 6;
            data.inventory[BlockKind::Stone as usize] += 4;
            play_sfx("powerup");
            push_craft_toast(&mut data, "+6 BRICK  +6 WOOD  +4 STONE");
        }
        CraftModalAction::SelectShape(sh) => {
            data.build_shape = sh;
            set_touch_craft_shape(sh.short_tag());
            play_sfx("esxi_click");
            push_craft_toast(&mut data, format!("STAMP: {} (R-Click to Place!)", sh.label()));
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            stop_voice();
            return;
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
            if !stats.craft_pub217_rebuilt && data.pub_blocks_placed >= PUB217_TARGET_BLOCKS {
                stats.craft_pub217_rebuilt = true;
            }
            let raw_save = encode_craft_world_save(&data, stats.craft_pub217_rebuilt);
            save_craft_world(&raw_save);
            let sent = broadcast_craft_world_sync(
                &raw_save,
                data.blocks.len() as u32,
                stats.craft_pub217_rebuilt,
                data.house_completed,
            );
            if sent > 0 {
                push_craft_toast(&mut data, format!("AUTO-BUILT & SHARED SYNC INVITE ({})!", sent));
            }
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            return;
        }
        CraftModalAction::DerekHyperfocusHouse => {
            if let Some(assets) = assets {
                // Auto-build Stan & the boys' new Brick + Wood + Glass + TendyGold house!
                for gx in 3..=11 {
                    for gz in -6..=2 {
                        let edge = gx == 3 || gx == 11 || gz == -6 || gz == 2;
                        let door = gx == 3 && (gz == -2 || gz == -1);
                        if edge && !door {
                            for gy in 0..=2 {
                                let k = if gy == 1 && (gx == 7 || gz == -2) {
                                    BlockKind::Glass
                                } else if gy == 2 {
                                    BlockKind::TendyGold
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
            if stats.craft_pub217_rebuilt && !data.house_completed && data.house_blocks_placed >= HOUSE_TARGET_BLOCKS {
                data.house_completed = true;
            }
            let raw_save = encode_craft_world_save(&data, stats.craft_pub217_rebuilt);
            save_craft_world(&raw_save);
            let sent = broadcast_craft_world_sync(
                &raw_save,
                data.blocks.len() as u32,
                stats.craft_pub217_rebuilt,
                data.house_completed,
            );
            if sent > 0 {
                push_craft_toast(&mut data, format!("HOUSE BUILT & SHARED SYNC INVITE ({})!", sent));
            }
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            return;
        }
        CraftModalAction::OpenSyncModal => {
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            stop_voice();
            let raw_save = encode_craft_world_save(&data, stats.craft_pub217_rebuilt);
            open_craft_sync_modal(
                &raw_save,
                data.blocks.len() as u32,
                stats.craft_pub217_rebuilt,
                data.house_completed,
                true,
            );
            return;
        }
        CraftModalAction::WatchTutorial => {
            for e in &modal_q {
                commands.entity(e).despawn_recursive();
            }
            stats.craft_modal_open = false;
            stats.casino_esc_grace = true;
            set_touch_playing(true);
            stop_voice();
            open_derek_tutorial_video();
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
            | CraftModalAction::CraftStone
            | CraftModalAction::CraftGlass
            | CraftModalAction::CraftNeon
            | CraftModalAction::FreeScrapKit
    ) {
        persist_craft_world(&data, stats.craft_pub217_rebuilt);
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

#[derive(bevy::ecs::system::SystemParam)]
struct CraftHudQueries<'w, 's> {
    quest_q: Query<'w, 's, &'static mut Text, With<CraftQuestText>>,
    cross_q: Query<
        'w,
        's,
        &'static mut Visibility,
        (
            With<CraftCrosshair>,
            Without<CraftTargetPill>,
            Without<CraftFeedbackPill>,
            Without<CraftPromptPill>,
        ),
    >,
    target_pill_q: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility, &'static mut BorderColor),
        (
            With<CraftTargetPill>,
            Without<CraftCrosshair>,
            Without<CraftFeedbackPill>,
            Without<CraftPromptPill>,
            Without<CraftHotbarBtn>,
        ),
    >,
    target_txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftTargetText>,
            Without<CraftQuestText>,
            Without<CraftFeedbackText>,
            Without<CraftPromptText>,
            Without<CraftHotbarText>,
            Without<CraftShapeBtnText>,
            Without<CraftViewBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    feedback_pill_q: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility),
        (
            With<CraftFeedbackPill>,
            Without<CraftCrosshair>,
            Without<CraftTargetPill>,
            Without<CraftPromptPill>,
        ),
    >,
    feedback_txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftFeedbackText>,
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftPromptText>,
            Without<CraftHotbarText>,
            Without<CraftShapeBtnText>,
            Without<CraftViewBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    pill_q: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility),
        (
            With<CraftPromptPill>,
            Without<CraftCrosshair>,
            Without<CraftTargetPill>,
            Without<CraftFeedbackPill>,
        ),
    >,
    ptext_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftPromptText>,
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftFeedbackText>,
            Without<CraftHotbarText>,
            Without<CraftShapeBtnText>,
            Without<CraftViewBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    slot_q: Query<
        'w,
        's,
        (&'static CraftHotbarBtn, &'static mut BorderColor, &'static mut BackgroundColor),
        Without<CraftTargetPill>,
    >,
    slot_txt_q: Query<
        'w,
        's,
        (&'static CraftHotbarText, &'static mut Text),
        (
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftFeedbackText>,
            Without<CraftPromptText>,
            Without<CraftShapeBtnText>,
            Without<CraftViewBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    shape_txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftShapeBtnText>,
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftFeedbackText>,
            Without<CraftPromptText>,
            Without<CraftHotbarText>,
            Without<CraftViewBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    view_txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftViewBtnText>,
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftFeedbackText>,
            Without<CraftPromptText>,
            Without<CraftHotbarText>,
            Without<CraftShapeBtnText>,
            Without<CraftSyncBtnText>,
        ),
    >,
    sync_txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<CraftSyncBtnText>,
            Without<CraftQuestText>,
            Without<CraftTargetText>,
            Without<CraftFeedbackText>,
            Without<CraftPromptText>,
            Without<CraftHotbarText>,
            Without<CraftShapeBtnText>,
            Without<CraftViewBtnText>,
        ),
    >,
}

fn update_craft_hud(
    stats: Res<PlayerStats>,
    data: Res<CraftWorldData>,
    prompt: Res<CraftInteractPrompt>,
    net: Res<crate::net::NetState>,
    mut q: CraftHudQueries,
) {
    let hide_play_hud =
        stats.craft_modal_open || is_derek_tutorial_open() || is_craft_sync_modal_open();

    if let Ok(mut qt) = q.quest_q.get_single_mut() {
        let next_msg = if !stats.craft_pub217_rebuilt {
            format!(
                "QUEST 1: REBUILD PUB 217 ({}/{} blocks) -- Amber Plot West",
                data.pub_blocks_placed.min(PUB217_TARGET_BLOCKS),
                PUB217_TARGET_BLOCKS
            )
        } else if !data.house_completed {
            format!(
                "PUB 217 REBUILT! | QUEST 2: BUILD STAN'S HOUSE ({}/{}) -- Cyan Plot East",
                data.house_blocks_placed.min(HOUSE_TARGET_BLOCKS),
                HOUSE_TARGET_BLOCKS
            )
        } else {
            "PUB 217 & STAN'S HOUSE COMPLETE! Party inside or South Portal -> City!".to_string()
        };
        if qt.0 != next_msg {
            qt.0 = next_msg;
        }
    }

    if let Ok(mut vis) = q.cross_q.get_single_mut() {
        *vis = if hide_play_hud {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }

    if let Ok((mut node, mut vis, mut border)) = q.target_pill_q.get_single_mut() {
        if hide_play_hud || data.target_label.is_empty() {
            node.display = Display::None;
            *vis = Visibility::Hidden;
        } else {
            node.display = Display::Flex;
            *vis = Visibility::Inherited;
            border.0 = if data.target_out_of_stock && data.target_can_place && !data.target_has_hit {
                Color::srgb(1.0, 0.32, 0.28)
            } else if data.target_has_hit {
                Color::srgb(1.0, 0.56, 0.24)
            } else {
                Color::srgb(0.28, 0.90, 1.0)
            };
            if let Ok(mut tt) = q.target_txt_q.get_single_mut() {
                if tt.0 != data.target_label {
                    tt.0 = data.target_label.clone();
                }
            }
        }
    }

    if let Ok((mut node, mut vis)) = q.feedback_pill_q.get_single_mut() {
        if !hide_play_hud && data.feedback_timer > 0.0 && !data.feedback_msg.is_empty() {
            node.display = Display::Flex;
            *vis = Visibility::Inherited;
            if let Ok(mut ft) = q.feedback_txt_q.get_single_mut() {
                if ft.0 != data.feedback_msg {
                    ft.0 = data.feedback_msg.clone();
                }
            }
        } else {
            node.display = Display::None;
            *vis = Visibility::Hidden;
        }
    }

    if let Ok((mut node, mut vis)) = q.pill_q.get_single_mut() {
        match prompt.target {
            Some(tgt) if !hide_play_hud => {
                node.display = Display::Flex;
                *vis = Visibility::Inherited;
                let msg = match tgt {
                    CraftPromptTarget::Portal => "MYSTERIOUS PORTAL -> CEDAR RAPIDS [F/E]",
                    CraftPromptTarget::BeerTower => "POUR COLD BEER AT PUB 217 [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Maria) => "TALK TO MARIA [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Derek) => "DEREK'S WORKBENCH [F/E / B]",
                    CraftPromptTarget::Npc(CraftNpcRole::Mike) => "SMOKE BREAK WITH MIKE [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Tj) => "TALK TO TJ [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Nate) => "TALK TO NATE [F/E]",
                    CraftPromptTarget::Npc(CraftNpcRole::Glenn) => "TALK TO GLENN [F/E]",
                };
                if let Ok(mut pt) = q.ptext_q.get_single_mut() {
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

    for (btn, mut border, mut bg) in &mut q.slot_q {
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

    for (st, mut txt) in &mut q.slot_txt_q {
        let i = st.0.min(5);
        let label = format!("[{}] {} x{}", i + 1, HOTBAR_BLOCKS[i].short_name(), data.inventory[i]);
        if txt.0 != label {
            txt.0 = label;
        }
    }

    if let Ok(mut st) = q.shape_txt_q.get_single_mut() {
        let want = format!("[G] {}", data.build_shape.label());
        if st.0 != want {
            st.0 = want;
        }
    }

    if let Ok(mut vt) = q.view_txt_q.get_single_mut() {
        let want = if stats.craft_first_person {
            "[V] 1ST PERSON"
        } else {
            "[V] 3RD PERSON"
        };
        if vt.0 != want {
            vt.0 = want.to_string();
        }
    }

    if let Ok(mut st) = q.sync_txt_q.get_single_mut() {
        let peers = net.peer_count();
        let want = if has_craft_sync_offer() {
            "[Y] SYNC OFFER!".to_string()
        } else if peers > 0 {
            format!("[Y] SYNC ({} ON)", peers)
        } else {
            "[Y] SYNC WORLD".to_string()
        };
        if st.0 != want {
            st.0 = want;
        }
    }
}
