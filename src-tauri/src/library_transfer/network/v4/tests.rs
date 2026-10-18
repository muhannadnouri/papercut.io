use super::*;
use std::io::Cursor;
use std::net::TcpListener;
use std::sync::Barrier;
use std::thread;

fn peers(mode: Mode) -> (TcpListener, Session, Session) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let l = Session::listener(10, mode, vec![endpoint]).unwrap();
    let c = Session::connector(20, mode, endpoint, &l.pairing_code().unwrap()).unwrap();
    (listener, l, c)
}

fn exchange(
    listener: &TcpListener,
    l: &Session,
    c: &Session,
    server: impl FnOnce(&mut Channel<'_>) + Send,
    client: impl FnOnce(&mut Channel<'_>),
) {
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            let (socket, _) = listener.accept().unwrap();
            socket.set_nonblocking(true).unwrap();
            let mut channel = l.authenticate(socket).unwrap();
            server(&mut channel);
        });
        c.begin_connect().unwrap();
        let mut channel = c
            .authenticate(TcpStream::connect(listener.local_addr().unwrap()).unwrap())
            .unwrap();
        client(&mut channel);
        drop(channel);
        server.join().unwrap();
    });
}

fn offer(bytes: &[u8]) -> Offer {
    hash_archive(&mut &bytes[..], || Ok(())).unwrap()
}
fn progress() -> LibraryTransferProgress {
    LibraryTransferProgress {
        operation: "receive".into(),
        phase: "importing".into(),
        bytes_processed: None,
        bytes_total: None,
        items_processed: None,
        items_total: None,
        item: None,
    }
}

fn sender(channel: &mut Channel<'_>, bytes: &[u8], offset: u64) {
    assert!(channel.receive_offer().is_err());
    channel.send_offer(offer(bytes)).unwrap();
    assert_eq!(channel.wait_ready().unwrap(), offset);
    channel.send_archive(&mut Cursor::new(bytes)).unwrap();
    let mut updates = 0;
    channel
        .await_result(|p| {
            assert_eq!(p.phase, "importing");
            updates += 1;
        })
        .unwrap();
    assert_eq!(updates, 1);
}
fn receiver(channel: &mut Channel<'_>, bytes: &[u8], prefix: usize) {
    assert!(channel.session.admit_import().is_err());
    assert!(channel.send_offer(offer(bytes)).is_err());
    assert!(channel.receive_offer().unwrap() == offer(bytes));
    let mut staging = Cursor::new(bytes[..prefix].to_vec());
    channel.receive_archive(&mut staging, u64::MAX).unwrap();
    assert_eq!(staging.into_inner(), bytes);
    channel.session.admit_import().unwrap();
    assert!(channel.session.admit_import().is_err());
    channel.progress(progress()).unwrap();
    channel.finish_import(Ok(())).unwrap();
    assert!(channel.finish_import(Ok(())).is_err());
}

#[test]
fn real_tls_and_pake_transfer_both_modes_with_empty_partial_and_full_resume() {
    let bytes = vec![0x5a; BUFFER * 2 + 37];
    for mode in [Mode::Push, Mode::Pull] {
        for prefix in [0, 43, bytes.len()] {
            let (listener, l, c) = peers(mode);
            exchange(
                &listener,
                &l,
                &c,
                |channel| {
                    if mode == Mode::Pull {
                        sender(channel, &bytes, prefix as u64)
                    } else {
                        receiver(channel, &bytes, prefix)
                    }
                },
                |channel| {
                    if mode == Mode::Push {
                        sender(channel, &bytes, prefix as u64)
                    } else {
                        receiver(channel, &bytes, prefix)
                    }
                },
            );
            for session in [&l, &c] {
                assert_eq!(session.phase().unwrap(), Phase::Complete);
                let inner = session.lock().unwrap();
                assert!(
                    inner.code.is_none()
                        && inner.listener.is_none()
                        && inner.pinned_id.is_none()
                        && inner.deadline.is_none()
                        && inner.socket.is_none()
                );
            }
        }
    }
}

