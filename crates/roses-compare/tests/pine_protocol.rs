use roses_compare::pine::{EmulatorStatus, MemoryWatch, PineClient, PineError, ReadWidth};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

fn server(handler: impl FnOnce(TcpStream) + Send + 'static) -> (PineClient, JoinHandle<()>) {
    server_timeout(Duration::from_secs(2), handler)
}

fn server_timeout(
    timeout: Duration,
    handler: impl FnOnce(TcpStream) + Send + 'static,
) -> (PineClient, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        handler(stream);
    });
    (PineClient::connect_local(port, timeout).unwrap(), handle)
}

fn request(stream: &mut TcpStream, expected: &[u8]) {
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    assert_eq!(u32::from_le_bytes(length) as usize, 4 + expected.len());
    let mut body = vec![0; expected.len()];
    stream.read_exact(&mut body).unwrap();
    assert_eq!(body, expected);
}

fn response(stream: &mut TcpStream, body: &[u8]) {
    let mut frame = ((5 + body.len()) as u32).to_le_bytes().to_vec();
    frame.push(0);
    frame.extend_from_slice(body);
    // Separate writes exercise framing without assuming read packet boundaries.
    for chunk in frame.chunks(2) {
        stream.write_all(chunk).unwrap();
    }
}

fn string(value: &str) -> Vec<u8> {
    let mut body = ((value.len() + 1) as u32).to_le_bytes().to_vec();
    body.extend_from_slice(value.as_bytes());
    body.push(0);
    body
}

fn watch(width: ReadWidth) -> MemoryWatch {
    MemoryWatch {
        name: format!("synthetic_{}", u8::from(width)),
        address: 0x1234,
        width,
    }
}

#[test]
fn identifies_version_status_and_game_with_exact_opcodes() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[8, 15]);
        let mut body = string("PCSX2 synthetic-test");
        body.extend_from_slice(&1u32.to_le_bytes());
        response(&mut stream, &body);
        request(&mut stream, &[11, 12, 13, 14]);
        let body: Vec<_> = ["Synthetic Game", "TEST-00000", "12345678", "0.0"]
            .into_iter()
            .flat_map(string)
            .collect();
        response(&mut stream, &body);
    });
    let info = client.identify().unwrap();
    assert_eq!(info.emulator_version, "PCSX2 synthetic-test");
    assert_eq!(info.status, EmulatorStatus::Paused);
    let game = info.game.unwrap();
    assert_eq!(game.serial, "TEST-00000");
    assert_eq!(game.disc_crc, "12345678");
    handle.join().unwrap();
}

#[test]
fn shutdown_does_not_request_unavailable_game_identity() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[8, 15]);
        let mut body = string("PCSX2 synthetic-test");
        body.extend_from_slice(&2u32.to_le_bytes());
        response(&mut stream, &body);
    });
    let info = client.identify().unwrap();
    assert_eq!(info.status, EmulatorStatus::Shutdown);
    assert!(info.game.is_none());
    handle.join().unwrap();
}

#[test]
fn reads_all_widths_little_endian_with_paused_checks() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[15]);
        response(&mut stream, &1u32.to_le_bytes());
        request(
            &mut stream,
            &[
                0, 0x34, 0x12, 0, 0, 1, 0x34, 0x12, 0, 0, 2, 0x34, 0x12, 0, 0, 3, 0x34, 0x12, 0, 0,
            ],
        );
        response(
            &mut stream,
            &[
                0x12, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02,
                0x01,
            ],
        );
        request(&mut stream, &[15]);
        response(&mut stream, &1u32.to_le_bytes());
    });
    let watches: Vec<_> = [
        ReadWidth::Bits8,
        ReadWidth::Bits16,
        ReadWidth::Bits32,
        ReadWidth::Bits64,
    ]
    .into_iter()
    .map(watch)
    .collect();
    let capture = client.read_watches(&watches).unwrap();
    assert!(!capture.atomic);
    assert_eq!(capture.status_before, EmulatorStatus::Paused);
    assert_eq!(capture.status_after, EmulatorStatus::Paused);
    let values: Vec<_> = capture
        .observations
        .iter()
        .map(|value| value.value)
        .collect();
    assert_eq!(values, [0x12, 0x1234, 0x12345678, 0x0102030405060708]);
    handle.join().unwrap();
}

#[test]
fn running_emulator_never_receives_memory_read() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[15]);
        response(&mut stream, &0u32.to_le_bytes());
    });
    assert!(matches!(
        client.read_watches(&[watch(ReadWidth::Bits8)]),
        Err(PineError::NotPaused(EmulatorStatus::Running))
    ));
    handle.join().unwrap();
}

