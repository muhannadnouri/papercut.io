//! Synthetic provider tests establish structural behavior, not model translation quality.
use super::*;
use crate::translation::{
    dom::DomPlan,
    inline_markup::{protected::ProtectedSource, provider::pieces::ExactPieceTranslator},
    job::plan_translation_job,
    types::TranslationStartRequest,
};

pub(crate) struct Engine {
    pub(crate) seen: Vec<String>,
    pub(crate) limit: usize,
}
impl ExactPieceTranslator for Engine {
    fn source_fits(&self, text: &str) -> Result<bool, String> {
        Ok(text.chars().count() <= self.limit)
    }
    fn translate_exact(&mut self, source: &[&str]) -> Result<Vec<String>, String> {
        assert!(source.len() <= 16);
        self.seen.extend(source.iter().map(|s| s.to_string()));
        Ok(source
            .iter()
            .map(|s| match *s {
                "Je ne sais pas." => "I don't know.".into(),
                "Una casa roja." => "A red house.".into(),
                "Bonjour le monde." => "Hello world.".into(),
                _ => s.to_uppercase(),
            })
            .collect())
    }
}
impl TranslationEngine for Engine {
    fn exact_translator(&mut self) -> Option<&mut dyn ExactPieceTranslator> {
        Some(self)
    }
    fn translate_batch(
        &mut self,
        _: crate::translation::engine::TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        panic!("boundary work entered the legacy phrase-probe path")
    }
}

/// Retain the whole DOM block; internal MT units may split without duplicating authored identities.
fn source_plan(html: &str, engine: &dyn ExactPieceTranslator) -> Result<SourcePlan, String> {
    let dom = DomPlan::new(html);
    let block = &dom.blocks[0];
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: block.inline.text.chars().count(),
        text: block.inline.text.clone(),
    };
    let protected = ProtectedSource::for_boundaries(&block.node)?
        .fragment(0..segment.source_end, &segment.text)?;
    SourcePlan::from_sentences(&segment, protected, engine)
}

/// Only proven sentence-wide scopes enter source-owned reconstruction; review cases use the helper below.
fn translate(html: &str, engine: &mut Engine) -> Result<(String, ProtectedPlan, String), String> {
    let plan = source_plan(html, engine)?;
    assert!(!plan.formatting_needs_review(), "{html}");
    let translated = plan.translate(engine, |_, _| Ok(true), || false)?;
    let protected = &plan.protected;
    let transport = protected.boundary_output(&translated)?;
    let restored = protected.restore_boundaries(&transport)?;
    Ok((restored.html, protected.clone(), transport))
}

/// Uncertain style ownership must not fragment the sentence or invent a target formatting range.
fn review_text(html: &str, engine: &mut Engine) -> String {
    let plan = source_plan(html, engine).unwrap();
    assert!(plan.formatting_needs_review(), "{html}");
    plan.translate(engine, |_, _| Ok(true), || false)
        .unwrap()
        .text
}

/// Sentence-wide scopes retain copied joins, notes and assets with their original owners.
#[test]
fn sentence_first_boundary_planning_keeps_nested_markup_and_exact_joins() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, _, _) = translate("<p><strong id='s'><em>Salut. Merci !</em><a role='doc-noteref' href='#n'>1</a><img data-papercut-asset='asset.png'></strong> <a id='link' href='https://example.org'>Hola. Adiós.</a></p>", &mut engine).unwrap();
    assert_eq!(engine.seen, ["Salut.", "Merci !", "Hola.", "Adiós."]);
    let doc = crate::translation::html::parse_html_document(format!("<div>{html}</div>"));
    assert_eq!(doc.select("strong#s").unwrap().count(), 1);
    assert_eq!(
        doc.select_first("strong#s > em").unwrap().text_contents(),
        "SALUT. MERCI !"
    );
    assert_eq!(
        doc.select_first("a#link").unwrap().text_contents(),
        "HOLA. ADIÓS."
    );
    assert!(doc
        .select_first("strong#s > a[role=doc-noteref] + img")
        .is_ok());
    assert!(html.contains("</strong> <a"));

    // The strict research contract still retains sentence context across emphasis.
    let dom = DomPlan::new("<p><strong>Salut. Merci !</strong></p>");
    let text = &dom.blocks[0].inline.text;
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: text.chars().count(),
        text: text.clone(),
    };
    let protected = ProtectedSource::for_boundaries(&dom.blocks[0].node)
        .unwrap()
        .fragment(0..segment.source_end, text)
        .unwrap();
    let strict = SourcePlan::from_protected(&segment, protected, &engine, |_, _| Ok(true)).unwrap();
    engine.seen.clear();
    strict
        .translate(&mut engine, |_, _| Ok(true), || false)
        .unwrap();
    assert_eq!(engine.seen, ["Salut. Merci !"]);
}