#[test]
fn full_length_corrupt_partial_never_admits_import_in_either_mode() {
    let bytes = b"immutable archive";
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let run = |channel: &mut Channel<'_>| {
            if channel.session.mode.content(channel.session.side) == Content::Sender {
                channel.send_offer(offer(bytes)).unwrap();
                assert_eq!(channel.wait_ready().unwrap(), bytes.len() as u64);
                let _ = channel.send_archive(&mut Cursor::new(bytes));
                assert!(matches!(
                    channel.await_result(|_| {}),
                    Err(Error::OutcomeUnknown)
                ));
            } else {
                channel.receive_offer().unwrap();
                let mut partial = Cursor::new(vec![b'x'; bytes.len()]);
                assert!(matches!(
                    channel.receive_archive(&mut partial, 0),
                    Err(Error::DigestMismatch)
                ));
                assert!(channel.session.admit_import().is_err());
                assert_eq!(channel.session.phase().unwrap(), Phase::Failed);
            }
        };
        exchange(&listener, &l, &c, run, run);
    }
}

#[test]
fn incomplete_copy_can_resume_with_fresh_tls_and_pake_in_same_generation() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let bytes = vec![7; BUFFER * 2 + 29];
        let partial = Mutex::new(Cursor::new(Vec::new()));
        let interrupted = |channel: &mut Channel<'_>| {
            if channel.session.mode.content(channel.session.side) == Content::Sender {
                channel.send_offer(offer(&bytes)).unwrap();
                channel.wait_ready().unwrap();
                channel.stream.write_all(&bytes[..BUFFER + 23]).unwrap();
                channel.stream.flush().unwrap();
            } else {
                channel.receive_offer().unwrap();
                assert!(matches!(
                    channel.receive_archive(&mut *partial.lock().unwrap(), u64::MAX),
                    Err(Error::Io(_))
                ));
            }
        };
        exchange(&listener, &l, &c, interrupted, interrupted);
        assert_eq!(partial.lock().unwrap().get_ref().len(), BUFFER);
        assert_eq!(l.phase().unwrap(), Phase::WaitingResume);
        assert_eq!(c.phase().unwrap(), Phase::WaitingResume);
        let id = c.lock().unwrap().pinned_id.unwrap();
        let deadline = c.lock().unwrap().deadline;
        let resumed = |channel: &mut Channel<'_>| {
            if channel.session.mode.content(channel.session.side) == Content::Sender {
                sender(channel, &bytes, BUFFER as u64)
            } else {
                channel.receive_offer().unwrap();
                channel
                    .receive_archive(&mut *partial.lock().unwrap(), u64::MAX)
                    .unwrap();
                assert_eq!(partial.lock().unwrap().get_ref(), &bytes);
                channel.session.admit_import().unwrap();
                channel.progress(progress()).unwrap();
                channel.finish_import(Ok(())).unwrap();
            }
        };
        assert_eq!(c.generation, 20);
        // Authenticate again before completing so pins/deadline can be examined while live.
        exchange(
            &listener,
            &l,
            &c,
            |channel| {
                assert_eq!(channel.session.lock().unwrap().pinned_id, Some(id));
                resumed(channel);
            },
            |channel| {
                assert_eq!(channel.session.lock().unwrap().pinned_id, Some(id));
                assert_eq!(channel.session.lock().unwrap().deadline, deadline);
                resumed(channel);
            },
        );
    }
}

#[test]
fn malformed_tls_is_noise_but_complete_wrong_proof_consumes_listener_code() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let deadline = l.lock().unwrap().deadline;
        let code = l.pairing_code().unwrap();
        let mut unrelated = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        unrelated.write_all(b"not a TLS connection").unwrap();
        drop(unrelated);
        assert!(l.authenticate(listener.accept().unwrap().0).is_err());
        assert_eq!(l.phase().unwrap(), Phase::Waiting);
        assert_eq!(l.lock().unwrap().deadline, deadline);
        assert_eq!(l.pairing_code().unwrap(), code);
        c.lock().unwrap().code = Some(
            if code == "23456789ABCD" {
                "3456789ABCDE"
            } else {
                "23456789ABCD"
            }
            .into(),
        );
        thread::scope(|scope| {
            let server = scope.spawn(|| {
                assert!(matches!(
                    l.authenticate(listener.accept().unwrap().0),
                    Err(Error::WrongProof)
                ));
            });
            c.begin_connect().unwrap();
            assert!(c
                .authenticate(TcpStream::connect(listener.local_addr().unwrap()).unwrap())
                .is_err());
            server.join().unwrap();
        });
        assert_eq!(l.phase().unwrap(), Phase::Failed);
        assert!(l.pairing_code().is_err());
        assert!(l.lock().unwrap().listener.is_none());
    }
}

