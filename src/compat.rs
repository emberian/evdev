//! Compatibility layer for non-Linux builds.
//!
//!

// input_absinfo, input_id, input_keymap_entry, uinput_abs_setup, uinput_setup input_event

// ff_envelope ff_condition_effect ff_trigger ff_replay

// EV_CNT INPUT_PROP_CNT REL_CNT ABS_CNT SW_CNT LED_CNT MSC_CNT FF_CNT SND_CNT

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(any(
        target_os = "linux",
        target_os = "l4re",
        target_os = "android",
        target_os = "emscripten"
    ))] {
        pub(crate) use libc::{
            ff_condition_effect, ff_constant_effect, ff_envelope, ff_periodic_effect, ff_ramp_effect,
            ff_replay, ff_rumble_effect, ff_trigger, input_absinfo, input_event, input_id,
            input_keymap_entry, uinput_abs_setup, uinput_setup, ABS_CNT, EV_CNT, FF_CNT, INPUT_PROP_CNT,
            KEY_CNT, LED_CNT, MSC_CNT, REL_CNT, SND_CNT, SW_CNT, UINPUT_MAX_NAME_SIZE,
        };
    } else {
        mod non_linux;
        pub(crate) use non_linux::{
            ff_condition_effect, ff_constant_effect, ff_envelope, ff_periodic_effect, ff_ramp_effect,
            ff_replay, ff_rumble_effect, ff_trigger, input_absinfo, input_event, input_id,
            input_keymap_entry, uinput_abs_setup, uinput_setup, ABS_CNT, EV_CNT, FF_CNT, INPUT_PROP_CNT,
            KEY_CNT, LED_CNT, MSC_CNT, REL_CNT, SND_CNT, SW_CNT, UINPUT_MAX_NAME_SIZE,
        };
    }
}
