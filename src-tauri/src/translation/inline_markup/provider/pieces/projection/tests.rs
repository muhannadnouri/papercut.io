use super::*;
use crate::translation::{
    dom::DomPlan,
    inline_markup::{
        protected::ProtectedSource,
        provider::contract::{
            tests::{rules, tokens},
            TokenizedText,
        },
    },
};
use std::cell::Cell;

struct Translator {
    limit: usize,
}
impl ExactPieceTranslator for Translator {
    fn source_fits(&self, source: &str) -> Result<bool, String> {
        Ok(source.split_whitespace().count() <= self.limit)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        Ok(sources.iter().map(|s| s.to_uppercase()).collect())
    }
}
fn plan(html: &str, limit: usize) -> SourcePlan {
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
    SourcePlan::new(&segment, &source, &Translator { limit }, |_, _| Ok(true)).unwrap()
}
fn align<'a>(source: &'a str, target: &'a str) -> Result<WordLinks<'a>, Rejection> {
    let st = tokens(source);
    let tt = tokens(target);
    let links = (0..st.len() - 2).map(|i| (i, i)).collect();
    WordLinks::new(
        source,
        target,
        &rules(),
        &TokenizedText {
            normalized: source,
            tokens: &st,
        },
        &TokenizedText {
            normalized: target,
            tokens: &tt,
        },
        links,
    )
}

#[test]
fn both_test_providers_produce_the_same_mapping_without_engine_specific_hints() {
    struct ProviderB;
    impl ExactPieceTranslator for ProviderB {
        fn source_fits(&self, _: &str) -> Result<bool, String> {
            Ok(true)
        }
        fn translate_exact(&mut self, source: &[&str]) -> Result<Vec<String>, String> {
            assert_eq!(source, ["Un livre ici"]);
            Ok(vec!["UN LIVRE ICI".into()])
        }
    }
    let plan = plan("<p>Un <strong><em>livre</em></strong> ici</p>", 10);
    let translated = plan
        .translate(&mut Translator { limit: 10 }, |_, _| Ok(true), || false)
        .unwrap();
    let a = translated.project(align, None, || Ok(())).unwrap();
    let translated_b = plan
        .translate(&mut ProviderB, |_, _| Ok(true), || false)
        .unwrap();
    let b = translated_b.project(align, None, || Ok(())).unwrap();
    assert_eq!(a.nodes(), b.nodes());
    assert!(std::ptr::eq(a.translation, &translated));
    assert_eq!(a.nodes()[0].targets, [3..8]);
    assert_eq!(a.policy(), super::super::super::contract::PROJECTION_POLICY);
}

#[test]
fn actual_subdivisions_and_copied_literal_whitespace_keep_one_authored_identity() {
    let plan = plan(
        "<p><span id='one'>Un deux; Trois <code>secret🙂</code> quatre; Cinq six.</span></p>",
        2,
    );
    let identity = plan.protected.identity();
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let mut calls = 0;
    let mapped = translated
        .project(
            |s, t| {
                calls += 1;
                assert!(!s.contains("secret"));
                align(s, t)
            },
            None,
            || Ok(()),
        )
        .unwrap();
    assert!(calls > 1);
    assert_eq!(
        mapped.nodes()[0].targets,
        [0..translated.text.chars().count()]
    );
    assert_eq!(
        mapped.nodes().iter().filter(|n| n.source_id == 0).count(),
        1
    );
    assert_eq!(plan.protected.identity(), identity);
    assert!(translated.text.contains("secret🙂"));
}

#[test]
fn known_scope_blockers_and_zero_width_attachments_fail_without_inference() {
    let plan = plan("<p><em>Un livre</em></p>", 10);
    let translated = plan
        .translate(&mut Translator { limit: 10 }, |_, _| Ok(true), || false)
        .unwrap();
    for issue in [
        ScopeIssue::Auxiliary,
        ScopeIssue::FusedMeaning,
        ScopeIssue::Attachment,
    ] {
        let error = translated
            .project::<Rejection>(
                |_, _| panic!("known unresolved must not infer"),
                Some(issue),
                || Ok(()),
            )
            .err();
        assert_eq!(error, Some(Rejection::Unresolved(issue)));
    }
    let plan = self::plan("<p>Un livre<a role='doc-noteref' href='#n'>1</a></p>", 10);
    let translated = plan
        .translate(&mut Translator { limit: 10 }, |_, _| Ok(true), || false)
        .unwrap();
    assert_eq!(
        translated
            .project::<Rejection>(|_, _| panic!("attachments remain Stage 6"), None, || Ok(()))
            .err(),
        Some(Rejection::AttachmentRequiresEvidence)
    );
}

