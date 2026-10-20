use super::*;
use crate::translation::alignment::{runtime::Embedding, tests::synthetic};

#[test]
fn fixed_softmax_intersection_handles_reversal_subwords_and_rejects_one_direction_only() {
    assert_eq!(
        intersect(&[0., 20., 20., 0.], &[0, 1], &[0, 1]).unwrap(),
        [(0, 1), (1, 0)]
    );
    assert_eq!(
        intersect(&[20., 10., 0., 0.], &[0, 1], &[0, 1]).unwrap(),
        [(0, 0)]
    );
    assert_eq!(
        intersect(
            &[20., 0., 0., 0., 20., 0., 0., 0., 20.],
            &[0, 0, 1],
            &[0, 1, 1]
        )
        .unwrap(),
        [(0, 0), (0, 1), (1, 1)]
    );
    assert_eq!(
        intersect(&[10_000., 0., 0., 10_000.], &[0, 1], &[0, 1]).unwrap(),
        [(0, 0), (1, 1)]
    );
    // Scores on either side of the fixed threshold; no cosine scaling or threshold tuning.
    assert_eq!(
        intersect(&[0., 1000f32.ln()], &[0], &[0, 1]).unwrap(),
        [(0, 1)]
    );
    assert_eq!(
        intersect(&[0., 998f32.ln()], &[0], &[0, 1]).unwrap(),
        [(0, 0), (0, 1)]
    );
}

#[test]
fn dense_malformed_and_nonfinite_matrices_have_bounded_failure() {
    for scores in [
        vec![],
        vec![f32::NAN],
        vec![f32::INFINITY],
        vec![f32::NEG_INFINITY],
        vec![0., 1.],
    ] {
        assert!(intersect(&scores, &[0], &[0]).is_err());
    }
    assert!(intersect(&[], &[], &[]).is_err());
    let ids = (0..MAX_TOKENS - 2).collect::<Vec<_>>();
    assert_eq!(
        intersect(&vec![0.; ids.len() * ids.len()], &ids, &ids).unwrap_err(),
        SessionError::Mapping(Rejection::Limit)
    );
}

#[test]
fn actual_raw_tokenizer_word_ids_exclude_frames_and_merge_combining_subwords() {
    let tokenizer = synthetic();
    let pair = synthetic_pair(&tokenizer);
    let links = word_links(&pair, &tokenizer.rules(), || false).unwrap();
    assert_eq!(links.links(), [(0, 1), (1, 0)]);
    assert_eq!(
        word_links(&pair, &tokenizer.rules(), || true).err(),
        Some(SessionError::Cancelled)
    );
    let mut bad = pair;
    bad.target.values.pop();
    assert!(word_links(&bad, &tokenizer.rules(), || false).is_err());
    bad.source.values[WIDTH] = f32::NAN;
    assert!(word_links(&bad, &tokenizer.rules(), || false).is_err());
}

/// The same valid raw-tokenizer pair exercises successful extraction and cancellation checkpoints.
fn synthetic_pair(tokenizer: &super::super::CandidateTokenizer) -> PairEmbeddings<'static> {
    let source = tokenizer.encode("café book", Side::Source).unwrap();
    let target = tokenizer.encode("book café", Side::Target).unwrap();
    fn embed(encoding: super::super::EncodedText<'_>, reverse: bool) -> Embedding<'_> {
        let mut values = vec![0.; encoding.evidence().tokens.len() * WIDTH];
        for (i, token) in encoding.evidence().tokens.iter().enumerate() {
            if let Some(word) = token.word {
                values[i * WIDTH + if reverse { 1 - word } else { word }] = 10.;
            } else {
                values[i * WIDTH] = f32::MAX;
            } // A framing dot product would overflow if it were incorrectly included.
        }
        Embedding { encoding, values }
    }
    PairEmbeddings {
        source: embed(source, false),
        target: embed(target, true),
    }
}

