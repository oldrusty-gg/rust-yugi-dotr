use super::*;
use crate::card_block::{CardRecord, RECENT_LEN};

/// The typeable password the synthetic record of a card decodes to.
fn synthetic_password(card: u16) -> [u8; 8] {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut pw = [0u8; 8];
    for (j, ch) in pw.iter_mut().enumerate() {
        *ch = ALPHABET[(usize::from(card) * 5 + j * 7) % 36];
    }
    pw
}

/// Synthetic static data: monsters 0..683 (kind 0), spells/traps after;
/// cost = id % 20 + 1; ATK = id·7 % 3000, DEF = id·11 % 3000; cards 1..=10
/// carry a password; every even id is reincarnation-eligible; name order
/// = reversed id.
fn cards() -> StaticCards {
    let n = usize::from(CARD_COUNT);
    let mut records = vec![[0u8; RECORD_BYTES]; n];
    for (c, r) in records.iter_mut().enumerate() {
        let kind: u8 = if c < usize::from(RANKED_CARDS) { 0 } else { 1 };
        r[0] = (kind << 5) | (c % 7) as u8;
        r[2] = ((c % 5) << 4) as u8 | (c % 16) as u8;
        r[3] = (c % 20 + 1) as u8;
        let effect: u16 = if c % 3 == 0 { 0xFFFF } else { 5 };
        r[4..6].copy_from_slice(&effect.to_le_bytes());
        r[8..10].copy_from_slice(&((c * 7 % 3000) as u16).to_le_bytes());
        r[10..12].copy_from_slice(&((c * 11 % 3000) as u16).to_le_bytes());
        if c % 2 == 0 {
            r[9] |= 0x80;
        }
        if (1..=10).contains(&c) {
            r[9] |= 0x40;
            for j in 0..8 {
                r[0xC + j] = synthetic_password(c as u16)[j].wrapping_sub(b"HANNIBAL"[(c + j) & 7]);
            }
        }
    }
    StaticCards {
        records,
        name_order: (0..CARD_COUNT).rev().collect(),
        password_key: *b"HANNIBAL",
    }
}

struct Fixture {
    block: CardBlock,
    cards: StaticCards,
    ca: u8,
    rng: u64,
}

impl Fixture {
    /// Deck 0 = cards 1..=40 (one copy each, category 0), leader 100
    /// (rank 1), trunk copies: 1 ×2, 41, 42 ×3, 100.
    fn new() -> Self {
        let mut block = CardBlock::empty(0x1234_5678);
        let mut rec = DeckRecord::EMPTY;
        for (i, slot) in rec.cards.iter_mut().enumerate() {
            *slot = i as u16 + 1;
            give(&mut block, i as u16 + 1, 1, 0, false);
        }
        rec.leader = 100;
        write_deck_record(&mut block, 1, &DeckRecord::EMPTY);
        write_deck_record(&mut block, 2, &DeckRecord::EMPTY);
        give(&mut block, 100, 1, 0, true);
        give(&mut block, 100, 1, 3, false);
        write_deck_record(&mut block, 0, &rec);
        block.records[100].rank_word = OWNED_BIT | 1;
        give(&mut block, 1, 2, 3, false);
        give(&mut block, 41, 1, 3, false);
        give(&mut block, 42, 3, 3, false);
        let cards = cards();
        recompute_deck_cost(&mut block, &cards, 0);
        Self {
            block,
            cards,
            ca: 0,
            rng: 0x1234,
        }
    }
    fn env(&mut self) -> EditorEnv<'_> {
        EditorEnv {
            block: &mut self.block,
            duel_counter: &mut self.ca,
            cards: &self.cards,
            rng: &mut self.rng,
            graphics_busy: false,
        }
    }
    fn editor(&mut self) -> DeckEditor {
        DeckEditor::new(0, &mut self.env())
    }
    fn deck(&self, d: u8) -> DeckRecord {
        deck_record(&self.block, d)
    }
    fn categories(&self, card: u16) -> Vec<(u8, bool)> {
        self.block.records[usize::from(card)]
            .instances
            .iter()
            .filter(|i| !i.is_free())
            .map(|&i| (instance_category(i), instance_is_leader(i)))
            .collect()
    }
}

/// Add `n` occupied instance slots with the category and leader flag.
fn give(block: &mut CardBlock, card: u16, n: usize, category: u8, leader: bool) {
    let record = &mut block.records[usize::from(card)];
    let mut added = 0;
    for slot in record.instances.iter_mut() {
        if added == n {
            break;
        }
        if slot.is_free() {
            *slot = CardInstance {
                owner: 0x1234_5678 | if leader { 0x8000_0000 } else { 0 },
                id: (u32::from(category) << 30) | (0x1000 + u32::from(card)),
            };
            added += 1;
        }
    }
    record.rank_word |= OWNED_BIT;
}

fn press(ed: &mut DeckEditor, fx: &mut Fixture, word: u16) -> Vec<EditorEvent> {
    let mut ev = ed.step(PadInput { word, sticks: None }, &mut fx.env());
    ev.extend(ed.step(PadInput::default(), &mut fx.env()));
    ev
}

fn wait(ed: &mut DeckEditor, fx: &mut Fixture, n: usize) -> Vec<EditorEvent> {
    let mut ev = Vec::new();
    for _ in 0..n {
        ev.extend(ed.step(PadInput::default(), &mut fx.env()));
    }
    ev
}

fn sounds(ev: &[EditorEvent]) -> Vec<u8> {
    ev.iter()
        .filter_map(|e| match e {
            EditorEvent::Sound(s) => Some(*s),
            _ => None,
        })
        .collect()
}

/// Move the trunk cursor onto a card.
fn goto_trunk(ed: &mut DeckEditor, fx: &mut Fixture, card: u16) {
    let idx = ed.trunk_list().iter().position(|&c| c == card).unwrap();
    let s = ed.snapshot();
    let mut at = usize::from(s.trunk_window) + usize::from(s.trunk_row);
    while at < idx {
        press(ed, fx, pad::DOWN);
        at += 1;
    }
    while at > idx {
        press(ed, fx, pad::UP);
        at -= 1;
    }
    assert_eq!(ed.highlighted(ListSide::Trunk), card);
}

/// Move the deck cursor to the leader row.
fn goto_deck_top(ed: &mut DeckEditor, fx: &mut Fixture) {
    loop {
        let s = ed.snapshot();
        if s.deck_window == 0 && s.deck_row == 0 {
            break;
        }
        press(ed, fx, pad::UP);
    }
}

