//! Narrow adapter for the complete researched PAL watch set. Metadata identifies
//! supplied bytes; it does not authenticate a savestate's game or field meaning.
use crate::*;
use roses_compare::pine::{MemoryObservation, MemoryWatch, ReadWidth};
use sha2::{Digest, Sha256};

const UNIT_BASE: u32 = 0x2DB990;
const UNIT_SIDE_STRIDE: u32 = 0x3F4;
const UNIT_STRIDE: u32 = 0x5C;
const BOARD_BASE: u32 = 0x2DC570;
const CELL_STRIDE: u32 = 28;
const CARD_BASE: u32 = 0x2BE400;
const WATCH_COUNT: usize = 305;

#[derive(Deserialize)]
struct CaptureInput {
    schema_version: String,
    state_sha256: String,
    ee_memory_sha256: String,
    state_bytes: u64,
    ee_memory_bytes: usize,
    compression_method: String,
    observations: Vec<MemoryObservation>,
    atomic_saved_memory: bool,
    normalized_game_state: bool,
}

/// Exact names/addresses/widths of the retained 305-watch interpretation. This
/// function defines no PINE connection and does not operate an emulator.
pub fn pal_watches() -> Vec<MemoryWatch> {
    let mut watches = Vec::with_capacity(WATCH_COUNT);
    let mut add = |name: String, address, width| {
        watches.push(MemoryWatch {
            name,
            address,
            width,
        })
    };
    for side in 0..2 {
        add(
            format!("candidate_side{side}_lp"),
            0x2DAEDA + side * 0x350,
            ReadWidth::Bits16,
        );
        add(
            format!("unknown_side{side}_resource_byte"),
            0x2DAEE2 + side * 0x350,
            ReadWidth::Bits8,
        );
    }
    for (name, address) in [
        ("candidate_selected_map_index", 0x2DB8D7),
        ("unknown_duel_byte_b8f2", 0x2DB8F2),
        ("unknown_duel_byte_b8f3", 0x2DB8F3),
        ("candidate_active_side", 0x2DB8F4),
        ("unknown_duel_process_byte_b8f7", 0x2DB8F7),
    ] {
        add(name.into(), address, ReadWidth::Bits8);
    }
    for group in 0..3 {
        for slot in 0..11 {
            let base = UNIT_BASE + group * UNIT_SIDE_STRIDE + slot * UNIT_STRIDE;
            for (suffix, offset, width) in [
                ("const_info_ptr", 0, ReadWidth::Bits32),
                ("card_id", 0x3C, ReadWidth::Bits16),
                ("col", 0x40, ReadWidth::Bits8),
                ("row", 0x41, ReadWidth::Bits8),
                ("side", 0x44, ReadWidth::Bits8),
                ("slot", 0x45, ReadWidth::Bits8),
            ] {
                add(
                    format!("candidate_unit_{group}_{slot}_{suffix}"),
                    base + offset,
                    width,
                );
            }
        }
    }
    for row in 0..7 {
        for column in 0..7 {
            let base = BOARD_BASE + (row * 7 + column) * CELL_STRIDE;
            add(
                format!("candidate_cell_{row}_{column}_unit_ptr"),
                base,
                ReadWidth::Bits32,
            );
            add(
                format!("candidate_cell_{row}_{column}_terrain"),
                base + 0x16,
                ReadWidth::Bits8,
            );
        }
    }
    watches
}

pub(crate) fn validate_card_pointer(card_id: u16, pointer: u32) -> Result<()> {
    match card_id {
        999 if pointer == 0 => Ok(()),
        id if usize::from(id) < roses_assets::CARD_COUNT
            && pointer == CARD_BASE + u32::from(id) * 20 =>
        {
            Ok(())
        }
        _ => Err(invalid(format!(
            "Card ID {card_id} and catalog pointer 0x{pointer:08X} disagree or are unsupported."
        ))),
    }
}