/// Sentence subdivision must spend the same decoder/work quota and retain Unicode graphemes and spaces.
#[test]
fn sentence_first_boundary_planning_enforces_prose_quota_and_unicode_cuts() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, _, _) = translate(
        "<p><em>Éte\u{301}. ¿Hola? 日本語。 Next.</em></p>",
        &mut engine,
    )
    .unwrap();
    assert_eq!(engine.seen, ["Éte\u{301}.", "¿Hola?", "日本語。", "Next."]);
    assert_eq!(html, "<em>ÉTE\u{301}. ¿HOLA? 日本語。 NEXT.</em>");
    engine.seen.clear();
    translate(&format!("<p>{}</p>", "Un. ".repeat(128)), &mut engine).unwrap();
    assert_eq!(engine.seen.len(), 128);
    engine.seen.clear();
    assert!(translate(&format!("<p>{}</p>", "Un. ".repeat(129)), &mut engine).is_err());
    assert!(engine.seen.is_empty());
}

#[test]
fn uniform_formatting_is_retained_and_partial_scopes_keep_the_sentence_for_review() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, _, _) = translate(
        "<p><strong><em>Bonjour le monde.</em></strong></p>",
        &mut engine,
    )
    .unwrap();
    assert_eq!(engine.seen, ["Bonjour le monde."]);
    assert_eq!(html, "<strong><em>Hello world.</em></strong>");
    engine.seen.clear();
    assert_eq!(
        review_text(
            "<p>Je <em>ne</em> sais <strong>pas</strong>.</p>",
            &mut engine
        ),
        "I don't know."
    );
    assert_eq!(engine.seen, ["Je ne sais pas."]);
    engine.seen.clear();
    assert_eq!(
        review_text("<p>Una <strong>casa</strong> roja.</p>", &mut engine),
        "A red house."
    );
    assert_eq!(engine.seen, ["Una casa roja."]);
    engine.seen.clear();
    let (html, _, _) = translate("<p><strong>Je ne sais pas.</strong></p>", &mut engine).unwrap();
    assert_eq!(engine.seen, ["Je ne sais pas."]);
    assert_eq!(html, "<strong>I don&apos;t know.</strong>");
}

/// A coincidental tokenizer cut cannot turn uncertain word ownership into an accepted mapping.
#[test]
fn budget_subdivision_never_certifies_partial_sentence_formatting_or_attachments() {
    for limit in [900, 10] {
        let mut engine = Engine {
            seen: vec![],
            limit,
        };
        for html in [
            "<p><strong>Bonjour le</strong> monde entier.</p>",
            "<p>Bonjour le<a role='doc-noteref' href='#n'>1</a> monde entier.</p>",
            "<p>Bonjour le<img data-papercut-asset='asset.png'> monde entier.</p>",
        ] {
            review_text(html, &mut engine);
        }
        assert_eq!(engine.seen.len() == 3, limit == 900);
    }
}

