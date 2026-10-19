use super::*;
use crate::translation::engine::{TranslationSegmentContext, TranslationSegmentInput};
use std::thread;

fn input() -> TranslationBatchInput {
    TranslationBatchInput {
        model_id: "bergamot-worker-prototype".into(),
        source_language: "fr".into(),
        target_language: "en".into(),
        quality_mode: "balanced".into(),
        repair_mode: Default::default(),
        glossary: vec![],
        segments: vec![TranslationSegmentInput {
            markup: None,
            id: "b0:s0".into(),
            text: "Résumé 🙂".into(),
            context: TranslationSegmentContext::default(),
        }],
    }
}
fn setup() -> (
    Arc<WorkerBridge>,
    WorkerEngine,
    mpsc::Receiver<WorkerRequest>,
) {
    let bridge = Arc::new(WorkerBridge::default());
    let (tx, rx) = mpsc::channel();
    let engine = bridge
        .open(
            "main".into(),
            "job".into(),
            Arc::default(),
            move |message| tx.send(message).map_err(|_| "closed".into()),
        )
        .unwrap();
    (bridge, engine, rx)
}
fn batch(rx: &mpsc::Receiver<WorkerRequest>) -> (String, u32) {
    match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
        WorkerRequest::Batch {
            attempt_id,
            batch_id,
            segments,
            ..
        } => {
            assert_eq!(segments[0].text, "Résumé 🙂");
            (attempt_id, batch_id)
        }
        _ => panic!("expected batch"),
    }
}
fn result(id: &str, batch: u32, source: &str) -> WorkerReply {
    WorkerReply::Result {
        attempt_id: id.into(),
        batch_id: batch,
        outputs: vec![TranslationSegmentOutput {
            markup: None,
            id: "b0:s0".into(),
            source_text: source.into(),
            text: "Summary 🙂".into(),
        }],
    }
}

#[test]
fn sequential_batches_reject_other_windows_stale_ids_and_duplicate_replies() {
    let (bridge, mut engine, rx) = setup();
    assert!(bridge
        .open("main".into(), "other".into(), Arc::default(), |_| Ok(()))
        .is_err());
    let worker = thread::spawn(move || {
        let one = engine.translate_batch(input());
        let two = engine.translate_batch(input());
        (one, two)
    });
    let (id, first) = batch(&rx);
    assert!(!bridge
        .reply("other-window", result(&id, first, "Résumé 🙂"))
        .unwrap());
    assert!(!bridge
        .reply("main", result("old-attempt", first, "Résumé 🙂"))
        .unwrap());
    assert!(bridge
        .reply("main", result(&id, first, "Résumé 🙂"))
        .unwrap());
    assert!(!bridge
        .reply("main", result(&id, first, "Résumé 🙂"))
        .unwrap());
    let (same_id, second) = batch(&rx);
    assert_eq!(id, same_id);
    assert_eq!(second, first + 1);
    assert!(!bridge
        .reply("main", result(&id, first, "Résumé 🙂"))
        .unwrap());
    assert!(bridge
        .reply("main", result(&id, second, "Résumé 🙂"))
        .unwrap());
    let (one, two) = worker.join().unwrap();
    assert_eq!(one.unwrap()[0].text, "Summary 🙂");
    assert!(two.is_ok());
    assert!(matches!(rx.recv().unwrap(), WorkerRequest::Stop { .. }));
    assert!(bridge
        .open("main".into(), "new-job".into(), Arc::default(), |_| Ok(()))
        .is_ok());
}

#[test]
fn cancellation_wins_over_a_simultaneously_delivered_result_and_closes_attempt() {
    let (bridge, mut engine, rx) = setup();
    let cancelled = engine.cancelled.clone();
    let worker = thread::spawn(move || engine.translate_batch(input()));
    let (id, sequence) = batch(&rx);
    cancelled.lock().unwrap().insert("job".into());
    let _ = bridge.reply("main", result(&id, sequence, "Résumé 🙂"));
    assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
    assert!(!bridge
        .reply("main", result(&id, sequence, "Résumé 🙂"))
        .unwrap());
    assert!(bridge
        .open("main".into(), "new-job".into(), Arc::default(), |_| Ok(()))
        .is_ok());
}

