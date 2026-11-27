//! City arsenal: switchable Lightsaber / 9mm Pistol / Tendy Shotgun / Rapid SMG (#15),
//! bouncing high-explosive Grenades (#19), crew lightsaber visuals (#9), and bullet/blast physics.

use bevy::prelude::*;

use crate::audio::play_sfx;
use crate::boss::{HeroSaberBlade, HERO_SABER_AURA_COLOR, HERO_SABER_LIGHT};
use crate::city::{
    CityCar, CityCarPaint, CityNpc, CityWorldMarker, DestructibleProp, DestructiblePropKind,
    DestructibleWindow, LexieBoss, launch_npc_ragdoll, push_car_out_of_pub217, wrap_angle,
};
use crate::net::{CrewStrideArm, LocalCrewVisual, NetCoopEvents, EVENT_GRENADE_BLAST, pack_grenade_blast};
use crate::player::Player;
use crate::ragdoll::Ragdoll;
use crate::state::{GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::transition::SceneTransition;
use crate::ui::gameplay_running;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponCooldowns>()
            .init_resource::<PendingBlasts>()
            .add_systems(OnEnter(GameState::CityPlay), (spawn_weapon_hud, init_destruction_assets))
            .add_systems(OnExit(GameState::CityPlay), cleanup_weapons_city)
            .add_systems(
                Update,
                (
                    handle_weapon_input,
                    sync_weapon_visuals,
                    update_bullets,
                    update_grenades,
                    update_exploded_cars,
                    apply_blast_destruction,
                    update_destructibles_and_debris,
                    update_explosions,
                    update_weapon_hud,
                )
                    .chain()
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(gameplay_running),
            );
    }
}

#[derive(Resource, Default)]
struct WeaponCooldowns {
    fire_cd: f32,
    grenade_cd: f32,
}

#[derive(Clone, Copy)]
struct BlastEvent {
    center: Vec3,
    radius: f32,
    from_car: bool,
}

#[derive(Resource, Default)]
struct PendingBlasts(Vec<BlastEvent>);

#[derive(Resource)]
struct DestructionAssets {
    chunk_mesh: Handle<Mesh>,
    puff_mesh: Handle<Mesh>,
    charred_car_mat: Handle<StandardMaterial>,
    glass_shard_mat: Handle<StandardMaterial>,
    fire_mat: Handle<StandardMaterial>,
    smoke_mat: Handle<StandardMaterial>,
    metal_debris_mat: Handle<StandardMaterial>,
    leaf_mat: Handle<StandardMaterial>,
    water_mat: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DebrisKind {
    MetalChunk,
    GlassShard,
    FirePuff,
    SmokePuff,
    LeafChunk,
    WaterDrop,
}

#[derive(Component)]
struct FlyingDebris {
    vel: Vec3,
    ang_vel: Vec3,
    age: f32,
    max_age: f32,
    kind: DebrisKind,
    base_scale: Vec3,
}

#[derive(Component)]
struct WeaponEntity;

#[derive(Component)]
struct WeaponHudRoot;

#[derive(Component, Clone, Copy)]
struct WeaponHudButton(u8);

#[derive(Component)]
struct WeaponHudBtnText(u8);

#[derive(Component)]
pub struct CrewHeroSaber;

#[derive(Component)]
pub struct HeldGunVisual {
    pub is_crew: bool,
}

#[derive(Component)]
struct HeldGunSlide;

#[derive(Component)]
struct BulletProjectile {
    vel: Vec3,
    life: f32,
    damage: f32,
    impulse: f32,
    car_knock: f32,
}

#[derive(Component)]
struct ThrownGrenade {
    vel: Vec3,
    fuse: f32,
    ang_vel: Vec3,
}

#[derive(Component)]
struct GrenadeFuseLed;

#[derive(Component)]
struct ExplosionVfx {
    age: f32,
    max_age: f32,
    max_radius: f32,
}

fn weapon_label(slot: u8) -> &'static str {
    match slot {
        1 => "9mm Pistol",
        2 => "Tendy Shotgun",
        3 => "Rapid SMG",
        _ => "Lightsaber",
    }
}

fn spawn_weapon_hud(mut commands: Commands) {
    commands
        .spawn((
            WeaponHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                bottom: Val::Px(54.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.12, 0.80)),
            BorderColor(Color::srgb(0.95, 0.78, 0.25)),
            BorderRadius::all(Val::Px(8.0)),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            let labels = [
                (0_u8, "1:SABER"),
                (1_u8, "2:PISTOL"),
                (2_u8, "3:SHOTGUN"),
                (3_u8, "4:SMG"),
                (4_u8, "G:GREN x5"),
            ];
            for (slot, label) in labels {
                root.spawn((
                    WeaponHudButton(slot),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(if slot == 4 {
                        Color::srgba(0.10, 0.36, 0.18, 0.90)
                    } else {
                        Color::srgba(0.10, 0.14, 0.24, 0.88)
                    }),
                    BorderColor(Color::srgba(0.95, 0.82, 0.35, 0.55)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        WeaponHudBtnText(slot),
                        Text::new(label),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.94, 0.82)),
                    ));
                });
            }
        });
}

fn init_destruction_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(DestructionAssets {
        chunk_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        puff_mesh: meshes.add(Sphere::new(1.0)),
        charred_car_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.07, 0.07),
            perceptual_roughness: 0.96,
            metallic: 0.08,
            ..default()
        }),
        glass_shard_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.70, 0.90, 1.0, 0.84),
            emissive: LinearRgba::rgb(0.28, 0.48, 0.68),
            metallic: 0.55,
            perceptual_roughness: 0.12,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        fire_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.46, 0.08, 0.90),
            emissive: LinearRgba::rgb(4.0, 1.4, 0.16),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        smoke_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.16, 0.16, 0.18, 0.72),
            perceptual_roughness: 0.95,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        metal_debris_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.19, 0.17),
            metallic: 0.65,
            perceptual_roughness: 0.68,
            ..default()
        }),
        leaf_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.20, 0.56, 0.22),
            perceptual_roughness: 0.85,
            ..default()
        }),
        water_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.45, 0.86, 1.0, 0.84),
            emissive: LinearRgba::rgb(0.24, 0.72, 1.15),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

fn cleanup_weapons_city(
    mut commands: Commands,
    hud_q: Query<Entity, With<WeaponHudRoot>>,
    ent_q: Query<Entity, With<WeaponEntity>>,
    mut stan_saber_q: Query<&mut Visibility, With<HeroSaberBlade>>,
    mut pending: ResMut<PendingBlasts>,
) {
    crate::audio::set_touch_city_weapons(false, 0, 0);
    pending.0.clear();
    for mut v in &mut stan_saber_q {
        if *v != Visibility::Inherited {
            *v = Visibility::Inherited;
        }
    }
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
}

