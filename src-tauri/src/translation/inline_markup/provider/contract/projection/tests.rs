use super::super::tests::{plan, rules, tokens};
use super::*;

/// Supplied gold graphs test projection policy, not the neural model's ability to discover scope.
fn linked<'a>(source: &'a str, target: &'a str, links: &[(usize, usize)]) -> WordLinks<'a> {
    WordLinks::new(
        source,
        target,
        &rules(),
        &TokenizedText {
            normalized: source,
            tokens: &tokens(source),
        },
        &TokenizedText {
            normalized: target,
            tokens: &tokens(target),
        },
        links.to_vec(),
    )
    .unwrap()
}

fn mapped(
    html: &str,
    target: &str,
    links: &[(usize, usize)],
) -> Result<Vec<NodeMapping>, Rejection> {
    let plan = plan(html);
    linked(plan.source_text(), target, links).project(&plan.source_codes().collect::<Vec<_>>())
}

#[test]
fn constructs_nested_reordered_and_repeated_scopes_from_word_ids() {
    let nodes = mapped(
        "<p><strong><em>rouge</em> livre</strong> ici</p>",
        "book red here",
        &[(0, 1), (1, 0), (2, 2)],
    )
    .unwrap();
    assert_eq!(nodes[0].targets, [0..8]);
    assert_eq!(nodes[1].targets, [5..8]);
    let nodes = mapped(
        "<p>Orion voit <em>Orion</em></p>",
        "Orion sees Orion",
        &[(0, 0), (1, 1), (2, 2)],
    )
    .unwrap();
    assert_eq!(nodes[0].targets, [11..16]);
}

#[test]
fn composed_source_and_combining_target_use_scalar_ranges_without_splitting_graphemes() {
    let nodes = mapped("<p><em>café</em> ici</p>", "here café", &[(0, 1), (1, 0)]).unwrap();
    assert_eq!(nodes[0].targets, [5..10]);
}

#[test]
fn disjoint_many_to_many_stays_disjoint_and_authored_ids_are_never_cloned() {
    let links = [(0, 0), (1, 1), (1, 3), (2, 2)];
    let nodes = mapped(
        "<p>Il <strong>travaille</strong> encore</p>",
        "He is still working",
        &links,
    )
    .unwrap();
    assert_eq!(nodes[0].targets, [3..5, 12..19]);
    assert_eq!(
        mapped(
            "<p>Il <span id='one'>travaille</span> encore</p>",
            "He is still working",
            &links
        )
        .unwrap_err(),
        Rejection::IdentitySplit
    );
}

#[test]
fn missing_auxiliary_omitted_source_and_mixed_fused_words_remain_unresolved() {
    assert_eq!(
        mapped(
            "<p>Il <em>reviendra</em> demain</p>",
            "He will return tomorrow",
            &[(0, 0), (1, 2), (2, 3)]
        )
        .unwrap_err(),
        Rejection::UnresolvedTarget
    );
    assert_eq!(
        mapped("<p>Un <em>livre</em></p>", "A book", &[(0, 0)]).unwrap_err(),
        Rejection::UnresolvedSource
    );
    assert!(matches!(
        mapped(
            "<p>Un <em>livre</em></p>",
            "A book",
            &[(0, 0), (0, 1), (1, 1)]
        ),
        Err(Rejection::MixedOwnership)
    ));
    assert_eq!(
        mapped("<p>Un <em>li</em>vre</p>", "A book", &[(0, 0), (1, 1)]).unwrap_err(),
        Rejection::PartialWord
    );
}

#[test]
fn whole_unit_inherits_auxiliaries_but_partial_scope_never_guesses_punctuation() {
    let nodes = mapped(
        "<p><strong><em>Il reviendra</em></strong></p>",
        "He will return.",
        &[],
    )
    .unwrap();
    assert!(nodes.iter().all(|n| n.targets == [0..15]));
    let nodes = mapped(
        "<p>Elle a <em>terminé</em></p>",
        "She has finished!",
        &[(0, 0), (1, 1), (2, 2)],
    )
    .unwrap();
    assert_eq!(nodes[0].targets, [8..16]); // No guessed auxiliary or trailing punctuation.
    let nodes = mapped(
        "<p><em>Un livre</em> ici</p>",
        "A, book here",
        &[(0, 0), (1, 2), (2, 3)],
    )
    .unwrap();
    assert_eq!(nodes[0].targets, [0..1, 3..7]); // Never widen across the unlinked comma.
}

#[test]
fn hierarchy_unknown_tokens_and_bad_graphs_fail_before_returning_a_projection() {
    assert!(matches!(
        mapped(
            "<p><em>un</em> <strong>deux</strong></p>",
            "one",
            &[(0, 0), (1, 0)]
        ),
        Err(Rejection::Hierarchy | Rejection::MixedOwnership)
    ));
    let source = "Un livre";
    let target = "A book";
    for links in [vec![(0, 0), (0, 0)], vec![(99, 0)], vec![(0, 99)]] {
        assert!(WordLinks::new(
            source,
            target,
            &rules(),
            &TokenizedText {
                normalized: source,
                tokens: &tokens(source)
            },
            &TokenizedText {
                normalized: target,
                tokens: &tokens(target)
            },
            links
        )
        .is_err());
    }
    let mut bad = tokens(source);
    bad[1].id = 100;
    assert!(matches!(
        WordLinks::new(
            source,
            target,
            &rules(),
            &TokenizedText {
                normalized: source,
                tokens: &bad
            },
            &TokenizedText {
                normalized: target,
                tokens: &tokens(target)
            },
            vec![]
        ),
        Err(Rejection::UnknownToken(Side::Source))
    ));
}

#[test]
fn complete_wrong_graphs_are_not_semantic_certification() {
    // Preserve this counterexample: correspondence coverage cannot certify translation or scope quality.
    assert!(mapped(
        "<p>La <em>maison</em> est grande</p>",
        "The cat is hungry",
        &[(0, 0), (1, 1), (2, 2), (3, 3)]
    )
    .is_ok());
}

#[test]
fn combined_hierarchy_work_rejects_excessive_ranges_before_pairwise_comparisons() {
    let source = plan("<p><em>mot</em></p>");
    let bad = vec![NodeMapping {
        source_id: 0,
        targets: vec![0..1; MAX_MAPPED_RANGES + 1],
    }];
    assert_eq!(
        check_projected_nodes(&source, "word", &bad),
        Err(Rejection::Limit)
    );
}