#[test]
fn absolute_timeout_and_failed_transport_release_the_slot_without_reusing_nonce() {
    let (bridge, mut engine, rx) = setup();
    let old = engine.id.clone();
    engine.timeout = Duration::from_millis(20);
    let worker = thread::spawn(move || engine.translate_batch(input()));
    let (_, sequence) = batch(&rx);
    assert!(!bridge
        .reply("main", result("stale-attempt", sequence, "Résumé 🙂"))
        .unwrap());
    assert!(worker.join().unwrap().unwrap_err().contains("timed out"));
    assert!(matches!(rx.recv().unwrap(), WorkerRequest::Stop { .. }));
    assert!(!bridge.reply("main", result(&old, 1, "Résumé 🙂")).unwrap());
    let mut new = bridge
        .open("main".into(), "next".into(), Arc::default(), |_| {
            Err("lost channel".into())
        })
        .unwrap();
    assert_ne!(old, new.id);
    assert!(new
        .translate_batch(input())
        .unwrap_err()
        .contains("lost channel"));
    assert!(new.closed);
}

#[test]
fn worker_loss_bad_identity_and_oversize_replies_never_become_engine_outputs() {
    for mode in ["lost", "source", "size", "empty", "count"] {
        let (bridge, mut engine, rx) = setup();
        let worker = thread::spawn(move || engine.translate_batch(input()));
        let (id, sequence) = batch(&rx);
        let mut reply = result(
            &id,
            sequence,
            if mode == "source" {
                "stale"
            } else {
                "Résumé 🙂"
            },
        );
        if mode == "lost" {
            reply = WorkerReply::Failed {
                attempt_id: id,
                batch_id: sequence,
                reason: Failure::WorkerLost,
            };
        } else if let WorkerReply::Result { outputs, .. } = &mut reply {
            if mode == "size" {
                outputs[0].text = "x".repeat(MAX_OUTPUT_BYTES + 1);
            }
            if mode == "empty" {
                outputs[0].text = " \n".into();
            }
            if mode == "count" {
                outputs.clear();
            }
        }
        assert!(bridge.reply("main", reply).unwrap());
        assert!(worker.join().unwrap().is_err(), "{mode}");
        assert!(bridge
            .open("main".into(), "retry".into(), Arc::default(), |_| Ok(()))
            .is_ok());
    }
}

