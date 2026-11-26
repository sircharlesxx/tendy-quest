//! City arsenal: switchable Lightsaber / 9mm Pistol / Tendy Shotgun / Rapid SMG (#15),
//! bouncing high-explosive Grenades (#19), crew lightsaber visuals (#9), and bullet/blast physics.

use bevy::prelude::*;

use crate::audio::play_sfx;
use crate::boss::{HeroSaberBlade, HERO_SABER_AURA_COLOR, HERO_SABER_LIGHT};
use crate::city::{CityCar, CityNpc, CityWorldMarker, LexieBoss, launch_npc_ragdoll, wrap_angle};
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
            .add_systems(OnEnter(GameState::CityPlay), spawn_weapon_hud)
            .add_systems(OnExit(GameState::CityPlay), cleanup_weapons_city)
            .add_systems(
                Update,
                (
                    handle_weapon_input,
                    sync_weapon_visuals,
                    update_bullets,
                    update_grenades,
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

#[derive(Component)]
struct WeaponEntity;

#[derive(Component)]
struct WeaponHudRoot;

#[derive(Component)]
struct WeaponHudText;

#[derive(Component)]
pub struct CrewHeroSaber;

#[derive(Component)]
pub struct HeldGunVisual;

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
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.12, 0.80)),
            BorderColor(Color::srgb(0.95, 0.78, 0.25)),
            BorderRadius::all(Val::Px(8.0)),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                WeaponHudText,
                Text::new("[1] SABER  [2] PISTOL  [3] SHOTGUN  [4] SMG | [G] GRENADE x5"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.94, 0.82)),
            ));
        });
}

fn cleanup_weapons_city(
    mut commands: Commands,
    hud_q: Query<Entity, With<WeaponHudRoot>>,
    ent_q: Query<Entity, With<WeaponEntity>>,
) {
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
}

fn update_weapon_hud(
    stats: Res<PlayerStats>,
    mut text_q: Query<&mut Text, With<WeaponHudText>>,
    mut vis_q: Query<&mut Visibility, With<WeaponHudRoot>>,
) {
    let hide = stats.char_select_open
        || stats.phone_open
        || stats.bartender_open
        || stats.riding_wheel;
    let want = if hide {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut vis_q {
        if *v != want {
            *v = want;
        }
    }
    if hide {
        return;
    }
    let slot = |idx: u8, tag: &str| -> String {
        if stats.equipped_weapon == idx {
            format!("[*{}*]", tag)
        } else {
            tag.to_string()
        }
    };
    let line = format!(
        "{}  {}  {}  {}  [C] Cycle | [G] Grenades: {}",
        slot(0, "1:SABER"),
        slot(1, "2:PISTOL"),
        slot(2, "3:SHOTGUN"),
        slot(3, "4:SMG"),
        stats.grenades
    );
    if let Ok(mut txt) = text_q.get_single_mut() {
        if txt.0 != line {
            txt.0 = line;
        }
    }
}

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
        || ragdoll.active()
    {
        return;
    }

    // Weapon selection [1..4] or [C] cycle
    let mut switched = false;
    if keys.just_pressed(KeyCode::Digit1) {
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
    } else if keys.just_pressed(KeyCode::KeyC) {
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

    // Throw Grenade [G] (#19)
    if keys.just_pressed(KeyCode::KeyG) && cd.grenade_cd <= 0.0 {
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
            HeldGunVisual,
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
        &mut Visibility,
        (
            With<CrewHeroSaber>,
            Without<HeroSaberBlade>,
            Without<HeldGunVisual>,
        ),
    >,
    mut gun_q: Query<
        (&mut Visibility, &mut Transform),
        (
            With<HeldGunVisual>,
            Without<HeroSaberBlade>,
            Without<CrewHeroSaber>,
        ),
    >,
) {
    let show_saber = stats.equipped_weapon == 0 && stats.driving_car.is_none();
    let show_gun = stats.equipped_weapon > 0 && stats.driving_car.is_none();

    // 1. Stan's HeroRightArm gets HeldGunVisual if missing
    if let Some((r_arm, _)) = arm_q.iter().find(|(_, n)| n.as_str() == "HeroRightArm") {
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
            );
        }
    }

    // 2. Local crew rig's right arm (belongs to LocalCrewVisual ancestor) gets Saber + HeldGunVisual
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
        if !has_saber {
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
            );
        }
    }

    let saber_vis = if show_saber {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut stan_saber_q {
        if *v != saber_vis {
            *v = saber_vis;
        }
    }
    for mut v in &mut crew_saber_q {
        if *v != saber_vis {
            *v = saber_vis;
        }
    }

    let gun_vis = if show_gun {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let gun_scale = match stats.equipped_weapon {
        2 => Vec3::new(1.25, 1.15, 1.75), // Shotgun longer/chunkier
        3 => Vec3::new(1.10, 1.25, 1.35), // SMG compact tactical
        _ => Vec3::ONE,                   // 9mm Pistol
    };
    for (mut v, mut tf) in &mut gun_q {
        if *v != gun_vis {
            *v = gun_vis;
        }
        if tf.scale != gun_scale {
            tf.scale = gun_scale;
        }
    }
}

fn update_bullets(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut bullet_q: Query<(Entity, &mut Transform, &mut BulletProjectile)>,
    mut npc_q: Query<
        (&mut Transform, &mut CityNpc),
        (Without<BulletProjectile>, Without<CityCar>, Without<LexieBoss>, Without<PlatformBox>),
    >,
    mut car_q: Query<
        (&Transform, &mut CityCar),
        (Without<BulletProjectile>, Without<CityNpc>, Without<LexieBoss>, Without<PlatformBox>),
    >,
    mut lexie_q: Query<
        (&Transform, &mut LexieBoss),
        (Without<BulletProjectile>, Without<CityNpc>, Without<CityCar>, Without<PlatformBox>),
    >,
    box_q: Query<
        (&Transform, &PlatformBox),
        (Without<BulletProjectile>, Without<CityNpc>, Without<CityCar>, Without<LexieBoss>),
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

        // Hit Unoccupied Cars
        for (c_tf, mut car) in &mut car_q {
            if car.occupied {
                continue;
            }
            let flat = Vec2::new(p.x - c_tf.translation.x, p.z - c_tf.translation.z).length();
            if flat < 2.25 && (p.y - c_tf.translation.y).abs() < 1.8 {
                let dir = Vec2::new(b.vel.x, b.vel.z).normalize_or_zero();
                car.knock_vel += dir * b.car_knock;
                car.yaw = wrap_angle(car.yaw + (dir.x * 0.08));
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
struct GrenadeTargets<'w, 's> {
    npc_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut CityNpc),
        (Without<ThrownGrenade>, Without<GrenadeFuseLed>, Without<CityCar>, Without<LexieBoss>, Without<PlatformBox>, Without<Player>),
    >,
    car_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut CityCar),
        (Without<ThrownGrenade>, Without<GrenadeFuseLed>, Without<CityNpc>, Without<LexieBoss>, Without<PlatformBox>, Without<Player>),
    >,
    lexie_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut LexieBoss),
        (Without<ThrownGrenade>, Without<GrenadeFuseLed>, Without<CityNpc>, Without<CityCar>, Without<PlatformBox>, Without<Player>),
    >,
    box_q: Query<
        'w,
        's,
        (&'static Transform, &'static PlatformBox),
        (Without<ThrownGrenade>, Without<GrenadeFuseLed>, Without<CityNpc>, Without<CityCar>, Without<LexieBoss>, Without<Player>),
    >,
    player_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut Player),
        (Without<ThrownGrenade>, Without<GrenadeFuseLed>, Without<CityNpc>, Without<CityCar>, Without<LexieBoss>, Without<PlatformBox>),
    >,
}