/// Punctuation around translated scopes must stay source-owned, including styled and linked punctuation.
#[test]
fn punctuation_symbols_and_emoji_keep_markup_and_never_enter_inference() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, _, _) = translate("<p><strong id='word'><em>¿</em></strong> <a href='https://example.org'>?</a> <span>！？—…،؛؟</span> 👩‍💻 &amp; + =<a role='doc-noteref' href='#note'>1</a></p>", &mut engine).unwrap();
    assert!(engine.seen.is_empty());
    let doc = crate::translation::html::parse_html_document(format!("<div>{html}</div>"));
    assert_eq!(
        doc.select_first("strong#word > em")
            .unwrap()
            .text_contents(),
        "¿"
    );
    assert_eq!(
        doc.select_first("a[href='https://example.org']")
            .unwrap()
            .text_contents(),
        "?"
    );
    assert_eq!(
        doc.select_first("span").unwrap().text_contents(),
        "！？—…،؛؟"
    );
    assert!(doc.select_first("a[role=doc-noteref]").is_ok());
    assert!(html.starts_with("<strong"));
    assert!(html.contains("👩‍💻 &amp; + ="));
    engine.seen.clear();
    review_text(
        "<p><em>été</em> <strong>عَرَبِيّ</strong> <i>日本語</i> <b>42</b> <u>١٢٣</u> !</p>",
        &mut engine,
    );
    assert_eq!(engine.seen, ["été عَرَبِيّ 日本語 42 ١٢٣ !"]);
    engine.seen.clear();
    engine.limit = 7;
    let (html, _, _) = translate("<p><strong>bonjour !</strong></p>", &mut engine).unwrap();
    assert_eq!(engine.seen, ["bonjour"]);
    assert_eq!(html, "<strong>BONJOUR !</strong>");
}

/// A punctuation-only block needs neither tokenizer nor inference; strict planning keeps its existing policy.
#[test]
fn punctuation_only_blocks_bypass_runtime_but_keep_cancellation_and_strict_planning() {
    struct NoRuntime;
    impl ExactPieceTranslator for NoRuntime {
        fn source_fits(&self, _: &str) -> Result<bool, String> {
            panic!("copied punctuation was tokenized")
        }
        fn translate_exact(&mut self, _: &[&str]) -> Result<Vec<String>, String> {
            panic!("copied punctuation was translated")
        }
    }
    let dom = DomPlan::new("<p><strong>¿</strong><em>?!</em> «» … 👩‍💻</p>");
    let block = &dom.blocks[0];
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: block.inline.text.chars().count(),
        text: block.inline.text.clone(),
    };
    let protected = ProtectedSource::for_boundaries(&block.node)
        .unwrap()
        .fragment(0..segment.source_end, &segment.text)
        .unwrap();
    let mut engine = NoRuntime;
    let plan = SourcePlan::from_sentences(&segment, protected.clone(), &engine).unwrap();
    assert!(plan
        .translate(&mut engine, |_, _| Ok(true), || true)
        .err()
        .unwrap()
        .contains("cancelled"));
    let mut translated = plan
        .translate(&mut engine, |_, _| Ok(true), || false)
        .unwrap();
    assert_eq!(translated.text, segment.text);
    let markup = protected.boundary_output(&translated).unwrap();
    assert_eq!(
        protected.restore_boundaries(&markup).unwrap().text,
        segment.text
    );
    translated.text = translated.text.replacen('¿', "X", 1);
    assert!(protected
        .boundary_output(&translated)
        .err()
        .unwrap()
        .contains("copied source content"));
    let engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let strict = SourcePlan::from_protected(&segment, protected, &engine, |_, _| Ok(true)).unwrap();
    assert!(strict
        .units()
        .iter()
        .any(|u| u.kind == crate::translation::inline_markup::provider::pieces::Kind::Prose));
}