#[test]
fn listener_releases_no_confirmation_for_incorrect_connector_proof() {
    let (listener, l, _) = peers(Mode::Push);
    let code = l.pairing_code().unwrap();
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            assert!(matches!(
                l.authenticate(listener.accept().unwrap().0),
                Err(Error::WrongProof)
            ));
        });
        let mut stream = SecureStream::tls(
            TcpStream::connect(listener.local_addr().unwrap()).unwrap(),
            Side::Connector,
            None,
            Instant::now() + AUTH_TIMEOUT,
        )
        .unwrap();
        let (_, message) = spake2::Spake2::<spake2::Ed25519Group>::start_a(
            &spake2::Password::new(&code),
            &spake2::Identity::new(b"papercut-library-transfer-v4-connector"),
            &spake2::Identity::new(b"papercut-library-transfer-v4-listener"),
        );
        stream.write_all(b"PCLAN004\x01").unwrap();
        stream.write_all(&message).unwrap();
        stream.flush().unwrap();
        stream.read_exact(&mut [0; 58]).unwrap();
        stream.write_all(&[0; 32]).unwrap();
        stream.flush().unwrap();
        assert!(stream.read_exact(&mut [0; 32]).is_err());
        server.join().unwrap();
    });
}

#[test]
fn authenticated_changed_offer_and_listener_identity_cannot_repurpose_resume() {
    let (listener, l, c) = peers(Mode::Push);
    let original = offer(b"original");
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
        },
        |channel| {
            channel.send_offer(original).unwrap();
        },
    );
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            assert!(matches!(
                channel.receive_offer(),
                Err(Error::ChangedPackage)
            ));
            assert!(channel.session.admit_import().is_err());
        },
        |channel| {
            channel
                .write_frame(Frame::Offer(offer(b"substituted package")))
                .unwrap();
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::Failed);

    let (listener, l, c) = peers(Mode::Pull);
    exchange(&listener, &l, &c, |_| {}, |_| {});
    let code = l.pairing_code().unwrap();
    let replacement =
        Session::listener(11, Mode::Pull, vec![listener.local_addr().unwrap()]).unwrap();
    replacement.lock().unwrap().listener.as_mut().unwrap().code = code;
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            let _ = replacement.authenticate(listener.accept().unwrap().0);
        });
        c.begin_connect().unwrap();
        assert!(matches!(
            c.authenticate(TcpStream::connect(listener.local_addr().unwrap()).unwrap()),
            Err(Error::ChangedSession)
        ));
        server.join().unwrap();
    });
    assert_eq!(c.phase().unwrap(), Phase::Failed);
}

#[test]
fn storage_rejection_allows_retry_only_before_import_admission() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            assert!(
                matches!(channel.receive_archive(&mut Cursor::new(Vec::new()),0),Err(Error::Remote(e)) if e.is_retryable())
            );
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            assert!(matches!(channel.wait_ready(),Err(Error::Remote(e)) if e.is_retryable()));
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::WaitingResume);
    assert_eq!(c.phase().unwrap(), Phase::WaitingResume);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            channel
                .receive_archive(&mut Cursor::new(Vec::new()), 100)
                .unwrap();
            channel.session.admit_import().unwrap();
            channel
                .finish_import(Err(LibraryTransferError::insufficient_space(200, 100)))
                .unwrap();
            assert!(channel.session.admit_import().is_err());
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            channel.send_archive(&mut Cursor::new(b"archive")).unwrap();
            assert!(
                matches!(channel.await_result(|_|{}),Err(Error::Remote(e)) if e.is_retryable())
            );
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::Failed);
    assert_eq!(c.phase().unwrap(), Phase::Failed);
    assert!(c.begin_connect().is_err());
}

