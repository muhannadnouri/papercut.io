//! Synthetic boundary checks run without a shared library; explicit external tests exercise actual ORT.
use super::*;
use ort::value::{Shape, SymbolicDimensions};

fn tensor(ty: TensorElementType, shape: &[i64]) -> ValueType {
    ValueType::Tensor {
        ty,
        shape: Shape::new(shape.iter().copied()),
        dimension_symbols: SymbolicDimensions::new(vec![String::new(); shape.len()]),
    }
}

#[test]
fn graph_signature_rejects_wrong_names_types_ranks_widths_and_extra_inputs() {
    let input = tensor(TensorElementType::Int64, &[-1, -1]);
    let output = tensor(TensorElementType::Float32, &[-1, -1, WIDTH as i64]);
    let inputs = [("input_ids", &input), ("attention_mask", &input)];
    assert_eq!(
        check_signature(&inputs, &[("last_hidden_state", &output)]),
        Ok(())
    );
    for (name, ty, shape) in [
        ("invented", TensorElementType::Float32, vec![-1, -1, 768]),
        (
            "last_hidden_state",
            TensorElementType::Int64,
            vec![-1, -1, 768],
        ),
        (
            "last_hidden_state",
            TensorElementType::Float32,
            vec![-1, 768],
        ),
        (
            "last_hidden_state",
            TensorElementType::Float32,
            vec![-1, -1, 512],
        ),
    ] {
        let bad = tensor(ty, &shape);
        assert_eq!(
            check_signature(&inputs, &[(name, &bad)]),
            Err(SessionError::Graph)
        );
    }
    for (name, ty, shape) in [
        ("tokens", TensorElementType::Int64, vec![-1, -1]),
        ("input_ids", TensorElementType::Int32, vec![-1, -1]),
        ("input_ids", TensorElementType::Int64, vec![-1]),
    ] {
        let bad = tensor(ty, &shape);
        assert_eq!(
            check_signature(
                &[(name, &bad), inputs[1]],
                &[("last_hidden_state", &output)]
            ),
            Err(SessionError::Graph)
        );
    }
    assert!(check_signature(&[], &[]).is_err());
    assert!(check_signature(
        &[inputs[0], inputs[1], inputs[0]],
        &[("last_hidden_state", &output)]
    )
    .is_err());
}

#[test]
fn result_bounds_and_non_finite_values_reject_before_copying() {
    let values = vec![0.5; 3 * WIDTH];
    assert_eq!(validated_values(&[1, 3, 768], &values, 3).unwrap(), values);
    for shape in [
        vec![1, 3, 767],
        vec![2, 3, 768],
        vec![1, 4, 768],
        vec![3, 768],
        vec![-1, 3, 768],
    ] {
        assert_eq!(
            validated_values(&shape, &values, 3),
            Err(SessionError::Output)
        );
    }
    assert!(validated_values(&[1, 3, 768], &values[..values.len() - 1], 3).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bad = values.clone();
        bad[WIDTH] = value;
        assert_eq!(
            validated_values(&[1, 3, 768], &bad, 3),
            Err(SessionError::Output)
        );
    }
    for n in [0, 2, MAX_TOKENS + 1, usize::MAX] {
        assert_eq!(
            validated_values(&[1, 3, 768], &values, n),
            Err(SessionError::Output)
        );
    }
}

#[test]
fn cancellation_and_failed_loads_release_the_existing_model_guard_without_runtime() {
    let state = TranslationState::default();
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let missing = std::env::temp_dir().join(format!("papercut-missing-alignment-{nonce:x?}"));
    let cancelled = Cancellation::default();
    cancelled.cancel().unwrap();
    assert_eq!(
        AlignmentSession::load(&state, &missing, &cancelled).err(),
        Some(SessionError::Cancelled)
    );
    let claim = state.claim_model_read(assets::MODEL_KEY).unwrap();
    assert_eq!(
        AlignmentSession::load(&state, &missing, &Cancellation::default()).err(),
        Some(SessionError::Busy)
    );
    drop(claim);
    assert!(matches!(
        AlignmentSession::load(&state, &missing, &Cancellation::default()).err(),
        Some(SessionError::Admission(_))
    ));
    assert!(state.claim_model_removal(assets::MODEL_KEY).is_ok());
    assert!(state.claim_model_read(assets::MODEL_KEY).is_ok());
}