#[test]
fn deck_records_live_in_the_raw_tail() {
    let mut block = CardBlock::empty(1);
    let rec = DeckRecord {
        cards: [7; DECK_SIZE],
        leader: 392,
        cost: 822,
    };
    write_deck_record(&mut block, 2, &rec);
    assert_eq!(deck_record(&block, 2), rec);
    // An all-zero tail is not a new game (untraced [H]): it reads as card 0s.
    assert_eq!(deck_record(&block, 1).leader, 0);
    write_deck_record(&mut block, 1, &DeckRecord::EMPTY);
    assert_eq!(deck_record(&block, 1), DeckRecord::EMPTY);
    let bytes = block.to_bytes().unwrap();
    let base = 0xFD94 + 2 * 0x54;
    assert_eq!(&bytes[base..base + 2], &[7, 0]);
    assert_eq!(&bytes[base + 0x50..base + 0x54], &[0x88, 0x01, 0x36, 0x03]);
    assert!(set_current_deck(&mut block, 2));
    assert!(!set_current_deck(&mut block, 3));
    assert_eq!(current_deck(&block), 2);
    assert_eq!(block.to_bytes().unwrap()[0xFF9C] & 3, 2);
    assert_eq!(current_deck(&CardBlock::from_bytes(&bytes).unwrap()), 0);
}

#[test]
fn init_builds_sorted_lists_and_recomputes_the_cost() {
    let mut fx = Fixture::new();
    fx.block.recent = [42, EMPTY_CARD, EMPTY_CARD, EMPTY_CARD, EMPTY_CARD];
    let ed = fx.editor();
    let snap = ed.snapshot();
    assert_eq!(snap.state, state::TRUNK_LIST);
    assert_eq!(snap.trunk_keys, DEFAULT_SORT_KEYS);
    assert_eq!(snap.trunk_count, 43);
    // Recent first, then byte 0 (id % 7), then id.
    assert_eq!(snap.trunk[0], 42);
    assert_eq!(snap.trunk[1..5], [7, 14, 21, 28]);
    assert_eq!(snap.deck_list[0], 100);
    assert_eq!(snap.deck_list[1..5], [7, 14, 21, 28]);
    let expected: u16 = (1..=40u16).map(|c| c % 20 + 1).sum();
    assert_eq!(fx.deck(0).cost, expected);
    assert_eq!(ed.trunk_list().len(), 43);
    assert!(!ed.trunk_list().contains(&EMPTY_CARD));
}

#[test]
fn add_and_remove_follow_0x214c70_and_0x214f00() {
    let mut fx = Fixture::new();
    let cards = fx.cards.clone();
    let cost0 = fx.deck(0).cost;
    // 41 cards: no free slot.
    assert!(!add_to_deck(&mut fx.block, &cards, 0, 41));
    assert!(remove_from_deck(&mut fx.block, &cards, 0, 4));
    assert_eq!(fx.deck(0).cards[4], EMPTY_CARD);
    assert_eq!(fx.deck(0).cost, cost0 - 6);
    assert_eq!(fx.categories(5), vec![(3, false)]);
    // Card 42 has three trunk copies; only the first add fits the slot.
    assert!(add_to_deck(&mut fx.block, &cards, 0, 42));
    assert_eq!(fx.deck(0).cards[4], 42);
    assert_eq!(fx.deck(0).cost, cost0 - 6 + 3);
    assert_eq!(fx.categories(42), vec![(0, false), (3, false), (3, false)]);
    assert!(!add_to_deck(&mut fx.block, &cards, 0, 42));
    // Three copies already → refused even with free slots and trunk copies.
    for slot in 0..3 {
        assert!(remove_from_deck(&mut fx.block, &cards, 0, slot));
    }
    assert!(add_to_deck(&mut fx.block, &cards, 0, 42));
    assert!(add_to_deck(&mut fx.block, &cards, 0, 42));
    assert!(!add_to_deck(&mut fx.block, &cards, 0, 42));
    assert_eq!(fx.deck(0).cards.iter().filter(|&&c| c == 42).count(), 3);
    // Card 1 has trunk copies: adds go into the first free slot.
    assert!(add_to_deck(&mut fx.block, &cards, 0, 1));
    assert_eq!(fx.deck(0).cards[2], 1);
    // No trunk copy (card 2) → refused; the leader copy does not count.
    assert!(!add_to_deck(&mut fx.block, &cards, 0, 2));
    assert!(!add_to_deck(&mut fx.block, &cards, 0, EMPTY_CARD));
    assert!(!add_to_deck(&mut fx.block, &cards, 0, 854));
    // Removing an empty slot or an out-of-range slot changes nothing.
    assert!(remove_from_deck(&mut fx.block, &cards, 0, 3));
    let before = fx.deck(0);
    assert_eq!(before.cards[3], EMPTY_CARD);
    assert!(!remove_from_deck(&mut fx.block, &cards, 0, 3));
    assert!(!remove_from_deck(&mut fx.block, &cards, 0, 40));
    assert_eq!(fx.deck(0), before);
}

#[test]
fn leader_rules_follow_0x214a10_and_0x214b60() {
    let mut fx = Fixture::new();
    // A second leader is refused while one is set.
    assert!(!set_leader(&mut fx.block, 0, 42));
    assert!(remove_leader(&mut fx.block, 0));
    assert_eq!(fx.deck(0).leader, EMPTY_CARD);
    assert_eq!(fx.categories(100), vec![(3, false), (3, false)]);
    assert!(!remove_leader(&mut fx.block, 0));
    assert!(set_leader(&mut fx.block, 0, 42));
    assert_eq!(fx.deck(0).leader, 42);
    assert_eq!(fx.categories(42), vec![(0, true), (3, false), (3, false)]);
    // The leader is outside the 40 and the cost.
    assert_eq!(fx.deck(0).count(), 40);
    assert!(fx.deck(0).is_valid());
    assert!(deck_is_valid(&fx.block, 0));
    assert!(!deck_is_valid(&fx.block, 1));
    // No trunk copy → refused.
    assert!(remove_leader(&mut fx.block, 0));
    assert!(!set_leader(&mut fx.block, 0, 2));
}

#[test]
fn start_sets_the_leader_only_for_ranked_trunk_monsters() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    // Card 1 has trunk copies but rank 0.
    goto_trunk(&mut ed, &mut fx, 1);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::START)), [6]);
    assert_eq!(fx.deck(0).leader, 100);
    // Card 42: give it a rank, no copy in the deck.
    fx.block.records[42].rank_word |= 2;
    let mut ed = fx.editor();
    goto_trunk(&mut ed, &mut fx, 42);
    let ev = press(&mut ed, &mut fx, pad::START);
    assert_eq!(sounds(&ev), [4]);
    assert!(ev.contains(&EditorEvent::ReleaseGraphics));
    assert_eq!(fx.deck(0).leader, 42);
    assert_eq!(ed.deck_list()[0], 42);
    assert_eq!(fx.categories(100), vec![(3, false), (3, false)]);
    // The reload flag fired on the next (release) frame.
    assert_eq!(ev.last(), Some(&EditorEvent::ReloadLeader));
    assert!(!ed.snapshot().reload_pending);
    // □ on the leader row of the deck list removes it.
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.state(), state::DECK_LIST);
    goto_deck_top(&mut ed, &mut fx);
    let ev = press(&mut ed, &mut fx, pad::SQUARE);
    assert_eq!(sounds(&ev), [4]);
    assert_eq!(fx.deck(0).leader, EMPTY_CARD);
    assert_eq!(ed.deck_list()[0], EMPTY_CARD);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::SQUARE)), [6]);
    // × on the leader row does nothing at all.
    assert!(press(&mut ed, &mut fx, pad::CROSS).is_empty());
}

