#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playBgm)]
        fn js_play_bgm();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playCutsceneMusic)]
        fn js_play_cutscene_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playBossMusic)]
        fn js_play_boss_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = speakVoice)]
        fn js_speak_voice(text: &str, role: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = isVoiceSpeaking)]
        fn js_is_voice_speaking() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playSfx)]
        fn js_play_sfx(name: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = toggleMute)]
        fn js_toggle_mute() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = isMuted)]
        fn js_is_muted() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = moveX)]
        fn js_move_x() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = moveY)]
        fn js_move_y() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeLookDX)]
        fn js_take_look_dx() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeLookDY)]
        fn js_take_look_dy() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeZoomDelta)]
        fn js_take_zoom_delta() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeJump)]
        fn js_take_jump() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeDash)]
        fn js_take_dash() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeAttack)]
        fn js_take_attack() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeActionTap)]
        fn js_take_action_tap() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeHelp)]
        fn js_take_help() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setPlaying)]
        fn js_set_playing(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setBossMode)]
        fn js_set_boss_mode(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = initMode)]
        fn js_init_mode() -> u32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeCutsceneStartTime)]
        fn js_take_cutscene_start_time() -> f32;
    }

    pub fn play_bgm() {
        js_play_bgm();
    }

    pub fn play_cutscene_music() {
        js_play_cutscene_music();
    }

    pub fn play_boss_music() {
        js_play_boss_music();
    }

    pub fn speak_voice(text: &str, role: &str) {
        js_speak_voice(text, role);
    }

    pub fn is_voice_speaking() -> bool {
        js_is_voice_speaking()
    }

    pub fn play_sfx(name: &str) {
        js_play_sfx(name);
    }

    pub fn toggle_mute() -> bool {
        js_toggle_mute()
    }

    pub fn is_muted() -> bool {
        js_is_muted()
    }

    pub fn touch_move_x() -> f32 {
        js_move_x()
    }

    pub fn touch_move_y() -> f32 {
        js_move_y()
    }

    pub fn take_touch_look_dx() -> f32 {
        js_take_look_dx()
    }

    pub fn take_touch_look_dy() -> f32 {
        js_take_look_dy()
    }

    pub fn take_touch_zoom_delta() -> f32 {
        js_take_zoom_delta()
    }

    pub fn take_touch_jump() -> bool {
        js_take_jump()
    }

    pub fn take_touch_dash() -> bool {
        js_take_dash()
    }

    pub fn take_touch_attack() -> bool {
        js_take_attack()
    }

    pub fn take_touch_action_tap() -> bool {
        js_take_action_tap()
    }

    pub fn take_touch_help() -> bool {
        js_take_help()
    }

    pub fn set_touch_playing(active: bool) {
        js_set_playing(active);
    }

    pub fn set_touch_boss_mode(active: bool) {
        js_set_boss_mode(active);
    }

    pub fn init_test_mode() -> u32 {
        js_init_mode()
    }

    pub fn take_cutscene_start_time() -> f32 {
        js_take_cutscene_start_time()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub fn play_bgm() {}

    pub fn play_cutscene_music() {}

    pub fn play_boss_music() {}

    pub fn speak_voice(_text: &str, _role: &str) {}

    pub fn is_voice_speaking() -> bool {
        false
    }

    pub fn play_sfx(_name: &str) {}

    pub fn toggle_mute() -> bool {
        false
    }

    pub fn is_muted() -> bool {
        false
    }

    pub fn touch_move_x() -> f32 {
        0.0
    }

    pub fn touch_move_y() -> f32 {
        0.0
    }

    pub fn take_touch_look_dx() -> f32 {
        0.0
    }

    pub fn take_touch_look_dy() -> f32 {
        0.0
    }

    pub fn take_touch_zoom_delta() -> f32 {
        0.0
    }

    pub fn take_touch_jump() -> bool {
        false
    }

    pub fn take_touch_dash() -> bool {
        false
    }

    pub fn take_touch_attack() -> bool {
        false
    }

    pub fn take_touch_action_tap() -> bool {
        false
    }

    pub fn take_touch_help() -> bool {
        false
    }

    pub fn set_touch_playing(_active: bool) {}

    pub fn set_touch_boss_mode(_active: bool) {}

    pub fn init_test_mode() -> u32 {
        0
    }

    pub fn take_cutscene_start_time() -> f32 {
        -1.0
    }
}

pub use imp::{
    init_test_mode, is_muted, is_voice_speaking, play_bgm, play_boss_music, play_cutscene_music,
    play_sfx, set_touch_boss_mode, set_touch_playing, speak_voice, take_cutscene_start_time,
    take_touch_action_tap, take_touch_attack, take_touch_dash, take_touch_help, take_touch_jump,
    take_touch_look_dx, take_touch_look_dy, take_touch_zoom_delta, toggle_mute, touch_move_x,
    touch_move_y,
};