#[allow(clippy::too_many_arguments)]
fn update_weapon_hud(
    stats: Res<PlayerStats>,
    mut root_q: Query<&mut Visibility, With<WeaponHudRoot>>,
    mut btn_q: Query<
        (&WeaponHudButton, &mut BackgroundColor, &mut BorderColor),
        Without<WeaponHudRoot>,
    >,
    mut text_q: Query<(&WeaponHudBtnText, &mut Text)>,
    mut last_touch: Local<(bool, u8, u32)>,
) {
    let active_in_foot = stats.driving_car.is_none()
        && !stats.riding_wheel
        && !stats.phone_open
        && !stats.bartender_open
        && !stats.char_select_open
        && !stats.casino_open
        && !stats.arcade_open;

    let cur_touch = (active_in_foot, stats.equipped_weapon, stats.grenades);
    if *last_touch != cur_touch {
        *last_touch = cur_touch;
        crate::audio::set_touch_city_weapons(cur_touch.0, cur_touch.1, cur_touch.2);
    }

    let hide = !active_in_foot || crate::audio::skip_touch_ui();
    let want = if hide {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut root_q {
        if *v != want {
            *v = want;
        }
    }
    if hide {
        return;
    }

    for (btn, mut bg, mut border) in &mut btn_q {
        if btn.0 == 4 {
            bg.0 = if stats.grenades > 0 {
                Color::srgba(0.10, 0.42, 0.20, 0.92)
            } else {
                Color::srgba(0.16, 0.14, 0.14, 0.72)
            };
            border.0 = if stats.grenades > 0 {
                Color::srgb(0.95, 0.82, 0.28)
            } else {
                Color::srgba(0.5, 0.5, 0.5, 0.4)
            };
        } else {
            let selected = stats.equipped_weapon == btn.0;
            bg.0 = if selected {
                Color::srgb(0.76, 0.46, 0.08)
            } else {
                Color::srgba(0.10, 0.14, 0.24, 0.86)
            };
            border.0 = if selected {
                Color::srgb(1.0, 0.90, 0.35)
            } else {
                Color::srgba(0.65, 0.75, 0.92, 0.45)
            };
        }
    }
    for (t, mut txt) in &mut text_q {
        let label = match t.0 {
            0 => "1:SABER".to_string(),
            1 => "2:PISTOL".to_string(),
            2 => "3:SHOTGUN".to_string(),
            3 => "4:SMG".to_string(),
            _ => format!("G:GREN x{}", stats.grenades),
        };
        if txt.0 != label {
            txt.0 = label;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_weapon_input(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
    mut cd: ResMut<WeaponCooldowns>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut player_q: Query<(&Transform, &mut Player)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<Player>)>,
    btn_q: Query<(&Interaction, &WeaponHudButton), Changed<Interaction>>,
    ragdoll: Res<Ragdoll>,
) {
    let dt = time.delta_secs().min(0.05);
    cd.fire_cd = (cd.fire_cd - dt).max(0.0);
    cd.grenade_cd = (cd.grenade_cd - dt).max(0.0);

    // Regenerate grenades gradually so players never stay empty.
    if stats.grenades < 5 {
        stats.grenade_regen_timer += dt;
        if stats.grenade_regen_timer >= 6.5 {
            stats.grenade_regen_timer = 0.0;
            stats.grenades += 1;
        }
    } else {
        stats.grenade_regen_timer = 0.0;
    }

    if stats.driving_car.is_some()
        || stats.riding_wheel
        || stats.phone_open
        || stats.bartender_open
        || stats.char_select_open
        || stats.casino_open
        || stats.arcade_open
        || ragdoll.active()
    {
        return;
    }

    // Drain mobile / clickable HUD button actions cleanly
    let touch_cycle = crate::audio::take_touch_weapon_cycle();
    let touch_gren = crate::audio::take_touch_grenade();
    let mut ui_weapon: Option<u8> = None;
    let mut ui_gren = false;
    for (inter, btn) in &btn_q {
        if *inter == Interaction::Pressed {
            if btn.0 == 4 {
                ui_gren = true;
            } else {
                ui_weapon = Some(btn.0);
            }
        }
    }

    // Weapon selection [1..4], [C] cycle, touch WEP button, or desktop HUD pill
    let mut switched = false;
    if let Some(slot) = ui_weapon {
        stats.equipped_weapon = slot;
        switched = true;
    } else if keys.just_pressed(KeyCode::Digit1) {
        stats.equipped_weapon = 0;
        switched = true;
    } else if keys.just_pressed(KeyCode::Digit2) {
        stats.equipped_weapon = 1;
        switched = true;
    } else if keys.just_pressed(KeyCode::Digit3) {
        stats.equipped_weapon = 2;
        switched = true;
    } else if keys.just_pressed(KeyCode::Digit4) {
        stats.equipped_weapon = 3;
        switched = true;
    } else if keys.just_pressed(KeyCode::KeyC) || touch_cycle {
        stats.equipped_weapon = (stats.equipped_weapon + 1) % 4;
        switched = true;
    }
    if switched {
        play_sfx("hack_blip");
        stats.hint_text = format!("EQUIPPED: {}!", weapon_label(stats.equipped_weapon));
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }

    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let cam_fwd = cam_q
        .get_single()
        .map(|c| {
            let mut f = *c.forward();
            f.y = (f.y + 0.04).clamp(-0.35, 0.55);
            f.normalize_or(Vec3::NEG_Z)
        })
        .unwrap_or_else(|_| Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos()));
    let cam_right = Vec3::Y.cross(cam_fwd).normalize_or(Vec3::X);

    // Throw Grenade [G] or Mobile GREN button or HUD G button (#19)
    if (keys.just_pressed(KeyCode::KeyG) || touch_gren || ui_gren) && cd.grenade_cd <= 0.0 {
        if stats.grenades == 0 {
            play_sfx("hack_deny");
            stats.hint_text = "Out of grenades -- recharging!".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        } else {
            stats.grenades -= 1;
            cd.grenade_cd = 0.55;
            player.facing_yaw = (-cam_fwd.x).atan2(-cam_fwd.z);
            let s = stats.body_scale().max(0.6);
            let origin = p_tf.translation + Vec3::Y * (1.25 * s) + cam_fwd * 0.7;
            let vel = cam_fwd * 22.0 + Vec3::Y * 8.6 + player.velocity * 0.35;
            spawn_grenade(&mut commands, &mut meshes, &mut materials, origin, vel);
            play_sfx("dash");
        }
    }

    // Gun firing (#15)
    if stats.equipped_weapon == 0 {
        return;
    }
    let want_shot = match stats.equipped_weapon {
        3 => {
            mouse.pressed(MouseButton::Left)
                || keys.pressed(KeyCode::KeyJ)
                || keys.pressed(KeyCode::KeyX)
                || crate::audio::touch_attack_held()
        }
        _ => {
            mouse.just_pressed(MouseButton::Left)
                || keys.just_pressed(KeyCode::KeyJ)
                || keys.just_pressed(KeyCode::KeyX)
                || crate::audio::take_touch_attack()
        }
    };
    if !want_shot || cd.fire_cd > 0.0 {
        return;
    }

    player.facing_yaw = (-cam_fwd.x).atan2(-cam_fwd.z);
    let s = stats.body_scale().max(0.6);
    let muzzle = p_tf.translation + Vec3::Y * (1.22 * s) + cam_fwd * (0.85 * s) - cam_right * (0.18 * s);
    let tracer_mesh = meshes.add(Cuboid::new(0.08, 0.08, 0.65));
    let tracer_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.35),
        emissive: LinearRgba::rgb(4.0, 3.0, 0.8),
        unlit: true,
        ..default()
    });

    match stats.equipped_weapon {
        1 => {
            // 9mm Pistol
            cd.fire_cd = 0.24;
            play_sfx("gunshot");
            stats.shake_request = stats.shake_request.max(0.08);
            spawn_bullet(
                &mut commands,
                &tracer_mesh,
                &tracer_mat,
                muzzle,
                cam_fwd * 92.0,
                20.0,
                13.5,
                6.5,
            );
        }
        2 => {
            // Tendy Shotgun (6-pellet cone blast)
            cd.fire_cd = 0.72;
            play_sfx("shotgun");
            stats.shake_request = stats.shake_request.max(0.24);
            let offsets = [-0.09_f32, -0.054, -0.018, 0.018, 0.054, 0.09];
            for (k, &ox) in offsets.iter().enumerate() {
                let oy = ((k as f32 * 2.1).sin()) * 0.035;
                let dir = (cam_fwd + cam_right * ox + Vec3::Y * oy).normalize_or(cam_fwd);
                spawn_bullet(
                    &mut commands,
                    &tracer_mesh,
                    &tracer_mat,
                    muzzle,
                    dir * 78.0,
                    11.0,
                    18.5,
                    13.0,
                );
            }
        }
        _ => {
            // Rapid SMG
            cd.fire_cd = 0.095;
            play_sfx("smg_shot");
            stats.shake_request = stats.shake_request.max(0.05);
            let t = time.elapsed_secs();
            let jitter = cam_right * ((t * 47.0).sin() * 0.028) + Vec3::Y * ((t * 61.0).cos() * 0.02);
            let dir = (cam_fwd + jitter).normalize_or(cam_fwd);
            spawn_bullet(
                &mut commands,
                &tracer_mesh,
                &tracer_mat,
                muzzle,
                dir * 96.0,
                11.5,
                9.5,
                4.0,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_bullet(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    origin: Vec3,
    vel: Vec3,
    damage: f32,
    impulse: f32,
    car_knock: f32,
) {
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        WeaponEntity,
        BulletProjectile {
            vel,
            life: 1.35,
            damage,
            impulse,
            car_knock,
        },
        Mesh3d(mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(origin).looking_to(vel.normalize_or(Vec3::NEG_Z), Vec3::Y),
    ));
}

fn spawn_grenade(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    vel: Vec3,
) {
    let body_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.34, 0.16),
        metallic: 0.45,
        perceptual_roughness: 0.4,
        ..default()
    });
    let spoon_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.18),
        metallic: 0.85,
        perceptual_roughness: 0.2,
        ..default()
    });
    let led_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.18, 0.12),
        emissive: LinearRgba::rgb(3.5, 0.4, 0.15),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            WeaponEntity,
            ThrownGrenade {
                vel,
                fuse: 1.9,
                ang_vel: Vec3::new(9.5, 6.0, -7.5),
            },
            Transform::from_translation(origin),
            Visibility::default(),
        ))
        .with_children(|g| {
            g.spawn((
                Mesh3d(meshes.add(Sphere::new(0.18))),
                MeshMaterial3d(body_mat),
                Transform::from_scale(Vec3::new(0.9, 1.15, 0.9)),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.06, 0.12))),
                MeshMaterial3d(spoon_mat),
                Transform::from_xyz(0.0, 0.21, 0.0),
            ));
            g.spawn((
                GrenadeFuseLed,
                Mesh3d(meshes.add(Sphere::new(0.08))),
                MeshMaterial3d(led_mat),
                Transform::from_xyz(0.0, 0.26, 0.0),
            ));
        });
}