#[test]
fn stale_pairs_changed_literal_and_partial_failures_never_return_a_completed_map() {
    let plan = plan("<p>Un <em>livre</em></p>", 1);
    let mut translated = plan
        .translate(&mut Translator { limit: 1 }, |_, _| Ok(true), || false)
        .unwrap();
    let mut calls = 0;
    assert_eq!(
        translated
            .project(
                |s, t| {
                    calls += 1;
                    if calls == 2 {
                        return Err(Rejection::UnresolvedSource);
                    }
                    align(s, t)
                },
                None,
                || Ok(()),
            )
            .err(),
        Some(Rejection::UnresolvedSource)
    );
    assert_eq!(calls, 2);
    assert_eq!(
        translated
            .project(|_, _| align("stale", "STALE"), None, || Ok(()))
            .err(),
        Some(Rejection::Binding)
    );
    translated.targets[0].end = usize::MAX;
    assert_eq!(
        translated.project(align, None, || Ok(())).err(),
        Some(Rejection::Binding)
    );

    let plan = self::plan("<p><code>secret</code></p>", 1);
    let mut translated = plan
        .translate(&mut Translator { limit: 1 }, |_, _| Ok(true), || false)
        .unwrap();
    translated.text = "public".into();
    assert_eq!(
        translated.project(align, None, || Ok(())).err(),
        Some(Rejection::Binding)
    );
}

#[derive(Debug, PartialEq, Eq)]
enum CancelError {
    Mapping(Rejection),
    Cancelled,
}
impl From<Rejection> for CancelError {
    fn from(error: Rejection) -> Self {
        Self::Mapping(error)
    }
}

#[test]
fn late_cancellation_between_pairs_stops_before_aligning_the_next_pair() {
    let plan = plan("<p><span id='one'>Un deux; Trois quatre.</span></p>", 2);
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let cancelled = Cell::new(false);
    let calls = Cell::new(0);
    let result = translated.project(
        |s, t| {
            calls.set(calls.get() + 1);
            let links = align(s, t)?;
            cancelled.set(true); // The first successful pair completes before cancellation arrives.
            Ok::<_, CancelError>(links)
        },
        None,
        || {
            if cancelled.get() {
                Err(CancelError::Cancelled)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result.err(), Some(CancelError::Cancelled));
    assert_eq!(calls.get(), 1);
    assert!(
        plan.units()
            .iter()
            .filter(|u| u.kind == Kind::Prose)
            .count()
            > 1
    );
}

#[test]
fn late_cancellation_after_final_pair_discards_the_assembled_projection() {
    let plan = plan("<p><span id='one'>Un deux; Trois quatre.</span></p>", 2);
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let pairs = plan
        .units()
        .iter()
        .filter(|u| u.kind == Kind::Prose)
        .count();
    assert_eq!(plan.units().last().unwrap().kind, Kind::Prose);
    // Prove this exact input, all pairs and the final authored-identity assembly succeed without cancellation.
    let control = translated.project(align, None, || Ok(())).unwrap();
    assert_eq!(
        control.nodes()[0].targets,
        [0..translated.text.chars().count()]
    );
    let cancelled = Cell::new(false);
    let calls = Cell::new(0);
    let result = translated.project(
        |s, t| {
            let links = align(s, t)?;
            calls.set(calls.get() + 1);
            if calls.get() == pairs {
                cancelled.set(true);
            }
            Ok::<_, CancelError>(links)
        },
        None,
        || {
            if cancelled.get() {
                Err(CancelError::Cancelled)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(calls.get(), pairs); // All pairs returned valid links; there is no next piece to catch cancellation.
    assert_eq!(result.err(), Some(CancelError::Cancelled));
}