#[test]
fn background_after_restore_admission_closes_network_but_keeps_local_result() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            channel
                .receive_archive(&mut Cursor::new(Vec::new()), 100)
                .unwrap();
            channel.session.admit_import().unwrap();
            channel.session.cancel().unwrap();
            assert_eq!(channel.session.phase().unwrap(), Phase::FinishingImport);
            assert!(channel.session.lock().unwrap().socket.is_none());
            channel.progress(progress()).unwrap();
            channel.finish_import(Ok(())).unwrap();
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            let _ = channel.send_archive(&mut Cursor::new(b"archive"));
            assert!(matches!(
                channel.await_result(|_| {}),
                Err(Error::OutcomeUnknown)
            ));
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::Complete);
    assert_eq!(c.phase().unwrap(), Phase::OutcomeUnknown);
    assert!(c.begin_connect().is_err());
}

#[test]
fn cancellation_and_restore_admission_have_one_atomic_winner() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            channel
                .receive_archive(&mut Cursor::new(Vec::new()), 100)
                .unwrap();
            let barrier = Barrier::new(2);
            thread::scope(|scope| {
                let cancel = scope.spawn(|| {
                    barrier.wait();
                    channel.session.cancel().unwrap();
                });
                barrier.wait();
                let admitted = channel.session.admit_import();
                cancel.join().unwrap();
                assert_eq!(
                    channel.session.phase().unwrap(),
                    if admitted.is_ok() {
                        Phase::FinishingImport
                    } else {
                        Phase::Cancelled
                    }
                );
            });
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            let _ = channel.send_archive(&mut Cursor::new(b"archive"));
        },
    );
}

#[test]
fn cancellation_interrupts_authentication_and_never_revives_old_generation() {
    let (listener, l, c) = peers(Mode::Pull);
    c.begin_connect().unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_silent, _) = listener.accept().unwrap();
    thread::scope(|scope| {
        let connecting = scope.spawn(|| assert!(c.authenticate(socket).is_err()));
        while c.phase().unwrap() != Phase::Authenticating {
            thread::yield_now();
        }
        let start = Instant::now();
        c.cancel().unwrap();
        connecting.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
    });
    assert_eq!(c.phase().unwrap(), Phase::Cancelled);
    assert!(c.begin_connect().is_err());
    assert!(l.pairing_code().is_ok());
}

#[test]
fn absolute_eligibility_and_authentication_budgets_do_not_renew() {
    let (listener, l, c) = peers(Mode::Push);
    let start = Instant::now();
    assert_eq!(c.begin_connect_at(start).unwrap(), start + IO_TIMEOUT);
    assert!(c.begin_connect_at(start).is_err());
    c.connect_failed().unwrap();
    c.begin_connect_at(start + Duration::from_secs(599))
        .unwrap();
    assert_eq!(c.lock().unwrap().deadline, Some(start + WINDOW));
    c.connect_failed().unwrap();
    assert!(matches!(
        c.begin_connect_at(start + WINDOW),
        Err(Error::Expired)
    ));
    l.lock().unwrap().deadline = Some(start - Duration::from_secs(1));
    assert!(matches!(l.pairing_code(), Err(Error::Expired)));

    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_silent, _) = listener.accept().unwrap();
    let deadline = Instant::now() + Duration::from_millis(100);
    assert!(SecureStream::tls(socket, Side::Connector, None, deadline).is_err());
    assert!(Instant::now().duration_since(deadline) < Duration::from_secs(1));
}

