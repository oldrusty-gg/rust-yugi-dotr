//! Off-screen cameras with nothing to draw sit out the frame (issue #20).
//!
//! The duel board has 14 cameras, most of them rendering into 512x512
//! canvases (the fade, effect, cover and unit canvases) that are empty most
//! of the time. Bevy still ran its whole 3D pass for each of them every
//! frame (view setup, light clustering, mesh preprocessing, clear, passes,
//! the copy into the image) only to clear the image to the same colour.
//! Once a canvas has been cleared with nothing drawn, its cameras sit out
//! the frames in which they still have nothing to draw under the same
//! clear: the image already holds exactly that. They render again from the
//! first frame with something to draw, so every image is the same, frame
//! for frame (checked against 06c on a recorded 140 s run: map, story
//! pages, the duel board and a battle, every frame identical). Not touched:
//! cameras drawing into the window, cameras the game itself switches off,
//! and images that a camera draws into without clearing (Bevy keeps such a
//! camera's drawing in its view texture between frames). The game always
//! sees its cameras as it set them: they are switched back on at the start
//! of every frame.
//!
//! Bevy's light clustering stays on although the game has no lights: the
//! mode-change fade is Bevy's own `StandardMaterial`, which needs it.
//!
//! `ROSES_RENDER_DEFAULTS=1` renders every camera every frame (to compare).

use bevy::{
    app::{App, First, Last},
    asset::AssetId,
    color::ColorToComponents,
    camera::{Camera, ClearColor, ClearColorConfig, RenderTarget, visibility::VisibleEntities},
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        schedule::{IntoScheduleConfigs, SystemSet},
        system::{Commands, Query, Res},
    },
    image::Image,
};
use std::collections::BTreeMap;

pub fn defaults_kept() -> bool {
    std::env::var("ROSES_RENDER_DEFAULTS").is_ok_and(|v| v == "1")
}

pub fn plugin(app: &mut App) {
    if defaults_kept() {
        eprintln!("Render: ROSES_RENDER_DEFAULTS=1, idle off-screen cameras render every frame");
        return;
    }
    app.add_systems(First, resume_cameras)
        .add_systems(Last, rest_idle_cameras.in_set(CamerasRested));
}

/// [`rest_idle_cameras`]: after it, `Camera::is_active` is what renders.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CamerasRested;

/// What an idle camera's image holds: its target and clear colour.
#[derive(Clone, Copy, PartialEq)]
struct ClearKey(AssetId<Image>, u8, [f32; 4]);

/// Rendered frames in a row with nothing drawn, under `key`.
#[derive(Component)]
struct Idle {
    empty: u32,
    key: ClearKey,
}

/// Switched off by [`rest_idle_cameras`] for this frame's render.
#[derive(Component)]
struct Resting;

fn resume_cameras(mut commands: Commands, mut cameras: Query<(Entity, &mut Camera), With<Resting>>) {
    for (entity, mut camera) in &mut cameras {
        camera.is_active = true;
        commands.entity(entity).remove::<Resting>();
    }
}

fn rest_idle_cameras(
    mut commands: Commands,
    clear: Option<Res<ClearColor>>,
    mut cameras: Query<(Entity, &mut Camera, Option<&RenderTarget>, &VisibleEntities, Option<&mut Idle>)>,
) {
    let empty = |v: &VisibleEntities| v.entities.values().all(Vec::is_empty);
    // Visibility not computed (no camera sees anything): leave all alone.
    if cameras.iter().all(|(_, c, _, v, _)| !c.is_active || empty(v)) {
        return;
    }
    let default = clear.map_or([0.0; 4], |c| c.0.to_linear().to_f32_array());
    // Per target image: whether every active camera drawing into it can rest.
    let mut targets: BTreeMap<AssetId<Image>, bool> = BTreeMap::new();
    let mut resting = Vec::new();
    for (entity, camera, target, visible, idle) in &mut cameras {
        if !camera.is_active {
            continue;
        }
        let Some(image) = target.and_then(RenderTarget::as_image).map(|h| h.id()) else {
            continue;
        };
        let key = match camera.clear_color {
            ClearColorConfig::Default => ClearKey(image, 0, default),
            ClearColorConfig::Custom(c) => ClearKey(image, 1, c.to_linear().to_f32_array()),
            ClearColorConfig::None => ClearKey(image, 2, [0.0; 4]),
        };
        let nothing = empty(visible);
        // A camera that draws over its image without clearing keeps what it
        // drew in Bevy's view texture across frames, which a rest would
        // let go: its image never rests.
        let clears = !matches!(camera.clear_color, ClearColorConfig::None);
        let can_rest = clears && nothing && idle.as_ref().is_some_and(|i| i.empty > 0 && i.key == key);
        *targets.entry(image).or_insert(true) &= can_rest;
        if can_rest {
            resting.push((entity, image));
            continue;
        }
        // This frame renders.
        let empty = match &idle {
            Some(i) if nothing && i.key == key => i.empty + 1,
            _ => u32::from(nothing),
        };
        match idle {
            Some(mut i) => *i = Idle { empty, key },
            None => {
                commands.entity(entity).insert(Idle { empty, key });
            }
        }
    }
    for (entity, image) in resting {
        if targets.get(&image) == Some(&true) {
            if let Ok((_, mut camera, ..)) = cameras.get_mut(entity) {
                camera.is_active = false;
                commands.entity(entity).insert(Resting);
            }
        }
    }
}