#[test]
fn invalid_or_unbounded_inputs_fail_before_dispatch() {
    for mode in ["empty", "chars", "count", "duplicate", "pair", "id"] {
        let (_, mut engine, rx) = setup();
        let mut request = input();
        match mode {
            "empty" => request.segments.clear(),
            "chars" => request.segments[0].text = "🙂".repeat(901),
            "count" => request.segments = vec![request.segments[0].clone(); 17],
            "duplicate" => request.segments.push(request.segments[0].clone()),
            "pair" => request.source_language = "ar".into(),
            "id" => request.segments[0].id = "x".repeat(129),
            _ => unreachable!(),
        }
        assert!(engine.translate_batch(request).is_err(), "{mode}");
        assert!(matches!(rx.recv().unwrap(), WorkerRequest::Stop { .. }));
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn strict_wire_shape_and_public_error_codes_do_not_accept_runtime_details() {
    let reply = r#"{"kind":"failed","attemptId":"nonce","batchId":1,"reason":"workerLost"}"#;
    assert!(serde_json::from_str::<WorkerReply>(reply).is_ok());
    assert!(
        serde_json::from_str::<WorkerReply>(&reply.replace("workerLost", "secret/path/stack"))
            .is_err()
    );
    assert!(
        serde_json::from_str::<WorkerReply>(&reply.replace("\"batchId\":1", "\"batchId\":-1"))
            .is_err()
    );
    assert!(serde_json::from_str::<WorkerReply>(
        &reply.replace("\"reason\":", "\"path\":\"private\",\"reason\":")
    )
    .is_err());
}

#[test]
fn real_command_and_tauri_channel_round_trip_keeps_job_claims_and_camel_case_binding() {
    use crate::translation::{commands, state::TranslationState};
    use tauri::{ipc::Channel, Manager};
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier =
        format!("io.papercut.bergamot-command-test-{}", std::process::id());
    let app = tauri::test::mock_builder()
        .manage(TranslationState::default())
        .build(context)
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .data_directory(std::env::temp_dir())
        .build()
        .unwrap();
    let handle = app.handle().clone();
    let channel = Channel::<WorkerRequest>::new(move |body| {
        let wire: serde_json::Value = body.deserialize()?;
        if wire["kind"] == "batch" {
            assert_eq!(wire["batchId"], 1);
            assert_eq!(wire["sourceLanguage"], "fr");
            assert_eq!(wire["segments"][0]["text"], "Résumé 🙂");
            let reply = result(wire["attemptId"].as_str().unwrap(), 1, "Résumé 🙂");
            assert!(commands::translation_bergamot_reply(
                handle.state(),
                handle.get_webview_window("main").unwrap(),
                reply
            )
            .unwrap());
        }
        Ok(())
    });
    let reply = tauri::async_runtime::block_on(commands::translation_bergamot_batch(
        app.state(),
        window,
        BatchRequest {
            job_id: "round-trip".into(),
            source_language: "fr".into(),
            target_language: "en".into(),
            segments: vec![Segment {
                html: false,
                id: "b0:s0".into(),
                text: "Résumé 🙂".into(),
            }],
        },
        channel,
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&reply).unwrap()[0]["sourceText"],
        "Résumé 🙂"
    );
    assert_eq!(reply[0].text, "Summary 🙂");
    let state = app.state::<TranslationState>();
    assert!(state
        .claim_job("bergamot-worker-prototype", "bergamot-worker-prototype")
        .is_ok());
}

#[test]
fn save_checkpoint_requires_its_own_owner_nonce_and_confirmation_shape() {
    for valid in [true, false] {
        let (bridge, mut engine, rx) = setup();
        let worker = thread::spawn(move || engine.before_save());
        let WorkerRequest::Confirm {
            attempt_id,
            batch_id,
        } = rx.recv().unwrap()
        else {
            panic!("expected save checkpoint")
        };
        assert_eq!(batch_id, 1); // All-cache jobs still contact their live owner.
        assert!(!bridge
            .reply(
                "other",
                WorkerReply::Confirmed {
                    attempt_id: attempt_id.clone(),
                    batch_id
                }
            )
            .unwrap());
        assert!(!bridge
            .reply(
                "main",
                WorkerReply::Confirmed {
                    attempt_id: "stale".into(),
                    batch_id
                }
            )
            .unwrap());
        assert!(!bridge
            .reply(
                "main",
                WorkerReply::Confirmed {
                    attempt_id: attempt_id.clone(),
                    batch_id: 0
                }
            )
            .unwrap());
        let reply = if valid {
            WorkerReply::Confirmed {
                attempt_id,
                batch_id,
            }
        } else {
            result(&attempt_id, batch_id, "Résumé 🙂")
        };
        assert!(bridge.reply("main", reply).unwrap());
        assert_eq!(worker.join().unwrap().is_ok(), valid);
        assert!(matches!(rx.recv().unwrap(), WorkerRequest::Stop { .. }));
    }
}

#[test]
fn protected_worker_transports_only_opaque_codes_and_rebinds_source_after_validation() {
    let (bridge, mut engine, rx) = setup();
    let dom = crate::translation::dom::DomPlan::new(
        "<p>Gardez <code>private_id</code> et <strong>or</strong>.</p>",
    );
    let source = dom.blocks[0].inline.text.clone();
    let mapping = crate::translation::inline_markup::protected::plan_fragment(
        &dom.blocks[0].node,
        0..source.chars().count(),
        &source,
    )
    .unwrap();
    let mut input = input();
    input.segments[0].text = source.clone();
    input.segments[0].markup = Some(mapping);
    let work = thread::spawn(move || engine.translate_batch(input));
    let WorkerRequest::Batch {
        attempt_id,
        batch_id,
        segments,
        ..
    } = rx.recv().unwrap()
    else {
        panic!("expected batch")
    };
    assert!(segments[0].html);
    assert!(!segments[0].text.contains("private_id"));
    assert!(!segments[0].text.contains("<code>"));
    let target =
        "Keep <span data-pc='0' translate='no'>PC0</span> and <span data-pc='1'>gold</span>.";
    bridge
        .reply(
            "main",
            WorkerReply::Result {
                attempt_id,
                batch_id,
                outputs: vec![TranslationSegmentOutput {
                    id: segments[0].id.clone(),
                    source_text: segments[0].text.clone(),
                    text: target.into(),
                    markup: None,
                }],
            },
        )
        .unwrap();
    let output = work.join().unwrap().unwrap().remove(0);
    assert_eq!(output.source_text, source);
    assert_eq!(output.text, "Keep private_id and gold.");
    assert_eq!(output.markup.as_deref(), Some(target));
}