fn spawn_crew_saber(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hand_pos: Vec3,
) {
    let hilt_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.23, 0.28),
        metallic: 0.85,
        perceptual_roughness: 0.2,
        ..default()
    });
    let gold_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.78, 0.10),
        emissive: LinearRgba::rgb(0.45, 0.30, 0.02),
        metallic: 0.65,
        ..default()
    });
    let saber_core = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.92, 1.0),
        emissive: LinearRgba::rgb(2.2, 1.5, 4.5),
        unlit: true,
        ..default()
    });
    let saber_aura = materials.add(StandardMaterial {
        base_color: HERO_SABER_AURA_COLOR,
        emissive: LinearRgba::rgb(1.1, 0.4, 2.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let saber_e = commands
        .spawn((
            LevelEntity,
            WeaponEntity,
            CrewHeroSaber,
            Transform::from_translation(hand_pos).with_rotation(Quat::from_rotation_x(1.18)),
            Visibility::Inherited,
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 0.28))),
                MeshMaterial3d(hilt_mat),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.055, 0.045))),
                MeshMaterial3d(gold_mat),
                Transform::from_xyz(0.0, 0.19, 0.0),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.036, 1.65))),
                MeshMaterial3d(saber_core),
                Transform::from_xyz(0.0, 1.02, 0.0),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.078, 1.72))),
                MeshMaterial3d(saber_aura),
                Transform::from_xyz(0.0, 1.02, 0.0),
            ));
            s.spawn(PointLight {
                color: Color::srgb(0.68, 0.38, 1.0),
                intensity: HERO_SABER_LIGHT * 0.55,
                range: 10.0,
                shadows_enabled: false,
                ..default()
            });
        })
        .id();
    commands.entity(parent).add_child(saber_e);
}

fn spawn_held_gun(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    fwd_z: f32,
    is_crew: bool,
) {
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.18, 0.22),
        metallic: 0.82,
        perceptual_roughness: 0.28,
        ..default()
    });
    let grip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.20, 0.10),
        perceptual_roughness: 0.65,
        ..default()
    });
    let glow_sight = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 1.0, 0.55),
        emissive: LinearRgba::rgb(0.4, 2.2, 0.8),
        unlit: true,
        ..default()
    });
    let gun_e = commands
        .spawn((
            LevelEntity,
            WeaponEntity,
            HeldGunVisual { is_crew },
            Transform::from_translation(pos),
            Visibility::Hidden,
        ))
        .with_children(|g| {
            g.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.11, 0.22, 0.13))),
                MeshMaterial3d(grip),
                Transform::from_xyz(0.0, -0.06, -fwd_z * 0.06),
            ));
            g.spawn((
                HeldGunSlide,
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.14, 0.56))),
                MeshMaterial3d(steel),
                Transform::from_xyz(0.0, 0.08, fwd_z * 0.18),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.05, 0.04, 0.05))),
                MeshMaterial3d(glow_sight),
                Transform::from_xyz(0.0, 0.16, fwd_z * 0.42),
            ));
        })
        .id();
    commands.entity(parent).add_child(gun_e);
}