#[test]
fn codec_rejects_out_of_order_oversized_truncated_and_malformed_controls() {
    fn parse(bytes: &[u8], expected: Expected) -> Result<Frame> {
        let (&kind, rest) = bytes
            .split_first()
            .ok_or(Error::Protocol("Missing header"))?;
        codec::read_body(&mut &rest[..], kind, expected)
    }
    let mut bytes = Vec::new();
    codec::write_frame(&mut bytes, &Frame::Offer(offer(b"archive"))).unwrap();
    assert_eq!(&bytes[..5], &[0x10, 0, 0, 0, 40]);
    for n in 0..bytes.len() {
        assert!(parse(&bytes[..n], Expected::Offer).is_err());
    }
    assert!(parse(&bytes, Expected::Result).is_err());
    for size in [0, u64::MAX, 8 * 1024 * 1024 * 1024 + 1] {
        bytes[5..13].copy_from_slice(&size.to_be_bytes());
        assert!(parse(&bytes, Expected::Offer).is_err());
    }
    for header in [
        [0x10, 0, 0, 0, 39],
        [0x10, 255, 255, 255, 255],
        [0x99, 0, 0, 0, 0],
        [0x12, 0, 1, 0, 1],
        [0x12, 0, 0, 0, 0],
    ] {
        assert!(parse(&header, Expected::Result).is_err());
    }
    bytes.clear();
    codec::write_frame(&mut bytes, &Frame::Ready(42)).unwrap();
    assert_eq!(&bytes[..5], &[0x11, 0, 0, 0, 8]);
    assert!(parse(&bytes, Expected::Ready { size: 41 }).is_err());
    assert!(parse(&bytes, Expected::Ready { size: 42 }).is_ok());
    bytes.clear();
    codec::write_frame(&mut bytes, &Frame::Status(ReceiverMessage::Complete)).unwrap();
    assert!(parse(&bytes, Expected::Ready { size: 1 }).is_err());
    for body in [
        &b"\xff"[..],
        b"{",
        br#"{"kind":"unknown"}"#,
        br#"{"kind":"progress","value":{"bytesProcessed":-1}}"#,
    ] {
        let mut bytes = vec![0x12];
        bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
        bytes.extend_from_slice(body);
        assert!(parse(&bytes, Expected::Result).is_err());
    }
    let mut body = br#"{"kind":"complete"}"#.to_vec();
    body.resize(codec::STATUS_LIMIT, b' ');
    let mut bytes = vec![0x12, 0, 1, 0, 0];
    bytes.extend(body);
    assert!(matches!(
        parse(&bytes, Expected::Result),
        Ok(Frame::Status(ReceiverMessage::Complete))
    ));
}

#[test]
fn bounded_hash_checks_cancellation_and_rejects_empty_archives() {
    assert!(hash_archive(&mut &b""[..], || Ok(())).is_err());
    let bytes = vec![5; BUFFER * 3];
    let mut reads = 0;
    assert!(matches!(
        hash_archive(&mut &bytes[..], || {
            reads += 1;
            if reads == 2 {
                Err(Error::Cancelled)
            } else {
                Ok(())
            }
        }),
        Err(Error::Cancelled)
    ));
    assert_eq!(reads, 2);
}

#[test]
fn incompatible_mode_and_reflected_pake_are_rejected_over_tls_without_consuming_code() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, _) = peers(mode);
        let code = l.pairing_code().unwrap();
        let deadline = l.lock().unwrap().deadline;
        let (_, reflected) = spake2::Spake2::<spake2::Ed25519Group>::start_b(
            &spake2::Password::new(&code),
            &spake2::Identity::new(b"a"),
            &spake2::Identity::new(b"b"),
        );
        let mut wrong_side = b"PCLAN004".to_vec();
        wrong_side.push(mode as u8);
        wrong_side.extend(reflected);
        let mut bad_group = b"PCLAN004".to_vec();
        bad_group.push(mode as u8);
        bad_group.push(b'A');
        bad_group.push(2); // Compressed Edwards y=2 has no curve point.
        bad_group.extend([0; 31]);
        for bytes in [
            b"PCLAN003".to_vec(),
            [
                b"PCLAN004".as_slice(),
                &[if mode == Mode::Push { 2 } else { 1 }],
            ]
            .concat(),
            wrong_side,
            bad_group,
        ] {
            thread::scope(|scope| {
                let server = scope.spawn(|| {
                    assert!(l.authenticate(listener.accept().unwrap().0).is_err());
                });
                let mut stream = SecureStream::tls(
                    TcpStream::connect(listener.local_addr().unwrap()).unwrap(),
                    Side::Connector,
                    None,
                    Instant::now() + AUTH_TIMEOUT,
                )
                .unwrap();
                stream.write_all(&bytes).unwrap();
                stream.flush().unwrap();
                assert!(stream.read_exact(&mut [0; 58]).is_err());
                server.join().unwrap();
            });
            assert_eq!(l.phase().unwrap(), Phase::Waiting);
            assert_eq!(l.pairing_code().unwrap(), code);
            assert_eq!(l.lock().unwrap().deadline, deadline);
        }
    }
}