#[allow(clippy::too_many_arguments)]
fn update_grenades(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut coop: ResMut<NetCoopEvents>,
    mut next_state: SceneTransition,
    mut ragdoll: ResMut<Ragdoll>,
    mut gren_q: Query<(Entity, &mut Transform, &mut ThrownGrenade), Without<GrenadeFuseLed>>,
    mut led_q: Query<&mut Transform, (With<GrenadeFuseLed>, Without<ThrownGrenade>)>,
    mut targets: GrenadeTargets,
) {
    let dt = time.delta_secs().min(0.05);
    let mut blasts: Vec<Vec3> = Vec::new();
    for (bx, bz) in coop.grenade_blasts.drain(..) {
        blasts.push(Vec3::new(bx, 0.3, bz));
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
        for (pb_tf, pb) in &targets.box_q {
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
            blasts.push(pos);
            commands.entity(g_e).despawn_recursive();
        }
    }

    // Pulse red fuse LEDs faster before detonation
    let pulse = 0.7 + 0.6 * (time.elapsed_secs() * 24.0).sin().abs();
    for mut l_tf in &mut led_q {
        l_tf.scale = Vec3::splat(pulse);
    }

    if blasts.is_empty() {
        return;
    }

    for center in blasts {
        spawn_explosion_vfx(&mut commands, &mut meshes, &mut materials, center, 11.5, 0.75);
        play_sfx("explosion");
        stats.shake_request = stats.shake_request.max(0.65);

        // Blast NPCs into high-flying ragdolls!
        for (mut n_tf, mut npc) in &mut targets.npc_q {
            let delta = n_tf.translation - center;
            let dist = delta.length();
            if dist < 13.0 {
                let k = (1.0 - dist / 13.0).max(0.25);
                let away = delta.with_y(0.0).normalize_or(Vec3::X);
                let imp = away * (26.0 * k) + Vec3::Y * (16.0 * k + 4.0);
                let ko = launch_npc_ragdoll(&mut npc, &mut n_tf, imp, 40.0 * k);
                stats.score += if ko { 200 } else { 90 };
                if ko {
                    stats.add_tendy_wendies(1);
                }
            }
        }

        // Blast unoccupied cars sideways!
        for (c_tf, mut car) in &mut targets.car_q {
            if car.occupied {
                continue;
            }
            let diff = Vec2::new(c_tf.translation.x - center.x, c_tf.translation.z - center.z);
            let dist = diff.length();
            if dist < 14.0 {
                let k = (1.0 - dist / 14.0).max(0.2);
                let away = if dist > 0.01 { diff / dist } else { Vec2::X };
                car.knock_vel += away * (32.0 * k);
                car.yaw = wrap_angle(car.yaw + 0.85 * k);
                play_sfx("car_crash");
                stats.score += 150;
            }
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

        // Self-blast / close-range friend blast!
        if stats.driving_car.is_none() && !stats.riding_wheel {
            if let Ok((p_tf, mut player)) = targets.player_q.get_single_mut() {
                let delta = p_tf.translation - center;
                let dist = delta.length();
                if dist < 6.0 && player.invuln_timer <= 0.0 && player.roll_timer <= 0.0 {
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