/// Keeps Lightsaber vs Gun visibility & crew lightsabers (#9 + #15) synchronized for all 5 heroes.
#[allow(clippy::too_many_arguments)]
fn sync_weapon_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stats: Res<PlayerStats>,
    player_q: Query<Entity, With<Player>>,
    arm_q: Query<(Entity, &Name)>,
    crew_vis_q: Query<&LocalCrewVisual>,
    crew_arm_q: Query<(Entity, &CrewStrideArm)>,
    children_q: Query<&Children>,
    parent_q: Query<&Parent>,
    mut stan_saber_q: Query<
        &mut Visibility,
        (
            With<HeroSaberBlade>,
            Without<CrewHeroSaber>,
            Without<HeldGunVisual>,
        ),
    >,
    mut crew_saber_q: Query<
        (Entity, &mut Visibility),
        (
            With<CrewHeroSaber>,
            Without<HeroSaberBlade>,
            Without<HeldGunVisual>,
        ),
    >,
    mut gun_q: Query<
        (&mut Visibility, &mut Transform, &HeldGunVisual),
        (Without<HeroSaberBlade>, Without<CrewHeroSaber>),
    >,
) {
    let show_saber = stats.equipped_weapon == 0 && stats.driving_car.is_none();
    let show_gun = stats.equipped_weapon > 0 && stats.driving_car.is_none();

    let player_ent = player_q.get_single().ok();
    let has_ancestor = |mut cur: Entity, target: Entity| -> bool {
        while let Ok(p) = parent_q.get(cur) {
            cur = p.get();
            if cur == target {
                return true;
            }
        }
        false
    };

    // 1. Local Stan's HeroRightArm gets HeldGunVisual if missing (never attach to remote Stans)
    if let Some(p_e) = player_ent {
        if let Some((r_arm, _)) = arm_q
            .iter()
            .find(|(e, n)| n.as_str() == "HeroRightArm" && has_ancestor(*e, p_e))
        {
            let has_gun = children_q
                .get(r_arm)
                .is_ok_and(|k| k.iter().any(|c| gun_q.get(*c).is_ok()));
            if !has_gun {
                spawn_held_gun(
                    &mut commands,
                    r_arm,
                    &mut meshes,
                    &mut materials,
                    Vec3::new(0.23, -0.44, -0.10),
                    -1.0,
                    false,
                );
            }
        }
    }

    // 2. Local crew rig's right arm (belongs to LocalCrewVisual ancestor) gets ONE Saber + ONE HeldGunVisual
    let mut saber_count = crew_saber_q.iter().count();
    for (arm_e, arm) in &crew_arm_q {
        if arm.is_left {
            continue;
        }
        // Only attach combat gear to the LOCAL player's crew rig (not remote peers' rigs).
        let mut is_local = false;
        let mut cur = arm_e;
        while let Ok(p) = parent_q.get(cur) {
            cur = p.get();
            if crew_vis_q.get(cur).is_ok() {
                is_local = true;
                break;
            }
        }
        if !is_local {
            continue;
        }
        let kids = children_q.get(arm_e).ok();
        let has_saber = kids.is_some_and(|k| k.iter().any(|c| crew_saber_q.get(*c).is_ok()));
        let has_gun = kids.is_some_and(|k| k.iter().any(|c| gun_q.get(*c).is_ok()));
        if !has_saber && saber_count == 0 {
            saber_count += 1;
            spawn_crew_saber(
                &mut commands,
                arm_e,
                &mut meshes,
                &mut materials,
                Vec3::new(0.04, -0.58, 0.10),
            );
        }
        if !has_gun {
            spawn_held_gun(
                &mut commands,
                arm_e,
                &mut meshes,
                &mut materials,
                Vec3::new(0.04, -0.58, 0.14),
                1.0,
                true,
            );
        }
    }

    // Never show Stan's HeroSaberBlade while playing as a crew character (TJ/Nate/Glenn/Derek).
    let using_crew_rig = !crew_vis_q.is_empty();
    let stan_saber_vis = if show_saber && !using_crew_rig {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut stan_saber_q {
        if *v != stan_saber_vis {
            *v = stan_saber_vis;
        }
    }
    let crew_saber_vis = if show_saber && using_crew_rig {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (idx, (e, mut v)) in crew_saber_q.iter_mut().enumerate() {
        if idx > 0 {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
            continue;
        }
        if *v != crew_saber_vis {
            *v = crew_saber_vis;
        }
    }

    let gun_scale = match stats.equipped_weapon {
        2 => Vec3::new(1.25, 1.15, 1.75), // Shotgun longer/chunkier
        3 => Vec3::new(1.10, 1.25, 1.35), // SMG compact tactical
        _ => Vec3::ONE,                   // 9mm Pistol
    };
    for (mut v, mut tf, gun) in &mut gun_q {
        let gun_vis = if show_gun && gun.is_crew == using_crew_rig {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != gun_vis {
            *v = gun_vis;
        }
        if tf.scale != gun_scale {
            tf.scale = gun_scale;
        }
    }
}

fn spawn_debris_particle(
    commands: &mut Commands,
    vfx: &DestructionAssets,
    pos: Vec3,
    vel: Vec3,
    ang_vel: Vec3,
    max_age: f32,
    kind: DebrisKind,
    base_scale: Vec3,
) {
    let (mesh, mat) = match kind {
        DebrisKind::MetalChunk => (vfx.chunk_mesh.clone(), vfx.metal_debris_mat.clone()),
        DebrisKind::GlassShard => (vfx.chunk_mesh.clone(), vfx.glass_shard_mat.clone()),
        DebrisKind::FirePuff => (vfx.puff_mesh.clone(), vfx.fire_mat.clone()),
        DebrisKind::SmokePuff => (vfx.puff_mesh.clone(), vfx.smoke_mat.clone()),
        DebrisKind::LeafChunk => (vfx.chunk_mesh.clone(), vfx.leaf_mat.clone()),
        DebrisKind::WaterDrop => (vfx.puff_mesh.clone(), vfx.water_mat.clone()),
    };
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        WeaponEntity,
        FlyingDebris {
            vel,
            ang_vel,
            age: 0.0,
            max_age,
            kind,
            base_scale,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos).with_scale(base_scale),
    ));
}

fn spawn_glass_shards(commands: &mut Commands, vfx: &DestructionAssets, origin: Vec3, outward: Vec3, count: usize, seed_f: f32) {
    let tangent = Vec3::new(-outward.z, 0.0, outward.x);
    for i in 0..count {
        let a = seed_f * 1.7 + i as f32 * 2.399;
        let lateral = tangent * (a.sin() * 4.8);
        let up = 2.5 + (a.cos().abs()) * 5.2;
        let vel = outward * (6.5 + (i % 4) as f32 * 2.2) + lateral + Vec3::Y * up;
        let pos = origin + tangent * (a.sin() * 1.2) + Vec3::Y * (a.cos() * 0.7);
        spawn_debris_particle(
            commands,
            vfx,
            pos,
            vel,
            Vec3::new(9.5, -12.0, 8.2),
            1.6 + (i % 3) as f32 * 0.35,
            DebrisKind::GlassShard,
            Vec3::new(0.28 + (i % 2) as f32 * 0.14, 0.32, 0.05),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn detonate_car(
    commands: &mut Commands,
    car_e: Entity,
    car_pos: Vec3,
    car: &mut CityCar,
    away_xz: Vec2,
    power: f32,
    chain: bool,
    children_q: &Query<&Children>,
    paint_q: &mut Query<(&mut MeshMaterial3d<StandardMaterial>, &CityCarPaint), Without<CityCar>>,
    vfx: Option<&DestructionAssets>,
    stats: &mut PlayerStats,
    out_blasts: &mut Vec<BlastEvent>,
) {
    car.exploded_timer = 12.0;
    car.car_hp = 0.0;
    car.chain_fuse = 0.0;
    car.burn_vfx_cd = 0.04;
    car.knock_vel = Vec2::ZERO;
    car.velocity = Vec2::ZERO;
    car.park = false;
    car.explode_vel = Vec3::new(away_xz.x, 0.0, away_xz.y) * (10.0 + 7.5 * power)
        + Vec3::Y * (12.5 + 5.0 * power);
    car.explode_ang_vel = Vec3::new(3.8, 2.7, -4.4);

    if let Some(vfx) = vfx {
        if let Ok(kids) = children_q.get(car_e) {
            for &kid in kids.iter() {
                if let Ok((mut mat, _)) = paint_q.get_mut(kid) {
                    mat.0 = vfx.charred_car_mat.clone();
                }
            }
        }
        for i in 0..12_usize {
            let a = i as f32 * 0.5236 + car_pos.x;
            let vel = Vec3::new(a.cos() * 14.0, 8.0 + (i % 4) as f32 * 3.0, a.sin() * 14.0);
            spawn_debris_particle(
                commands,
                vfx,
                car_pos + Vec3::Y * 0.9,
                vel,
                Vec3::new(6.0, -8.5, 7.2),
                2.2,
                if i % 3 == 0 { DebrisKind::FirePuff } else { DebrisKind::MetalChunk },
                if i % 3 == 0 { Vec3::splat(0.7) } else { Vec3::new(0.45, 0.18, 0.52) },
            );
        }
    }

    stats.score += 500;
    stats.wanted_stars = (stats.wanted_stars + 1).min(5);
    stats.wanted_decay_timer = 14.0;
    stats.hint_text = if chain {
        "BOOM! CHAIN CAR EXPLOSION! +500".to_string()
    } else {
        "BOOM! CAR EXPLODED! +500".to_string()
    };
    stats.hint_bump = stats.hint_bump.wrapping_add(1);
    out_blasts.push(BlastEvent {
        center: car_pos + Vec3::Y * 1.1,
        radius: 15.0,
        from_car: true,
    });
}

#[allow(clippy::too_many_arguments)]
fn update_bullets(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    vfx: Option<Res<DestructionAssets>>,
    mut bullet_q: Query<
        (Entity, &mut Transform, &mut BulletProjectile),
        (
            Without<CityNpc>,
            Without<CityCar>,
            Without<LexieBoss>,
            Without<PlatformBox>,
            Without<DestructibleWindow>,
        ),
    >,
    mut npc_q: Query<
        (&mut Transform, &mut CityNpc),
        (
            Without<BulletProjectile>,
            Without<CityCar>,
            Without<LexieBoss>,
            Without<PlatformBox>,
            Without<DestructibleWindow>,
        ),
    >,
    mut car_q: Query<
        (&Transform, &mut CityCar),
        (
            Without<BulletProjectile>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<PlatformBox>,
            Without<DestructibleWindow>,
        ),
    >,
    mut lexie_q: Query<
        (&Transform, &mut LexieBoss),
        (
            Without<BulletProjectile>,
            Without<CityNpc>,
            Without<CityCar>,
            Without<PlatformBox>,
            Without<DestructibleWindow>,
        ),
    >,
    mut win_q: Query<
        (&Transform, &mut DestructibleWindow, &mut Visibility),
        (
            Without<BulletProjectile>,
            Without<CityNpc>,
            Without<CityCar>,
            Without<LexieBoss>,
            Without<PlatformBox>,
        ),
    >,
    box_q: Query<
        (&Transform, &PlatformBox),
        (
            Without<BulletProjectile>,
            Without<CityNpc>,
            Without<CityCar>,
            Without<LexieBoss>,
            Without<DestructibleWindow>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    for (b_e, mut b_tf, mut b) in &mut bullet_q {
        b.life -= dt;
        b_tf.translation += b.vel * dt;
        let p = b_tf.translation;
        if b.life <= 0.0 || p.y <= 0.08 || p.x.abs() > 140.0 || p.z.abs() > 140.0 {
            commands.entity(b_e).despawn_recursive();
            continue;
        }

        // Hit NPCs (#8 ragdoll!)
        let mut hit = false;
        for (mut n_tf, mut npc) in &mut npc_q {
            let flat = Vec2::new(p.x - n_tf.translation.x, p.z - n_tf.translation.z).length();
            if flat < 0.95 && p.y >= n_tf.translation.y - 0.2 && p.y <= n_tf.translation.y + 1.9 {
                let dir = b.vel.with_y(0.0).normalize_or(Vec3::Z);
                let imp = dir * b.impulse + Vec3::Y * (4.5 + b.impulse * 0.28);
                let ko = launch_npc_ragdoll(&mut npc, &mut n_tf, imp, b.damage);
                play_sfx("hit");
                stats.score += if ko { 150 } else { 60 };
                if ko {
                    stats.add_tendy_wendies(1);
                }
                hit = true;
                break;
            }
        }
        if hit {
            commands.entity(b_e).despawn_recursive();
            continue;
        }

        // Hit Lexie
        for (l_tf, mut lexie) in &mut lexie_q {
            if stats.lexie_defeated || lexie.on_tower {
                continue;
            }
            let flat = Vec2::new(p.x - l_tf.translation.x, p.z - l_tf.translation.z).length();
            if flat < 4.2 && p.y < 18.0 {
                lexie.hp = if stats.tendies < 50 {
                    (lexie.hp - b.damage * 0.5).max(75.0)
                } else {
                    lexie.hp - b.damage * 1.4
                };
                stats.boss_hp = lexie.hp.max(0.0);
                lexie.hit_flash = 0.18;
                stats.score += 80;
                play_sfx("hit");
                hit = true;
                break;
            }
        }
        if hit {
            commands.entity(b_e).despawn_recursive();
            continue;
        }

        // Hit Unoccupied Cars (heavy gunfire can detonate cars!)
        for (c_tf, mut car) in &mut car_q {
            if car.occupied || car.exploded_timer > 0.0 {
                continue;
            }
            let flat = Vec2::new(p.x - c_tf.translation.x, p.z - c_tf.translation.z).length();
            if flat < 2.25 && (p.y - c_tf.translation.y).abs() < 1.8 {
                let dir = Vec2::new(b.vel.x, b.vel.z).normalize_or_zero();
                car.knock_vel += dir * b.car_knock;
                car.yaw = wrap_angle(car.yaw + (dir.x * 0.08));
                car.car_hp -= b.damage;
                if car.car_hp <= 0.0 && car.chain_fuse <= 0.0 {
                    car.chain_fuse = 0.02;
                }
                if b.car_knock > 8.0 {
                    play_sfx("car_crash");
                } else {
                    play_sfx("grind_lock");
                }
                hit = true;
                break;
            }
        }
        if hit {
            commands.entity(b_e).despawn_recursive();
            continue;
        }

        // Shatterable building storefront/pub windows
        for (w_tf, mut win, mut w_vis) in &mut win_q {
            if !win.intact {
                continue;
            }
            let d = p - w_tf.translation;
            let n = win.outward_normal;
            let tang = Vec3::new(-n.z, 0.0, n.x);
            if d.dot(n).abs() < 0.75
                && d.y.abs() < win.half_size.y + 0.35
                && d.dot(tang).abs() < win.half_size.x + 0.35
            {
                win.intact = false;
                win.respawn_timer = 35.0;
                *w_vis = Visibility::Hidden;
                stats.score += 75;
                play_sfx("glass_shatter");
                if let Some(ref assets) = vfx {
                    spawn_glass_shards(&mut commands, assets, w_tf.translation, n, 6, p.x + p.z);
                }
                hit = true;
                break;
            }
        }
        if hit {
            commands.entity(b_e).despawn_recursive();
            continue;
        }

        // Building obstacle check
        for (pb_tf, pb) in &box_q {
            if pb_tf.translation.y + pb.half_extents.y <= 0.3 {
                continue;
            }
            if (p.x - pb_tf.translation.x).abs() < pb.half_extents.x
                && (p.z - pb_tf.translation.z).abs() < pb.half_extents.z
                && (p.y - pb_tf.translation.y).abs() < pb.half_extents.y
            {
                hit = true;
                break;
            }
        }
        if hit {
            commands.entity(b_e).despawn_recursive();
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct BlastTargets<'w, 's> {
    npc_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut CityNpc),
        (
            Without<CityCar>,
            Without<CityCarPaint>,
            Without<DestructibleWindow>,
            Without<DestructibleProp>,
            Without<LexieBoss>,
            Without<Player>,
        ),
    >,
    car_q: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static mut CityCar),
        (
            Without<CityNpc>,
            Without<CityCarPaint>,
            Without<DestructibleWindow>,
            Without<DestructibleProp>,
            Without<LexieBoss>,
            Without<Player>,
        ),
    >,
    win_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut DestructibleWindow, &'static mut Visibility),
        (
            Without<CityNpc>,
            Without<CityCar>,
            Without<CityCarPaint>,
            Without<DestructibleProp>,
            Without<LexieBoss>,
            Without<Player>,
        ),
    >,
    prop_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut DestructibleProp),
        (
            Without<CityNpc>,
            Without<CityCar>,
            Without<CityCarPaint>,
            Without<DestructibleWindow>,
            Without<LexieBoss>,
            Without<Player>,
        ),
    >,
    lexie_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut LexieBoss),
        (
            Without<CityNpc>,
            Without<CityCar>,
            Without<CityCarPaint>,
            Without<DestructibleWindow>,
            Without<DestructibleProp>,
            Without<Player>,
        ),
    >,
    player_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut Player),
        (
            Without<CityNpc>,
            Without<CityCar>,
            Without<CityCarPaint>,
            Without<DestructibleWindow>,
            Without<DestructibleProp>,
            Without<LexieBoss>,
        ),
    >,
}

#[allow(clippy::too_many_arguments)]
fn update_grenades(
    mut commands: Commands,
    time: Res<Time>,
    mut coop: ResMut<NetCoopEvents>,
    mut pending_blasts: ResMut<PendingBlasts>,
    mut gren_q: Query<(Entity, &mut Transform, &mut ThrownGrenade), Without<GrenadeFuseLed>>,
    mut led_q: Query<&mut Transform, (With<GrenadeFuseLed>, Without<ThrownGrenade>)>,
    box_q: Query<(&Transform, &PlatformBox), (Without<ThrownGrenade>, Without<GrenadeFuseLed>)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (bx, bz) in coop.grenade_blasts.drain(..) {
        pending_blasts.0.push(BlastEvent {
            center: Vec3::new(bx, 0.3, bz),
            radius: 14.0,
            from_car: false,
        });
    }

    for (g_e, mut g_tf, mut g) in &mut gren_q {
        g.fuse -= dt;
        g.vel.y -= 26.0 * dt;
        g_tf.translation += g.vel * dt;
        g_tf.rotate_x(g.ang_vel.x * dt);
        g_tf.rotate_y(g.ang_vel.y * dt);

        if g_tf.translation.y <= 0.22 {
            g_tf.translation.y = 0.22;
            if g.vel.y < -3.0 {
                g.vel.y = -g.vel.y * 0.45;
                g.vel.x *= 0.72;
                g.vel.z *= 0.72;
                g.ang_vel *= 0.75;
                play_sfx("grenade_bounce");
            } else {
                g.vel.y = 0.0;
                g.vel.x *= (1.0 - 5.0 * dt).max(0.0);
                g.vel.z *= (1.0 - 5.0 * dt).max(0.0);
            }
        }
        // Wall bounce off buildings
        let gp = g_tf.translation;
        for (pb_tf, pb) in &box_q {
            if pb_tf.translation.y + pb.half_extents.y <= 0.3 {
                continue;
            }
            let min = pb_tf.translation - pb.half_extents - Vec3::splat(0.25);
            let max = pb_tf.translation + pb.half_extents + Vec3::splat(0.25);
            if gp.x > min.x && gp.x < max.x && gp.y > min.y && gp.y < max.y && gp.z > min.z && gp.z < max.z {
                let d_xl = gp.x - min.x;
                let d_xr = max.x - gp.x;
                let d_zl = gp.z - min.z;
                let d_zr = max.z - gp.z;
                let m = d_xl.min(d_xr).min(d_zl).min(d_zr);
                if (m - d_xl).abs() < 0.01 || (m - d_xr).abs() < 0.01 {
                    g.vel.x = -g.vel.x * 0.5;
                } else {
                    g.vel.z = -g.vel.z * 0.5;
                }
                play_sfx("grenade_bounce");
                break;
            }
        }

        if g.fuse <= 0.0 {
            let pos = g_tf.translation;
            crate::audio::net_event(EVENT_GRENADE_BLAST, pack_grenade_blast(pos.x, pos.z));
            pending_blasts.0.push(BlastEvent {
                center: pos,
                radius: 14.0,
                from_car: false,
            });
            commands.entity(g_e).despawn_recursive();
        }
    }

    // Pulse red fuse LEDs faster before detonation
    let pulse = 0.7 + 0.6 * (time.elapsed_secs() * 24.0).sin().abs();
    for mut l_tf in &mut led_q {
        l_tf.scale = Vec3::splat(pulse);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_exploded_cars(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut pending_blasts: ResMut<PendingBlasts>,
    vfx: Option<Res<DestructionAssets>>,
    children_q: Query<&Children>,
    mut car_q: Query<(Entity, &mut Transform, &mut CityCar), Without<CityCarPaint>>,
    mut paint_q: Query<(&mut MeshMaterial3d<StandardMaterial>, &CityCarPaint), Without<CityCar>>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut new_blasts = Vec::new();
    let vfx_ref = vfx.as_deref();

    for (car_e, mut c_tf, mut car) in &mut car_q {
        if car.chain_fuse > 0.0 && car.exploded_timer <= 0.0 && !car.occupied {
            car.chain_fuse -= dt;
            if car.chain_fuse <= 0.0 {
                let away_xz = Vec2::new(car.yaw.cos(), -car.yaw.sin());
                detonate_car(
                    &mut commands,
                    car_e,
                    c_tf.translation,
                    &mut car,
                    away_xz,
                    0.75,
                    true,
                    &children_q,
                    &mut paint_q,
                    vfx_ref,
                    &mut stats,
                    &mut new_blasts,
                );
            }
        }
        if car.exploded_timer <= 0.0 {
            continue;
        }

        car.exploded_timer -= dt;
        car.explode_vel.y -= 28.0 * dt;
        c_tf.translation += car.explode_vel * dt;
        c_tf.translation.x = c_tf.translation.x.clamp(-130.0, 130.0);
        c_tf.translation.z = c_tf.translation.z.clamp(-130.0, 130.0);
        if push_car_out_of_pub217(&mut c_tf, car.yaw).is_some() {
            car.explode_vel.x = 0.0;
            car.explode_vel.z = 0.0;
        }
        if c_tf.translation.y <= 0.45 {
            c_tf.translation.y = 0.45;
            if car.explode_vel.y < -3.5 {
                car.explode_vel.y = -car.explode_vel.y * 0.30;
                car.explode_vel.x *= 0.65;
                car.explode_vel.z *= 0.65;
                car.explode_ang_vel *= 0.65;
            } else {
                car.explode_vel.y = 0.0;
                let damp = (1.0 - 4.5 * dt).max(0.0);
                car.explode_vel.x *= damp;
                car.explode_vel.z *= damp;
                car.explode_ang_vel *= (1.0 - 5.5 * dt).max(0.0);
            }
        }
        c_tf.rotate_x(car.explode_ang_vel.x * dt);
        c_tf.rotate_y(car.explode_ang_vel.y * dt);
        c_tf.rotate_z(car.explode_ang_vel.z * dt);

        if car.exploded_timer > 1.8 {
            car.burn_vfx_cd -= dt;
            if car.burn_vfx_cd <= 0.0 {
                car.burn_vfx_cd = 0.16;
                if let Some(assets) = vfx_ref {
                    let jitter = ((time.elapsed_secs() * 19.0).sin()) * 0.65;
                    spawn_debris_particle(
                        &mut commands,
                        assets,
                        c_tf.translation + Vec3::new(jitter, 1.0, -jitter * 0.7),
                        Vec3::new(jitter * 0.8, 3.8, 0.4),
                        Vec3::ZERO,
                        0.75,
                        DebrisKind::FirePuff,
                        Vec3::splat(0.65),
                    );
                    spawn_debris_particle(
                        &mut commands,
                        assets,
                        c_tf.translation + Vec3::new(-jitter * 0.5, 1.4, jitter * 0.5),
                        Vec3::new(-jitter * 0.6, 5.4, 0.5),
                        Vec3::ZERO,
                        1.45,
                        DebrisKind::SmokePuff,
                        Vec3::splat(0.85),
                    );
                }
            }
        }

        if car.exploded_timer <= 0.0 {
            if let Ok(kids) = children_q.get(car_e) {
                for &kid in kids.iter() {
                    if let Ok((mut mat, orig)) = paint_q.get_mut(kid) {
                        mat.0 = orig.original.clone();
                    }
                }
            }
            car.car_hp = 100.0;
            car.explode_vel = Vec3::ZERO;
            car.explode_ang_vel = Vec3::ZERO;
            car.knock_vel = Vec2::ZERO;
            car.velocity = Vec2::ZERO;
            car.yaw = car.base_yaw;
            c_tf.rotation = Quat::from_rotation_y(car.base_yaw);
            c_tf.translation = if car.parked {
                car.spawn_pos
            } else if car.axis_x {
                Vec3::new(-126.0 * car.dir, 0.0, car.lane_coord)
            } else {
                let road = (car.lane_coord / 46.0).round() * 46.0;
                let z = if crate::skatepark::bigpark_uturn_z(road).is_some() {
                    115.0
                } else {
                    -126.0 * car.dir
                };
                Vec3::new(car.lane_coord, 0.0, z)
            };
        }
    }
    pending_blasts.0.extend(new_blasts);
}

#[allow(clippy::too_many_arguments)]
fn apply_blast_destruction(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: SceneTransition,
    mut ragdoll: ResMut<Ragdoll>,
    mut pending_blasts: ResMut<PendingBlasts>,
    vfx: Option<Res<DestructionAssets>>,
    children_q: Query<&Children>,
    mut paint_q: Query<(&mut MeshMaterial3d<StandardMaterial>, &CityCarPaint), Without<CityCar>>,
    mut targets: BlastTargets,
) {
    if pending_blasts.0.is_empty() {
        return;
    }
    let blasts: Vec<BlastEvent> = pending_blasts.0.drain(..).take(8).collect();
    let vfx_ref = vfx.as_deref();
    let t = time.elapsed_secs();

    for blast in blasts {
        let center = blast.center;
        let max_age = if blast.from_car { 0.92 } else { 0.75 };
        spawn_explosion_vfx(&mut commands, &mut meshes, &mut materials, center, blast.radius, max_age);
        play_sfx(if blast.from_car { "car_explode" } else { "explosion" });
        stats.shake_request = stats.shake_request.max(if blast.from_car { 0.85 } else { 0.65 });

        // Blast NPCs into high-flying ragdolls!
        for (mut n_tf, mut npc) in &mut targets.npc_q {
            let delta = n_tf.translation - center;
            let dist = delta.length();
            if dist < blast.radius {
                let k = (1.0 - dist / blast.radius).max(0.25);
                let away = delta.with_y(0.0).normalize_or(Vec3::X);
                let imp = away * (26.0 * k) + Vec3::Y * (16.0 * k + 4.0);
                let ko = launch_npc_ragdoll(&mut npc, &mut n_tf, imp, 40.0 * k);
                stats.score += if ko { 200 } else { 90 };
                if ko {
                    stats.add_tendy_wendies(1);
                }
            }
        }

        // Explode or chain-detonate unoccupied cars!
        for (car_e, c_tf, mut car) in &mut targets.car_q {
            if car.occupied || car.exploded_timer > 0.0 {
                continue;
            }
            let diff = Vec2::new(c_tf.translation.x - center.x, c_tf.translation.z - center.z);
            let dist = diff.length();
            if dist < blast.radius {
                let k = (1.0 - dist / blast.radius).max(0.25);
                let away = if dist > 0.01 { diff / dist } else { Vec2::X };
                if !blast.from_car {
                    detonate_car(
                        &mut commands,
                        car_e,
                        c_tf.translation,
                        &mut car,
                        away,
                        k,
                        false,
                        &children_q,
                        &mut paint_q,
                        vfx_ref,
                        &mut stats,
                        &mut pending_blasts.0,
                    );
                } else if dist < 11.5 && car.chain_fuse <= 0.0 {
                    car.chain_fuse = 0.25;
                    car.knock_vel += away * (16.0 * k);
                }
            }
        }

        // Shatter storefront / Pub 217 windows within blast radius!
        let mut shattered = 0_u32;
        for (w_tf, mut win, mut w_vis) in &mut targets.win_q {
            if !win.intact {
                continue;
            }
            if w_tf.translation.distance(center) < blast.radius + 2.5 {
                win.intact = false;
                win.respawn_timer = 35.0;
                *w_vis = Visibility::Hidden;
                shattered += 1;
                stats.score += 75;
                if let Some(assets) = vfx_ref {
                    spawn_glass_shards(&mut commands, assets, w_tf.translation, win.outward_normal, 7, t + w_tf.translation.x);
                }
            }
        }
        if shattered > 0 {
            play_sfx("glass_shatter");
        }

        // Launch street props (hydrants gush water, trees shed leaves, benches fly)!
        let mut popped_hydrant = false;
        for (p_tf, mut prop) in &mut targets.prop_q {
            if prop.broken {
                continue;
            }
            let delta = p_tf.translation - center;
            let dist = delta.length();
            if dist < blast.radius {
                let k = (1.0 - dist / blast.radius).max(0.25);
                let away = delta.with_y(0.0).normalize_or(Vec3::Z);
                prop.broken = true;
                prop.respawn_timer = 26.0;
                prop.geyser_cd = 0.02;
                prop.vel = away * (14.0 * k + 6.0) + Vec3::Y * (11.0 * k + 5.0);
                prop.ang_vel = Vec3::new(5.2, 3.4, -4.8);
                stats.score += 100;
                if let Some(assets) = vfx_ref {
                    match prop.kind {
                        DestructiblePropKind::Tree => {
                            for i in 0..10_usize {
                                let a = i as f32 * 0.628 + t;
                                spawn_debris_particle(
                                    &mut commands,
                                    assets,
                                    prop.home_pos + Vec3::Y * 3.4,
                                    Vec3::new(a.cos() * 7.5, 6.0 + (i % 3) as f32 * 2.5, a.sin() * 7.5),
                                    Vec3::new(5.0, 7.0, 4.0),
                                    2.0,
                                    DebrisKind::LeafChunk,
                                    Vec3::splat(0.32),
                                );
                            }
                        }
                        DestructiblePropKind::Hydrant => {
                            popped_hydrant = true;
                            for i in 0..8_usize {
                                let a = i as f32 * 0.785 + t;
                                spawn_debris_particle(
                                    &mut commands,
                                    assets,
                                    prop.home_pos + Vec3::Y * 0.4,
                                    Vec3::new(a.cos() * 2.2, 13.5 + (i % 3) as f32 * 1.8, a.sin() * 2.2),
                                    Vec3::ZERO,
                                    1.2,
                                    DebrisKind::WaterDrop,
                                    Vec3::splat(0.24),
                                );
                            }
                        }
                        DestructiblePropKind::Bench => {
                            for i in 0..5_usize {
                                let a = i as f32 * 1.25 + t;
                                spawn_debris_particle(
                                    &mut commands,
                                    assets,
                                    prop.home_pos + Vec3::Y * 0.4,
                                    Vec3::new(a.cos() * 8.0, 7.5, a.sin() * 8.0),
                                    Vec3::new(7.0, -6.0, 5.0),
                                    1.6,
                                    DebrisKind::MetalChunk,
                                    Vec3::new(0.25, 0.14, 0.35),
                                );
                            }
                        }
                    }
                }
            }
        }
        if popped_hydrant {
            play_sfx("hydrant_pop");
        }

        // Blast Lexie if present!
        for (l_tf, mut lexie) in &mut targets.lexie_q {
            if stats.lexie_defeated || lexie.on_tower {
                continue;
            }
            let flat = Vec2::new(l_tf.translation.x - center.x, l_tf.translation.z - center.z).length();
            if flat < 16.0 {
                let dmg = 45.0;
                lexie.hp = if stats.tendies < 50 {
                    (lexie.hp - dmg).max(75.0)
                } else {
                    lexie.hp - dmg
                };
                stats.boss_hp = lexie.hp.max(0.0);
                lexie.hit_flash = 0.35;
                stats.score += 450;
            }
        }

        // Self-blast / close-range blast!
        let self_r = if blast.from_car { 7.5 } else { 6.0 };
        if stats.driving_car.is_none() && !stats.riding_wheel {
            if let Ok((p_tf, mut player)) = targets.player_q.get_single_mut() {
                let delta = p_tf.translation - center;
                let dist = delta.length();
                if dist < self_r && player.invuln_timer <= 0.0 && player.roll_timer <= 0.0 {
                    let away = delta.with_y(0.0).normalize_or(Vec3::Z);
                    let imp = away * 15.0 + Vec3::Y * 9.5;
                    if crate::ragdoll::can_ragdoll(&stats) {
                        ragdoll.request(Some(player.velocity.with_y(0.0)), imp, -6.5);
                    } else {
                        player.velocity = imp;
                    }
                    player.invuln_timer = 1.1;
                    stats.hurt_flash = 0.7;
                    let min_hp = if stats.free_roam { 1 } else { 0 };
                    stats.hearts = (stats.hearts - 1).max(min_hp);
                    if stats.hearts <= 0 && !stats.free_roam {
                        play_sfx("gameover");
                        next_state.set(GameState::GameOver);
                    }
                }
            }
        }
    }
}

fn update_destructibles_and_debris(
    mut commands: Commands,
    time: Res<Time>,
    vfx: Option<Res<DestructionAssets>>,
    mut win_q: Query<
        (&mut DestructibleWindow, &mut Visibility),
        (Without<DestructibleProp>, Without<FlyingDebris>),
    >,
    mut prop_q: Query<
        (&mut Transform, &mut DestructibleProp),
        (Without<DestructibleWindow>, Without<FlyingDebris>),
    >,
    mut debris_q: Query<
        (Entity, &mut Transform, &mut FlyingDebris),
        (Without<DestructibleWindow>, Without<DestructibleProp>),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    for (e, mut tf, mut d) in &mut debris_q {
        d.age += dt;
        if d.age >= d.max_age {
            commands.entity(e).despawn_recursive();
            continue;
        }
        let u = d.age / d.max_age;
        match d.kind {
            DebrisKind::FirePuff | DebrisKind::SmokePuff => {
                d.vel.y += 2.2 * dt;
                tf.translation += d.vel * dt;
                let s = (1.0 + u * 1.1) * (1.0 - u * 0.85).max(0.05);
                tf.scale = d.base_scale * s;
            }
            _ => {
                d.vel.y -= 26.0 * dt;
                tf.translation += d.vel * dt;
                tf.rotate_x(d.ang_vel.x * dt);
                tf.rotate_y(d.ang_vel.y * dt);
                tf.rotate_z(d.ang_vel.z * dt);
                if tf.translation.y <= 0.10 {
                    tf.translation.y = 0.10;
                    if d.vel.y < -3.0 {
                        d.vel.y = -d.vel.y * 0.35;
                        d.vel.x *= 0.65;
                        d.vel.z *= 0.65;
                    } else {
                        d.vel.y = 0.0;
                        d.vel.x *= (1.0 - 5.0 * dt).max(0.0);
                        d.vel.z *= (1.0 - 5.0 * dt).max(0.0);
                        d.ang_vel *= (1.0 - 6.0 * dt).max(0.0);
                    }
                }
                if u > 0.75 {
                    tf.scale = d.base_scale * ((1.0 - u) * 4.0).max(0.05);
                }
            }
        }
    }

    for (mut win, mut vis) in &mut win_q {
        if win.intact {
            continue;
        }
        win.respawn_timer -= dt;
        if win.respawn_timer <= 0.0 {
            win.intact = true;
            win.respawn_timer = 0.0;
            *vis = Visibility::Inherited;
        }
    }

    let vfx_ref = vfx.as_deref();
    for (mut tf, mut prop) in &mut prop_q {
        if !prop.broken {
            continue;
        }
        prop.respawn_timer -= dt;
        prop.vel.y -= 25.0 * dt;
        tf.translation += prop.vel * dt;
        tf.rotate_x(prop.ang_vel.x * dt);
        tf.rotate_y(prop.ang_vel.y * dt);
        tf.rotate_z(prop.ang_vel.z * dt);
        let floor_y = if prop.kind == DestructiblePropKind::Bench { 0.35 } else { 0.15 };
        if tf.translation.y <= floor_y {
            tf.translation.y = floor_y;
            if prop.vel.y < -3.0 {
                prop.vel.y = -prop.vel.y * 0.32;
                prop.vel.x *= 0.62;
                prop.vel.z *= 0.62;
                prop.ang_vel *= 0.65;
            } else {
                prop.vel.y = 0.0;
                prop.vel.x *= (1.0 - 5.0 * dt).max(0.0);
                prop.vel.z *= (1.0 - 5.0 * dt).max(0.0);
                prop.ang_vel *= (1.0 - 6.0 * dt).max(0.0);
            }
        }

        if prop.kind == DestructiblePropKind::Hydrant && prop.respawn_timer > 10.0 {
            prop.geyser_cd -= dt;
            if prop.geyser_cd <= 0.0 {
                prop.geyser_cd = 0.075;
                if let Some(assets) = vfx_ref {
                    for j in 0..2 {
                        let a = t * 11.0 + j as f32 * 3.14;
                        spawn_debris_particle(
                            &mut commands,
                            assets,
                            prop.home_pos + Vec3::Y * 0.32,
                            Vec3::new(a.cos() * 1.4, 12.0 + j as f32 * 2.0, a.sin() * 1.4),
                            Vec3::ZERO,
                            0.95,
                            DebrisKind::WaterDrop,
                            Vec3::splat(0.21),
                        );
                    }
                }
            }
        }

        if prop.respawn_timer <= 0.0 {
            prop.broken = false;
            prop.respawn_timer = 0.0;
            prop.vel = Vec3::ZERO;
            prop.ang_vel = Vec3::ZERO;
            tf.translation = prop.home_pos;
            tf.rotation = prop.home_rot;
        }
    }
}

pub fn spawn_explosion_vfx(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    max_radius: f32,
    max_age: f32,
) {
    let fire_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.52, 0.08, 0.85),
        emissive: LinearRgba::rgb(4.8, 1.8, 0.25),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.25, 0.75),
        emissive: LinearRgba::rgb(3.6, 2.5, 0.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            WeaponEntity,
            ExplosionVfx {
                age: 0.0,
                max_age,
                max_radius,
            },
            Transform::from_translation(center).with_scale(Vec3::splat(0.4)),
            Visibility::default(),
        ))
        .with_children(|ex| {
            ex.spawn((
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(fire_mat),
                Transform::default(),
            ));
            ex.spawn((
                Mesh3d(meshes.add(Torus::new(0.18, 1.0))),
                MeshMaterial3d(ring_mat),
                Transform::from_xyz(0.0, -0.1, 0.0),
            ));
            ex.spawn(PointLight {
                color: Color::srgb(1.0, 0.55, 0.15),
                intensity: 140_000.0,
                range: max_radius * 3.0,
                shadows_enabled: false,
                ..default()
            });
        });
}

fn update_explosions(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut ExplosionVfx)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut tf, mut ex) in &mut q {
        ex.age += dt;
        let u = (ex.age / ex.max_age).clamp(0.0, 1.0);
        if u >= 1.0 {
            commands.entity(e).despawn_recursive();
            continue;
        }
        let ease = 1.0 - (1.0 - u).powi(3);
        let r = 0.4 + (ex.max_radius - 0.4) * ease;
        let fade_y = r * (1.0 - u * 0.45);
        tf.scale = Vec3::new(r, fade_y.max(0.2), r);
    }
}