#[test]
fn exit_needs_a_valid_current_deck() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    press(&mut ed, &mut fx, pad::RIGHT);
    press(&mut ed, &mut fx, pad::DOWN);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::SQUARE)), [4]);
    assert_eq!(fx.deck(0).count(), 39);
    let ev = press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(sounds(&ev), [6]);
    assert_eq!(ed.state(), state::MSG_DECK_INVALID);
    assert_eq!(ed.snapshot().return_state, state::DECK_LIST);
    // Only △○×□ dismiss the message.
    assert!(press(&mut ed, &mut fx, pad::START).is_empty());
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::TRIANGLE)), [4]);
    assert_eq!(ed.state(), state::DECK_LIST);
    // Put the card back from the trunk list, then leave.
    press(&mut ed, &mut fx, pad::LEFT);
    assert_eq!(ed.state(), state::TRUNK_LIST);
    assert_eq!(ed.snapshot().trunk_row, 1);
    let removed = fx.deck(0).cards;
    assert_eq!(removed.iter().filter(|&&c| c == EMPTY_CARD).count(), 1);
    press(&mut ed, &mut fx, pad::UP);
    goto_trunk(&mut ed, &mut fx, 41);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert!(fx.deck(0).is_valid());
    let ev = press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ev, [EditorEvent::Sound(sound::LEAVE), EditorEvent::Exit]);
    assert!(ed.finished());
    assert!(press(&mut ed, &mut fx, pad::CROSS).is_empty());
}

#[test]
fn comparators_order_like_0x2905b0() {
    let mut fx = Fixture::new();
    fx.block.recent = [20, 10, EMPTY_CARD, EMPTY_CARD, EMPTY_CARD];
    fx.block.records[5].rank_word = OWNED_BIT | 3;
    let (b, c) = (&fx.block, &fx.cards);
    let less = Ordering::Less;
    // 0: recent list first, 999 last.
    assert_eq!(compare_key(0, b, c, 20, 3), less);
    assert_eq!(compare_key(0, b, c, 10, 20), Ordering::Equal);
    assert_eq!(compare_key(0, b, c, 3, EMPTY_CARD), less);
    // 1: name order (reversed ids here).
    assert_eq!(compare_key(1, b, c, 9, 3), less);
    assert_eq!(compare_key(1, b, c, 9, EMPTY_CARD), less);
    // 2/3: ATK and DEF descending, 999 last.
    assert_eq!(compare_key(2, b, c, 9, 3), less);
    assert_eq!(compare_key(3, b, c, 9, 3), less);
    assert_eq!(compare_key(2, b, c, 3, EMPTY_CARD), less);
    assert_eq!(compare_key(2, b, c, EMPTY_CARD, 3), Ordering::Greater);
    // 4: byte 0 (kind·32 + sub-type).
    assert_eq!(compare_key(4, b, c, 7, 1), less);
    assert_eq!(compare_key(4, b, c, 6, 683), less);
    // 5: attribute for monsters, then the kind-derived values.
    assert_eq!(derived_order(0, 15), 15);
    assert_eq!(derived_order(1, 0), 6);
    assert_eq!(derived_order(4, 9), 7);
    assert_eq!(derived_order(5, 0), 8);
    assert_eq!(derived_order(6, 0), 11);
    assert_eq!(derived_order(7, 0), 12);
    assert_eq!(compare_key(5, b, c, 1, 2), less);
    // 6/7: byte 2 >> 4 and cost.
    assert_eq!(compare_key(6, b, c, 5, 6), less);
    assert_eq!(compare_key(7, b, c, 20, 1), less);
    // 8: stored rank, unranked ids last.
    assert_eq!(compare_key(8, b, c, 1, 5), less);
    assert_eq!(compare_key(8, b, c, 5, 683), less);
    assert_eq!(compare_key(8, b, c, 683, 700), Ordering::Equal);
    // 9: effect monsters, plain monsters, then kind + 6.
    assert_eq!(compare_key(9, b, c, 1, 3), less);
    assert_eq!(compare_key(9, b, c, 3, 683), less);
    assert_eq!(compare_key(9, b, c, 3, 6), Ordering::Equal);
    // 10: id.
    assert_eq!(compare_key(10, b, c, 3, 683), less);
    // Repeated keys are skipped; ties fall through to the next key.
    assert_eq!(compare_keys(&[0, 0, 10], b, c, 20, 10), Ordering::Greater);
    assert_eq!(compare_keys(&[0, 4, 10], b, c, 7, 1), less);
}

#[test]
fn sort_keys_cycle_with_l2_r2_and_select() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::R2)), [0x13]);
    assert_eq!(ed.snapshot().trunk_keys, [1, 4, 10]);
    // Names are reversed ids: the highest id first.
    assert_eq!(ed.trunk_list()[0], 100);
    press(&mut ed, &mut fx, pad::SELECT | pad::R2);
    assert_eq!(ed.snapshot().trunk_keys, [1, 5, 10]);
    press(&mut ed, &mut fx, pad::L2);
    assert_eq!(ed.snapshot().trunk_keys, [0, 5, 10]);
    press(&mut ed, &mut fx, pad::L2);
    assert_eq!(ed.snapshot().trunk_keys, [9, 5, 10]);
    // Key 0xA is never reached.
    for _ in 0..10 {
        press(&mut ed, &mut fx, pad::R2);
    }
    assert_eq!(ed.snapshot().trunk_keys, [9, 5, 10]);
    // Repeated key: SELECT+L2 ×4 brings key 1 to 1 … no sound on 5→5.
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.snapshot().deck_keys, DEFAULT_SORT_KEYS);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::R2)), [0x13]);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::R2)), [0x13]);
    assert_eq!(ed.snapshot().deck_keys, [2, 4, 10]);
    let list = ed.deck_list();
    assert_eq!(list[0], 100);
    let atk = |c: u16| u32::from(c) * 7 % 3000;
    assert!(list[1..].windows(2).all(|w| atk(w[0]) >= atk(w[1])));
}