#[test]
fn late_cancellation_during_extraction_discards_partial_scores() {
    use std::cell::Cell;
    let tokenizer = synthetic();
    let pair = synthetic_pair(&tokenizer);
    let checks = Cell::new(0);
    let result = word_links(&pair, &tokenizer.rules(), || {
        checks.set(checks.get() + 1);
        checks.get() == 2 // The first row completed; the next row must observe cancellation.
    });
    assert_eq!(result.err(), Some(SessionError::Cancelled));
    assert_eq!(checks.get(), 2);
}

#[test]
fn late_cancellation_after_extraction_discards_completed_links() {
    use std::cell::Cell;
    let tokenizer = synthetic();
    let pair = synthetic_pair(&tokenizer);
    let rows = pair.source.encoding.evidence().tokens.len() - 2;
    let checks = Cell::new(0);
    let result = word_links(&pair, &tokenizer.rules(), || {
        checks.set(checks.get() + 1);
        checks.get() == rows + 1 // Every row and the softmax intersection finished before this checkpoint.
    });
    assert_eq!(result.err(), Some(SessionError::Cancelled));
    assert_eq!(checks.get(), rows + 1);
}

#[test]
fn reference_attribution_and_policy_are_retained_without_a_python_dependency() {
    let license = include_str!("LICENSE");
    assert!(license.contains("Copyright (c) 2020, Zi-Yi Dou"));
    assert!(license.contains("BSD 3-Clause"));
    assert!(POLICY.contains("intersection-gt-0.001"));
}

/// Frozen raw-tokenizer links from the pinned Python guide head on bundled ORT 1.27.0 tensors.
/// Independent annotation checks selected target scopes; it does not certify arbitrary model output.
#[test]
#[ignore = "requires externally pinned INT8 assets and official ORT library; no downloads"]
fn pinned_reference_word_links_and_formatted_scopes() {
    use crate::translation::{
        alignment::runtime::{AlignmentSession, Cancellation},
        state::TranslationState,
    };
    let dir = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    let mut session = AlignmentSession::load(
        &TranslationState::default(),
        std::path::Path::new(&dir),
        &Cancellation::default(),
    )
    .unwrap();
    // Word IDs include punctuation, exclude framing, and come from raw encoding rather than word splitting.
    let tokenizer = super::super::CandidateTokenizer::load(std::path::Path::new(&dir)).unwrap();
    for (html, target, expected, scope, unresolved) in [
        (
            "<p>Un <em>livre</em></p>",
            "A book",
            vec![(0, 0), (1, 1)],
            Some(2..6),
            None,
        ),
        (
            "<p>Un <em>libro</em></p>",
            "A book",
            vec![(0, 0), (1, 1)],
            Some(2..6),
            None,
        ),
        (
            "<p>Le <strong>livre <em>rouge</em></strong></p>",
            "The red book",
            vec![(0, 0), (1, 2), (2, 1)],
            Some(4..7),
            None,
        ),
        (
            "<p>El libro <em>rojo</em></p>",
            "The red book",
            vec![(0, 0), (1, 2)],
            Some(4..7),
            Some(Rejection::UnresolvedSource), // The intended red scope still fails; no relabeling as a positive pass.
        ),
        (
            "<p>Il <em>reviendra</em> demain.</p>",
            "He will return tomorrow.",
            vec![(0, 0), (1, 2), (2, 3), (3, 4)],
            Some(3..14),
            Some(Rejection::UnresolvedTarget),
        ),
        (
            "<p><em>Volverá</em> mañana.</p>",
            "She will return tomorrow.",
            vec![(0, 1), (0, 2), (1, 3), (2, 4)],
            Some(4..15),
            Some(Rejection::UnresolvedTarget),
        ),
        (
            "<p>Elle a <em>terminé</em>.</p>",
            "She has finished.",
            vec![(0, 0), (1, 1), (2, 2), (3, 3)],
            Some(8..16),
            None,
        ),
        (
            "<p>Orion ve a <em>Orion</em>.</p>",
            "Orion sees Orion.",
            vec![(0, 0), (1, 1), (2, 1), (3, 2), (4, 3)],
            Some(11..16),
            None,
        ),
    ] {
        let plan = fixed_plan(html);
        let source = plan.protected.source_text();
        let embeddings = session
            .infer_pair(source, target, &Cancellation::default())
            .unwrap();
        let links = word_links(&embeddings, &tokenizer.rules(), || false).unwrap();
        assert_eq!(
            links.links(),
            expected,
            "reference links on bundled ORT 1.27.0: {source}"
        );
        let translated = plan
            .translate(
                &mut Fixed(target),
                |s, side| session.text_fits(s, side).map_err(|_| "admission".into()),
                || false,
            )
            .unwrap();
        let projected = session.project_translation(&translated, &Cancellation::default(), None);
        if let Some(error) = unresolved {
            assert_eq!(
                projected.err(),
                Some(SessionError::Mapping(error)),
                "unsupported intended scope {scope:?}"
            );
        } else if let Some(scope) = scope {
            let projected = projected.unwrap();
            assert_eq!(projected.nodes().last().unwrap().targets, [scope]);
        }
    }
}