#[test]
fn live_channel_blocks_another_attempt_even_after_recoverable_error() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.session.connection_lost();
            assert_eq!(channel.session.phase().unwrap(), Phase::WaitingResume);
            assert!(channel.session.pairing_code().is_err());
            assert!(channel.session.lock().unwrap().active);
        },
        |channel| {
            channel.session.connection_lost();
            assert!(channel.session.begin_connect().is_err());
            assert!(channel.session.connect_failed().is_err());
        },
    );
    assert!(!l.lock().unwrap().active);
    assert!(!c.lock().unwrap().active);
    exchange(
        &listener,
        &l,
        &c,
        |channel| receiver(channel, b"archive", 0),
        |channel| sender(channel, b"archive", 0),
    );
}

#[test]
fn invalid_post_copy_status_is_unknown_and_never_replays() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let run = |channel: &mut Channel<'_>| {
            if channel.session.mode.content(channel.session.side) == Content::Sender {
                channel.send_offer(offer(b"archive")).unwrap();
                channel.wait_ready().unwrap();
                channel.send_archive(&mut Cursor::new(b"archive")).unwrap();
                assert!(matches!(
                    channel.await_result(|_| {}),
                    Err(Error::OutcomeUnknown)
                ));
                assert_eq!(channel.session.phase().unwrap(), Phase::OutcomeUnknown);
            } else {
                channel.receive_offer().unwrap();
                channel
                    .receive_archive(&mut Cursor::new(Vec::new()), 100)
                    .unwrap();
                channel.session.admit_import().unwrap();
                channel.write_frame(Frame::Ready(0)).unwrap();
                // Import completed locally even though the remote result stream is invalid.
                channel.finish_import(Ok(())).unwrap();
            }
        };
        exchange(&listener, &l, &c, run, run);
    }
}

#[test]
fn failed_progress_forwarding_is_disabled_without_cancelling_import() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            channel
                .receive_archive(&mut Cursor::new(Vec::new()), 100)
                .unwrap();
            channel.session.admit_import().unwrap();
            let mut large = progress();
            large.item = Some("x".repeat(codec::STATUS_LIMIT));
            channel.progress(large).unwrap();
            assert!(!channel.forwarding);
            channel.progress(progress()).unwrap();
            channel.finish_import(Ok(())).unwrap();
            assert_eq!(channel.session.phase().unwrap(), Phase::Complete);
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            channel.send_archive(&mut Cursor::new(b"archive")).unwrap();
            assert!(matches!(
                channel.await_result(|_| panic!("Forwarding retried after its first failure")),
                Err(Error::OutcomeUnknown)
            ));
        },
    );
}

#[test]
fn abandoning_verification_is_terminal_and_cannot_start_another_restore() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| {
            channel.receive_offer().unwrap();
            channel
                .receive_archive(&mut Cursor::new(Vec::new()), 100)
                .unwrap();
        },
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            let _ = channel.send_archive(&mut Cursor::new(b"archive"));
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::Failed);
    assert_eq!(c.phase().unwrap(), Phase::OutcomeUnknown);
}

#[test]
fn both_modes_use_full_tls13_handshakes_and_fresh_matching_exporters_on_resume() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let exports = Mutex::new(Vec::new());
        let tls = l.lock().unwrap().listener.as_ref().unwrap().tls.clone();
        assert_eq!(tls.send_tls13_tickets, 0);
        assert_eq!(tls.max_early_data_size, 0);
        for _ in 0..2 {
            let inspect = |channel: &mut Channel<'_>| {
                let (version, kind, exporter) = match &channel.stream {
                    SecureStream::Connector(s) => (
                        s.conn.protocol_version(),
                        s.conn.handshake_kind(),
                        s.conn
                            .export_keying_material(
                                [0; 32],
                                b"EXPORTER-papercut-library-transfer-v4",
                                None,
                            )
                            .unwrap(),
                    ),
                    SecureStream::Listener(s) => (
                        s.conn.protocol_version(),
                        s.conn.handshake_kind(),
                        s.conn
                            .export_keying_material(
                                [0; 32],
                                b"EXPORTER-papercut-library-transfer-v4",
                                None,
                            )
                            .unwrap(),
                    ),
                };
                assert_eq!(version, Some(rustls::ProtocolVersion::TLSv1_3));
                assert_eq!(kind, Some(rustls::HandshakeKind::Full));
                exports.lock().unwrap().push(exporter);
            };
            exchange(&listener, &l, &c, inspect, inspect);
        }
        let exports = exports.into_inner().unwrap();
        assert_eq!(exports[0], exports[1]);
        assert_eq!(exports[2], exports[3]);
        assert_ne!(exports[0], exports[2]);
    }
}

