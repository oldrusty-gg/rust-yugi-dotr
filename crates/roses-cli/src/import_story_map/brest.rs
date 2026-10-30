//! Settled event 11020: source painting and letterbox, not a screenshot.
//!
//! 0x22964C loads TEXEVE 0/1 into slots 2/3 (and repeats them in 4/5).
//! 0x22EA44 sets UV 640x448 and each destination quad to raw 512x512.
//! At settled slide (-512, 0), only sprite 149 (slot 3, record 1) is visible.
//! 0x233750 draws black raw y0..144 and y368..512 over the painting before
//! the speaker panels. The import represents this settled state only.

use crate::import_story::{import_png, raw_rect, story_layer};
use crate::Result;
use roses_client::{Layer, Manifest};
use std::path::Path;

const EVENT: u16 = 11020;
const ASSET: &str = "brest-castle";

pub(super) fn import(
    disc: &mut roses_disc::Disc,
    output: &Path,
    manifest: &mut Manifest,
) -> Result<()> {
    let archive = disc.read_file(
        "DATA/TEXEVE.MRG",
        roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;
    let painting = roses_assets::decode_pal_opening_texture(
        &archive,
        roses_assets::OpeningArchiveKind::TexEve,
        1,
    )?;
    if (painting.width, painting.height) != (640, 448) {
        return Err("Brest painting does not cover its traced 640x448 UV extent".into());
    }
    import_png(output, manifest, ASSET, &painting)
}

/// The early Seto pages also use a figure-less bottom-panel set. Keep their
/// shared set separate so Brest's painting never replaces Stonehenge there.
pub(super) fn set_name(event: u16, panel_set: String) -> String {
    if event == EVENT {
        format!("brest-{panel_set}")
    } else {
        panel_set
    }
}

pub(super) fn layers(event: u16, panels: Vec<Layer>) -> Vec<Layer> {
    if event != EVENT {
        return panels;
    }
    let mut layers = vec![
        story_layer(ASSET, Some([0, 0, 640, 448]), raw_rect([0, 0, 512, 512])),
        story_layer("story-black", None, raw_rect([0, 0, 512, 144])),
        story_layer("story-black", None, raw_rect([0, 368, 512, 144])),
    ];
    layers.extend(panels);
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brest_is_isolated_from_the_figureless_seto_pages() {
        let set = "dialogue-figure-none-top-none".to_string();
        assert_ne!(set_name(11020, set.clone()), set_name(10080, set.clone()));
        for event in [10060, 10070, 10071, 10080, 10090, 11000] {
            assert_eq!(set_name(event, set.clone()), set);
            let panel = story_layer("synthetic-panel", None, [1, 2, 3, 4]);
            let unchanged = layers(event, vec![panel]);
            assert_eq!(unchanged.len(), 1);
            assert_eq!(unchanged[0].asset, "synthetic-panel");
        }
    }

    #[test]
    fn painting_and_masks_precede_the_panels() {
        let panel = story_layer("synthetic-panel", None, [1, 2, 3, 4]);
        let result = layers(11020, vec![panel]);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].source, Some([0, 0, 640, 448]));
        assert_eq!(result[0].destination, [0, 0, 640, 448]);
        assert_eq!(result[1].destination, [0, 0, 640, 126]);
        assert_eq!(result[2].destination, [0, 322, 640, 126]);
        assert_eq!(result[3].asset, "synthetic-panel");
        assert!(result[..3].iter().all(|l| l.opacity == 255));
    }

    #[test]
    fn private_geometry_matches_original_draw_descriptors() {
        let Some(path) = std::env::var_os("ROSES_BREST_ORACLE") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let read = |name: &str| -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(path.join(name)).unwrap()).unwrap()
        };
        let quads = read("quad-probe.json");
        let masks = read("letterbox-probe.json");
        let draws = quads["draws"].as_array().unwrap();
        let visible = draws
            .iter()
            .find(|d| d["shorts"][18].as_i64() == Some(149))
            .expect("original settled painting sprite");
        let num = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        let ox = num(&visible["args"][2]);
        let oy = num(&visible["args"][3]);
        let s = &visible["shorts"];
        let rectangle = [
            (num(&s[0]) + ox).try_into().unwrap(),
            (num(&s[1]) + oy).try_into().unwrap(),
            (num(&s[2]) - num(&s[0])).try_into().unwrap(),
            (num(&s[3]) - num(&s[1])).try_into().unwrap(),
        ];
        let layers = layers(EVENT, vec![]);
        assert_eq!(layers[0].destination, raw_rect(rectangle));
        let sprites = quads["sprite_descriptors"].as_array().unwrap();
        assert_eq!(sprites.len(), 80);
        let uv = |offset: usize| {
            u32::from(u16::from_le_bytes([
                sprites[20 + offset].as_u64().unwrap().try_into().unwrap(),
                sprites[21 + offset].as_u64().unwrap().try_into().unwrap(),
            ]))
        };
        assert_eq!(
            layers[0].source,
            Some([uv(0), uv(2), uv(4) - uv(0), uv(6) - uv(2)])
        );
        let masks = masks.as_array().unwrap();
        assert_eq!(masks.len(), 2);
        for (layer, mask) in layers[1..].iter().zip(masks) {
            let s = &mask["shorts"];
            let rectangle = [
                num(&s[0]).try_into().unwrap(),
                num(&s[1]).try_into().unwrap(),
                (num(&s[2]) - num(&s[0])).try_into().unwrap(),
                (num(&s[5]) - num(&s[1])).try_into().unwrap(),
            ];
            assert_eq!(layer.destination, raw_rect(rectangle));
        }
    }
}