#[test]
fn passwords_decode_with_the_key_and_grant_once() {
    let fx = Fixture::new();
    let pw = fx.cards.password(3).unwrap();
    assert_eq!(pw, synthetic_password(3));
    let rec = fx.cards.record(3).unwrap();
    for j in 0..8 {
        assert_eq!(pw[j], rec[0xC + j].wrapping_add(b"HANNIBAL"[(3 + j) & 7]));
    }
    assert_eq!(fx.cards.password(11), None);
    assert_eq!(fx.cards.find_password(&pw), Some(3));
    assert_eq!(fx.cards.find_password(b"zzzzzzzz"), None);
    // Pick card 9 (cost 10) whose password we can type: build it by
    // steering the characters.
    let pw9 = fx.cards.password(9).unwrap();
    let mut fx = fx;
    let mut ed = fx.editor();
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::R3)), [0x10]);
    assert_eq!(ed.state(), state::PASSWORD);
    type_password(&mut ed, &mut fx, &pw9);
    assert_eq!(ed.snapshot().password, pw9);
    let copies_before = count_copies(&fx.block, 9, None);
    let ev = press(&mut ed, &mut fx, pad::CROSS);
    assert!(ev.contains(&EditorEvent::Granted(9)));
    assert_eq!(sounds(&ev), [4]);
    assert_eq!(ed.state(), state::MSG_PASSWORD_OK);
    assert_eq!(count_copies(&fx.block, 9, None), copies_before + 1);
    assert!(password_used(&fx.block, 9));
    assert_eq!(fx.block.recent[0], 9);
    // The granted copy keeps the free slot's category bits (0 in an empty
    // block: the capture's slots carried 0xC0000000 = trunk) [H].
    assert_eq!(ed.snapshot().results[0], 9);
    press(&mut ed, &mut fx, pad::CROSS);
    assert_eq!(ed.state(), state::TRUNK_LIST);
    // Second use → "already used".
    press(&mut ed, &mut fx, pad::R3);
    type_password(&mut ed, &mut fx, &pw9);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(ed.state(), state::MSG_PASSWORD_USED);
    press(&mut ed, &mut fx, pad::SQUARE);
    // Wrong password → 0xB; ○ leaves the entry.
    press(&mut ed, &mut fx, pad::R3);
    assert_eq!(ed.snapshot().password, *b"00000000");
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::DOWN)), [3]);
    assert_eq!(ed.snapshot().password[0], b'z');
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::LEFT)), [3]);
    assert_eq!(ed.snapshot().counter, 7);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [6]);
    assert_eq!(ed.state(), state::MSG_PASSWORD_WRONG);
    press(&mut ed, &mut fx, pad::CIRCLE);
    press(&mut ed, &mut fx, pad::R3);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CIRCLE)), [0x11]);
    assert_eq!(ed.state(), state::TRUNK_LIST);
}

fn type_password(ed: &mut DeckEditor, fx: &mut Fixture, pw: &[u8; 8]) {
    for &ch in pw {
        let v = if ch < b'a' { ch - b'0' } else { ch - 0x57 };
        for _ in 0..v {
            press(ed, fx, pad::UP);
        }
        press(ed, fx, pad::RIGHT);
    }
}

#[test]
fn reincarnation_draws_follow_0x251d50() {
    let fx = Fixture::new();
    // Leader 100 has rank 1 → m = 1 → 2(m−1) + roll < 90 ⇔ roll < 90.
    let mut probe = fx.rng;
    let kind_roll = next_rand31(&mut probe) % 10;
    let cost_roll = next_rand31(&mut probe) % 100;
    let target = if cost_roll < 90 {
        21 - (next_rand31(&mut probe) % 12) as i32
    } else {
        21 + (next_rand31(&mut probe) % 10) as i32
    };
    assert!(target > 0);
    let mut rng = fx.rng;
    let result = reincarnate_one(&fx.block, &fx.cards, 59, &mut rng);
    let monsters = kind_roll < 8;
    let rec = fx.cards.record(result).unwrap();
    assert_ne!(result, 59);
    assert_eq!(result < RANKED_CARDS, monsters);
    assert!(rec[9] & 0x80 != 0);
    // The cost found is the first reachable one below the target.
    let mut t = target;
    let pool = if monsters { 0..683u16 } else { 683..854 };
    let found = loop {
        let any = pool.clone().any(|c| {
            c != 59 && fx.cards.cost(c) == Some(t as u8) && fx.cards.reincarnation_eligible(c)
        });
        if any || t <= 0 {
            break t;
        }
        t -= 1;
    };
    assert_eq!(i32::from(rec[3]), found);
    // No eligible candidate at any cost → 820.
    let mut none = fx.cards.clone();
    for r in none.records.iter_mut() {
        r[9] &= !0x80;
    }
    let mut rng2 = fx.rng;
    assert_eq!(
        reincarnate_one(&fx.block, &none, 59, &mut rng2),
        REINCARNATION_FALLBACK
    );
    // A target at or below 0 (cost 2 → 3 − rand%12) also falls back.
    let mut low = fx.rng;
    let mut probe3 = fx.rng;
    next_rand31(&mut probe3);
    let r100 = next_rand31(&mut probe3) % 100;
    let t = if r100 < 90 {
        3 - (next_rand31(&mut probe3) % 12) as i32
    } else {
        3 + (next_rand31(&mut probe3) % 10) as i32
    };
    let got = reincarnate_one(&fx.block, &fx.cards, 41, &mut low);
    if t <= 0 {
        assert_eq!(got, REINCARNATION_FALLBACK);
        assert_eq!(low, probe3);
    }
    // The target uses the leaders of decks 0 and 1 only.
    let mut high = fx.block.clone();
    let mut rec2 = DeckRecord::EMPTY;
    rec2.leader = 5;
    high.records[5].rank_word = OWNED_BIT | 12;
    write_deck_record(&mut high, 2, &rec2);
    let (mut a, mut b) = (fx.rng, fx.rng);
    assert_eq!(
        reincarnation_target_cost(&fx.block, 10, &mut a),
        reincarnation_target_cost(&high, 10, &mut b)
    );
    write_deck_record(&mut high, 1, &rec2);
    // m = 12 → 22 + roll < 90 ⇔ roll < 68.
    let mut probe2 = fx.rng;
    let roll = next_rand31(&mut probe2) % 100;
    let expected = if roll < 68 {
        11 - (next_rand31(&mut probe2) % 12) as i32
    } else {
        11 + (next_rand31(&mut probe2) % 10) as i32
    };
    let mut c = fx.rng;
    assert_eq!(reincarnation_target_cost(&high, 10, &mut c), expected);
    assert_eq!(c, probe2);
}

