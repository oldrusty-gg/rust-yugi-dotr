# roses-assets

An independently implemented, bounded Rust decoder for the first supported PAL
card metadata and picture formats. Input bytes come from the user's ISO; no
original card table or artwork is included in this crate.

## APIs

- `parse_pal_catalog(executable)` returns a serializable `CardCatalog`, after
  requiring the exact supported executable SHA-256. It parses 854 records.
- `decode_picture(archive, index)` returns a 128×128 `RgbaImage` with pixels in
  row-major RGBA order. It accepts up to 871 complete fixed-size records and
  validates the selected record. It does not write files or encode PNG.
- `parse_pal_rules(executable)` returns serializable `RuleTables` with 26,540
  fusion records and 46 terrain boards. The complete executable fingerprint is
  required, and no runtime rule is inferred merely from a table entry.
- `parse_pal_card_names(message_file)` returns `CardNames` with 854 English
  names indexed by card ID and message ID. It requires the exact supported
  `MSG/MSG_E.DAT` fingerprint and recognizes only the observed name glyphs.
- `Error` implements `std::error::Error` and distinguishes malformed input,
  unsupported structures, an unrecognized executable, and an invalid index.

The picture decoder checks structure rather than source authenticity. Obtain
the executable and archive from the same supported ISO. Records beyond the
854-card table are not automatically identified as cards. Asset conversion is
not evidence of rule or visual parity with a running game.

## Verified static layout

The known `SLES_515.79` SHA-256 is
`0b23138866f478cd6ef69190388925b32b1e105635b09298b3a1ddd91549c1a2`.
Its card table occupies file offsets `0x1C4D80..0x1C9038`, with 854 records of
20 bytes. The first load segment has file offset `0x6980`, virtual address
`0x100000`, and file size `0x1D3600`, so the table maps to EE `0x2BE400`.

| Record offset | Interpretation |
| --- | --- |
| 0 | Kind code, one byte |
| 1 | Opaque kind-offset byte |
| 2 | Level in the upper nibble, attribute in the lower nibble |
| 3 | Deck cost, one byte |
| 4 | Raw little-endian effect word |
| 6 | Raw little-endian xaxId word; meaning remains unverified |
| 8 | Attack in low 13 bits; complete word also retained |
| 10 | Defense in low 13 bits; complete word also retained |
| 12..20 | Eight opaque bytes retained without decoding |

