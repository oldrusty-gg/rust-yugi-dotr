//! The Profile page's jobs against a real hub on loopback (TLS with a
//! pinned self-signed certificate, open sign-up): sign up, the session
//! file the game reads, log out, log in, change the password, plain errors.

use platform_net::profile::{run, Job, Paths};
use platform_net::config::{OnlineConfig, SavedSession, SESSION_FILE};
use roses_server::accounts::{Accounts, PasswordCost};
use roses_server::hub::{Hub, HubAcceptor, HubData, HubSettings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The launcher's Profile paths for `data`, greeting the hub the way the
/// launcher does: with the built-in Roses manifest's `online.account_hello`
/// when the launcher's checkout is at hand (beside this crate in the
/// monorepo, or as the sibling `launcher` repository), else this
/// build's own protocol (`Compat::current("")`, what that manifest holds).
fn launcher_paths(data: &std::path::Path) -> Paths {
    let ours = serde_json::json!({ "compat": roses_net::Compat::current("") });
    let ours = ours.as_object().unwrap().clone();
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifests = [
        crate_dir.join("../roses-launcher/games/roses.json"),
        crate_dir.join("../../../launcher/crates/roses-launcher/games/roses.json"),
    ];
    let hello = match manifests.iter().find(|p| p.is_file()) {
        Some(path) => {
            let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let hello = manifest["online"]["account_hello"].as_object().cloned().expect("roses.json: online.account_hello");
            assert_eq!(hello, ours, "{}: online.account_hello must be this build's Compat::current(\"\")", path.display());
            hello
        }
        None => ours,
    };
    Paths::for_data_root(data).greeting("roses-launcher test", hello)
}

#[test]
fn the_profile_page_signs_up_logs_in_and_out_and_changes_the_password() {
    let root = std::env::temp_dir().join(format!("roses-launcher-profile-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings { name: "Roses".into(), password_cost: PasswordCost::TEST, open_signup: true, ..HubSettings::default() };
        let mut hub = Hub::new(settings, HubData::synthetic(false), Accounts::in_memory().unwrap(), |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    // The package: online.json beside the executables, roses-data below.
    let data = root.join("roses-data");
    std::fs::create_dir_all(&data).unwrap();
    let config = OnlineConfig { server: addr.to_string(), name: Some("Roses".into()), tls: true, pin: Some(pin) };
    std::fs::write(root.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
    let paths = launcher_paths(&data);
    assert_eq!(paths.config, root.join("online.json"));
    assert_eq!(paths.session, data.join(SESSION_FILE));

    let check = run(&paths, Job::Check).unwrap();
    assert_eq!(check.server_name.as_deref(), Some("Roses"));
    assert_eq!(check.invite_required, Some(false));
    assert_eq!(check.signed_in, None);

    let up = run(&paths, Job::SignUp { username: "Yugi".into(), password: "heart of the cards".into(), invite: String::new() }).unwrap();
    assert_eq!(up.signed_in.as_deref(), Some("Yugi"));
    let saved = SavedSession::load(&paths.session, &config.server).expect("the game's session file");
    assert_eq!(saved.username, "Yugi");
    assert_eq!(run(&paths, Job::Check).unwrap().signed_in.as_deref(), Some("Yugi"));

    // A second "player" on the same server finds the name taken.
    let other = root.join("other").join("roses-data");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
    let other = launcher_paths(&other);
    let e = run(&other, Job::SignUp { username: "yugi".into(), password: "blue eyes white".into(), invite: String::new() }).unwrap_err();
    assert_eq!(e, "That name is taken. Choose another one.");

    // Change the password: a wrong current one, then the right one.
    let e = run(&paths, Job::ChangePassword { old_password: "nope nope".into(), new_password: "millennium puzzle".into() }).unwrap_err();
    assert_eq!(e, "Your current password is wrong.");
    let changed = run(&paths, Job::ChangePassword { old_password: "heart of the cards".into(), new_password: "millennium puzzle".into() }).unwrap();
    assert!(changed.message.starts_with("Password changed"));
    assert_eq!(run(&paths, Job::Check).unwrap().signed_in.as_deref(), Some("Yugi"), "this sign-in stays");

    let out = run(&paths, Job::LogOut).unwrap();
    assert_eq!(out.signed_in, None);
    assert!(!paths.session.exists());
    let e = run(&paths, Job::LogIn { username: "Yugi".into(), password: "heart of the cards".into() }).unwrap_err();
    assert_eq!(e, "Wrong name or password.");
    let back = run(&paths, Job::LogIn { username: "yugi".into(), password: "millennium puzzle".into() }).unwrap();
    assert_eq!(back.signed_in.as_deref(), Some("Yugi"));
    assert!(paths.session.exists());

    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