#[test]
fn literal_assets_links_notes_and_repeated_words_keep_source_order_and_parents() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, plan, output) = translate("<p><strong id='s'><a role='doc-noteref' href='#n'>1</a>Été🙂. <em>Été🙂.</em><img data-papercut-asset='asset.png'></strong> <a id='link' href='https://example.org'>Été🙂.</a></p>", &mut engine).unwrap();
    assert_eq!(engine.seen.iter().filter(|s| *s == "Été🙂.").count(), 3);
    for secret in [
        "secret_key",
        "private_id",
        "asset.png",
        "https://",
        "#n",
        "data-pc",
    ] {
        assert!(engine.seen.iter().all(|s| !s.contains(secret)));
    }
    let doc = crate::translation::html::parse_html_document(format!("<div>{html}</div>"));
    let strong = doc.select_first("strong#s").unwrap();
    assert!(strong
        .as_node()
        .first_child()
        .unwrap()
        .as_element()
        .is_some());
    assert_eq!(
        doc.select_first("strong > em").unwrap().text_contents(),
        "ÉTÉ🙂."
    );
    assert!(doc.select_first("strong > img").is_ok());
    assert_eq!(
        doc.select_first("a#link").unwrap().text_contents(),
        "ÉTÉ🙂."
    );
    // Source-boundary cache admission never accepts reordered or repeated styles, unlike worker alignment.
    assert!(plan
        .restore_boundaries(&output.replace("data-pc=\"0\"", "data-pc=\"99\""))
        .is_err());
    engine.seen.clear();
    let text = review_text("<p><strong id='s'><a role='doc-noteref' href='#n'>1</a>Été🙂. <em>Été🙂.</em><img data-papercut-asset='asset.png'></strong> <a id='link' href='https://example.org'>Été🙂.</a> <code>secret_key</code><span translate='no'>private_id</span> &amp; &lt;tag&gt;</p>", &mut engine);
    assert!(text.contains("secret_keyprivate_id & <TAG>"));
    for retained in [
        "secret_key",
        "private_id",
        "asset.png",
        "https://",
        "#n",
        "data-pc",
    ] {
        assert!(engine.seen.iter().all(|s| !s.contains(retained)));
    }
}

#[test]
fn long_authored_wrapper_splits_only_prose_and_retains_table_context() {
    let source = "bonjour le monde ".repeat(90);
    let mut engine = Engine {
        seen: vec![],
        limit: 80,
    };
    let (html, _, _) = translate(
        &format!("<p><strong id='only'>{source}</strong></p>"),
        &mut engine,
    )
    .unwrap();
    assert!(engine.seen.len() > 1);
    assert!(engine.seen.iter().all(|s| s.chars().count() <= 80));
    assert_eq!(html.matches("id=\"only\"").count(), 1);
    assert_eq!(
        crate::translation::html::parse_html_document(html)
            .text_contents()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        source.to_uppercase().trim()
    );
    let (html, _, _) = translate(
        "<table><caption>Titre.</caption><tr><td><b>Été🙂.</b></td><td>Bleu.</td></tr></table>",
        &mut engine,
    )
    .unwrap();
    let doc = crate::translation::html::parse_html_document(html);
    assert_eq!(
        doc.select_first("table td b").unwrap().text_contents(),
        "ÉTÉ🙂."
    );
    assert_eq!(
        doc.select_first("table caption").unwrap().text_contents(),
        "TITRE."
    );
}

#[test]
fn malformed_markup_and_literal_grapheme_cuts_fail_while_partial_styles_need_review() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    assert!(source_plan("<p><code>e</code>\u{301} texte</p>", &engine).is_err());
    assert!(engine.seen.is_empty());
    review_text("<p><strong>e</strong>\u{301} texte</p>", &mut engine);
    assert_eq!(engine.seen, ["e\u{301} texte"]);
    let (_, plan, output) =
        translate("<p><strong>Été.</strong> <em>Été.</em></p>", &mut engine).unwrap();
    for bad in [
        "plain output".to_string(),
        output.repeat(2),
        output.replace("data-pc=\"0\"", "data-pc=\"1\""),
        "<span data-pc='1'>ÉTÉ</span> ET <span data-pc='0'>ÉTÉ</span>".into(),
        "<span data-pc='0'></span> ET <span data-pc='1'>ÉTÉ</span>".into(),
    ] {
        assert!(plan.restore_boundaries(&bad).is_err());
    }
}

