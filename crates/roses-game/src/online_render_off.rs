//! `online_render` in a build without the `online` feature (the original
//! game only, docs/ADDITIONS.md): there is no Online Play, so the game's
//! scene switch never enters it and the deck editor and duel are never
//! Online Play's.

use bevy::prelude::{App, World};

pub fn building(_: &World) -> bool {
    false
}

pub fn builder_inputs(_: &mut World) -> Option<Result<super::deck_editor_render::EditorInputs, String>> {
    None
}

pub fn builder_closed(_: &mut World, _: super::deck_editor_render::EditorResult) {}

pub fn active(_: &World) -> bool {
    false
}

pub fn enter_scene(_: &mut World) -> Result<(), String> {
    Err("Online Play is not in this build (the original game only)".into())
}

pub fn add_systems(_: &mut App) {}

pub fn duel_stopped(_: &mut World, _: &str) {}

pub fn watch(_: &mut World) {}

pub fn duel_scene(_: &mut World) -> Option<Result<(), String>> {
    None
}