pub fn from_saved_capture(bytes: &[u8]) -> Result<BoardSnapshot> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid("Saved-capture JSON exceeds the 1 MiB limit."));
    }
    let input: CaptureInput = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("Invalid saved-capture JSON: {error}")))?;
    if input.schema_version != "roses-saved-ee-memory-1"
        || !input.atomic_saved_memory
        || input.normalized_game_state
    {
        return Err(invalid(
            "Expected an unnormalized roses-saved-ee-memory-1 capture of one archived RAM buffer.",
        ));
    }
    if input.ee_memory_bytes != roses_compare::state::EE_MEMORY_BYTES
        || input.state_bytes == 0
        || input.state_bytes > roses_compare::state::MAX_STATE_BYTES
        || !matches!(
            input.compression_method.as_str(),
            "stored" | "deflate" | "zstandard"
        )
    {
        return Err(invalid(
            "Saved-capture size/compression metadata is unsupported.",
        ));
    }
    check_hash(&input.state_sha256)?;
    check_hash(&input.ee_memory_sha256)?;
    if input.observations.len() != WATCH_COUNT {
        return Err(invalid(format!("Expected all {WATCH_COUNT} exact PAL watches; received {}. Missing data cannot become an empty board.", input.observations.len())));
    }
    let expected: BTreeMap<_, _> = pal_watches()
        .into_iter()
        .map(|watch| (watch.name.clone(), watch))
        .collect();
    let mut by_address = BTreeMap::new();
    let mut names = BTreeSet::new();
    for observation in input.observations {
        let watch = expected
            .get(&observation.name)
            .ok_or_else(|| invalid(format!("Unexpected observation name: {}", observation.name)))?;
        if watch.address != observation.address || watch.width != observation.width {
            return Err(invalid(format!(
                "Observation {} has the wrong address or width.",
                observation.name
            )));
        }
        let width = u8::from(observation.width);
        if observation.value >= (1u64 << width) {
            return Err(invalid(format!(
                "Observation {} exceeds its {width}-bit width.",
                observation.name
            )));
        }
        if !names.insert(observation.name)
            || by_address
                .insert(observation.address, observation.value)
                .is_some()
        {
            return Err(invalid(
                "Duplicate observation name/address; another required watch is missing.",
            ));
        }
    }
    // Completeness and widths were checked above; map lookups cannot default.
    let read = |address: u32| -> u32 { by_address[&address] as u32 };
    let mut candidates = Vec::with_capacity(33);
    let mut unit_addresses = BTreeMap::new();
    for group in 0..3u8 {
        for slot in 0..11u8 {
            let base =
                UNIT_BASE + u32::from(group) * UNIT_SIDE_STRIDE + u32::from(slot) * UNIT_STRIDE;
            let candidate = UnlinkedCandidate {
                raw_location: RawUnitLocation {
                    array_group: group,
                    array_slot: slot,
                },
                card_id: read(base + 0x3C) as u16,
                constant_info_pointer: read(base),
                column_raw: read(base + 0x40) as u8,
                row_raw: read(base + 0x41) as u8,
                side_field_raw: read(base + 0x44) as u8,
                slot_field_raw: read(base + 0x45) as u8,
            };
            validate_card_pointer(candidate.card_id, candidate.constant_info_pointer)?;
            if candidate.column_raw > 7
                || candidate.row_raw > 7
                || candidate.side_field_raw > 2
                || candidate.slot_field_raw > 11
            {
                return Err(invalid(format!(
                    "Raw unit group {group} slot {slot} has unsupported coordinate/identity bytes."
                )));
            }
            unit_addresses.insert(base, candidates.len());
            candidates.push(candidate);
        }
    }
    let mut cells = Vec::with_capacity(CELL_COUNT);
    let mut units = Vec::new();
    let mut linked = BTreeSet::new();
    for row in 0..7u8 {
        for column in 0..7u8 {
            let position = GridPos { column, row };
            let base = BOARD_BASE + position.index()? as u32 * CELL_STRIDE;
            let pointer = read(base);
            let occupancy = if pointer == 0 {
                Occupancy::Empty
            } else {
                let index = *unit_addresses.get(&pointer).ok_or_else(|| invalid(format!("Cell ({column},{row}) points outside the known unit records: 0x{pointer:08X}.")))?;
                let candidate = &candidates[index];
                if candidate.card_id == 999
                    || (candidate.column_raw, candidate.row_raw) != (column, row)
                    || !linked.insert(index)
                {
                    return Err(invalid(format!("Cell ({column},{row}) has a duplicate, invalid, or nonreciprocal unit reference.")));
                }
                let key = index as u16;
                units.push(ViewUnit {
                    key,
                    card_id: candidate.card_id,
                    position,
                    raw_location: Some(candidate.raw_location),
                    card: None,
                });
                Occupancy::Occupied { unit_key: key }
            };
            cells.push(ViewCell {
                position,
                terrain_code: read(base + 0x16) as u8,
                occupancy,
            });
        }
    }
    let unlinked_candidates = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let sentinel = (
                candidate.card_id,
                candidate.constant_info_pointer,
                candidate.column_raw,
                candidate.row_raw,
                candidate.side_field_raw,
                candidate.slot_field_raw,
            ) == (999, 0, 7, 7, 2, 11);
            (!linked.contains(&index) && !sentinel).then_some(candidate)
        })
        .collect();
    BoardSnapshot::new(SnapshotDocument {
        schema_version: SNAPSHOT_SCHEMA.into(),
        label: "Captured PAL candidate board".into(),
        origin: ViewOrigin::SavedCapture {
            capture_json_sha256: format!("{:x}", Sha256::digest(bytes)),
            state_sha256: input.state_sha256,
            ee_memory_sha256: input.ee_memory_sha256,
            address_profile: roses_disc::PAL_PROFILE_ID.into(),
        },
        cells,
        units,
        lp_candidates_by_raw_group: [read(0x2DAEDA) as u16, read(0x2DB22A) as u16],
        unlinked_candidates,
        raw_candidate_fields: Some(RawCandidateFields {
            selected_map_index: read(0x2DB8D7) as u8,
            active_side_byte: read(0x2DB8F4) as u8,
            unknown_resource_bytes: [read(0x2DAEE2) as u8, read(0x2DB232) as u8],
            unknown_b8f2: read(0x2DB8F2) as u8,
            unknown_b8f3: read(0x2DB8F3) as u8,
            unknown_process_b8f7: read(0x2DB8F7) as u8,
        }),
    })
}
