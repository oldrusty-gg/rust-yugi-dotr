use super::*;
use serde_json::{json, Value};

fn pos(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn document() -> StateDocument {
    StateDocument {
        schema_version: STATE_SCHEMA.into(),
        terrain: vec![NORMAL_TERRAIN; CELL_COUNT],
        units: vec![
            Unit {
                id: UnitId(40),
                card_id: 12,
                side: 1,
                position: pos(5, 5),
            },
            Unit {
                id: UnitId(9),
                card_id: 12,
                side: 0,
                position: pos(2, 2),
            },
            Unit {
                id: UnitId(27),
                card_id: 81,
                side: 2,
                position: pos(0, 6),
            },
        ],
        life_points: [3700, 4200],
    }
}

fn context(unit: u16, direction: CardinalDirection) -> MoveContext {
    MoveContext::Restricted {
        eligible_unit: UnitId(unit),
        direction,
        permission: MovePermission::Available,
    }
}

fn command(unit: u16, column: u8, row: u8) -> Command {
    Command::MoveUnit {
        unit: UnitId(unit),
        destination: pos(column, row),
    }
}

#[test]
fn movement_preserves_every_non_position_value_and_distinct_instances() {
    let mut source = document();
    // Unrelated terrain is opaque to this isolated transition and must survive.
    source.terrain[48] = 3;
    let state = CoreState::new(source).unwrap();
    let context = context(9, CardinalDirection::DecreasingRow);
    let result = state.apply(&command(9, 2, 1), Some(&context)).unwrap();
    let mut expected = state.document().clone();
    expected
        .units
        .iter_mut()
        .find(|unit| unit.id == UnitId(9))
        .unwrap()
        .position = pos(2, 1);
    assert_eq!(result.state.document(), &expected);
    assert_eq!(result.state.unit_at(pos(2, 2)), None);
    assert_eq!(result.state.unit_at(pos(2, 1)).unwrap().id, UnitId(9));
    assert_eq!(result.state.unit(UnitId(40)).unwrap().position, pos(5, 5));
    assert_eq!(
        result.events,
        [Event::UnitMoved {
            unit: UnitId(9),
            from: pos(2, 2),
            to: pos(2, 1)
        }]
    );
    assert_eq!(state.unit(UnitId(9)).unwrap().position, pos(2, 2));
    assert!(matches!(
        context,
        MoveContext::Restricted {
            permission: MovePermission::Available,
            ..
        }
    ));
}

#[test]
fn missing_unknown_and_wrong_mover_contexts_cannot_authorize_a_move() {
    let state = CoreState::new(document()).unwrap();
    let command = command(9, 2, 1);
    assert_eq!(
        state.apply(&command, None),
        Err(ActionError::UnsupportedContext)
    );
    assert_eq!(
        state.apply(&command, Some(&MoveContext::Unknown)),
        Err(ActionError::UnsupportedContext)
    );
    assert_eq!(
        state.apply(
            &command,
            Some(&context(40, CardinalDirection::DecreasingRow))
        ),
        Err(ActionError::UnitNotEligible { unit: UnitId(9) })
    );
    assert_eq!(
        state.apply(
            &super::Command::MoveUnit {
                unit: UnitId(100),
                destination: pos(2, 1)
            },
            Some(&context(100, CardinalDirection::DecreasingRow))
        ),
        Err(ActionError::UnknownUnit { unit: UnitId(100) })
    );
}

#[test]
fn returned_context_consumes_the_model_permission_without_mutating_the_input() {
    let state = CoreState::new(document()).unwrap();
    let original_context = context(9, CardinalDirection::DecreasingRow);
    let first = state
        .apply(&command(9, 2, 1), Some(&original_context))
        .unwrap();
    let before = first.state.clone();
    assert_eq!(
        first.state.apply(&command(9, 2, 0), Some(&first.context)),
        Err(ActionError::PermissionConsumed)
    );
    assert_eq!(first.state, before);
    // Deterministic replay starts again with the original state and permission.
    assert_eq!(
        state
            .apply(&command(9, 2, 1), Some(&original_context))
            .unwrap(),
        first
    );
}

#[test]
fn rejected_commands_preserve_state_and_context() {
    let mut source = document();
    source
        .units
        .iter_mut()
        .find(|unit| unit.id == UnitId(40))
        .unwrap()
        .position = pos(2, 1);
    let state = CoreState::new(source).unwrap();
    let context = context(9, CardinalDirection::DecreasingRow);
    let state_bytes = serde_json::to_vec(&state).unwrap();
    let context_bytes = serde_json::to_vec(&context).unwrap();
    for (command, error) in [
        (
            command(9, 2, 1),
            ActionError::DestinationOccupied {
                occupant: UnitId(40),
            },
        ),
        (command(9, 2, 2), ActionError::NotOneCardinalStep),
        (command(9, 3, 1), ActionError::NotOneCardinalStep),
        (command(9, 2, 0), ActionError::NotOneCardinalStep),
        (command(9, 3, 2), ActionError::DirectionNotPermitted),
        (
            command(9, 255, 2),
            ActionError::OutOfBounds {
                destination: pos(255, 2),
            },
        ),
        (
            command(9, 2, 7),
            ActionError::OutOfBounds {
                destination: pos(2, 7),
            },
        ),
    ] {
        assert_eq!(state.apply(&command, Some(&context)), Err(error));
        assert_eq!(serde_json::to_vec(&state).unwrap(), state_bytes);
        assert_eq!(serde_json::to_vec(&context).unwrap(), context_bytes);
    }
}

#[test]
fn source_and_destination_terrain_interactions_are_unsupported() {
    for position in [pos(2, 2), pos(2, 1)] {
        for terrain in [0, 3, 8, 9] {
            let mut source = document();
            source.terrain[position.index().unwrap()] = terrain;
            let state = CoreState::new(source).unwrap();
            let context = context(9, CardinalDirection::DecreasingRow);
            assert_eq!(
                state.apply(&command(9, 2, 1), Some(&context)),
                Err(ActionError::UnsupportedTerrain)
            );
        }
    }
}

#[test]
fn candidate_directions_obey_board_edges_without_row_wraparound() {
    // Synthetic geometry coverage, not original-game movement-legality evidence.
    for row in 0..7u8 {
        for column in 0..7u8 {
            let source = pos(column, row);
            let state = CoreState::new(StateDocument {
                schema_version: STATE_SCHEMA.into(),
                terrain: vec![NORMAL_TERRAIN; CELL_COUNT],
                units: vec![Unit {
                    id: UnitId(63),
                    card_id: 8,
                    side: 1,
                    position: source,
                }],
                life_points: [0, 9999],
            })
            .unwrap();
            for (direction, dc, dr) in [
                (CardinalDirection::DecreasingRow, 0i16, -1i16),
                (CardinalDirection::IncreasingRow, 0, 1),
                (CardinalDirection::DecreasingColumn, -1, 0),
                (CardinalDirection::IncreasingColumn, 1, 0),
            ] {
                let next_column = i16::from(column) + dc;
                let next_row = i16::from(row) + dr;
                // A coordinate below zero is encoded as the explicitly invalid 255.
                let destination = pos(
                    u8::try_from(next_column).unwrap_or(255),
                    u8::try_from(next_row).unwrap_or(255),
                );
                let command = Command::MoveUnit {
                    unit: UnitId(63),
                    destination,
                };
                let result = state.apply(&command, Some(&context(63, direction)));
                if (0..7).contains(&next_column) && (0..7).contains(&next_row) {
                    let moved = result.unwrap();
                    assert_eq!(moved.state.unit(UnitId(63)).unwrap().position, destination);
                    assert_eq!(moved.state.document().units.len(), 1);
                    assert_eq!(moved.state.unit_at(source), None);
                } else {
                    assert_eq!(result, Err(ActionError::OutOfBounds { destination }));
                }
            }
        }
    }
}

#[test]
fn construction_and_deserialization_reject_invalid_state() {
    let original = serde_json::to_value(document()).unwrap();
    let mut cases = Vec::new();
    for (pointer, value) in [
        ("/schema_version", json!("future")),
        ("/terrain/0", json!(10)),
        ("/units/0/card_id", json!(854)),
        ("/units/0/side", json!(3)),
        ("/units/0/position/column", json!(7)),
        ("/units/0/position/row", json!(255)),
        ("/units/0/id", json!(9)),
        ("/units/0/position", json!({"column":2,"row":2})),
        ("/life_points/0", json!(10000)),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        cases.push(changed);
    }
    let mut missing_cell = original.clone();
    missing_cell["terrain"].as_array_mut().unwrap().pop();
    cases.push(missing_cell);
    let mut excess_units = original.clone();
    excess_units["units"] = Value::Array(vec![original["units"][0].clone(); 50]);
    cases.push(excess_units);
    for invalid in cases {
        let transport: StateDocument = serde_json::from_value(invalid.clone()).unwrap();
        assert!(CoreState::new(transport).is_err());
        assert!(serde_json::from_value::<CoreState>(invalid).is_err());
    }
    let mut unknown_field = original;
    unknown_field["guessed_turn"] = json!(0);
    assert!(serde_json::from_value::<CoreState>(unknown_field).is_err());
}

#[test]
fn canonical_state_and_serialized_replay_are_deterministic() {
    let state = CoreState::new(document()).unwrap();
    let mut reordered = document();
    reordered.units.reverse();
    assert_eq!(
        serde_json::to_vec(&state).unwrap(),
        serde_json::to_vec(&CoreState::new(reordered).unwrap()).unwrap()
    );
    let command = command(9, 1, 2);
    let context = context(9, CardinalDirection::DecreasingColumn);
    let expected = state.apply(&command, Some(&context)).unwrap();
    let replay_state: CoreState =
        serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    let replay_command: Command =
        serde_json::from_slice(&serde_json::to_vec(&command).unwrap()).unwrap();
    let replay_context: MoveContext =
        serde_json::from_slice(&serde_json::to_vec(&context).unwrap()).unwrap();
    let replay = replay_state
        .apply(&replay_command, Some(&replay_context))
        .unwrap();
    assert_eq!(replay, expected);
    assert_eq!(
        serde_json::to_vec(&replay).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
    let recovered: Transition =
        serde_json::from_slice(&serde_json::to_vec(&replay).unwrap()).unwrap();
    assert_eq!(recovered, expected);
}

#[test]
fn command_and_context_serialization_require_explicit_fields() {
    for json in [
        json!({"kind":"move_unit","unit":9}),
        json!({"kind":"move_unit","unit":9,"destination":{"column":2,"row":1},"expected_card_id":12}),
    ] {
        assert!(serde_json::from_value::<Command>(json).is_err());
    }
    for json in [
        json!({"kind":"restricted","eligible_unit":9,"direction":"decreasing_row"}),
        json!({"kind":"restricted","eligible_unit":9,"permission":"available"}),
        json!({"kind":"restricted","direction":"decreasing_row","permission":"available"}),
    ] {
        assert!(serde_json::from_value::<MoveContext>(json).is_err());
    }
}
