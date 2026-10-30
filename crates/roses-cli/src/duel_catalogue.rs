//! Duel card data from the user's executable: the card table, effect
//! entries, power-up tables and the fusion table, as `roses-core` needs them.
//! Built locally from the supplied disc; nothing here is distributed.

use roses_core::duel::{CardInfo, Catalogue, PowerUp};

pub fn build(executable: &[u8]) -> crate::Result<Catalogue> {
    let catalog = roses_assets::parse_pal_catalog(executable)?;
    let effects = roses_assets::parse_pal_card_effects(executable)?;
    let equips = roses_assets::parse_pal_equips(executable)?;
    let rules = roses_assets::parse_pal_rules(executable)?;
    let cards = catalog
        .cards
        .iter()
        .zip(&effects.cards)
        .map(|(c, e)| {
            let equip_bits = equips
                .compatibility
                .get(usize::from(c.id))
                .copied()
                .unwrap_or_default();
            CardInfo {
                id: c.id,
                kind: c.kind,
                level: c.level,
                attribute: c.attribute,
                attack: c.attack,
                defense: c.defense,
                effects: e.entries.map(|x| x.is_some()),
                // Terrain favour flag: bit 1 of 0x29C976 + id·7, i.e. byte 6
                // of the same per-monster row as the power-up bits.
                toon: equip_bits[6] & 2 != 0,
                kind_index: c.kind_offset_raw,
                entries: e.entries.map(|x| x.map(|x| [x.word0, x.word1])),
                equip_bits,
                power_up: equips
                    .power_ups
                    .iter()
                    .find(|p| p.id == c.id)
                    .map(|p| PowerUp {
                        category: p.category,
                        value: p.value,
                    }),
            }
        })
        .collect();
    let fusions = rules
        .fusions
        .iter()
        .map(|f| [f.ingredient_low, f.ingredient_high, f.result_id]);
    let rituals = roses_assets::parse_pal_rituals(executable)?
        .rituals
        .into_iter()
        .map(|r| (r.result, r.words));
    Ok(Catalogue::new(cards, fusions).with_rituals(rituals))
}

/// Fusion/equip tables for Weevil's planner, sliced from the executable.
pub fn fusion_data(
    executable: &[u8],
) -> crate::Result<Box<dyn roses_core::duel::ai::weevil::FusionData>> {
    let rules = roses_assets::parse_pal_rules(executable)?;
    let fusions: Vec<[u16; 3]> = rules
        .fusions
        .iter()
        .map(|f| [f.ingredient_low, f.ingredient_high, f.result_id])
        .collect();
    let tables = roses_core::duel::ai::weevil::Tables::new(&fusions, executable)
        .ok_or("the executable is too short for the planner tables")?;
    Ok(Box::new(tables))
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH)"]
    fn private_ritual_requirements_run_from_the_executable() {
        // Rows of claude-effects/RITUAL.md, whose table came from running
        // 0x2565B0 in the full interpreter.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let table = roses_assets::parse_pal_rituals(&elf).unwrap();
        assert_eq!(table.rituals.len(), 24);
        assert_eq!(
            (
                table.rituals.first().unwrap().ritual,
                table.rituals.last().unwrap().ritual
            ),
            (830, 853)
        );
        let row = |ritual: u16| {
            let r = table.rituals.iter().find(|r| r.ritual == ritual).unwrap();
            (r.result, r.words)
        };
        assert_eq!(row(848), (422, [0x018E_001C, 9, 9]));
        assert_eq!(row(830), (189, [0x07D0_0039; 3]));
        assert_eq!(row(832), (145, [0x0063_001C, 0x023D_001C, 0x0216_001C]));
        assert_eq!(row(833), (143, [0x009B_001C, 0x05DC_0025, 0x05DC_0025]));
        assert_eq!(row(851), (128, [2, 0xE, 9]));
    }

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_mini_picture_decodes_like_the_texture_map() {
        let iso = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").unwrap());
        let (mut disc, _) = crate::open_pal(&iso).unwrap();
        let archive = disc.read_file("DATA/PICMINI.MRG", 4 * 1024 * 1024).unwrap();
        for card in [392u16, 397] {
            let image = roses_assets::decode_pal_mini_picture_rgba(&archive, card).unwrap();
            let out = std::env::temp_dir().join(format!("roses-mini-{card}.rgba"));
            std::fs::write(&out, &image.pixels).unwrap();
            assert_eq!((image.width, image.height), (40, 32));
        }
    }
}
