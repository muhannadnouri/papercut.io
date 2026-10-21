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
                "viendra" => "will come".into(),
                "bague en or" => "gold ring".into(),
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
fn translate(html: &str, engine: &mut Engine) -> Result<(String, ProtectedPlan, String), String> {
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
    let plan = SourcePlan::from_boundaries(&segment, protected.clone(), engine)?;
    let translated = plan.translate(engine, |_, _| Ok(true), || false)?;
    let transport = protected.boundary_output(&translated)?;
    let restored = protected.restore_boundaries(&transport)?;
    Ok((restored.html, protected, transport))
}

#[test]
fn uniform_formatting_keeps_sentence_context_and_nested_scopes_expand_without_word_guessing() {
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
    let (html, _, _) = translate(
        "<p>Elle <strong><em>viendra</em></strong> demain; une <i>bague en or</i>.</p>",
        &mut engine,
    )
    .unwrap();
    assert_eq!(
        engine.seen,
        ["Elle", "viendra", "demain; une", "bague en or", "."]
    );
    assert_eq!(
        html,
        "ELLE <strong><em>will come</em></strong> DEMAIN; UNE <i>gold ring</i>."
    );
}

#[test]
fn literal_assets_links_notes_and_repeated_words_keep_source_order_and_parents() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    let (html, plan, output) = translate("<p><strong id='s'><a role='doc-noteref' href='#n'>1</a>été🙂 <em>été🙂</em><img data-papercut-asset='asset.png'></strong> <a id='link' href='https://example.org'>été🙂</a> <code>secret_key</code><span translate='no'>private_id</span> &amp; &lt;tag&gt;</p>", &mut engine).unwrap();
    assert_eq!(engine.seen.iter().filter(|s| *s == "été🙂").count(), 3);
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
        "ÉTÉ🙂"
    );
    assert!(doc.select_first("strong > img").is_ok());
    assert_eq!(doc.select_first("a#link").unwrap().text_contents(), "ÉTÉ🙂");
    assert!(html.contains("<code>secret_key</code>"));
    assert!(html.contains("&amp; &lt;TAG&gt;"));
    // Source-boundary cache admission never accepts reordered or repeated styles, unlike worker alignment.
    assert!(plan
        .restore_boundaries(&output.replace("data-pc=\"0\"", "data-pc=\"99\""))
        .is_err());
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
        "<table><caption>Titre</caption><tr><td><b>été🙂</b></td><td>bleu</td></tr></table>",
        &mut engine,
    )
    .unwrap();
    let doc = crate::translation::html::parse_html_document(html);
    assert_eq!(
        doc.select_first("table td b").unwrap().text_contents(),
        "ÉTÉ🙂"
    );
    assert_eq!(
        doc.select_first("table caption").unwrap().text_contents(),
        "TITRE"
    );
}

#[test]
fn malformed_markup_and_grapheme_cuts_fail_without_plain_fallback() {
    let mut engine = Engine {
        seen: vec![],
        limit: 900,
    };
    assert!(translate("<p><strong>e</strong>\u{301} texte</p>", &mut engine).is_err());
    assert!(engine.seen.is_empty());
    let (_, plan, output) =
        translate("<p><strong>été</strong> et <em>été</em></p>", &mut engine).unwrap();
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
        let plan = SourcePlan::from_boundaries(&segment, protected.clone(), &engine).unwrap();
        assert!(plan
            .translate(&mut engine, |_, _| Ok(true), || false)
            .is_err());
    }
}

#[cfg(feature = "native-translation-ctranslate2")]
#[test]
#[ignore = "requires externally cached pinned FR/ES packs; never downloads or runs an app smoke test"]
fn installed_french_spanish_boundary_units_preserve_nested_styles_and_literals() {
    use crate::translation::{ctranslate2::CTranslate2Engine, model_store::manifest_by_id};
    let root = std::env::var_os("PAPERCUT_CT2_MODELS_DIR").expect("Set PAPERCUT_CT2_MODELS_DIR");
    for (model, html, expected_word) in [
        ("opus-mt-fr-en-ctranslate2", "<p><strong id='s'><em>Bonjour le monde.</em></strong> <code>private_key</code><a role='doc-noteref' href='#n'>1</a></p>", "world"),
        ("opus-mt-es-en-ctranslate2", "<p><strong id='s'><em>Hola mundo.</em></strong> <code>private_key</code><a role='doc-noteref' href='#n'>1</a></p>", "world"),

        ("opus-mt-fr-en-ctranslate2", "<p>Une <strong id='s'><em>bague en or</em></strong> est ici.<a role='doc-noteref' href='#n'>1</a> <code>private_key</code></p>", "ring"),
        ("opus-mt-es-en-ctranslate2", "<p>Un <strong id='s'><em>libro azul</em></strong> está aquí.<a role='doc-noteref' href='#n'>1</a> <code>private_key</code></p>", "book"),
    ] {
        let dir = std::path::PathBuf::from(&root).join(model);
        manifest_by_id(model).unwrap().verify_files(&dir).unwrap();
        let mut engine = CTranslate2Engine::for_installed_model(model,dir).unwrap();
        let dom = DomPlan::new(html); let block=&dom.blocks[0];
        let segment = TranslationTextSegment { id:"b0:s0".into(), source_block_index:0, source_start:0,
            source_end:block.inline.text.chars().count(), text:block.inline.text.clone() };
        let protected = ProtectedSource::for_boundaries(&block.node).unwrap().fragment(0..segment.source_end,&segment.text).unwrap();
        let plan = SourcePlan::from_boundaries(&segment,protected.clone(),&engine).unwrap();
        let translated = plan.translate(&mut engine, |_,_| Ok(true), || false).unwrap();
        let output = protected.boundary_output(&translated).unwrap();
        let restored = protected.restore_boundaries(&output).unwrap();
        let parsed=crate::translation::html::parse_html_document(restored.html);
        let emphasis=parsed.select_first("strong#s > em").unwrap().text_contents();
        assert!(emphasis.to_lowercase().contains(expected_word), "{model}: {emphasis}");
        assert_eq!(parsed.select_first("code").unwrap().text_contents(),"private_key");
        assert!(parsed.select_first("a[role=doc-noteref]").is_ok());
    }
}

#[test]
fn direct_table_and_list_fragment_contexts_retain_structural_children() {
    use crate::translation::inline_markup::source_inline_model;
    for (selector, html) in [
        (
            "table",
            "<table><tr><td><b>bleu</b></td><td>rouge</td></tr></table>",
        ),
        (
            "tr",
            "<table><tr><td><b>bleu</b></td><td>rouge</td></tr></table>",
        ),
        ("li", "<ul><li><b>bleu</b> rouge</li></ul>"),
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
        let plan = SourcePlan::from_boundaries(&segment, protected.clone(), &engine).unwrap();
        let translated = plan
            .translate(&mut engine, |_, _| Ok(true), || false)
            .unwrap();
        let output = protected.boundary_output(&translated).unwrap();
        let restored = protected.restore_boundaries(&output).unwrap();
        assert_eq!(restored.text, "BLEU ROUGE");
        assert!(restored.html.contains("<b>BLEU</b>"));
        if selector != "li" {
            assert!(restored.html.contains("<td>"));
        }
    }
}