#[test]
fn reincarnation_flow_is_gated_and_timed() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    goto_trunk(&mut ed, &mut fx, 41);
    // Duel counter below 5 → refused.
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L3)), [6]);
    fx.ca = 5;
    // Busy graphics → refused.
    let ev = {
        let mut env = fx.env();
        env.graphics_busy = true;
        ed.step(
            PadInput {
                word: pad::L3,
                sticks: None,
            },
            &mut env,
        )
    };
    assert_eq!(sounds(&ev), [6]);
    wait(&mut ed, &mut fx, 1);
    // No trunk copy (card 2) → refused.
    let mut other = fx.editor();
    goto_trunk(&mut other, &mut fx, 2);
    assert_eq!(sounds(&press(&mut other, &mut fx, pad::L3)), [6]);
    // Accepted: prompt, RIGHT = no, LEFT = yes.
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L3)), [0x10]);
    assert_eq!(ed.state(), state::REINCARNATION_PROMPT);
    assert!(press(&mut ed, &mut fx, pad::RIGHT).is_empty());
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(ed.state(), state::TRUNK_LIST);
    press(&mut ed, &mut fx, pad::L3);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::LEFT)), [3]);
    let trunk_before = ed.trunk_list().len();
    let ev = press(&mut ed, &mut fx, pad::CROSS);
    assert_eq!(sounds(&ev), [4]);
    assert_eq!(ed.state(), state::REINCARNATION_RESULTS);
    assert_eq!(count_copies(&fx.block, 41, None), 0);
    // The list is not rebuilt until the first grant.
    assert_eq!(ed.trunk_list().len(), trunk_before);
    let results = ed.snapshot().results;
    // Grant at frame 0x7F of the sub-step: the × frame was counter 0.
    let ev = wait(&mut ed, &mut fx, 0x7E);
    assert!(ev.is_empty());
    let ev = wait(&mut ed, &mut fx, 1);
    assert!(ev.contains(&EditorEvent::Granted(results[0])));
    assert_eq!(sounds(&ev), [7]);
    assert_eq!(ed.trunk_list().len(), trunk_before);
    // × is ignored until the counter passed 0x7F.
    assert!(wait(&mut ed, &mut fx, 5).is_empty());
    for (i, &result) in results.iter().enumerate().skip(1) {
        assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
        assert_eq!(ed.snapshot().sub_step, i as u8);
        assert!(wait(&mut ed, &mut fx, 0x7E).is_empty());
        let ev = wait(&mut ed, &mut fx, 1);
        assert_eq!(sounds(&ev), [7]);
        assert!(ev.contains(&EditorEvent::Granted(result)));
    }
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(ed.snapshot().sub_step, 3);
    assert!(wait(&mut ed, &mut fx, 10).is_empty());
    assert_eq!(fx.ca, 5);
    let ev = press(&mut ed, &mut fx, pad::CROSS);
    assert!(ev.contains(&EditorEvent::DuelCounterReset));
    assert_eq!(ed.state(), state::TRUNK_LIST);
    assert_eq!(fx.ca, 0);
    assert_eq!(fx.block.recent[0], results[2]);
}

#[test]
fn reincarnation_needs_one_valid_deck_among_three() {
    let mut fx = Fixture::new();
    fx.ca = 5;
    assert!(remove_leader(&mut fx.block, 0));
    assert!(!deck_is_valid(&fx.block, 0));
    let mut ed = fx.editor();
    goto_trunk(&mut ed, &mut fx, 41);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L3)), [6]);
    let mut rec = deck_record(&fx.block, 0);
    rec.leader = 100;
    write_deck_record(&mut fx.block, 2, &rec);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L3)), [0x10]);
}

#[test]
fn deck_switch_cycles_the_three_records() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    press(&mut ed, &mut fx, pad::RIGHT);
    let ev = press(&mut ed, &mut fx, pad::START);
    assert_eq!(sounds(&ev), [9]);
    assert!(ev.contains(&EditorEvent::ReleaseGraphics));
    assert_eq!(current_deck(&fx.block), 1);
    assert_eq!(ed.deck_list(), &[EMPTY_CARD; DECK_LIST_LEN]);
    // × on an empty slot: refused; □ too.
    press(&mut ed, &mut fx, pad::DOWN);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [6]);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::SQUARE)), [6]);
    // Add from the trunk into deck 1.
    press(&mut ed, &mut fx, pad::LEFT);
    goto_trunk(&mut ed, &mut fx, 42);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(fx.deck(1).cards[0], 42);
    assert_eq!(fx.deck(1).cost, 42 % 20 + 1);
    assert_eq!(fx.deck(0).count(), 40);
    assert_eq!(ed.deck_list()[1], 42);
    // × on the deck card adds another copy of it.
    press(&mut ed, &mut fx, pad::RIGHT);
    goto_deck_top(&mut ed, &mut fx);
    press(&mut ed, &mut fx, pad::DOWN);
    assert_eq!(ed.highlighted(ListSide::Deck), 42);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [6]);
    assert_eq!(fx.deck(1).count(), 3);
    assert_eq!(fx.categories(42), vec![(1, false), (1, false), (1, false)]);
    press(&mut ed, &mut fx, pad::START);
    press(&mut ed, &mut fx, pad::START);
    assert_eq!(current_deck(&fx.block), 0);
    assert_eq!(ed.deck_list()[0], 100);
}

