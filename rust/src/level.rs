use bevy::asset::LoadState;
use bevy::prelude::*;
use crate::state::{
    FridgeGoal, LevelEntity, PlatformBox, PowerupKind, PowerupPickup, SceneRetryPath, TendyPickup,
};

pub fn retry_failed_scenes(
    asset_server: Res<AssetServer>,
    mut query: Query<(&mut SceneRoot, &mut SceneRetryPath)>,
) {
    for (mut scene, mut retry) in &mut query {
        if retry.attempts >= 4 {
            continue;
        }
        if matches!(
            asset_server.get_load_state(scene.0.id()),
            Some(LoadState::Failed(_))
        ) {
            retry.attempts += 1;
            scene.0 = asset_server.load(retry.path);
        }
    }
}

fn spawn_fallback_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    color: Color,
    perceptual_roughness: f32,
    metallic: f32,
) {
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness,
            metallic,
            ..default()
        })),
        Transform::from_translation(center),
    ));
}

fn spawn_solid_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    half_extents: Vec3,
    fallback_style: Option<(Color, f32, f32)>,
) {
    let mut ec = commands.spawn((
        LevelEntity,
        Transform::from_translation(center),
        PlatformBox { half_extents },
    ));
    if let Some((color, perceptual_roughness, metallic)) = fallback_style {
        // Inset slightly so high-detail .glb geometry never z-fights once streamed in.
        let inset_size = (half_extents * 2.0 - Vec3::splat(0.06)).max(Vec3::splat(0.05));
        ec.insert((
            Mesh3d(meshes.add(Cuboid::from_size(inset_size))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness,
                metallic,
                ..default()
            })),
        ));
    }
}

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
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Blender Katamari-scale Kitchen & Dining Room environment (countertop, island, bridge, stools, table, floor)
    let kitchen_path = "models/kitchen_environment.glb#Scene0";
    let kitchen_scene: Handle<Scene> = asset_server.load(kitchen_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(kitchen_scene),
        SceneRetryPath::new(kitchen_path),
        Transform::IDENTITY,
    ));

    let wood_col = Color::srgb(0.76, 0.56, 0.36);
    let cab_col = Color::srgb(0.46, 0.29, 0.16);
    let table_col = Color::srgb(0.64, 0.42, 0.24);
    let wall_col = Color::srgb(0.79, 0.83, 0.76);
    let tile_col = Color::srgb(0.50, 0.61, 0.50);

    // Zone 1: Prep Countertop: X in [-7, 7], Z in [-22, 22], top at Y = 0.0 (down to -6.0 floor)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -3.0, 0.0),
        Vec3::new(7.0, 3.0, 22.0),
        None,
    );
    // Two-tone instant fallback meshes (butcher-block slab + oak cabinet base)
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(13.92, 0.94, 43.92),
        wood_col,
        0.35,
        0.0,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -3.5, 0.0),
        Vec3::new(13.42, 4.94, 43.42),
        cab_col,
        0.5,
        0.0,
    );

    // Jumpable Kitchen Sponge platforms (visual at Y = 0, collider top at Y = 0.9)
    let sponge_path = "models/kitchen_sponge.glb#Scene0";
    let sponge_scene: Handle<Scene> = asset_server.load(sponge_path);
    for (x, z) in [(0.0_f32, 8.0_f32), (-2.5_f32, -5.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(sponge_scene.clone()),
            SceneRetryPath::new(sponge_path),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(1.15)),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.45, z),
            Vec3::new(1.25, 0.45, 0.8),
            None,
        );
    }

    // Giant Pancake Stack landmark at Z = -17.5 (top at Y = 1.9)
    let pancake_path = "models/pancake_stack.glb#Scene0";
    let pancake_scene: Handle<Scene> = asset_server.load(pancake_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(pancake_scene),
        SceneRetryPath::new(pancake_path),
        Transform::from_xyz(0.0, 0.0, -17.5).with_scale(Vec3::splat(1.1)),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 0.95, -17.5),
        Vec3::new(1.7, 0.95, 1.7),
        None,
    );

    // Giant Toaster stepping stone up to the Back-Left Upper Shelf (top at Y = 1.85)
    let toaster_path = "models/giant_toaster.glb#Scene0";
    let toaster_scene: Handle<Scene> = asset_server.load(toaster_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(toaster_scene),
        SceneRetryPath::new(toaster_path),
        Transform::from_xyz(-5.2, 0.0, -14.5).with_scale(Vec3::splat(1.25)),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-5.2, 0.925, -14.5),
        Vec3::new(1.5, 0.925, 1.2),
        None,
    );

    // Upper Kitchen Shelf collider (above Giant Toaster, top at Y = 2.85)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-2.6, 2.7, -20.2),
        Vec3::new(3.3, 0.15, 1.2),
        Some((wood_col, 0.45, 0.0)),
    );

    // Zone 2: Cutting-Board Bridge & Center Kitchen Island (top at Y = 0.0)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(9.0, 0.0, 2.0),
        Vec3::new(2.2, 0.13, 1.6),
        Some((wood_col, 0.45, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(15.0, -3.0, 0.0),
        Vec3::new(4.0, 3.0, 10.0),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(15.0, -0.4, 0.0),
        Vec3::new(7.92, 0.74, 19.92),
        wood_col,
        0.35,
        0.0,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(15.0, -3.4, 0.0),
        Vec3::new(7.52, 5.14, 19.32),
        cab_col,
        0.5,
        0.0,
    );

    // Giant Stove on Center Island (top at Y = 1.1)
    let stove_path = "models/giant_stove.glb#Scene0";
    let stove_scene: Handle<Scene> = asset_server.load(stove_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(stove_scene),
        SceneRetryPath::new(stove_path),
        Transform::from_xyz(15.0, 0.0, -6.5),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(15.0, 0.55, -6.5),
        Vec3::new(2.2, 0.55, 1.8),
        None,
    );

    // Zone 3: Upper & Lower Giant Stepping Stools (solid down to -6.0 floor) + Dining Table & Chair + Floor + Walls
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(8.8, -3.925, 12.5),
        Vec3::new(1.6, 2.075, 1.6),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(8.8, -2.0, 12.5),
        Vec3::new(3.12, 0.24, 3.12),
        table_col,
        0.4,
        0.0,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(8.8, -4.925, 16.5),
        Vec3::new(1.6, 1.075, 1.6),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(8.8, -4.0, 16.5),
        Vec3::new(3.12, 0.24, 3.12),
        table_col,
        0.4,
        0.0,
    );
    // Dining Table (solid down to -6.0 floor)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-13.0, -3.6, -9.0),
        Vec3::new(3.3, 2.4, 5.0),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(-13.0, -1.35, -9.0),
        Vec3::new(6.52, 0.24, 9.92),
        table_col,
        0.4,
        0.0,
    );
    // Dining Chair Seat (solid down to -6.0 floor) & Backrest
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-13.0, -4.6, -3.2),
        Vec3::new(1.4, 1.4, 1.4),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(-13.0, -3.32, -3.2),
        Vec3::new(2.72, 0.20, 2.72),
        table_col,
        0.4,
        0.0,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-13.0, -1.75, -1.9),
        Vec3::new(1.4, 1.5, 0.16),
        Some((table_col, 0.4, 0.0)),
    );

    // Full Kitchen & Dining Room Floor + Runner Rug fallback
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(1.0, -6.5, 0.0),
        Vec3::new(23.0, 0.5, 26.0),
        None,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(1.0, -6.28, 0.0),
        Vec3::new(45.9, 0.44, 51.9),
        Color::srgb(0.58, 0.43, 0.28),
        0.48,
        0.0,
    );
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(9.0, -5.98, -4.0),
        Vec3::new(6.8, 0.04, 25.8),
        Color::srgb(0.72, 0.22, 0.18),
        0.85,
        0.0,
    );

    // Solid Room Walls & Left Tiled Backsplash Wall (south half Z in [-2, 22], leaving Z < -2 open to Dining Room)
    spawn_fallback_mesh(
        commands,
        meshes,
        materials,
        Vec3::new(-7.35, 3.4, 10.0),
        Vec3::new(0.44, 7.5, 23.9),
        tile_col,
        0.2,
        0.0,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-7.1, 4.2, 10.0),
        Vec3::new(0.42, 4.2, 12.0),
        None,
    );
    // Left Upper Wall Cabinet & Back-Left Raised Shelf colliders (prevent camera clipping)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-6.2, 11.0, 8.0),
        Vec3::new(1.15, 2.2, 10.0),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-2.6, 8.3, -21.7),
        Vec3::new(3.3, 0.45, 0.8),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(1.0, 5.0, -22.8),
        Vec3::new(24.0, 11.0, 0.4),
        Some((wall_col, 0.72, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-22.3, 5.0, 0.0),
        Vec3::new(0.3, 11.0, 26.0),
        Some((wall_col, 0.72, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(24.3, 5.0, 0.0),
        Vec3::new(0.3, 11.0, 26.0),
        Some((wall_col, 0.72, 0.0)),
    );
    // Invisible south room boundary at Z = 25.5 so player cannot walk off the floor edge
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(1.0, 4.0, 25.5),
        Vec3::new(23.5, 10.0, 0.5),
        None,
    );

    // Zone 4: Towering Stainless-Steel Refrigerator on the Kitchen Floor (door opens when Adult Size reached!)
    let fridge_path = "models/giant_fridge.glb#Scene0";
    let fridge_scene: Handle<Scene> = asset_server.load(fridge_path);
    commands.spawn((
        LevelEntity,
        FridgeGoal,
        SceneRoot(fridge_scene),
        SceneRetryPath::new(fridge_path),
        Transform::from_xyz(15.0, -6.0, -18.5),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(15.0, 0.6, -19.2),
        Vec3::new(3.4, 6.6, 2.0),
        None,
    );

    // Built-in Countertop Appliances & Props in kitchen_environment.glb (all solid!)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(5.0, 0.9, -20.8),
        Vec3::new(2.7, 0.9, 1.2),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-6.55, 2.05, 12.0),
        Vec3::new(0.35, 2.05, 1.25),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-5.9, 1.4, 10.0),
        Vec3::new(0.52, 1.4, 0.52),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-6.0, 1.5, 6.8),
        Vec3::new(0.75, 1.5, 0.75),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-5.85, 0.68, 3.8),
        Vec3::new(1.05, 0.68, 1.05),
        None,
    );

    // Decorative Kitchen Props (all solid PlatformBoxes!)
    let plate_path = "models/plate_with_egg.glb#Scene0";
    let plate_egg: Handle<Scene> = asset_server.load(plate_path);
    for (x, z) in [(4.6_f32, 14.0_f32), (-4.8_f32, 2.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(plate_egg.clone()),
            SceneRetryPath::new(plate_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.14, z),
            Vec3::new(1.55, 0.14, 1.55),
            None,
        );
    }

    let syrup_path = "models/syrup_bottle.glb#Scene0";
    let syrup: Handle<Scene> = asset_server.load(syrup_path);
    for (x, z) in [(-5.2_f32, 12.2_f32), (5.2_f32, -8.0_f32), (4.8_f32, -14.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(syrup.clone()),
            SceneRetryPath::new(syrup_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 1.4, z),
            Vec3::new(0.68, 1.4, 0.68),
            None,
        );
    }

    let salt_path = "models/salt_shaker.glb#Scene0";
    let salt: Handle<Scene> = asset_server.load(salt_path);
    for (x, z) in [(-5.0_f32, 15.2_f32), (5.0_f32, 4.0_f32), (5.0_f32, -15.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(salt.clone()),
            SceneRetryPath::new(salt_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.65, z),
            Vec3::new(0.48, 0.65, 0.48),
            None,
        );
    }

    // 12 Golden Chicken Tendies guiding you from 15cm Toy Size up to 180cm Adult Size!
    let tendy_path = "models/golden_tendy.glb#Scene0";
    let tendy_scene: Handle<Scene> = asset_server.load(tendy_path);
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
        (15.0, 0.6, 2.0, 1.25),
        // Zone 3: Stove, Stepping Stool, Dining Table & Kitchen Floor (in front of Fridge)
        (15.0, 1.6, -6.5, 1.35),
        (8.8, -1.3, 12.5, 1.45),
        (-13.0, -0.5, -9.0, 1.55),
        (9.0, -5.2, -13.0, 1.80),
    ];
    for (i, (x, y, z, scale)) in tendy_spots.into_iter().enumerate() {
        commands.spawn((
            LevelEntity,
            SceneRoot(tendy_scene.clone()),
            SceneRetryPath::new(tendy_path),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(scale)),
            TendyPickup {
                base_y: y,
                phase: i as f32 * 0.7,
                is_final_goal: i == 11,
            },
        ));
    }

    // Power-ups: Honey Mustard (speed) & Ranch Shield (absorbs 1 hit)
    let mustard_path = "models/honey_mustard_cup.glb#Scene0";
    let mustard_scene: Handle<Scene> = asset_server.load(mustard_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(mustard_scene),
        SceneRetryPath::new(mustard_path),
        Transform::from_xyz(-3.5, 0.4, 12.0),
        PowerupPickup {
            kind: PowerupKind::HoneyMustard,
            base_y: 0.4,
            phase: 0.0,
        },
    ));

    let ranch_path = "models/ranch_shield.glb#Scene0";
    let ranch_scene: Handle<Scene> = asset_server.load(ranch_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(ranch_scene),
        SceneRetryPath::new(ranch_path),
        Transform::from_xyz(3.5, 0.4, -1.0),
        PowerupPickup {
            kind: PowerupKind::RanchShield,
            base_y: 0.4,
            phase: 1.5,
        },
    ));
}
