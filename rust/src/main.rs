mod audio;
mod boss;
mod city;
mod enemies;
mod level;
mod player;
mod state;
mod ui;

use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use boss::{
    ensure_hero_eye_reflections, ensure_hero_vikings_gear, enter_boss_fight, enter_cutscene,
    enter_headphone_cutscene, exit_boss_fight, exit_cutscene, exit_headphone_cutscene,
    update_boss_fight, update_cutscene, update_enraged_visuals, update_headphone_cutscene,
};
use city::{
    animate_city_intro_lexie, enter_city_intro_cutscene, enter_city_play,
    enter_mom_victory_cutscene, exit_city_intro_cutscene, exit_city_play,
    exit_mom_victory_cutscene, update_city_intro_cutscene, update_city_intro_subtitles,
    update_city_lexie, update_city_minimap, update_city_npcs, update_city_pickups,
    update_city_traffic, update_mom_victory_cutscene, update_mom_victory_subtitles,
};
use enemies::{animate_enemies, spawn_initial_enemies, update_enemies};
use level::{retry_failed_scenes, setup_lighting_and_camera, spawn_kitchen_level};
use player::{
    animate_and_collect_pickups, animate_player_limbs, camera_mouse_look, player_movement,
    spawn_player, unlock_cursor, update_camera, victory_dance,
};
use state::{CameraOrbit, GameState, PlayerStats};
use ui::{
    adapt_ui_to_viewport, check_url_test_mode, despawn_tutorial_overlay, handle_restart_input,
    handle_tutorial_input, spawn_game_over_overlay, spawn_hud, spawn_stage_clear_overlay,
    spawn_tutorial_overlay, toggle_audio_hotkey, toggle_tutorial_hotkey, update_hud,
};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tendy Quest — Giant Kitchen".into(),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .init_state::<GameState>()
        .init_resource::<PlayerStats>()
        .init_resource::<CameraOrbit>()
        .add_systems(
            Startup,
            (
                setup_lighting_and_camera,
                spawn_kitchen_level,
                spawn_player,
                spawn_initial_enemies,
                spawn_hud,
            ),
        )
        .add_systems(
            Update,
            (
                retry_failed_scenes,
                toggle_audio_hotkey,
                update_enraged_visuals,
                update_hud,
                adapt_ui_to_viewport,
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
                update_camera,
            )
                .run_if(in_state(GameState::Tutorial)),
        )
        .add_systems(OnEnter(GameState::Playing), player::lock_touch_playing)
        .add_systems(OnExit(GameState::Playing), unlock_cursor)
        .add_systems(
            Update,
            (
                camera_mouse_look,
                player_movement,
                animate_player_limbs,
                update_camera,
                animate_and_collect_pickups,
                update_enemies,
                animate_enemies,
                toggle_tutorial_hotkey,
            )
                .run_if(in_state(GameState::Playing)),
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
                .run_if(in_state(GameState::HeadphoneCutscene)),
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
                .run_if(in_state(GameState::Cutscene)),
        )
        .add_systems(
            OnEnter(GameState::BossFight),
            (player::lock_touch_playing, enter_boss_fight),
        )
        .add_systems(
            OnExit(GameState::BossFight),
            (unlock_cursor, exit_boss_fight),
        )
        .add_systems(
            Update,
            (
                ensure_hero_vikings_gear,
                camera_mouse_look,
                player_movement,
                animate_player_limbs,
                update_camera,
                update_boss_fight,
                update_enemies,
                animate_enemies,
                toggle_tutorial_hotkey,
            )
                .run_if(in_state(GameState::BossFight)),
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
                .run_if(in_state(GameState::MomVictoryCutscene)),
        )
        .add_systems(OnEnter(GameState::CityIntroCutscene), enter_city_intro_cutscene)
        .add_systems(OnExit(GameState::CityIntroCutscene), exit_city_intro_cutscene)
        .add_systems(
            Update,
            (
                update_city_intro_cutscene,
                update_city_intro_subtitles,
                animate_city_intro_lexie,
                update_city_traffic,
                update_city_npcs,
                ensure_hero_vikings_gear,
                animate_player_limbs,
            )
                .run_if(in_state(GameState::CityIntroCutscene)),
        )
        .add_systems(OnEnter(GameState::CityPlay), enter_city_play)
        .add_systems(OnExit(GameState::CityPlay), (unlock_cursor, exit_city_play))
        .add_systems(
            Update,
            (
                ensure_hero_vikings_gear,
                camera_mouse_look,
                player_movement,
                animate_player_limbs,
                update_camera,
                update_city_traffic,
                update_city_npcs,
                update_city_pickups,
                update_city_lexie,
                update_city_minimap,
                toggle_tutorial_hotkey,
            )
                .run_if(in_state(GameState::CityPlay)),
        )
        .add_systems(OnEnter(GameState::StageClear), spawn_stage_clear_overlay)
        .add_systems(
            Update,
            (victory_dance, animate_player_limbs, update_camera, handle_restart_input)
                .run_if(in_state(GameState::StageClear)),
        )
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over_overlay)
        .add_systems(
            Update,
            handle_restart_input.run_if(in_state(GameState::GameOver)),
        )
        .run();
}