#[test]
fn cursors_windows_and_panes() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    for _ in 0..7 {
        assert_eq!(sounds(&press(&mut ed, &mut fx, pad::DOWN)), [3]);
    }
    let s = ed.snapshot();
    assert_eq!((s.trunk_row, s.trunk_window), (5, 2));
    for _ in 0..10 {
        press(&mut ed, &mut fx, pad::R1);
    }
    assert_eq!(ed.snapshot().trunk_window, 43 - 6);
    assert!(press(&mut ed, &mut fx, pad::DOWN).is_empty());
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L1)), [3]);
    assert_eq!(ed.snapshot().trunk_window, 31);
    assert_eq!(ed.snapshot().idle, IDLE_FRAMES - 1);
    // Deck list: the row is copied, window maximum 35.
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.snapshot().deck_row, 5);
    for _ in 0..7 {
        press(&mut ed, &mut fx, pad::R1);
    }
    assert_eq!(ed.snapshot().deck_window, 35);
    press(&mut ed, &mut fx, pad::LEFT);
    assert_eq!(ed.state(), state::TRUNK_LIST);
    // △ opens the detail pane: the slide locks input for 6 frames.
    let ev = press(&mut ed, &mut fx, pad::TRIANGLE);
    assert_eq!(sounds(&ev), [0x10]);
    assert_eq!(ed.state(), state::TRUNK_LIST_DETAIL);
    assert_eq!(ed.snapshot().slide, 0x2D);
    assert!(press(&mut ed, &mut fx, pad::DOWN).is_empty());
    wait(&mut ed, &mut fx, 4);
    assert_eq!(ed.snapshot().slide, 0x100);
    // △ again: viewer for a known monster.
    let ev = press(&mut ed, &mut fx, pad::TRIANGLE);
    assert_eq!(
        ev,
        [
            EditorEvent::Viewer(ed.highlighted(ListSide::Trunk)),
            EditorEvent::Sound(4)
        ]
    );
    // LEFT → detail pane; pages wrap; START → catalogue.
    press(&mut ed, &mut fx, pad::LEFT);
    assert_eq!(ed.state(), state::TRUNK_DETAIL);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::UP)), [0x13]);
    assert_eq!(ed.snapshot().trunk_page, 3);
    press(&mut ed, &mut fx, pad::DOWN);
    assert_eq!(ed.snapshot().trunk_page, 0);
    press(&mut ed, &mut fx, pad::START);
    assert_eq!(ed.state(), state::CATALOGUE);
    wait(&mut ed, &mut fx, 12);
    assert_eq!(ed.snapshot().slide, 0x200);
    // Catalogue cursor: rows 0..11, columns wrap, base clamps at 0x4A.
    press(&mut ed, &mut fx, pad::LEFT);
    assert_eq!(ed.snapshot().catalogue, [0, 9, 0]);
    assert_eq!(ed.catalogue_card(), 9);
    assert!(press(&mut ed, &mut fx, pad::UP).is_empty());
    for _ in 0..12 {
        press(&mut ed, &mut fx, pad::DOWN);
    }
    assert_eq!(ed.snapshot().catalogue, [1, 9, 11]);
    for _ in 0..8 {
        press(&mut ed, &mut fx, pad::R1);
    }
    assert_eq!(ed.snapshot().catalogue[0], CATALOGUE_MAX_BASE);
    assert_eq!(ed.catalogue_card(), 9 + 10 * (0x4A + 11));
    // L2 on an unknown card is refused; on an owned card it pages.
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::L2)), [6]);
    press(&mut ed, &mut fx, pad::L1);
    for _ in 0..7 {
        press(&mut ed, &mut fx, pad::L1);
    }
    assert_eq!(ed.snapshot().catalogue, [0, 9, 11]);
    for _ in 0..11 {
        press(&mut ed, &mut fx, pad::UP);
    }
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.catalogue_card(), 0);
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.catalogue_card(), 1);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::R2)), [0x13]);
    assert_eq!(ed.snapshot().trunk_page, 1);
    assert_eq!(sounds(&press(&mut ed, &mut fx, pad::CROSS)), [4]);
    // ○ ○ ○ back to the list.
    press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ed.state(), state::TRUNK_DETAIL);
    wait(&mut ed, &mut fx, 12);
    press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ed.state(), state::TRUNK_LIST_DETAIL);
    press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ed.state(), state::TRUNK_LIST);
    wait(&mut ed, &mut fx, 12);
    assert_eq!(ed.snapshot().slide, 0);
    // Deck detail pane.
    press(&mut ed, &mut fx, pad::RIGHT);
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.state(), state::DECK_LIST_DETAIL);
    wait(&mut ed, &mut fx, 12);
    assert_eq!(ed.snapshot().slide, -0x100);
    press(&mut ed, &mut fx, pad::RIGHT);
    assert_eq!(ed.state(), state::DECK_DETAIL);
    press(&mut ed, &mut fx, pad::DOWN);
    assert_eq!(ed.snapshot().deck_page, 1);
    press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ed.state(), state::DECK_LIST_DETAIL);
    press(&mut ed, &mut fx, pad::CIRCLE);
    assert_eq!(ed.state(), state::DECK_LIST);
}

#[test]
fn analog_sticks_scroll_the_windows() {
    let mut fx = Fixture::new();
    let mut ed = fx.editor();
    let push = |ed: &mut DeckEditor, fx: &mut Fixture, left: u8, right: u8| {
        ed.step(
            PadInput {
                word: 0,
                sticks: Some(Sticks {
                    right_y: right,
                    left_y: left,
                }),
            },
            &mut fx.env(),
        )
    };
    // Dead zone.
    assert!(push(&mut ed, &mut fx, 0xB0, 0x80).is_empty());
    assert_eq!(stick_rows(0xFF, 18), ((0x7F - 0x30) * 18) >> 7);
    assert_eq!(stick_rows(0x00, 18), ((-0x80 + 0x30) * 18) >> 7);
    assert_eq!(stick_rows(0xB1, 18), 0);
    assert_eq!(stick_rows(0xFF, 6), 3);
    let ev = push(&mut ed, &mut fx, 0xFF, 0x80);
    assert_eq!(sounds(&ev), [3]);
    assert_eq!(ed.snapshot().trunk_window, 11);
    // The right stick scrolls the deck window in states 3..6.
    assert_eq!(sounds(&push(&mut ed, &mut fx, 0x80, 0xFF)), [3]);
    assert_eq!(ed.snapshot().deck_window, 3);
    press(&mut ed, &mut fx, pad::RIGHT);
    push(&mut ed, &mut fx, 0x80, 0xFF);
    assert_eq!(ed.snapshot().deck_window, 6);
    // The left stick still scrolls the trunk in state 4 (states 1..4),
    // not in the detail states 5 and 6.
    assert_eq!(sounds(&push(&mut ed, &mut fx, 0x00, 0x80)), [3]);
    assert_eq!(ed.snapshot().trunk_window, 0);
    press(&mut ed, &mut fx, pad::RIGHT);
    wait(&mut ed, &mut fx, 12);
    assert_eq!(ed.state(), state::DECK_LIST_DETAIL);
    assert!(push(&mut ed, &mut fx, 0xFF, 0x80).is_empty());
    assert_eq!(ed.snapshot().trunk_window, 0);
}

#[test]
fn pad_repeat_follows_the_delay_and_rate() {
    let mut p = PadState::default();
    p.poll(pad::DOWN);
    assert!(p.new_press(pad::DOWN) && p.repeat(pad::DOWN) && p.is_held(pad::DOWN));
    let mut fires = Vec::new();
    for frame in 1..80 {
        p.poll(pad::DOWN);
        assert!(!p.new_press(pad::DOWN));
        if p.repeat(pad::DOWN) {
            fires.push(frame);
        }
    }
    // 0x203180 / 0x203380 [F]: the counter starts on the second held
    // sample, the first repeat is 13 frames after the press, then every
    // frame (rate = 0x21 & 0xF = 1).
    assert_eq!(fires, (13..80).collect::<Vec<_>>());
    // The frame the release is first seen still repeats (the counter
    // comes from the older pair), the next does not.
    let mut q = p.clone();
    q.poll(0);
    assert!(q.repeat(pad::DOWN) && !q.is_held(pad::DOWN));
    q.poll(0);
    assert!(!q.repeat(pad::DOWN));
    p.restart(pad::DOWN);
    assert!(!p.repeat(pad::DOWN));
    p.poll(0);
    assert!(!p.is_held(pad::DOWN));
    p.poll(pad::DOWN);
    assert!(p.new_press(pad::DOWN));
}

