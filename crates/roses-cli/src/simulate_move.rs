//! Explicit baseline-to-core adapter. No expected endpoint is an input.
use crate::Result;
use roses_compare::pine::MemoryObservation;
use roses_compare::{Checkpoint, Provenance, SnapshotDocument, SnapshotMetadata};
use roses_core::{
    Command, CoreState, GridPos, MoveContext, StateDocument, Transition, Unit, UnitId,
};
use roses_viewer::{BoardSnapshot, ViewOrigin};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

const REQUEST_SCHEMA: &str = "roses-simulate-move-request-1";
const PROFILE: &str = "roses-private-observed-board-core-1";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const OBSERVER_VERSION: &str = "roses-cli/simulate-move-1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: String,
    scenario: String,
    profile: String,
    metadata: SourceMetadata,
    command: Command,
    context: MoveContext,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceMetadata {
    disc_sha256: String,
    executable_sha256: String,
}

#[derive(Deserialize)]
struct RawCapture {
    observations: Vec<MemoryObservation>,
}

struct Simulation {
    snapshot: SnapshotDocument,
    trace: Value,
    before: CoreState,
    transition: Transition,
    source_board: BoardSnapshot,
    request_sha256: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn request(bytes: &[u8]) -> Result<Request> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err("move request exceeds the 16 KiB limit".into());
    }
    let request: Request = serde_json::from_slice(bytes)?;
    if request.schema_version != REQUEST_SCHEMA || request.profile != PROFILE {
        return Err("unsupported move request schema or observation projection profile".into());
    }
    if request.scenario.trim().is_empty()
        || request.scenario.chars().count() > 120
        || request.scenario.chars().any(char::is_control)
    {
        return Err("scenario must contain 1..120 printable characters".into());
    }
    for fingerprint in [
        &request.metadata.disc_sha256,
        &request.metadata.executable_sha256,
    ] {
        if fingerprint.len() != 64 || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("caller-supplied source fingerprints must be 64 hexadecimal digits".into());
        }
    }
    Ok(request)
}

fn initialize_core(board: &BoardSnapshot, capture_bytes: &[u8]) -> Result<CoreState> {
    // The strict adapter already checked this exact byte buffer against all 305
    // names, addresses and widths. Retain actual side bytes separately rather
    // than inventing ownership from the array group stored in a viewer key.
    let raw: RawCapture = serde_json::from_slice(capture_bytes)?;
    let observations: BTreeMap<_, _> = raw
        .observations
        .into_iter()
        .map(|observation| (observation.address, observation.value))
        .collect();
    let mut units = Vec::with_capacity(board.document().units.len());
    for unit in &board.document().units {
        let location = unit
            .raw_location
            .ok_or("saved-capture unit lacks its source array location")?;
        let key = u16::from(location.array_group) * 11 + u16::from(location.array_slot);
        if unit.key != key {
            return Err("saved-capture unit key disagrees with its source array location".into());
        }
        let side_address = 0x2DB990
            + u32::from(location.array_group) * 0x3F4
            + u32::from(location.array_slot) * 0x5C
            + 0x44;
        let side = u8::try_from(
            *observations
                .get(&side_address)
                .ok_or("validated side observation is missing")?,
        )?;
        units.push(Unit {
            id: UnitId(key),
            card_id: unit.card_id,
            side,
            position: GridPos {
                column: unit.position.column,
                row: unit.position.row,
            },
        });
    }
    Ok(CoreState::new(StateDocument {
        schema_version: roses_core::STATE_SCHEMA.into(),
        terrain: board
            .document()
            .cells
            .iter()
            .map(|cell| cell.terrain_code)
            .collect(),
        units,
        life_points: board.document().lp_candidates_by_raw_group,
    })?)
}

fn unit_key(id: UnitId) -> String {
    format!("{}:{}", id.0 / 11, id.0 % 11)
}

/// Projection contains only the declared observable board fields. Names,
/// source pointers, arbitrary emulator bytes and caller metadata stay outside.
fn project(state: &CoreState) -> Value {
    let mut units: Vec<_> = state.document().units.iter().collect();
    units.sort_by_key(|unit| unit_key(unit.id));
    let occupied_units: Vec<_> = units
        .iter()
        .map(|unit| {
            json!({
                "unit_key": unit_key(unit.id), "card_id": unit.card_id,
                "column": unit.position.column, "row": unit.position.row,
            })
        })
        .collect();
    units.sort_by_key(|unit| (unit.position.row, unit.position.column));
    let occupied_cells: Vec<_> = units.iter().map(|unit| json!({
        "column": unit.position.column, "row": unit.position.row, "unit_key": unit_key(unit.id),
    })).collect();
    json!({
        "schema_version": PROFILE, "board_width": 7, "board_height": 7,
        "occupied_units": occupied_units, "occupied_cells": occupied_cells,
        "lp_by_array_side": state.document().life_points,
        "terrain_row_major": state.document().terrain,
    })
}

