use bevy::prelude::*;
use crate::state::{FridgeGoal, LevelEntity, PlatformBox, PowerupKind, PowerupPickup, TendyPickup};

pub fn setup_lighting_and_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 620.0,
    });

    // Single warm sunny kitchen directional light (WebGL2 limit = 1 directional light)
    commands.spawn((
        DirectionalLight {
            illuminance: 20000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.86),
            ..default()
        },
        Transform::from_xyz(6.0, 26.0, 15.0).looking_at(Vec3::new(4.0, -2.0, -6.0), Vec3::Y),
    ));

    // Warm overhead kitchen & dining room fill lights
    for (x, z) in [(-4.0_f32, 10.0_f32), (12.0_f32, 0.0_f32), (8.0_f32, -16.0_f32)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.94, 0.84),
                intensity: 140000.0,
                range: 48.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 9.0, z),
        ));
    }

    // 3rd-person follow camera (updated by update_camera)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.2, 21.8).looking_at(Vec3::new(0.0, 0.7, 16.0), Vec3::Y),
    ));
}

pub fn spawn_kitchen_level(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_kitchen_level_inner(&mut commands, &asset_server, &mut meshes, &mut materials);
}

pub fn spawn_kitchen_level_inner(
    commands: &mut Commands,
    asset_server: &AssetServer,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Blender Katamari-scale Kitchen & Dining Room environment (countertop, island, bridge, stools, table, floor)
    let kitchen_scene: Handle<Scene> = asset_server.load("models/kitchen_environment.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(kitchen_scene),
        Transform::IDENTITY,
    ));

    // Zone 1: Prep Countertop: X in [-7, 7], Z in [-22, 22], top at Y = 0.0 (down to -6.0 floor)
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(0.0, -3.0, 0.0),
        PlatformBox {
            half_extents: Vec3::new(7.0, 3.0, 22.0),
        },
    ));

    // Jumpable Kitchen Sponge platforms (visual at Y = 0, collider top at Y = 0.9)
    let sponge_scene: Handle<Scene> = asset_server.load("models/kitchen_sponge.glb#Scene0");
    for (x, z) in [(0.0_f32, 8.0_f32), (-2.5_f32, -5.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(sponge_scene.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(1.15)),
        ));
        commands.spawn((
            LevelEntity,
            Transform::from_xyz(x, 0.45, z),
            PlatformBox {
                half_extents: Vec3::new(1.25, 0.45, 0.8),
            },
        ));
    }

    // Giant Pancake Stack landmark at Z = -17.5 (top at Y = 1.9)
    let pancake_scene: Handle<Scene> = asset_server.load("models/pancake_stack.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(pancake_scene),
        Transform::from_xyz(0.0, 0.0, -17.5).with_scale(Vec3::splat(1.1)),
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(0.0, 0.95, -17.5),
        PlatformBox {
            half_extents: Vec3::new(1.7, 0.95, 1.7),
        },
    ));

    // Giant Toaster stepping stone up to the Back-Left Upper Shelf (top at Y = 1.85)
    let toaster_scene: Handle<Scene> = asset_server.load("models/giant_toaster.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(toaster_scene),
        Transform::from_xyz(-5.2, 0.0, -14.5).with_scale(Vec3::splat(1.25)),
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(-5.2, 0.925, -14.5),
        PlatformBox {
            half_extents: Vec3::new(1.5, 0.925, 1.2),
        },
    ));

    // Upper Kitchen Shelf collider (above Giant Toaster, top at Y = 2.85)
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(-2.6, 2.7, -20.2),
        PlatformBox {
            half_extents: Vec3::new(3.3, 0.15, 1.3),
        },
    ));

    // Zone 2: Cutting-Board Bridge & Center Kitchen Island (top at Y = 0.0)
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(8.2, -0.05, 2.0),
        PlatformBox {
            half_extents: Vec3::new(2.0, 0.12, 1.6),
        },
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(13.5, -3.0, 0.0),
        PlatformBox {
            half_extents: Vec3::new(4.0, 3.0, 10.0),
        },
    ));

    // Giant Stove on Center Island (top at Y = 1.1)
    let stove_scene: Handle<Scene> = asset_server.load("models/giant_stove.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(stove_scene),
        Transform::from_xyz(13.5, 0.0, -6.5),
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(13.5, 0.55, -6.5),
        PlatformBox {
            half_extents: Vec3::new(2.2, 0.55, 1.8),
        },
    ));

    // Zone 3: Upper & Lower Giant Stepping Stools + Dining Table & Chair + Full Kitchen Floor
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(8.5, -2.0, 12.5),
        PlatformBox {
            half_extents: Vec3::new(1.6, 0.15, 1.6),
        },
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(5.5, -4.0, 16.5),
        PlatformBox {
            half_extents: Vec3::new(1.6, 0.15, 1.6),
        },
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(-13.0, -3.6, -9.0),
        PlatformBox {
            half_extents: Vec3::new(3.3, 2.4, 5.0),
        },
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(-13.0, -4.6, -3.2),
        PlatformBox {
            half_extents: Vec3::new(1.4, 1.4, 1.4),
        },
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(1.0, -6.5, 0.0),
        PlatformBox {
            half_extents: Vec3::new(23.0, 0.5, 26.0),
        },
    ));

    // Zone 4: Towering Stainless-Steel Refrigerator on the Kitchen Floor (door opens when Adult Size reached!)
    let fridge_scene: Handle<Scene> = asset_server.load("models/giant_fridge.glb#Scene0");
    commands.spawn((
        LevelEntity,
        FridgeGoal,
        SceneRoot(fridge_scene),
        Transform::from_xyz(13.5, -6.0, -18.5),
    ));
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(13.5, 0.6, -19.2),
        PlatformBox {
            half_extents: Vec3::new(3.4, 6.6, 2.0),
        },
    ));

    // Decorative Kitchen Props
    let plate_egg: Handle<Scene> = asset_server.load("models/plate_with_egg.glb#Scene0");
    for (x, z) in [(4.6_f32, 14.0_f32), (-4.8_f32, 2.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(plate_egg.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let syrup: Handle<Scene> = asset_server.load("models/syrup_bottle.glb#Scene0");
    for (x, z) in [(-5.2_f32, 12.2_f32), (5.2_f32, -8.0_f32), (4.8_f32, -14.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(syrup.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let salt: Handle<Scene> = asset_server.load("models/salt_shaker.glb#Scene0");
    for (x, z) in [(-5.0_f32, 15.2_f32), (5.0_f32, 4.0_f32), (5.0_f32, -15.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(salt.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    // 12 Golden Chicken Tendies guiding you from 15cm Toy Size up to 180cm Adult Size!
    let tendy_scene: Handle<Scene> = asset_server.load("models/golden_tendy.glb#Scene0");
    let tendy_spots = [
        // Zone 1: Prep Countertop
        (0.0_f32, 0.4_f32, 13.0_f32, 1.0_f32),
        (-2.5, 0.4, 10.5, 1.0),
        (0.0, 1.35, 8.0, 1.0),
        (2.8, 0.4, 3.5, 1.0),
        // Zone 2: Sponge, Toaster, Upper Shelf & Center Island
        (-2.5, 1.35, -5.0, 1.1),
        (-5.2, 2.35, -14.5, 1.15),
        (-2.6, 3.35, -20.2, 1.2),
        (13.5, 0.6, 2.0, 1.25),
        // Zone 3: Stove, Stepping Stool, Dining Table & Kitchen Floor (in front of Fridge)
        (13.5, 1.6, -6.5, 1.35),
        (8.5, -1.3, 12.5, 1.45),
        (-13.0, -0.5, -9.0, 1.55),
        (6.0, -5.2, -13.0, 1.80),
    ];
    for (i, (x, y, z, scale)) in tendy_spots.into_iter().enumerate() {
        commands.spawn((
            LevelEntity,
            SceneRoot(tendy_scene.clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(scale)),
            TendyPickup {
                base_y: y,
                phase: i as f32 * 0.7,
                is_final_goal: i == 11,
            },
        ));
    }

    // Power-ups: Honey Mustard (speed) & Ranch Shield (absorbs 1 hit)
    let mustard_scene: Handle<Scene> = asset_server.load("models/honey_mustard_cup.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(mustard_scene),
        Transform::from_xyz(-3.5, 0.4, 12.0),
        PowerupPickup {
            kind: PowerupKind::HoneyMustard,
            base_y: 0.4,
            phase: 0.0,
        },
    ));

    let ranch_scene: Handle<Scene> = asset_server.load("models/ranch_shield.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(ranch_scene),
        Transform::from_xyz(3.5, 0.4, -1.0),
        PowerupPickup {
            kind: PowerupKind::RanchShield,
            base_y: 0.4,
            phase: 1.5,
        },
    ));
}
