# Privacy notice: Roses online server (DRAFT)

> **Draft, not yet in force.** Placeholders in angle brackets must be filled
> in by the server operator before the server is opened to the public. This
> notice covers the community server run by the project; anyone running
> their own `roses-server` is the controller of their own server's data.

## Who is responsible

The controller of the data on the community server is `<OPERATOR NAME>`.
The project has no e-mail contact: privacy questions and requests go
through an issue in this repository (see "Your rights"). The server is
hosted in
`<COUNTRY / HOSTING PROVIDER>`.

## What the game and launcher keep on your PC

The launcher and the game keep everything else locally, beside the launcher
in `roses-data\`: your imported game data, memory-card saves, settings, logs
and, when you sign in, a session token (`online-session.json`). Nothing of this is
sent anywhere except as described below. The launcher does not check for
updates or send usage statistics.

## What the server stores

| Data | Why | How long |
|---|---|---|
| Account name | Identifies you to other players | Until you delete the account |
| Password hash (Argon2id; the password itself is never stored) | Signing in | Until you delete the account or change the password |
| Account creation time, the invite code used (if any), disabled flag | Running the server, stopping abuse | Until you delete the account |
| Session tokens (only a SHA-256 hash of each) | Staying signed in | 30 days after last use, then pruned automatically |
| Match log: both players' names, start and end time, result, number of moves | Showing results, resolving disputes | `<N>` days (pruned with `roses-server accounts prune-matches`) |
| Match replays (decks and moves, no names; linked to the match log by its id) | Investigating bugs and cheating | `<N>` days (`roses-server hub --replay-days N`) |
| Server log: times, account names, match ids, events such as sign-ins | Operating and debugging the server | `<14>` days (journal retention) |

**IP addresses** are used by the server program only in memory, to limit
connections, sign-ups and sign-in attempts per address (time windows of up
to one hour); they are never written to its log or database and are gone
when it restarts. The hosting system's own logs (SSH, firewall, and any reverse
proxy) may record addresses of connections; they are kept for `<14>` days.

Nothing is sold or shared with third parties. There is no tracking, no
advertising and no analytics. Chat is not offered, so no messages are
stored.

## Why (legal basis)

Providing the online service you asked for when you made an account
(contract), and keeping the server secure and free of abuse (legitimate
interest).

## Your rights

You can ask for a copy of your data, for correction, or for erasure, and you
can object to processing. Open an issue in this repository on GitHub
saying what you want and naming your account; do not post your password or
anything else private there. A maintainer replies in the issue and, before
acting, asks you to confirm from inside the account that the request is
yours. Erasure
(`roses-server accounts delete <name>`) removes the account, its sessions
and reset codes, and replaces your name in the match log with "(deleted)";
the replays of your matches are deleted too. Backups of the account database
are kept for `<N>` days and then overwritten, so erased data disappears from
them within that time.

You can also complain to a data-protection authority, for example the one
where you live or `<AUTHORITY OF THE SERVER'S COUNTRY>`.

## Age

The online service is for people aged `<16>` or older (or the minimum age
for consent to online services where you live).

## Changes

Changes to this notice are published in the repository and announced
`<WHERE>`. Last updated: `<DATE>`.