The NTSC-U [structure documentation](https://github.com/GenericMadScientist/DotR-Documentation/blob/master/structs.h)
provided field-layout leads. The [enum documentation](https://github.com/GenericMadScientist/DotR-Documentation/blob/master/enums.h)
provided kind and attribute codes. PAL offsets were located independently and
checked against the local executable, its ELF mapping, and the full table's
ranges: 683 monster records, 171 other cards, documented kind codes, attributes
0–5, levels 0–12. These checks do not establish effect-word behavior, password
encoding, or any runtime rule. No third-party implementation was copied.

`DATA/PICTURE.MRG` has 871 records of `0x4800` bytes. Every inspected record has
RH2 magic, a declared content length of `0x4410` at offset 8, and dimensions
128×128 at offsets `0x54` and `0x56`. The decoder reads 256 little-endian RGB5551
palette entries at `0x150`, combines pixel blocks `0x3B0..0x23B0` and
`0x2410..0x4410`, applies the researched PS2 indexed layout and swaps palette
index bits 3/4. Alpha is expanded from one bit; this is not a general PS2 GS
texture/blending implementation. No broader RH2 support is claimed.

## Fusion and terrain records

The PAL fusion table starts at file `0x1A45A0` (EE address `0x29DC20`). Each
little-endian 32-bit record packs the higher ingredient ID in bits 0–9, the
lower ingredient ID in bits 10–19, and the result ID in bits 20–29. The high
two bits must be zero. Equal ingredient IDs are allowed: the local table has
19 such pairs. All 26,540 ingredient pairs are unique. The complete original
word and table index are retained alongside the three IDs. The following
`0x3E7F9FE7` marker decodes as 999/999/999; its possible terminator role is not
runtime verified and it is excluded from the fusion list.

The terrain table starts at file `0x1D5F60` (EE address `0x2CF5E0`) and ends at
`0x1D682E`. Each of 46 records contains 49 terrain bytes, each in 0–9. The
decoder retains their order and reports board dimensions of 7×7. A row-major
display is an interpretation of the documented format; the PAL game's board
orientation and map-to-opponent assignments remain unverified. Terrain names
can be taken from the linked public enum documentation.

Format leads came from public technical documentation and the recorded local
research; the PAL offsets, complete counts, marker, duplicate-pair checks,
packing, and terrain ranges were independently checked against the user's
executable. Parsers were written independently. No original table records are
embedded in production code or synthetic tests. Neither fusion priority and
context nor terrain effects are implemented by these data decoders.

## English card names

The message file fingerprint is
`c88ee92a21c1cd36baa7e5d56dd41386a7fa906cd1796d7537c62217c1a2c1e3`.
The supported file is 283,008 bytes and starts with `MWo3`. Its 4,111 offsets
at `0x100` count 16-bit tokens relative to the pool at `0x413C`. Card IDs 0–853
use message IDs 323–1176. The decoder validates the observed header and reads
bounded, monotonically indexed streams terminated by token bit 15.

A reference uses two tokens: the first carries the source raw-token offset
and expanded copy length; the second has marker bit 14 and a source message
ID in its lower 14 bits. Bit 15 of the second token can end the message. Every
reference must target an earlier message and a token boundary. The source
offset counts compressed tokens, not output characters. The importer retains
boundary-to-output mappings and decodes in table order, avoiding recursion.
References into operands, forward references, missing terminators, overruns,
and unsupported name glyphs are errors. Limits are 1,024 raw tokens and 4,096
expanded tokens per message, one million expanded tokens total, and 256
characters per name. Data following the last indexed stream is not interpreted.

This format was researched independently from the user's local message file.
The complete file was also observed unchanged at EE address `0x3B0000` in a
preserved original-game main-menu state; this confirms the loaded data mapping,
not every glyph's rendered appearance. The native decoder's 854 names agree
with an independently written Python decoder. Original names and message bytes
remain local: public tests use invented text. Dialogue, descriptions, general
control tokens, other languages, and game behavior are outside this API.

## Validation

The opening texture API also exposes `decode_pal_opening_indexed_texture` for
the custom renderer. It returns top-down palette indices after texture-page
unswizzling and the CLUT bit-3/4 permutation, alongside unchanged palette words.
This preserves the original GS alpha bytes, including values above128, and
unexpanded RGB5551 entries. The ordinary RGBA decoder retains its existing
display conversion. These source values are needed for the recovered historical
cloud effect; importing them alone does not implement its animation or blending.

`cargo test -p roses-assets` runs synthetic tests for field packing, raw-field
preservation, hash gating, bounds, color/alpha lanes, both pixel blocks,
palette permutation, and complete swizzle-address coverage.
Fusion/map tests also cover separated packed fields, same-card pairs, duplicate
pairs, reserved bits, invalid IDs, bounded counts, the trailing marker, terrain
ranges, and preservation of cell order.

An ignored private-data test parses the known executable and decodes all 871
pictures. It compares picture zero's decoded RGBA digest with a separately
generated local Python image that was visually inspected. Set
`ROSES_TEST_ELF_PATH` and `ROSES_TEST_PICTURE_PATH`, then run:

```text
cargo test -p roses-assets private_pal_catalog_and_all_pictures_decode -- --ignored
```

The separate `private_pal_rules_decode_with_expected_structural_properties`
ignored test requires only `ROSES_TEST_ELF_PATH`. It imports all fusion and map
records and verifies structural properties; it is not an emulator comparison.

`private_pal_names_match_all_independent_expected_records` requires
`ROSES_TEST_MESSAGE_PATH` and `ROSES_TEST_NAMES_JSON_PATH`. The latter is the
independent local JSON oracle, with `card_id`, `string_index`, and `name` fields.
It compares all 854 names and a canonical name-list digest. Synthetic text tests
exercise nested references, compressed-token offsets, operand markers, invalid
boundaries, termination, glyph mapping, and expansion limits.

Private asset files and generated images should remain in ignored local storage.
