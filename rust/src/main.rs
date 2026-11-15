mod enemies;
mod level;
mod player;
mod state;
mod ui;

use bevy::prelude::*;
use enemies::{spawn_initial_enemies, update_enemies};
use level::{setup_lighting_and_camera, spawn_kitchen_level};
use player::{
    animate_and_collect_pickups, camera_mouse_look, player_movement, spawn_player, unlock_cursor,
    update_camera, victory_dance,
};
use state::{CameraOrbit, GameState, PlayerStats};
use ui::{
    despawn_tutorial_overlay, handle_restart_input, handle_tutorial_input, spawn_game_over_overlay,
    spawn_hud, spawn_stage_clear_overlay, spawn_tutorial_overlay, toggle_tutorial_hotkey,
    update_hud,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tendy Quest — Giant Kitchen".into(),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
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
        .add_systems(OnEnter(GameState::Tutorial), spawn_tutorial_overlay)
        .add_systems(OnExit(GameState::Tutorial), despawn_tutorial_overlay)
        .add_systems(
            Update,
            (handle_tutorial_input, update_camera).run_if(in_state(GameState::Tutorial)),
        )
        .add_systems(OnExit(GameState::Playing), unlock_cursor)
        .add_systems(
            Update,
            (
                camera_mouse_look,
                player_movement,
                update_camera,
                animate_and_collect_pickups,
                update_enemies,
                toggle_tutorial_hotkey,
                update_hud,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnEnter(GameState::StageClear), spawn_stage_clear_overlay)
        .add_systems(
            Update,
            (victory_dance, update_camera, handle_restart_input)
                .run_if(in_state(GameState::StageClear)),
        )
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over_overlay)
        .add_systems(
            Update,
            handle_restart_input.run_if(in_state(GameState::GameOver)),
        )
        .run();
}