fn simulate(capture_bytes: &[u8], request_bytes: &[u8]) -> Result<Simulation> {
    let request = request(request_bytes)?;
    let source_board = roses_viewer::from_saved_capture(capture_bytes)?;
    let before = initialize_core(&source_board, capture_bytes)?;
    let transition = before.apply(&request.command, Some(&request.context))?;
    let required_fields = [
        "schema_version",
        "board_width",
        "board_height",
        "occupied_units",
        "occupied_cells",
        "lp_by_array_side",
        "terrain_row_major",
    ]
    .map(|field| format!("/{field}"))
    .to_vec();
    let snapshot = SnapshotDocument {
        schema_version: roses_compare::SCHEMA_VERSION,
        scenario: request.scenario.clone(),
        profile: request.profile.clone(),
        metadata: SnapshotMetadata {
            disc_sha256: request.metadata.disc_sha256.to_ascii_lowercase(),
            executable_sha256: request.metadata.executable_sha256.to_ascii_lowercase(),
            observer_version: OBSERVER_VERSION.into(),
            provenance: Provenance::Native,
        },
        required_fields,
        checkpoints: vec![
            Checkpoint {
                id: "before".into(),
                observed: project(&before),
            },
            Checkpoint {
                id: "after".into(),
                observed: project(&transition.state),
            },
        ],
    };
    let request_sha256 = hash(request_bytes);
    let trace = json!({
        "schema_version": "roses-native-move-trace-1", "engine_version": OBSERVER_VERSION,
        "request_sha256": request_sha256, "request": request,
        "source_capture": source_board.document().origin,
        "source_unit_mapping": source_board.document().units.iter().map(|unit| json!({
            "core_unit_id":unit.key, "projection_unit_key":unit_key(UnitId(unit.key)),
            "source_array_location":unit.raw_location,
            "source_side_byte":before.unit(UnitId(unit.key)).expect("initialized source unit").side,
        })).collect::<Vec<_>>(),
        "before_initialized_state": before,
        "computed_transition": transition,
        "source_identity_authenticated": false,
        "reference_endpoint_used": false,
        "gameplay_parity_verified": false,
        "scope": "Before is imported initialization from the declared PAL watch interpretation. After is computed by roses-core using only that state and the explicit request. Source disc/executable fingerprints are caller assertions, not authenticated by this command. This is one restricted movement policy, not a full turn, card-effect, terrain or battle simulation. Independent comparison is required to assess a reference case.",
    });
    Ok(Simulation {
        snapshot,
        trace,
        before,
        transition,
        source_board,
        request_sha256,
    })
}

fn viewer_snapshot(
    simulation: &Simulation,
    state: &CoreState,
    checkpoint: &str,
) -> Result<BoardSnapshot> {
    let ViewOrigin::SavedCapture {
        capture_json_sha256,
        state_sha256,
        ee_memory_sha256,
        address_profile,
    } = &simulation.source_board.document().origin
    else {
        return Err("native initialization requires a saved-capture source reference".into());
    };
    let cells = state
        .document()
        .terrain
        .iter()
        .enumerate()
        .map(|(index, &terrain_code)| {
            let position = GridPos {
                column: (index % 7) as u8,
                row: (index / 7) as u8,
            };
            roses_viewer::ViewCell {
                position: roses_viewer::GridPos {
                    column: position.column,
                    row: position.row,
                },
                terrain_code,
                occupancy: state
                    .unit_at(position)
                    .map_or(roses_viewer::Occupancy::Empty, |unit| {
                        roses_viewer::Occupancy::Occupied {
                            unit_key: unit.id.0,
                        }
                    }),
            }
        })
        .collect();
    let units = state
        .document()
        .units
        .iter()
        .map(|unit| roses_viewer::ViewUnit {
            key: unit.id.0,
            card_id: unit.card_id,
            position: roses_viewer::GridPos {
                column: unit.position.column,
                row: unit.position.row,
            },
            raw_location: None,
            card: None,
        })
        .collect();
    Ok(BoardSnapshot::new(roses_viewer::SnapshotDocument {
        schema_version: roses_viewer::SNAPSHOT_SCHEMA.into(),
        label: if checkpoint == "before" {
            "Native model initialized from captured board"
        } else {
            "Native model after one computed restricted move"
        }
        .into(),
        origin: ViewOrigin::NativeSimulation {
            source: roses_viewer::OriginalCaptureReference {
                capture_json_sha256: capture_json_sha256.clone(),
                state_sha256: state_sha256.clone(),
                ee_memory_sha256: ee_memory_sha256.clone(),
                address_profile: address_profile.clone(),
            },
            engine_version: OBSERVER_VERSION.into(),
            rules_profile: PROFILE.into(),
            request_sha256: simulation.request_sha256.clone(),
            checkpoint: checkpoint.into(),
        },
        cells,
        units,
        lp_candidates_by_raw_group: state.document().life_points,
        unlinked_candidates: Vec::new(),
        raw_candidate_fields: None,
    })?)
}

pub(super) fn run(capture_path: &Path, request_path: &Path, output: &Path) -> Result<()> {
    let capture_bytes = roses_viewer::read_bounded(capture_path)?;
    let mut request_bytes = Vec::new();
    File::open(request_path)?
        .take(MAX_REQUEST_BYTES as u64 + 1)
        .read_to_end(&mut request_bytes)?;
    let simulation = simulate(&capture_bytes, &request_bytes)?;
    let files = vec![
        (
            "native-snapshot.json",
            serde_json::to_vec_pretty(&simulation.snapshot)?,
        ),
        ("trace.json", serde_json::to_vec_pretty(&simulation.trace)?),
        (
            "viewer-before.json",
            serde_json::to_vec_pretty(
                viewer_snapshot(&simulation, &simulation.before, "before")?.document(),
            )?,
        ),
        (
            "viewer-after.json",
            serde_json::to_vec_pretty(
                viewer_snapshot(&simulation, &simulation.transition.state, "after")?.document(),
            )?,
        ),
    ];
    // Validate and serialize the entire operation before creating an output
    // directory. create_dir/create_new fail on existing files or directories.
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    for (name, bytes) in files {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(name))?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    eprintln!(
        "Computed one restricted move in {}. No original-game parity was inferred.",
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests;
