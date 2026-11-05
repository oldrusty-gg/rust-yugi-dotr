//! Small speaker portraits of the story panels as the face engine draws
//! them (PAL SLES_515.79; research `story/the project notes,
//! `story/the project notes §Faces).
//!
//! The speaker panels 0x232EF0 request a face (0x247310) instead of drawing
//! the TEXANM record whole: top panel at 0x2330B4 {speaker, expression =
//! 0x298DAC[4·speaker + W+546C], kind 1 (3 while talking), x = 400,
//! y = 0 + slide}; bottom panel at 0x23341C {speaker, 0x298DAC[4·speaker +
//! W+546B], kind 1/3, x = 16, y = 368 + slide} [F]. Speaker = TEXANM record
//! = name id − 196; at rest the Bosworth pages request (11, 0, 16, 368) and
//! (10, 0, 400, 0), moods 0 [emu, trace-win2.json]. A settled page shows the
//! face at frame 0 of every animated layer (eyes open, mouth at its first
//! frame): base + layer sprites from `face_anim`, scaled 0.8 × 1.125 in the
//! raw 512×512 space, then to the canvas.
//!
//! This pass rewrites every imported portrait layer (asset
//! `<speaker>-small-portrait`, the 128×128 crop at the bottom or top panel
//! slot) into those sprites, across all story stages of the manifest.
use crate::Result;
use roses_client::face_anim::FaceTables;
use roses_client::{Layer, Manifest, Stage};

/// Portrait asset → face-table speaker = texture-bank-7 record = name id
/// − 196 (Simon 197 → 1: the 595-y6 savestate's bottom face slot holds
/// speaker 1 [emu]; TEXANM records 0 and 1 are the same Simon image, the
/// opening imported record 0; Mako (211): the white route's 12040/12041
/// bottom panel [emu: face request speaker 15 at (16, 368),
/// trace-dover.json]). Shared with the runtime face animation.
pub(crate) const PORTRAITS: [(&str, u8); 7] = roses_client::face_anim::STORY_PORTRAITS;
/// Canvas destination of the old whole-crop layers → raw face origin.
const SLOTS: [([u32; 2], [i16; 2]); 2] = [([20, 322], [16, 368]), ([500, 0], [400, 0])];

/// The frame-0 sprites of `speaker`'s face for `mood` at raw origin `at`.
pub(crate) fn face_layers(
    tables: &FaceTables,
    asset: &str,
    speaker: u8,
    mood: u8,
    at: [i16; 2],
) -> Result<Vec<Layer>> {
    let expression = tables
        .expression(speaker, mood)
        .ok_or_else(|| format!("no expression table for face {speaker}"))?;
    let face = tables.face(speaker, expression, 1, at[0], at[1])?;
    let draws = face.draws();
    if draws.is_empty() {
        return Err(format!("face {speaker} draws nothing").into());
    }
    Ok(roses_client::face_anim::draw_layers(&draws, asset))
}

/// Replace the whole-crop portrait layers of a panel by face sprites;
/// `moods` = [bottom panel, top panel]. Returns how many were replaced.
pub(crate) fn rewrite(
    layers: &mut Vec<Layer>,
    tables: &FaceTables,
    moods: [u8; 2],
) -> Result<usize> {
    let mut count = 0;
    let mut out = Vec::with_capacity(layers.len());
    for layer in layers.drain(..) {
        let speaker = PORTRAITS
            .iter()
            .find(|(asset, _)| *asset == layer.asset)
            .map(|(_, s)| *s);
        let slot = SLOTS
            .iter()
            .position(|(dest, _)| layer.destination[..2] == dest[..]);
        match (speaker, slot, layer.source) {
            (Some(speaker), Some(i), Some([0, 0, 128, 128])) => {
                out.extend(face_layers(
                    tables,
                    &layer.asset,
                    speaker,
                    moods[i],
                    SLOTS[i].1,
                )?);
                count += 1;
            }
            (Some(_), _, Some([0, 0, 128, 128])) => {
                return Err(format!(
                    "portrait layer {} at {:?} is not a panel slot",
                    layer.asset, layer.destination
                )
                .into())
            }
            _ => out.push(layer),
        }
    }
    *layers = out;
    Ok(count)
}

/// Rewrite every whole-crop portrait layer of the manifest; returns how many.
pub(crate) fn apply(manifest: &mut Manifest, tables: &FaceTables) -> Result<usize> {
    let mut count = 0;
    for layers in manifest.layer_sets.values_mut() {
        count += rewrite(layers, tables, [0, 0])?;
    }
    for stage in &mut manifest.stages {
        if let Stage::Story { pages, .. } = stage {
            for page in pages {
                count += rewrite(&mut page.layers, tables, [0, 0])?;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ROSES_FACE_EXE` = the user's SLES_515.79: Simon's and Yugi's faces at
    /// the panel slots cover the traced 0.8 x 1.125 portrait rectangles.
    #[test]
    fn private_panel_faces_cover_the_portrait_slots() {
        let Some(exe) = std::env::var_os("ROSES_FACE_EXE") else {
            return;
        };
        let tables = FaceTables::from_executable(&std::fs::read(exe).unwrap()).unwrap();
        for (asset, speaker) in PORTRAITS {
            for (dest, at) in SLOTS {
                let layers = face_layers(&tables, asset, speaker, 0, at).unwrap();
                let x0 = layers.iter().map(|l| l.destination[0]).min().unwrap();
                let y0 = layers.iter().map(|l| l.destination[1]).min().unwrap();
                assert_eq!([x0, y0], dest, "{asset}");
                assert!(layers.iter().all(|l| l.asset == asset));
            }
        }
        // Simon (speaker 1): base 128x128 texels -> raw 102x144 at (16, 368).
        let simon = face_layers(&tables, "simon-small-portrait", 1, 0, [16, 368]).unwrap();
        assert_eq!(simon[0].source, Some([0, 0, 128, 128]));
        assert_eq!(simon[0].destination, [20, 322, 128, 126]);
    }
}
