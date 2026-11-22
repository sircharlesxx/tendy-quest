mod audio;
mod beer_fx;
mod boss;
mod casino;
mod chillary;
mod city;
mod controls;
mod derek;
mod enemies;
mod ferris;
mod gfx;
mod glenn;
mod level;
mod lexie_battle;
mod lexie_victory;
mod nate;
mod net;
mod player;
mod pub_portal;
mod scooter;
mod shapes;
mod skatepark;
mod skip;
mod state;
mod stunts;
mod tj;
mod transition;
mod ui;

use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use boss::{
    ensure_hero_eye_reflections, ensure_hero_vikings_gear, enter_boss_fight, enter_cutscene,
    enter_headphone_cutscene, exit_boss_fight, exit_cutscene, exit_headphone_cutscene,
    update_boss_fight, update_boss_healing_tendies, update_cutscene, update_enraged_visuals,
    update_headphone_cutscene,
};
use city::{
    animate_city_intro_lexie, enter_city_intro_cutscene, enter_city_play,
    enter_mom_victory_cutscene, exit_city_intro_cutscene, exit_city_play,
    exit_mom_victory_cutscene, record_city_facing, update_city_intro_cutscene, update_city_intro_subtitles,
    update_city_driving, update_city_lexie, update_city_minimap, update_city_npcs,
    update_city_pickups, update_city_traffic, update_mom_victory_cutscene,
    update_mom_victory_subtitles, update_city_crew_gate, update_pub217_door,
};
use derek::{
    DerekCutsceneState, enter_derek_cutscene, exit_derek_cutscene, update_city_derek,
    update_derek_cutscene,
};
use glenn::{
    GlennCutsceneState, enter_glenn_cutscene, exit_glenn_cutscene, update_city_glenn,
    update_glenn_cutscene,
};
use chillary::{
    ChillaryCutsceneState, ChillaryPrompt, enter_chillary_cutscene, exit_chillary_cutscene,
    update_chillary_cutscene, update_city_chillary,
};
use enemies::{animate_enemies, spawn_initial_enemies, update_enemies};
use level::{retry_failed_scenes, setup_lighting_and_camera, spawn_kitchen_level};
use lexie_victory::{
    LexieVictoryState, enter_lexie_victory_cutscene, exit_lexie_victory_cutscene,
    update_lexie_victory_cutscene,
};
use nate::{
    NateCutsceneState, enter_nate_cutscene, exit_nate_cutscene, update_city_nate,
    update_nate_cutscene,
};
use player::{
    CameraSet, animate_and_collect_pickups, animate_pickup_pops, reset_hit_reaction, animate_player_limbs, camera_begin_blend,
    camera_mouse_look, camera_snap_follow, player_movement, spawn_player, unlock_cursor,
    update_camera, victory_dance,
};
use scooter::{
    MikeCutsceneDialog, PhoneMazeState, enter_mike_cutscene, exit_mike_cutscene,
    update_city_scooter_and_phone, update_mike_cutscene,
};
use state::{CameraOrbit, GameState, PlayerStats};
use tj::{enter_tj_cutscene, exit_tj_cutscene, update_city_tj, update_tj_cutscene};
use ui::{
    HelpPause, adapt_ui_to_viewport, check_url_test_mode, clear_help_resume_grace,
    close_help_overlay, despawn_tutorial_overlay, gameplay_running, handle_restart_input,
    handle_tutorial_input, restart_world_after_game_over, restart_world_after_stage_clear,
    spawn_game_over_overlay, spawn_hud, spawn_hurt_flash_overlay, update_hurt_flash, spawn_stage_clear_overlay, spawn_tutorial_overlay,
    toggle_audio_hotkey, update_hud,
};
use transition::{SceneFade, drive_scene_fade, sim_running, spawn_fade_overlay, wait_for_boot_assets};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tendy Quest — Giant Kitchen".into(),
                        // Fill the responsive 100dvh container in index.html across desktop & mobile
                        fit_canvas_to_parent: true,
                        // Prevent Space/Arrow keys and touch drags from scrolling the browser page
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // Static GitHub Pages hosting has no .meta sidecar files; skip .meta HTTP checks
                    // so every .glb/.mp3/.wav load doesn't trigger extra 404 requests.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .add_plugins(gfx::GfxPlugin)
        .add_plugins(beer_fx::BeerFxPlugin)
        // Hold Enter / Start / touch SKIP to skip any cutscene (see skip.rs).
        .add_plugins(skip::CutsceneSkipPlugin)
        // Bottom-left controls strip + Esc / H / Start pause menu (see controls.rs).
        .add_plugins(controls::ControlsPlugin)
        // Riverfront Ferris wheel: spin, ride, ride camera (see ferris.rs).
        .add_plugins(ferris::FerrisPlugin)
        // Tendy Palace Casino: Level 3 Blackjack & Buffalo / Lucky Ladies slots in Tendy Wendies (casino.rs).
        .add_plugins(casino::CasinoPlugin)
        // Pub 217 Maria bartender, "ORDER 10 BEERS" prompt & Women's Bathroom Portal cutscene (pub_portal.rs).
        .add_plugins(pub_portal::PubPortalPlugin)
        // Distributed Free Mode P2P multiplayer (WebRTC mesh via public WebTorrent trackers, inert outside Free Mode; see net.rs).
        .add_plugins(net::NetPlugin)
        // Skate park on Greene Square + the lot south of it: pieces, ride physics, grinds (skatepark.rs).
        .add_plugins(skatepark::SkateParkPlugin)
        .init_state::<GameState>()
        .init_resource::<PlayerStats>()
        .init_resource::<CameraOrbit>()
        .init_resource::<player::LimbAnimState>()
        .init_resource::<PhoneMazeState>()
        .init_resource::<MikeCutsceneDialog>()
        .init_resource::<DerekCutsceneState>()
        .init_resource::<LexieVictoryState>()
        .init_resource::<lexie_battle::RpgBattleState>()
        .init_resource::<NateCutsceneState>()
        .init_resource::<GlennCutsceneState>()
        // Optional boardwalk easter-egg cutscene resources; independent of crew/boss progression.
        .init_resource::<ChillaryCutsceneState>()
        .init_resource::<ChillaryPrompt>()
        .init_resource::<stunts::StuntState>()
        // Fade-through-black scene transitions + in-game help pause (see transition.rs / ui.rs).
        .init_resource::<SceneFade>()
        .init_resource::<HelpPause>()
        // Look input -> everything that moves the hero -> follow camera, in every state.
        .configure_sets(
            Update,
            (CameraSet::Look, CameraSet::Motion, CameraSet::Follow).chain(),
        )
        .add_systems(
            Startup,
            (
                setup_lighting_and_camera,
                spawn_kitchen_level,
                spawn_player,
                spawn_initial_enemies,
                spawn_hud,
                spawn_hurt_flash_overlay.after(spawn_hud),
                spawn_fade_overlay.after(spawn_hud),
            ),
        )
        .add_systems(Update, wait_for_boot_assets)
        .add_systems(Update, player::update_hit_stop)
        .add_systems(PostUpdate, (drive_scene_fade, clear_help_resume_grace))
        .add_systems(OnExit(GameState::Playing), (close_help_overlay, reset_hit_reaction))
        .add_systems(OnExit(GameState::BossFight), (close_help_overlay, reset_hit_reaction))
        .add_systems(OnExit(GameState::CityPlay), (close_help_overlay, reset_hit_reaction))
        .add_systems(OnExit(GameState::GameOver), restart_world_after_game_over)
        .add_systems(OnExit(GameState::StageClear), restart_world_after_stage_clear)
        .add_systems(
            Update,
            (
                // Retries GLB loads if transient network hiccups occur on WASM
                retry_failed_scenes,
                toggle_audio_hotkey,
                update_enraged_visuals,
                update_hud,
                adapt_ui_to_viewport,
                // Hit feedback vignette and collected-pickup pops run in every state so a pop
                // or flash started just before a scene change still finishes and cleans up.
                update_hurt_flash,
                animate_pickup_pops,
                player::update_saber_fx,
                player::update_saber_charge_glow,
            ),
        )
        .add_systems(OnEnter(GameState::Tutorial), spawn_tutorial_overlay)
        .add_systems(OnExit(GameState::Tutorial), despawn_tutorial_overlay)
        .add_systems(
            Update,
            (
                check_url_test_mode,
                handle_tutorial_input,
                animate_player_limbs,
                update_camera.in_set(CameraSet::Follow),
            )
                .run_if(in_state(GameState::Tutorial))
                .run_if(sim_running),
        )
        .add_systems(
            OnEnter(GameState::Playing),
            (player::lock_touch_playing, camera_begin_blend),
        )
        .add_systems(OnExit(GameState::Playing), unlock_cursor)
        .add_systems(
            Update,
            (
                camera_mouse_look.in_set(CameraSet::Look),
                player_movement.in_set(CameraSet::Motion),
                animate_player_limbs,
                update_camera.in_set(CameraSet::Follow),
                animate_and_collect_pickups,
                update_enemies,
                animate_enemies,
            )
                .run_if(in_state(GameState::Playing))
                .run_if(gameplay_running),
        )
        .add_systems(OnEnter(GameState::HeadphoneCutscene), enter_headphone_cutscene)
        .add_systems(OnExit(GameState::HeadphoneCutscene), exit_headphone_cutscene)
        .add_systems(
            Update,
            (
                update_headphone_cutscene,
                ensure_hero_vikings_gear,
                animate_player_limbs,
            )
                .run_if(in_state(GameState::HeadphoneCutscene))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::Cutscene), enter_cutscene)
        .add_systems(OnExit(GameState::Cutscene), exit_cutscene)
        .add_systems(
            Update,
            (
                update_cutscene,
                ensure_hero_vikings_gear,
                ensure_hero_eye_reflections,
                animate_player_limbs,
            )
                .run_if(in_state(GameState::Cutscene))
                .run_if(sim_running),
        )
        .add_systems(
            OnEnter(GameState::BossFight),
            (player::lock_touch_playing, enter_boss_fight, camera_begin_blend),
        )
        .add_systems(
            OnExit(GameState::BossFight),
            (unlock_cursor, exit_boss_fight),
        )
        .add_systems(
            Update,
            (
                ensure_hero_vikings_gear,
                camera_mouse_look.in_set(CameraSet::Look),
                player_movement.in_set(CameraSet::Motion),
                animate_player_limbs,
                update_camera.in_set(CameraSet::Follow),
                update_boss_fight,
                boss::update_mom_guard_cues.after(update_boss_fight),
                update_boss_healing_tendies,
                update_enemies,
                animate_enemies,
            )
                .run_if(in_state(GameState::BossFight))
                .run_if(gameplay_running),
        )
        .add_systems(OnEnter(GameState::MomVictoryCutscene), enter_mom_victory_cutscene)
        .add_systems(OnExit(GameState::MomVictoryCutscene), exit_mom_victory_cutscene)
        .add_systems(
            Update,
            (
                update_mom_victory_cutscene,
                update_mom_victory_subtitles,
                ensure_hero_vikings_gear,
                animate_player_limbs,
            )
                .run_if(in_state(GameState::MomVictoryCutscene))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::CityIntroCutscene), enter_city_intro_cutscene)
        .add_systems(OnExit(GameState::CityIntroCutscene), exit_city_intro_cutscene)
        .add_systems(
            Update,
            (
                update_city_intro_cutscene,
                update_city_intro_subtitles,
                animate_city_intro_lexie,
                update_city_crew_gate,
                update_city_traffic,
                update_city_npcs,
                ensure_hero_vikings_gear,
                animate_player_limbs,
            )
                .run_if(in_state(GameState::CityIntroCutscene))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::CityPlay), (enter_city_play, camera_begin_blend))
        .add_systems(
            OnExit(GameState::CityPlay),
            (unlock_cursor, exit_city_play, record_city_facing, city::despawn_prop_prompt),
        )
        .add_systems(
            Update,
            // Beer tower, parked car and the license card. After the crew so a same-frame
            // interact hint from the bar wins over Glenn's free-roam line.
            // Why `.chain()` includes `update_city_chillary` ahead of `update_city_props`:
            // populates `ChillaryPrompt` same-frame with zero 1-frame UI lag, while `update_city_props`
            // keeps Ferris wheel boarding higher priority so players trying to ride aren't interrupted.
            (
                update_city_chillary,
                pub_portal::update_pub_bartender,
                city::update_city_props,
                city::update_license_card,
                city::sync_parked_car_collider,
            )
                .chain()
                .after(update_city_glenn)
                .before(update_city_driving)
                .run_if(in_state(GameState::CityPlay))
                .run_if(gameplay_running),
        )
        .add_systems(
            Update,
            (
                ensure_hero_vikings_gear,
                camera_mouse_look.in_set(CameraSet::Look),
                (
                    player_movement,
                    update_city_driving,
                    // No phone or scooter while riding the Ferris wheel.
                    update_city_scooter_and_phone.run_if(ferris::not_riding_wheel),
                )
                    .in_set(CameraSet::Motion),
                // After the skate park's on-foot pass so the hero's grounded pose is final.
                animate_player_limbs.after(skatepark::ParkWalkSet),
                update_camera.in_set(CameraSet::Follow),
                (update_city_crew_gate, update_pub217_door, city::update_crew_callouts),
                update_city_traffic.before(stunts::update_ramp_cars),
                update_city_npcs,
                update_city_tj,
                update_city_derek,
                update_city_nate,
                update_city_glenn,
                update_city_pickups,
                update_city_lexie,
                update_city_minimap,
            )
                .run_if(in_state(GameState::CityPlay))
                .run_if(gameplay_running),
        )
        .add_systems(
            Update,
            // Stunt ramps, boost rings, and backflips adjust scooter transform/velocity after
            // base scooter driving runs so stunts aren't overwritten by ground clamping.
            stunts::update_scooter_stunts
                .after(update_city_scooter_and_phone)
                .in_set(CameraSet::Motion)
                .run_if(in_state(GameState::CityPlay))
                .run_if(gameplay_running),
        )
        .add_systems(
            Update,
            // Wedge ramps are solid: on foot with the skate park's on-foot pass, cars after every
            // system that moves them (driving, traffic, park blocking), all before the camera.
            (
                stunts::update_ramp_walking.in_set(skatepark::ParkWalkSet),
                stunts::update_ramp_cars
                    .after(update_city_driving)
                    .in_set(CameraSet::Motion),
                stunts::debug_ramp_car.before(update_city_driving),
            )
                .run_if(in_state(GameState::CityPlay))
                .run_if(gameplay_running),
        )
        .add_systems(OnEnter(GameState::TjCutscene), enter_tj_cutscene)
        .add_systems(OnExit(GameState::TjCutscene), exit_tj_cutscene)
        .add_systems(
            Update,
            (
                update_tj_cutscene,
                ensure_hero_vikings_gear,
                ensure_hero_eye_reflections,
            )
                .run_if(in_state(GameState::TjCutscene))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::MikeCutscene), enter_mike_cutscene)
        .add_systems(OnExit(GameState::MikeCutscene), exit_mike_cutscene)
        .add_systems(
            Update,
            update_mike_cutscene.run_if(in_state(GameState::MikeCutscene)).run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::DerekCutscene), enter_derek_cutscene)
        .add_systems(OnExit(GameState::DerekCutscene), exit_derek_cutscene)
        .add_systems(
            Update,
            update_derek_cutscene.run_if(in_state(GameState::DerekCutscene)).run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::NateCutscene), enter_nate_cutscene)
        .add_systems(OnExit(GameState::NateCutscene), exit_nate_cutscene)
        .add_systems(
            Update,
            update_nate_cutscene.run_if(in_state(GameState::NateCutscene)).run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::GlennCutscene), enter_glenn_cutscene)
        .add_systems(OnExit(GameState::GlennCutscene), exit_glenn_cutscene)
        .add_systems(
            Update,
            update_glenn_cutscene.run_if(in_state(GameState::GlennCutscene)).run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::ChillaryCutscene), enter_chillary_cutscene)
        .add_systems(OnExit(GameState::ChillaryCutscene), exit_chillary_cutscene)
        .add_systems(
            Update,
            update_chillary_cutscene.run_if(in_state(GameState::ChillaryCutscene)).run_if(sim_running),
        )
        .add_systems(
            OnExit(GameState::CasinoPlay),
            (close_help_overlay, reset_hit_reaction, unlock_cursor),
        )
        .add_systems(
            Update,
            (
                camera_mouse_look.in_set(CameraSet::Look),
                player_movement.in_set(CameraSet::Motion),
                animate_player_limbs,
                update_camera.in_set(CameraSet::Follow),
            )
                .run_if(in_state(GameState::CasinoPlay))
                .run_if(gameplay_running),
        )
        .add_systems(
            OnEnter(GameState::LexieBattle),
            (unlock_cursor, lexie_battle::enter_lexie_battle),
        )
        .add_systems(OnExit(GameState::LexieBattle), lexie_battle::exit_lexie_battle)
        .add_systems(
            Update,
            (
                lexie_battle::update_lexie_battle,
                lexie_battle::animate_lexie_battle_stage,
                lexie_battle::animate_lexie_battle_limbs,
                lexie_battle::update_lexie_battle_ui,
            )
                .run_if(in_state(GameState::LexieBattle))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::LexieVictoryCutscene), enter_lexie_victory_cutscene)
        .add_systems(OnExit(GameState::LexieVictoryCutscene), exit_lexie_victory_cutscene)
        .add_systems(
            Update,
            update_lexie_victory_cutscene.run_if(in_state(GameState::LexieVictoryCutscene)).run_if(sim_running),
        )
        .add_systems(
            OnEnter(GameState::StageClear),
            (spawn_stage_clear_overlay, camera_snap_follow),
        )
        .add_systems(
            Update,
            (
                victory_dance.in_set(CameraSet::Motion),
                animate_player_limbs,
                update_camera.in_set(CameraSet::Follow),
                handle_restart_input,
            )
                .run_if(in_state(GameState::StageClear))
                .run_if(sim_running),
        )
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over_overlay)
        .add_systems(
            Update,
            handle_restart_input
                .run_if(in_state(GameState::GameOver))
                .run_if(sim_running),
        )
        .run();
}