#[test]
fn cancellation_is_job_scoped_monotonic_and_shareable_without_loading_ort() {
    let cancellation = Arc::new(Cancellation::default());
    let other = cancellation.clone();
    std::thread::spawn(move || other.cancel().unwrap())
        .join()
        .unwrap();
    assert_eq!(cancellation.check(), Err(SessionError::Cancelled));
    cancellation.cancel().unwrap();
    assert_eq!(cancellation.check(), Err(SessionError::Cancelled));
    assert_eq!(Cancellation::default().check(), Ok(()));
}

#[test]
#[ignore = "requires external pinned assets and the official shared ORT library; no downloads"]
fn pinned_graph_reuses_session_obeys_cancel_and_releases_lease() {
    let dir = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    let dir = Path::new(&dir);
    let state = TranslationState::default();
    let mut session = AlignmentSession::load(&state, dir, &Cancellation::default()).unwrap();
    assert_eq!(
        AlignmentSession::load(&state, dir, &Cancellation::default()).err(),
        Some(SessionError::Busy)
    );
    assert!(state.claim_model_removal(assets::MODEL_KEY).is_err());
    for source in ["Bonjour monde.", "Hola mundo.", "café"] {
        let result = session
            .infer_pair(source, "Hello world.", &Cancellation::default())
            .unwrap();
        assert_eq!(result.source.encoding.text(), source);
        assert_eq!(result.target.encoding.text(), "Hello world.");
        for value in [&result.source, &result.target] {
            assert_eq!(
                value.values.len(),
                value.encoding.evidence().tokens.len() * WIDTH
            );
            assert!(value.values.iter().all(|v| v.is_finite()));
        }
    }
    let cancelled = Cancellation::default();
    cancelled.cancel().unwrap();
    assert_eq!(
        session.infer_pair("Bonjour", "Hello", &cancelled).err(),
        Some(SessionError::Cancelled)
    );
    assert!(session
        .infer_pair("🙂", "Hello", &Cancellation::default())
        .is_err());
    let maximum = vec!["book"; MAX_TOKENS - 2].join(" ");
    let result = session
        .infer_pair(&maximum, "Hello", &Cancellation::default())
        .unwrap();
    assert_eq!(result.source.values.len(), MAX_TOKENS * WIDTH);
    assert!(session
        .infer_pair(
            "Bonjour",
            &format!("{maximum} book"),
            &Cancellation::default()
        )
        .is_err());
    // A concurrently cancelled registered run must reach ORT's own termination API, not just a Rust flag.
    let cancellation = Arc::new(Cancellation::default());
    let options = Arc::new(RunOptions::new().unwrap());
    let active = cancellation.register(options.clone()).unwrap();
    assert_eq!(
        cancellation.register(options).err(),
        Some(SessionError::Busy)
    );
    let other = cancellation.clone();
    std::thread::spawn(move || other.cancel().unwrap())
        .join()
        .unwrap();
    let ids = Tensor::from_array(([1usize, 3], vec![101i64, 34551, 102])).unwrap();
    let mask = Tensor::from_array(([1usize, 3], vec![1i64; 3])).unwrap();
    assert!(session
        .session
        .run_with_options(
            ort::inputs!["input_ids"=>ids,"attention_mask"=>mask],
            &*active.options
        )
        .is_err());
    drop(active);
    assert!(cancellation.active.lock().unwrap().is_none());
    assert!(session
        .infer_pair("Bonjour", "Hello", &Cancellation::default())
        .is_ok());
    assert_eq!(
        load_graph(b"not an ONNX graph").err(),
        Some(SessionError::Graph)
    );
    drop(session);
    assert!(state.claim_model_removal(assets::MODEL_KEY).is_ok());
    // Repeated jobs load/drop without a leaked claim or stale native termination flag.
    let mut next = AlignmentSession::load(&state, dir, &Cancellation::default()).unwrap();
    assert!(next
        .infer_pair("Hola", "Hello", &Cancellation::default())
        .is_ok());
}

#[test]
#[ignore = "run separately with external pinned assets and NO discoverable shared ORT library"]
fn missing_runtime_returns_a_typed_error_and_releases_the_model_guard() {
    let dir = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    let state = TranslationState::default();
    let error = AlignmentSession::load(&state, Path::new(&dir), &Cancellation::default()).err();
    assert_eq!(
        error,
        Some(SessionError::RuntimeUnavailable),
        "This test requires an isolated process without a discoverable ORT library"
    );
    assert!(state.claim_model_removal(assets::MODEL_KEY).is_ok());
    assert!(state.claim_model_read(assets::MODEL_KEY).is_ok());
}