struct Fixed<'a>(&'a str);
impl crate::translation::inline_markup::provider::pieces::ExactPieceTranslator for Fixed<'_> {
    fn source_fits(&self, _: &str) -> Result<bool, String> {
        Ok(true)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        assert_eq!(sources.len(), 1);
        Ok(vec![self.0.into()])
    }
}

fn fixed_plan(html: &str) -> crate::translation::inline_markup::provider::pieces::SourcePlan {
    use crate::translation::{
        dom::DomPlan,
        inline_markup::{protected::ProtectedSource, provider::pieces::SourcePlan},
        segment::TranslationTextSegment,
    };
    let dom = DomPlan::new(html);
    let block = &dom.blocks[0];
    let source = ProtectedSource::new(&block.node).unwrap();
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: block.inline.text.chars().count(),
        text: block.inline.text.clone(),
    };
    SourcePlan::new(&segment, &source, &Fixed(""), |_, _| Ok(true)).unwrap()
}

#[cfg(feature = "native-translation-ctranslate2")]
#[test]
#[ignore = "requires external pinned CT2/INT8 assets and official ORT library; no downloads"]
fn actual_native_french_spanish_pieces_project_through_the_shared_contract() {
    use crate::translation::{
        alignment::runtime::{AlignmentSession, Cancellation},
        ctranslate2::CTranslate2Engine,
        model_store::manifest_by_id,
        state::TranslationState,
    };
    let root = std::env::var_os("PAPERCUT_CT2_MODELS_DIR").expect("Set PAPERCUT_CT2_MODELS_DIR");
    let assets = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    let mut session = AlignmentSession::load(
        &TranslationState::default(),
        std::path::Path::new(&assets),
        &Cancellation::default(),
    )
    .unwrap();
    for (model, html, expected) in [
        (
            "opus-mt-fr-en-ctranslate2",
            "<p>Un <em>livre</em></p>",
            "A Book",
        ),
        (
            "opus-mt-es-en-ctranslate2",
            "<p>Un <em>libro</em></p>",
            "A book",
        ),
    ] {
        let path = std::path::Path::new(&root).join(model);
        manifest_by_id(model).unwrap().verify_files(&path).unwrap();
        let mut engine = CTranslate2Engine::for_installed_model(model, path).unwrap();
        let plan = fixed_plan(html);
        let translated = plan
            .translate(
                &mut engine,
                |s, side| session.text_fits(s, side).map_err(|_| "admission".into()),
                || false,
            )
            .unwrap();
        assert_eq!(translated.text, expected);
        let projected = session
            .project_translation(&translated, &Cancellation::default(), None)
            .unwrap();
        assert_eq!(projected.nodes()[0].targets, [2..6]);
        let restored = projected.restore::<SessionError>(|| Ok(())).unwrap();
        assert_eq!(restored.text, expected);
        assert_eq!(
            restored.html,
            format!("{}<em>{}</em>", &expected[..2], &expected[2..])
        );
        let cancelled = Cancellation::default();
        cancelled.cancel().unwrap();
        assert_eq!(
            session
                .project_translation(&translated, &cancelled, None)
                .err(),
            Some(SessionError::Cancelled)
        );
    }
}