#[test]
fn static_cards_are_sliced_from_the_image() {
    let mut image = vec![0u8; elf::NAME_ORDER + 5 * elf::NAME_ORDER_STRIDE];
    image[elf::CARD_TABLE + 20 * 3 + 3] = 42;
    image[elf::PASSWORD_KEY..elf::PASSWORD_KEY + 8].copy_from_slice(b"HANNIBAL");
    image[elf::NAME_ORDER + elf::NAME_ORDER_STRIDE + 2 * 3] = 7;
    let cards = StaticCards::from_executable(&image, 1).unwrap();
    assert_eq!(cards.cost(3), Some(42));
    assert_eq!(cards.name_order[3], 7);
    assert_eq!(cards.password_key, *b"HANNIBAL");
    assert_eq!(cards.record(EMPTY_CARD), None);
    assert_eq!(cards.record(REJECTED_DECK_CARD), None);
    assert_eq!(cards.record(854), None);
    assert!(StaticCards::from_executable(&image[..1000], 0).is_none());
    assert_eq!(
        StaticCards::from_executable(&image, 9).unwrap().name_order[3],
        0
    );
}

#[test]
fn delete_trunk_copy_clears_the_slot() {
    let mut fx = Fixture::new();
    assert!(delete_trunk_copy(&mut fx.block, 41));
    assert_eq!(count_copies(&fx.block, 41, None), 0);
    assert_eq!(
        fx.block.records[41].instances[0],
        CardInstance {
            owner: 0,
            id: 0xC000_0000
        }
    );
    assert!(!delete_trunk_copy(&mut fx.block, 41));
    // The deck copy of card 2 is not a trunk copy.
    assert!(!delete_trunk_copy(&mut fx.block, 2));
    assert_eq!(count_copies(&fx.block, 2, Some(0)), 1);
    assert_eq!(count_copies(&fx.block, 2, Some(3)), 0);
    assert_eq!(find_instance(&fx.block, 100, 0, true), Some(0));
    assert_eq!(find_instance(&fx.block, 100, 0, false), None);
    assert!(seen(&fx.block, 100) && !seen(&fx.block, 101));
    assert!(set_password_used(&mut fx.block, 7, true));
    assert!(password_used(&fx.block, 7));
    assert_eq!(fx.block.stored_rank(7), Some(0));
}

// ---------------------------------------------------------------------------
// Private fixtures: the game's own code driven in the EE interpreter
// (`work/research/modes/claude-deck-editor/tools/fixtures_deck.py`).

mod private {
    use super::*;
    use serde::Deserialize;
    use std::path::PathBuf;

    #[derive(Deserialize)]
    struct File {
        base: Base,
        scenarios: Vec<Scenario>,
    }
    #[derive(Deserialize)]
    struct Base {
        block_hex: String,
        language: u8,
        rng_start: u64,
        analog_present: bool,
        repeat_delay: i32,
        repeat_rate: i32,
    }
    #[derive(Deserialize)]
    struct Frame {
        pad: u16,
        sticks: Option<[u8; 2]>,
    }
    #[derive(Deserialize)]
    struct Scenario {
        name: String,
        ca_start: u8,
        frames: Vec<Frame>,
        log: Vec<(u64, String, u32, u32, u32)>,
        state: State,
        decks: Vec<Deck>,
        current_deck: u8,
        ca_end: u8,
        rng_end: u64,
        digest: Vec<Digest>,
    }
    #[derive(Deserialize)]
    struct State {
        state: u8,
        ret: u8,
        sub: u8,
        cnt: u8,
        pos: i16,
        tgt: i16,
        tcount: u16,
        toff: u16,
        trow: u8,
        dcount: u16,
        doff: u16,
        drow: u8,
        tsort: [u8; 3],
        dsort: [u8; 3],
        view: (u8, u8),
        cat: (u8, u8, u8),
        idle: u16,
        reload: u8,
        trunk: Vec<u16>,
        deckbuf: Vec<u16>,
        pw: String,
        res: [u16; 3],
    }
    #[derive(Deserialize)]
    struct Deck {
        cards: Vec<u16>,
        leader: u16,
        dc: u16,
    }
    #[derive(Deserialize)]
    struct Digest {
        inst: Vec<[u32; 2]>,
        rank: u16,
        exp: u16,
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../work")
    }