#[test]
fn whole_block_planning_preserves_anchor_ids_and_separates_legacy_and_strict_cache_keys() {
    let request = TranslationStartRequest {
        job_id: None,
        document_url: "/uploads/example.html".into(),
        source_language: "fr".into(),
        target_language: "en".into(),
        model_id: "opus-mt-fr-en-ctranslate2".into(),
        quality_mode: "balanced".into(),
        preserve_formatting: false,
        use_hardware_acceleration: false,
        repair_mode: Default::default(),
        glossary: vec![],
    };
    let source = "bonjour ".repeat(180);
    let plan = plan_translation_job(request.clone(), [&source], 900, 16).unwrap();
    assert_eq!(plan.total_segments, 1);
    let mut strict = request.clone();
    strict.preserve_formatting = true;
    let strict = plan_translation_job(strict, [&source], 900, 16).unwrap();
    assert!(strict.total_segments > 1);
    assert_ne!(strict.cache_key, plan.cache_key);
    assert!(plan_translation_job(request, ["é".repeat(20000)], 900, 16).is_err());
}

#[test]
fn missing_empty_and_oversized_fragment_replies_never_produce_boundary_markup() {
    struct Broken(Vec<String>);
    impl ExactPieceTranslator for Broken {
        fn source_fits(&self, _: &str) -> Result<bool, String> {
            Ok(true)
        }
        fn translate_exact(&mut self, _: &[&str]) -> Result<Vec<String>, String> {
            Ok(self.0.clone())
        }
    }
    let dom = DomPlan::new("<p><strong>bonjour</strong></p>");
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: 7,
        text: "bonjour".into(),
    };
    let protected = ProtectedSource::for_boundaries(&dom.blocks[0].node)
        .unwrap()
        .fragment(0..7, "bonjour")
        .unwrap();
    for values in [
        vec![],
        vec!["".into()],
        vec!["hello".into(), "extra".into()],
        vec!["x".repeat(32769)],
    ] {
        let mut engine = Broken(values);
        let plan = SourcePlan::from_sentences(&segment, protected.clone(), &engine).unwrap();
        assert!(plan
            .translate(&mut engine, |_, _| Ok(true), || false)
            .is_err());
    }
}

