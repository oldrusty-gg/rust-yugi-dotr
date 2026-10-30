#!/usr/bin/env python3
"""Make a MEMORY CARD slot-2 save for testing two players (Custom Duel 2P, Trade).

Copies a slot-1 save file (the port's virtual card, PAL BESLES-51579) into
<save dir>/memory-card/slot2/BESLES-51579/BESLES-51579, where the game looks
for slot 2 (`memory_card_store::VirtualCard::slot2`; ROSES_SAVE_DIR or the
game import folder's parent).

  python scripts/make-slot2-save.py --save-dir <dir> [--from <file>] [--fresh-ids] [--remove]

--from       the source file (default: <save dir>/memory-card/slot1/BESLES-51579/BESLES-51579)
--fresh-ids  give every card instance of the copy a different 30-bit id (XOR 0x155555), so a
             trade between the two copies is not refused as "same ID at the destination"
             (an exact copy shares every id with slot 1 — the original refuses those moves)
--remove     delete the slot-2 folder again (no card in slot 2)
--grant C:N,...   test aid: add N trunk copies of card C to the slot-2 copy (owner = the block's
             key, ids 0x2000000 + 0x100*C + k); --grant1 does the same to slot 1's file in
             place. A new game's block has its 41 cards in deck 0 and nothing to trade until
             the reels grant trunk cards.

The copy is a copy of the user's own save; nothing from the game is written.
The checksum (0x240D70: XOR of the file's 16-bit words but word 1, ^ 0x4C6B) is rewritten.
"""
import argparse, os, shutil, struct, sys

NAME = 'BESLES-51579'
FILE_BYTES = 0x100F0
BLOCK = 4          # the card block's offset in the file
CARDS, RECORD, SLOTS = 0x356, 0x4C, 9


def checksum(b):
    x = 0
    for i in range(0, len(b), 2):
        if i // 2 != 1:
            x ^= struct.unpack_from('<H', b, i)[0]
    return x ^ 0x4C6B


def grant(data, spec, base):
    key = struct.unpack_from('<I', data, BLOCK + 0xFF98)[0] & 0x7FFFFFFF
    n_added = 0
    for tok in filter(None, spec.split(',')):
        c, n = (int(x, 0) for x in tok.split(':'))
        for k in range(SLOTS):
            at = BLOCK + c * RECORD + 8 * k
            if n and not struct.unpack_from('<I', data, at)[0] & 0x7FFFFFFF:
                struct.pack_into('<II', data, at, key, 0xC0000000 | (base + 0x100 * c + k))
                n -= 1
                n_added += 1
        data[BLOCK + c * RECORD + 0x49] |= 0x40
    struct.pack_into('<H', data, 2, checksum(data))
    return n_added


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--save-dir', required=True)
    ap.add_argument('--from', dest='src')
    ap.add_argument('--fresh-ids', action='store_true')
    ap.add_argument('--remove', action='store_true')
    ap.add_argument('--grant', default='')
    ap.add_argument('--grant1', default='')
    a = ap.parse_args()
    slot2 = os.path.join(a.save_dir, 'memory-card', 'slot2')
    if a.remove:
        shutil.rmtree(slot2, ignore_errors=True)
        print('slot 2 removed:', slot2)
        return
    src = a.src or os.path.join(a.save_dir, 'memory-card', 'slot1', NAME, NAME)
    data = bytearray(open(src, 'rb').read())
    if len(data) != FILE_BYTES:
        sys.exit('%s is %#x bytes, not a save file (%#x)' % (src, len(data), FILE_BYTES))
    if struct.unpack_from('<H', data, 2)[0] != checksum(data):
        sys.exit('%s: the checksum does not match' % src)
    changed = 0
    if a.fresh_ids:
        for c in range(CARDS):
            for k in range(SLOTS):
                at = BLOCK + c * RECORD + 8 * k
                owner, ident = struct.unpack_from('<II', data, at)
                if owner & 0x7FFFFFFF:
                    struct.pack_into('<I', data, at + 4, (ident & 0xC0000000) | ((ident ^ 0x155555) & 0x3FFFFFFF))
                    changed += 1
    if a.grant1:
        one = bytearray(data if not a.src else open(src, 'rb').read())
        print('slot 1: %d trunk copies added' % grant(one, a.grant1, 0x1000000))
        open(src, 'wb').write(one)
        if not a.src:
            data[:] = one
            if a.fresh_ids:
                sys.exit('use --grant1 before --fresh-ids in a separate run')
    struct.pack_into('<H', data, 2, checksum(data))
    if a.grant:
        print('slot 2: %d trunk copies added' % grant(data, a.grant, 0x2000000))
    out_dir = os.path.join(slot2, NAME)
    os.makedirs(out_dir, exist_ok=True)
    marker = os.path.join(slot2, 'UNFORMATTED')
    if os.path.exists(marker):
        os.remove(marker)
    out = os.path.join(out_dir, NAME)
    open(out, 'wb').write(data)
    print('slot 2 save written: %s (from %s%s)' % (out, src, ', %d instance ids changed' % changed if a.fresh_ids else ''))


if __name__ == '__main__':
    main()