#[test]
fn trickled_tls_application_bytes_cannot_renew_an_absolute_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            let mut stream = SecureStream::tls(
                listener.accept().unwrap().0,
                Side::Listener,
                Some(server_tls_config().unwrap()),
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
            for _ in 0..6 {
                thread::sleep(Duration::from_millis(40));
                if stream.write_all(&[1]).and_then(|_| stream.flush()).is_err() {
                    break;
                }
            }
        });
        let mut stream = SecureStream::tls(
            TcpStream::connect(listener.local_addr().unwrap()).unwrap(),
            Side::Connector,
            None,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        stream.limits(
            Duration::from_millis(200),
            Some(Instant::now() + Duration::from_millis(100)),
        );
        assert!(stream.read_exact(&mut [0; 6]).is_err());
        server.join().unwrap();
    });
}

#[test]
fn authenticated_transfer_can_outlive_pairing_expiry_but_cannot_expose_code() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let run = |channel: &mut Channel<'_>| {
            channel.session.lock().unwrap().deadline =
                Some(Instant::now() - Duration::from_secs(1));
            assert!(channel.session.pairing_code().is_err());
            assert_eq!(channel.session.phase().unwrap(), Phase::Offer);
            if channel.session.mode.content(channel.session.side) == Content::Sender {
                sender(channel, b"archive", 0)
            } else {
                receiver(channel, b"archive", 0)
            }
        };
        exchange(&listener, &l, &c, run, run);
        assert_eq!(l.phase().unwrap(), Phase::Complete);
        assert_eq!(c.phase().unwrap(), Phase::Complete);
    }
}

#[test]
fn cancellation_wins_over_a_late_buffered_complete_status() {
    let (listener, l, c) = peers(Mode::Push);
    exchange(
        &listener,
        &l,
        &c,
        |channel| receiver(channel, b"archive", 0),
        |channel| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            channel.send_archive(&mut Cursor::new(b"archive")).unwrap();
            let session = channel.session;
            assert!(matches!(
                channel.await_result(|_| session.cancel().unwrap()),
                Err(Error::OutcomeUnknown)
            ));
        },
    );
    assert_eq!(l.phase().unwrap(), Phase::Complete);
    assert_eq!(c.phase().unwrap(), Phase::OutcomeUnknown);
}

#[test]
fn disconnect_at_final_delivery_is_unknown_not_an_automatic_resume() {
    for mode in [Mode::Push, Mode::Pull] {
        let (listener, l, c) = peers(mode);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        let send = move |channel: &mut Channel<'_>| {
            channel.send_offer(offer(b"archive")).unwrap();
            channel.wait_ready().unwrap();
            ready_tx.send(()).unwrap();
            closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            match channel.send_archive(&mut Cursor::new(b"archive")) {
                Ok(()) => assert!(matches!(
                    channel.await_result(|_| {}),
                    Err(Error::OutcomeUnknown)
                )),
                Err(error) => assert!(matches!(error, Error::OutcomeUnknown)),
            }
            assert_eq!(channel.session.phase().unwrap(), Phase::OutcomeUnknown);
        };
        let receive = move |channel: &mut Channel<'_>| {
            channel.receive_offer().unwrap();
            channel.write_frame(Frame::Ready(0)).unwrap();
            ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            channel.session.cancel().unwrap();
            closed_tx.send(()).unwrap();
        };
        if mode == Mode::Push {
            exchange(&listener, &l, &c, receive, send)
        } else {
            exchange(&listener, &l, &c, send, receive)
        }
    }
}
