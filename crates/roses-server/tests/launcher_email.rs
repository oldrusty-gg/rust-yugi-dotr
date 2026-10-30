//! The Profile page's email jobs against a real hub on loopback (TLS,
//! open sign-up, mail written to a log file): add an address, verify it
//! with the mailed code, reset a forgotten password by email.

use platform_net::profile::{run, Job, Paths};
use platform_net::config::OnlineConfig;
use platform_net::account::EmailState;
use roses_server::accounts::{Accounts, PasswordCost};
use roses_server::hub::email_hub::EmailArgs;
use roses_server::hub::{Hub, HubAcceptor, HubData, HubSettings};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// The newest mail's code, waiting until `count` mails are in the log.
fn code(mail: &Path, count: usize) -> String {
    let start = Instant::now();
    loop {
        let lines: Vec<serde_json::Value> = std::fs::read_to_string(mail)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if lines.len() >= count {
            let text = lines.last().unwrap()["text"].as_str().unwrap().to_string();
            return text.lines().map(str::trim).find(|l| l.len() == 9 && l.as_bytes()[4] == b'-').unwrap().to_string();
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no mail {count}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_profile_page_verifies_an_address_and_resets_a_password_by_email() {
    let root = std::env::temp_dir().join(format!("roses-launcher-email-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let db = certs.join("accounts.db");
    let mail = certs.join("mail.jsonl");
    let mut email = EmailArgs::default();
    let log = mail.to_string_lossy().into_owned();
    assert!(email.take("--email-log", || Ok(log.clone())).unwrap());
    let email = email.build(&db).unwrap().unwrap();
    let accounts = Accounts::open(&db).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings { name: "Roses".into(), password_cost: PasswordCost::TEST, open_signup: true, ..HubSettings::default() };
        let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |l| eprintln!("hub: {l}")).with_email(email);
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let data = root.join("roses-data");
    std::fs::create_dir_all(&data).unwrap();
    let config = OnlineConfig { server: addr.to_string(), name: Some("Roses".into()), tls: true, pin: Some(pin) };
    std::fs::write(root.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
    let paths = launcher_paths(&data);

    let up = run(&paths, Job::SignUp { username: "Yugi".into(), password: "heart of the cards".into(), invite: String::new() }).unwrap();
    assert_eq!(up.email_offered, Some(true));
    let status = up.email.expect("the account's email state");
    assert_eq!((status.state, status.prompt_email), (EmailState::NoEmail, true));

    let e = run(&paths, Job::SetEmail { email: "not-an-address".into() }).unwrap_err();
    assert_eq!(e, "That is not an email address.");
    let sent = run(&paths, Job::SetEmail { email: "yugi@example.com".into() }).unwrap();
    assert!(sent.message.contains("yugi@example.com"), "{}", sent.message);
    assert_eq!(sent.email.unwrap().state, EmailState::Unverified);
    let wrong = run(&paths, Job::VerifyEmail { code: "ZZZZ-ZZZZ".into() }).unwrap_err();
    assert!(wrong.starts_with("That code is not right"), "{wrong}");
    let verified = run(&paths, Job::VerifyEmail { code: code(&mail, 1) }).unwrap();
    let status = verified.email.unwrap();
    assert_eq!((status.state, status.email.as_deref()), (EmailState::Verified, Some("yugi@example.com")));
    assert_eq!(run(&paths, Job::Check).unwrap().email.unwrap().state, EmailState::Verified);

    // Forgot the password: from a signed-out PC.
    run(&paths, Job::LogOut).unwrap();
    let asked = run(&paths, Job::RequestReset { account: "yugi@example.com".into() }).unwrap();
    assert!(asked.message.starts_with("If that account has a verified email address"));
    let reset = run(&paths, Job::ResetPassword { account: "Yugi".into(), code: code(&mail, 2), new_password: "millennium puzzle".into() }).unwrap();
    assert!(reset.message.starts_with("Your password is reset"));
    let e = run(&paths, Job::LogIn { username: "Yugi".into(), password: "heart of the cards".into() }).unwrap_err();
    assert_eq!(e, "Wrong name or password.");
    assert_eq!(run(&paths, Job::LogIn { username: "Yugi".into(), password: "millennium puzzle".into() }).unwrap().signed_in.as_deref(), Some("Yugi"));

    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