#[cfg(feature = "native-translation-ctranslate2")]
#[test]
#[ignore = "requires externally cached pinned FR/ES packs; never downloads or runs an app smoke test"]
fn installed_french_spanish_sentences_retain_safe_styles_and_identify_review_cases() {
    use crate::translation::{ctranslate2::CTranslate2Engine, model_store::manifest_by_id};
    let root = std::env::var_os("PAPERCUT_CT2_MODELS_DIR").expect("Set PAPERCUT_CT2_MODELS_DIR");
    for (model, html, expected, review) in [
        ("opus-mt-fr-en-ctranslate2", "<p><strong id='s'><em>Bonjour le monde. Merci beaucoup.</em></strong><a role='doc-noteref' href='#n'>1</a></p>", "world", false),
        ("opus-mt-es-en-ctranslate2", "<p><strong id='s'><em>Hola mundo. Muchas gracias.</em></strong><a role='doc-noteref' href='#n'>1</a></p>", "world", false),
        ("opus-mt-fr-en-ctranslate2", "<p>Je <em>ne</em> sais <strong>pas</strong>.</p>", "I don't know.", true),
        ("opus-mt-es-en-ctranslate2", "<p>Una <strong>casa</strong> roja.</p>", "A red house.", true),
        ("opus-mt-fr-en-ctranslate2", "<p><strong id='s'><em>Bonjour le monde.</em></strong> <code>private_key</code><a role='doc-noteref' href='#n'>1</a></p>", "world", true),
        ("opus-mt-es-en-ctranslate2", "<p><strong id='s'><em>Hola mundo.</em></strong> <code>private_key</code><a role='doc-noteref' href='#n'>1</a></p>", "world", true),
    ] {
        let dir = std::path::PathBuf::from(&root).join(model);
        manifest_by_id(model).unwrap().verify_files(&dir).unwrap();
        let mut engine = CTranslate2Engine::for_installed_model(model,dir).unwrap();
        let dom = DomPlan::new(html); let block=&dom.blocks[0];
        let segment = TranslationTextSegment { id:"b0:s0".into(), source_block_index:0, source_start:0,
            source_end:block.inline.text.chars().count(), text:block.inline.text.clone() };
        let protected = ProtectedSource::for_boundaries(&block.node).unwrap().fragment(0..segment.source_end,&segment.text).unwrap();
        let plan = SourcePlan::from_sentences(&segment,protected.clone(),&engine).unwrap();
        assert_eq!(plan.formatting_needs_review(), review, "{html}");
        if html.contains("Merci beaucoup.") || html.contains("Muchas gracias.") {
            assert_eq!(plan.units().iter().filter(|u| u.kind == crate::translation::inline_markup::provider::pieces::Kind::Prose).count(), 2);
        }
        let translated = plan.translate(&mut engine, |_,_| Ok(true), || false).unwrap();
        if review {
            if html.contains("<code>") {
                assert!(translated.text.to_lowercase().contains(expected));
                assert!(translated.text.ends_with("private_key"));
            } else {
                assert_eq!(translated.text, expected);
                assert!(protected.boundary_output(&translated).is_err());
            }
            continue;
        }
        let output = protected.boundary_output(&translated).unwrap();
        let restored = protected.restore_boundaries(&output).unwrap();
        let parsed=crate::translation::html::parse_html_document(restored.html);
        let emphasis=parsed.select_first("strong#s > em").unwrap().text_contents();
        assert!(emphasis.to_lowercase().contains(expected), "{model}: {emphasis}");
        assert!(parsed.select_first("a[role=doc-noteref]").is_ok());
    }
}

#[test]
fn direct_table_and_list_fragment_contexts_retain_structural_children() {
    use crate::translation::inline_markup::source_inline_model;
    for (selector, html) in [
        (
            "table",
            "<table><tr><td><b>Bleu.</b></td><td>Rouge.</td></tr></table>",
        ),
        (
            "tr",
            "<table><tr><td><b>Bleu.</b></td><td>Rouge.</td></tr></table>",
        ),
        ("li", "<ul><li><b>Bleu.</b> Rouge.</li></ul>"),
    ] {
        let doc = crate::translation::html::parse_html_document(html);
        let node = doc.select_first(selector).unwrap();
        let source = source_inline_model(node.as_node()).text;
        let segment = TranslationTextSegment {
            id: "b0:s0".into(),
            source_block_index: 0,
            source_start: 0,
            source_end: source.chars().count(),
            text: source.clone(),
        };
        let protected = ProtectedSource::for_boundaries(node.as_node())
            .unwrap()
            .fragment(0..segment.source_end, &source)
            .unwrap();
        let mut engine = Engine {
            seen: vec![],
            limit: 900,
        };
        let plan = SourcePlan::from_sentences(&segment, protected.clone(), &engine).unwrap();
        assert!(!plan.formatting_needs_review());
        let translated = plan
            .translate(&mut engine, |_, _| Ok(true), || false)
            .unwrap();
        let output = protected.boundary_output(&translated).unwrap();
        let restored = protected.restore_boundaries(&output).unwrap();
        assert_eq!(restored.text, "BLEU. ROUGE.");
        assert!(restored.html.contains("<b>BLEU.</b>"));
        if selector != "li" {
            assert!(restored.html.contains("<td>"));
        }
    }
}
