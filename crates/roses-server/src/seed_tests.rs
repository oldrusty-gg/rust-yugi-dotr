//! `--match-seed` (rehearsals, a video aid) in MODES rooms: the N-player
//! match of a room uses the hub's fixed seed, so a rehearsal's room repeats
//! an in-process seed search.

use crate::friends_tests::{until, yugi_compat, Running};
use crate::hub::HubSettings;
use crate::multi::MultiReplay;
use crate::play::{play_bot, PlayOptions};
use roses_net::lobby::{LobbyRequest, RoomMode, RoomSettings};
use roses_net::{synthetic, Join};
use std::time::Duration;

/// One FFA room (a player and three computers) on a hub with
/// `--match-seed 77` played to its end; the replay the hub wrote.
fn fixed_seed_room(dir: &std::path::Path) -> MultiReplay {
    let settings = HubSettings {
        multi_computer_delay_ms: 0,
        fixed_seed: Some(77),
        replay_dir: Some(dir.to_path_buf()),
        ..HubSettings::default()
    };
    let (hub, invite) = Running::with_settings(true, 2, settings, None);
    let mut yugi = hub.lobby();
    yugi.send(LobbyRequest::SignUp { username: "Yugi".into(), password: "Yugi password".into(), invite: invite.clone() });
    until(&mut yugi, "sign-up", |c| c.user.is_some());
    let room = RoomSettings { mode: RoomMode::FreeForAll, turn_seconds: 0, ..RoomSettings::default() };
    yugi.send(LobbyRequest::CreateModeRoom { settings: room });
    until(&mut yugi, "room", |c| c.mode_room.is_some());
    yugi.send(LobbyRequest::StartModeRoom);
    until(&mut yugi, "ready", |c| c.multi_ready.is_some());
    let ready = yugi.multi_ready.clone().unwrap();
    assert_eq!(ready.seat, 0, "the host keeps seat 0");
    let options = PlayOptions {
        join: Join::Ticket { ticket: ready.ticket },
        deck: Some(synthetic::submissions()[0].clone()),
        timeout: Duration::from_secs(240),
        ..PlayOptions::default()
    };
    let report = play_bot(hub.match_connector(), &yugi_compat(), &synthetic::catalogue(), options, |_| {}).unwrap();
    assert!(report.state.ended.is_some());
    until(&mut yugi, "result", |c| c.over.is_some());
    hub.stop();
    let file = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("multi-"))
        .expect("the N-player replay");
    serde_json::from_slice(&std::fs::read(file.path()).unwrap()).unwrap()
}

#[test]
fn a_mode_room_on_a_hub_with_a_match_seed_uses_it() {
    let base = std::env::temp_dir().join(format!("roses-seed-test-{}", crate::random_u64()));
    let runs: Vec<MultiReplay> = (0..2)
        .map(|i| {
            let dir = base.join(i.to_string());
            std::fs::create_dir_all(&dir).unwrap();
            fixed_seed_room(&dir)
        })
        .collect();
    let _ = std::fs::remove_dir_all(&base);
    let setup = runs[0].setup.clone().expect("started");
    assert_eq!(setup.seed, 77);
    assert_eq!(setup.first, (77 % 4) as u8);
    // The same seed and the same plays: the same match.
    assert_eq!(runs[1].setup, runs[0].setup);
    assert_eq!(runs[1].final_state, runs[0].final_state);
}