    /// Every scenario of the fixture replayed frame by frame: the sound /
    /// viewer / exit log, the editor state, the deck records, the block
    /// digest, `SAVE+0xCA` and the LCG state must match the interpreter.
    #[test]
    #[ignore = "requires the private savestate fixtures and the PAL executable under work"]
    fn scenarios_match_the_interpreter() {
        let root = root();
        let image = std::fs::read(root.join("inspection/SLES_515.79")).unwrap();
        let text = std::fs::read_to_string(
            root.join("research/modes/claude-deck-editor/fixtures/deck-editor-622z0.json"),
        )
        .unwrap();
        let file: File = serde_json::from_str(&text).unwrap();
        let cards = StaticCards::from_executable(&image, file.base.language).unwrap();
        let block_bytes: Vec<u8> = (0..file.base.block_hex.len() / 2)
            .map(|i| u8::from_str_radix(&file.base.block_hex[2 * i..2 * i + 2], 16).unwrap())
            .collect();
        let base_block = CardBlock::from_bytes(&block_bytes).unwrap();
        assert_eq!(cards.find_password(b"l7ure9yt"), Some(3));
        assert_eq!(cards.find_password(b"ub0ha94h"), Some(0));
        assert_eq!(cards.password(2), None);
        for sc in &file.scenarios {
            let mut block = base_block.clone();
            let mut ca = sc.ca_start;
            let mut rng = file.base.rng_start;
            let (ed, log) = {
                let mut env = EditorEnv {
                    block: &mut block,
                    duel_counter: &mut ca,
                    cards: &cards,
                    rng: &mut rng,
                    graphics_busy: false,
                };
                let mut ed = DeckEditor::new(0, &mut env);
                ed.pad.repeat_delay = file.base.repeat_delay;
                ed.pad.repeat_rate = file.base.repeat_rate;
                let mut log = Vec::new();
                for (i, fr) in sc.frames.iter().enumerate() {
                    let sticks = file.base.analog_present.then(|| {
                        let s = fr.sticks.unwrap_or([0x7F, 0x7F]);
                        Sticks {
                            right_y: s[0],
                            left_y: s[1],
                        }
                    });
                    let input = PadInput {
                        word: fr.pad,
                        sticks,
                    };
                    for e in ed.step(input, &mut env) {
                        match e {
                            EditorEvent::Sound(id) => log.push((i as u64, "sfx", u32::from(id))),
                            EditorEvent::Viewer(c) => {
                                log.push((i as u64, "viewer_param", u32::from(c)));
                                log.push((i as u64, "mode_push", 7));
                            }
                            EditorEvent::Exit => log.push((i as u64, "mode_pop", 0)),
                            _ => {}
                        }
                    }
                }
                (ed, log)
            };
            let expected: Vec<(u64, &str, u32)> = sc
                .log
                .iter()
                .map(|(f, tag, a, b, _)| match tag.as_str() {
                    "sfx" => (*f, "sfx", *b),
                    "viewer_param" => (*f, "viewer_param", *a),
                    "mode_push" => (*f, "mode_push", *a),
                    _ => (*f, "mode_pop", 0),
                })
                .collect();
            assert_eq!(log, expected, "{}: event log", sc.name);
            let s = ed.snapshot();
            let x = &sc.state;
            assert_eq!(
                (s.state, s.return_state, s.sub_step, s.counter),
                (x.state, x.ret, x.sub, x.cnt),
                "{}: state",
                sc.name
            );
            assert_eq!(
                (s.slide, s.slide_target),
                (x.pos, x.tgt),
                "{}: slide",
                sc.name
            );
            assert_eq!(
                (s.trunk_count, s.trunk_window, s.trunk_row),
                (x.tcount, x.toff, x.trow),
                "{}: trunk cursor",
                sc.name
            );
            assert_eq!(
                (DECK_LIST_LEN as u16, s.deck_window, s.deck_row),
                (x.dcount, x.doff, x.drow),
                "{}: deck cursor",
                sc.name
            );
            assert_eq!(
                (s.trunk_keys, s.deck_keys),
                (x.tsort, x.dsort),
                "{}: keys",
                sc.name
            );
            assert_eq!((s.trunk_page, s.deck_page), x.view, "{}: pages", sc.name);
            assert_eq!(
                s.catalogue,
                [x.cat.0, x.cat.1, x.cat.2],
                "{}: catalogue",
                sc.name
            );
            // After the pop the interpreter kept running the (stubbed) mode;
            // the model stops, so its idle timer stays where the ○ left it.
            if !ed.finished() {
                assert_eq!(s.idle, x.idle, "{}: idle", sc.name);
            }
            assert_eq!(s.reload_pending, x.reload != 0, "{}: reload", sc.name);
            assert_eq!(s.trunk, x.trunk, "{}: trunk list", sc.name);
            assert_eq!(s.deck_list, x.deckbuf, "{}: deck list", sc.name);
            assert_eq!(
                s.password.to_vec(),
                x.pw.as_bytes(),
                "{}: password",
                sc.name
            );
            assert_eq!(s.results, x.res, "{}: results", sc.name);
            for (d, deck) in sc.decks.iter().enumerate() {
                let rec = deck_record(&block, d as u8);
                assert_eq!(
                    rec.cards.to_vec(),
                    deck.cards,
                    "{}: deck {d} cards",
                    sc.name
                );
                assert_eq!(
                    (rec.leader, rec.cost),
                    (deck.leader, deck.dc),
                    "{}: deck {d}",
                    sc.name
                );
            }
            assert_eq!(
                current_deck(&block),
                sc.current_deck,
                "{}: current deck",
                sc.name
            );
            assert_eq!(ca, sc.ca_end, "{}: SAVE+0xCA", sc.name);
            assert_eq!(rng, sc.rng_end, "{}: rng", sc.name);
            for (c, dg) in sc.digest.iter().enumerate() {
                let r: &CardRecord = &block.records[c];
                let inst: Vec<[u32; 2]> = r.instances.iter().map(|i| [i.owner, i.id]).collect();
                assert_eq!(inst, dg.inst, "{}: card {c} instances", sc.name);
                assert_eq!(
                    (r.rank_word, r.experience),
                    (dg.rank, dg.exp),
                    "{}: card {c}",
                    sc.name
                );
            }
            assert_eq!(block.recent.len(), RECENT_LEN);
        }
    }
}

#[test]
fn a_new_game_deck_is_laid_out_as_the_editor_expects() {
    let cards = cards();
    let mut block = CardBlock::empty(0x55);
    let mut rng = 3u64;
    // One granted instance per deck card (two copies of card 5) plus the
    // leader, as a new game's block would hold them.
    let mut deck: Vec<u16> = (1..=39).collect();
    deck.push(5);
    for &c in deck.iter().chain(std::iter::once(&40)) {
        assert!(block.grant(c, None, &mut rng));
    }
    block.records[40].rank_word |= 1;
    lay_out_new_game_deck(&mut block, &cards, 0, &deck, 40).unwrap();
    let record = deck_record(&block, 0);
    assert_eq!(&record.cards[..], &deck[..]);
    assert_eq!(record.leader, 40);
    assert_eq!(record.count(), 40);
    assert!(record.is_valid());
    let expected: u16 = deck
        .iter()
        .map(|&c| u16::from(cards.cost(c).unwrap()))
        .sum();
    assert_eq!(record.cost, expected);
    assert_eq!(current_deck(&block), 0);
    // The other two decks are empty, not the zeroed tail's "card 0 x40".
    assert_eq!(deck_record(&block, 1), DeckRecord::EMPTY);
    assert_eq!(deck_record(&block, 2), DeckRecord::EMPTY);
    assert!(!deck_is_valid(&block, 1));
    assert_eq!(count_copies(&block, 5, Some(0)), 2);
    assert_eq!(count_copies(&block, 5, Some(TRUNK_CATEGORY)), 0);
    assert!(find_instance(&block, 40, 0, true).is_some());
    assert_eq!(count_copies(&block, 40, None), 1);
    // The editor opens on it with 40 trunk kinds and all 41 list entries.
    let mut counter = 0u8;
    let mut env = EditorEnv {
        block: &mut block,
        duel_counter: &mut counter,
        cards: &cards,
        rng: &mut rng,
        graphics_busy: false,
    };
    let editor = DeckEditor::new(0, &mut env);
    assert_eq!(editor.trunk_list().len(), 40);
    assert!(editor.deck_list().iter().all(|&c| c != EMPTY_CARD));
    // Free slots carry the capture's word 1.
    assert_eq!(block.records[1].instances[1].id, 0xC000_0000);
    // A card without a spare instance is refused.
    let mut short = CardBlock::empty(0x55);
    assert!(short.grant(1, None, &mut rng));
    assert!(lay_out_new_game_deck(&mut short, &cards, 0, &[1, 2], 1).is_err());
}
