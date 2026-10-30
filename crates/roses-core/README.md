# roses-core

This crate computes one restricted board movement. It is not yet a duel rules engine.

The immutable `CoreState` validates a 7×7 board, 49 terrain codes, unique unit IDs and positions, card IDs, raw side values, and two LP values. Deserializing a `CoreState` performs the same validation. Serialization sorts units by ID so equivalent input ordering produces the same state representation. Card IDs may repeat: identity belongs to each unit instance.

`CoreState::apply(&command, Some(&context))` computes a new state and a `UnitMoved` event. A successful move preserves unit identity, card ID, side, terrain, LP and all other units. It requires an explicit `MoveContext::Restricted` that names one eligible unit, one allowed grid direction and an available permission. The destination must be one cardinal step away, empty, and within the board; both source and destination must have normal terrain code 7. Missing or unknown context is unsupported. The returned context consumes the supplied permission; that token is a model restriction, not proof of the original game's once-per-turn implementation.

The caller must establish that the isolated operation needs no battle, terrain interaction, card effect or turn logic. The core never infers that claim from a unit's side, card ID, or a guessed phase. It does not decide general movement legality, whose turn it is, input timing, summon cost, combat, effects, campaign progression, animation or network authority. Side values remain caller-supplied raw values.

All four grid directions can be selected as **candidate policies** for controlled experiments. Their implementation and synthetic tests do not establish that the original game permits all four directions in any given situation. Reference parity requires an independent before/after observation, a suitable no-input control, and repeated reference execution. This crate alone does not make that claim.

The model has no dependency on a viewer, launcher, emulator, extracted files, or an expected endpoint. Public tests contain synthetic states only. A separate adapter may project a researched reference snapshot into `StateDocument`, supply an explicit context and command, and compare the computed result with independently observed fields.

Run `cargo test --offline -p roses-core` from the workspace root.