#[test]
fn resuming_during_capture_does_not_return_success() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[15]);
        response(&mut stream, &1u32.to_le_bytes());
        request(&mut stream, &[0, 0x34, 0x12, 0, 0]);
        response(&mut stream, &[42]);
        request(&mut stream, &[15]);
        response(&mut stream, &0u32.to_le_bytes());
    });
    assert!(matches!(
        client.read_watches(&[watch(ReadWidth::Bits8)]),
        Err(PineError::NotPaused(EmulatorStatus::Running))
    ));
    handle.join().unwrap();
}

#[test]
fn invalid_watch_lists_fail_before_any_network_request() {
    let (mut client, handle) = server(|mut stream| {
        // The only legitimate request in this test is the final status query.
        request(&mut stream, &[15]);
        response(&mut stream, &2u32.to_le_bytes());
    });
    assert!(matches!(
        client.read_watches(&[]),
        Err(PineError::InvalidRequest(_))
    ));
    let duplicate = watch(ReadWidth::Bits32);
    assert!(matches!(
        client.read_watches(&[duplicate.clone(), duplicate]),
        Err(PineError::InvalidRequest(_))
    ));
    for address in [0x01ff_ffff, 0x0200_0000, 0xffff_ffff] {
        let mut invalid = watch(ReadWidth::Bits64);
        invalid.address = address;
        assert!(matches!(
            client.read_watches(&[invalid]),
            Err(PineError::InvalidRequest(_))
        ));
    }
    assert_eq!(client.status().unwrap(), EmulatorStatus::Shutdown);
    handle.join().unwrap();
}

#[test]
fn explicit_failure_and_invalid_status_poison_connection() {
    for (frame, rejected) in [(vec![5, 0, 0, 0, 255], true), (vec![5, 0, 0, 0, 4], false)] {
        let (mut client, handle) = server(move |mut stream| {
            request(&mut stream, &[15]);
            stream.write_all(&frame).unwrap();
        });
        let error = client.status().unwrap_err();
        assert_eq!(matches!(error, PineError::Rejected), rejected);
        assert!(matches!(client.status(), Err(PineError::Disconnected)));
        handle.join().unwrap();
    }
}

#[test]
fn rejects_invalid_frame_lengths_before_allocating_payload() {
    for length in [0u32, 4, 450_001, u32::MAX] {
        let (mut client, handle) = server(move |mut stream| {
            request(&mut stream, &[15]);
            stream.write_all(&length.to_le_bytes()).unwrap();
        });
        assert!(matches!(client.status(), Err(PineError::Protocol(_))));
        handle.join().unwrap();
    }
}

#[test]
fn rejects_unknown_status_trailing_bytes_and_truncated_payload() {
    for body in [vec![99, 0, 0, 0], vec![1, 0, 0, 0, 9], vec![1, 0]] {
        let (mut client, handle) = server(move |mut stream| {
            request(&mut stream, &[15]);
            response(&mut stream, &body);
        });
        assert!(matches!(client.status(), Err(PineError::Protocol(_))));
        handle.join().unwrap();
    }
}

#[test]
fn rejects_malformed_strings_without_panicking() {
    for body in [
        vec![0, 0, 0, 0],
        vec![2, 0, 0, 0, b'a', b'b'],
        vec![2, 0, 0, 0, 255, 0],
        vec![3, 0, 0, 0, 0, b'a', 0],
        vec![255, 255, 255, 255],
    ] {
        let (mut client, handle) = server(move |mut stream| {
            request(&mut stream, &[8, 15]);
            response(&mut stream, &body);
        });
        assert!(matches!(client.identify(), Err(PineError::Protocol(_))));
        handle.join().unwrap();
    }
}

#[test]
fn premature_eof_is_transport_error() {
    let (mut client, handle) = server(|mut stream| {
        request(&mut stream, &[15]);
        stream.write_all(&[9, 0, 0, 0, 0, 1]).unwrap();
    });
    assert!(matches!(client.status(), Err(PineError::Io(_))));
    handle.join().unwrap();
}

#[test]
fn silent_peer_times_out() {
    let (mut client, handle) = server_timeout(Duration::from_millis(30), |mut stream| {
        request(&mut stream, &[15]);
        thread::sleep(Duration::from_millis(100));
    });
    assert!(matches!(client.status(), Err(PineError::Io(_))));
    handle.join().unwrap();
}

#[test]
fn width_json_is_numeric_and_rejects_unsupported_sizes() {
    let value = serde_json::to_value(watch(ReadWidth::Bits32)).unwrap();
    assert_eq!(value["width"], 32);
    assert!(serde_json::from_str::<MemoryWatch>(r#"{"name":"x","address":0,"width":24}"#).is_err());
}
