//! Desktop-only retrieval experiment. This crate is deliberately outside the Tauri app.

mod storage;

use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

const MODEL_TOKENS: usize = 320;
const OVERLAP_TOKENS: usize = 40;
const EVIDENCE_BODY_TOKENS: usize = 32;

#[derive(Deserialize)]
struct Corpus {
    documents: Vec<Document>,
    queries: Vec<Query>,
    #[serde(default, rename = "snapshotSha256")]
    snapshot_sha256: Option<String>,
    #[serde(default, rename = "corpusSha256")]
    corpus_sha256: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueryInput {
    corpus_sha256: String,
    snapshot_sha256: Option<String>,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Document {
    id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
    sections: Vec<Section>,
}

#[derive(Deserialize)]
struct Section {
    heading: Option<String>,
    text: String,
    #[serde(default, alias = "pageIndex")]
    page_index: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    id: String,
    query: String,
    category: String,
    #[serde(default)]
    phrases: Vec<String>,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    relevant: Vec<Judgment>,
}

#[derive(Deserialize)]
struct Judgment {
    id: String,
    grade: u8,
}

#[derive(Deserialize)]
struct LexicalReport {
    queries: Vec<LexicalQuery>,
    #[serde(default)]
    corpus_sha256: Option<String>,
}

#[derive(Deserialize)]
struct LexicalQuery {
    id: String,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    request: Option<serde_json::Value>,
    ids: Vec<String>,
    #[serde(default)]
    response: Option<serde_json::Value>,
    #[serde(default)]
    retry: Option<serde_json::Value>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Passage {
    document: usize,
    section: usize,
    start: usize,
    end: usize,
    text: String,
    // Evidence selection metadata does not change the embedded text or vector cache identity.
    #[serde(skip_serializing, default)]
    body_tokens: usize,
}

struct RankedHit {
    passage: usize,
    score: f32,
    evidence: (usize, f32),
}

fn passages(
    model: &TextEmbedding,
    corpus: &Corpus,
) -> Result<Vec<Passage>, Box<dyn Error + Send + Sync>> {
    let mut tokenizer = model.tokenizer.clone();
    tokenizer.with_truncation(None)?;
    tokenizer.with_padding(None);
    let mut result = Vec::new();
    for (document, doc) in corpus.documents.iter().enumerate() {
        for (section, part) in doc.sections.iter().enumerate() {
            if part.text.trim().is_empty() {
                continue;
            }
            let header = format!(
                "passage: {}. {}. ",
                doc.title,
                part.heading.as_deref().unwrap_or("")
            );
            let header_tokens = tokenizer.encode(header.as_str(), false)?.len();
            let budget = MODEL_TOKENS
                .checked_sub(header_tokens + 4)
                .ok_or("title/heading exceeds the experiment's token budget")?;
            if budget <= OVERLAP_TOKENS {
                return Err("title/heading leaves too little room for passage text".into());
            }
            let encoding = tokenizer.encode(part.text.as_str(), false)?;
            let offsets = encoding.get_offsets();
            let mut first = 0;
            while first < offsets.len() {
                let last = (first + budget).min(offsets.len());
                let start = offsets[first].0;
                let end = offsets[last - 1].1;
                let source = part
                    .text
                    .get(start..end)
                    .ok_or("invalid tokenizer source offsets")?;
                let text = format!("{header}{source}");
                if tokenizer.encode(text.as_str(), true)?.len() > MODEL_TOKENS {
                    return Err("passage exceeds model token budget".into());
                }
                result.push(Passage {
                    document,
                    section,
                    start,
                    end,
                    text,
                    body_tokens: last - first,
                });
                if last == offsets.len() {
                    break;
                }
                first = last - OVERLAP_TOKENS;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
fn rank(
    corpus: &Corpus,
    passages: &[Passage],
    vectors: &[Vec<f32>],
    query: &[f32],
    scope: &[String],
) -> Vec<RankedHit> {
    rank_vectors(
        corpus,
        passages,
        vectors.iter().enumerate().map(Ok),
        query,
        scope,
        || Ok(()),
    )
    .unwrap()
}

fn rank_vectors<V: AsRef<[f32]>>(
    corpus: &Corpus,
    passages: &[Passage],
    vectors: impl Iterator<Item = Result<(usize, V), Box<dyn Error + Send + Sync>>>,
    query: &[f32],
    scope: &[String],
    mut check_cancelled: impl FnMut() -> Result<(), Box<dyn Error + Send + Sync>>,
) -> Result<Vec<RankedHit>, Box<dyn Error + Send + Sync>> {
    // ponytail: exact scan is O(passages), with O(documents) maxima; ANN only if device measurements require it.
    check_cancelled()?;
    let allowed = scope.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut scores = HashMap::<usize, (usize, f32)>::new();
    let mut evidence = HashMap::<usize, (usize, f32)>::new();
    for row in vectors {
        check_cancelled()?;
        let (index, vector) = row?;
        let passage = passages
            .get(index)
            .ok_or("invalid stored passage ordinal")?;
        let vector = vector.as_ref();
        if vector.len() != query.len() {
            return Err("scan vector dimension mismatch".into());
        }
        let id = corpus.documents[passage.document].id.as_str();
        if !scope.is_empty() && !allowed.contains(id) {
            continue;
        }
        let score = query.iter().zip(vector).map(|(a, b)| a * b).sum::<f32>();
        let best = scores.entry(passage.document).or_insert((index, score));
        if score > best.1 {
            *best = (index, score);
        }
        // ponytail: length chooses lead context, not relevance; also retain the score-winning source below.
        if passage.body_tokens >= EVIDENCE_BODY_TOKENS {
            let best = evidence.entry(passage.document).or_insert((index, score));
            if score > best.1 {
                *best = (index, score);
            }
        }
    }
    let mut hits = scores.into_values().collect::<Vec<_>>();
    hits.sort_by(|(left, left_score), (right, right_score)| {
        right_score.total_cmp(left_score).then_with(|| {
            corpus.documents[passages[*left].document]
                .id
                .cmp(&corpus.documents[passages[*right].document].id)
        })
    });
    Ok(hits
        .into_iter()
        .map(|(passage, score)| RankedHit {
            passage,
            score,
            evidence: evidence
                .get(&passages[passage].document)
                .copied()
                .unwrap_or((passage, score)),
        })
        .collect())
}

fn check_cancelled(path: Option<&Path>) -> Result<(), Box<dyn Error + Send + Sync>> {
    if path.map(Path::try_exists).transpose()?.unwrap_or(false) {
        return Err("experiment cancelled; no partial index or candidate report accepted".into());
    }
    Ok(())
}

fn source_hit(corpus: &Corpus, passage: &Passage, score: f32) -> serde_json::Value {
    let document = &corpus.documents[passage.document];
    let section = &document.sections[passage.section];
    source_evidence(
        document,
        section,
        passage,
        &section.text[passage.start..passage.end],
        score,
    )
}

fn semantic_evidence(
    hit: &RankedHit,
    mut load: impl FnMut(usize, f32) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>>,
) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let mut source = load(hit.evidence.0, hit.evidence.1)?;
    source["documentCosine"] = serde_json::json!(hit.score);
    if hit.passage != hit.evidence.0 {
        // Preserve context that displaced heading/citation noise without excluding concise answers.
        source["passages"] = serde_json::json!([source.clone(), load(hit.passage, hit.score)?]);
    }
    Ok(source)
}

fn source_evidence(
    document: &Document,
    section: &Section,
    passage: &Passage,
    source: &str,
    score: f32,
) -> serde_json::Value {
    // Semantic excerpts are authored source, escaped only for native-style inert review rendering.
    let excerpt = source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    serde_json::json!({"documentId": document.id, "title": document.title,
        "url": document.url, "sectionIndex": passage.section, "sectionTitle": section.heading,
        "pageIndex": section.page_index, "excerpt": excerpt, "byteStart": passage.start,
        "byteEnd": passage.end, "cosine": score})
}

#[derive(Serialize, Deserialize)]
struct VectorCache {
    key: String,
    vectors: Vec<Vec<f32>>,
}

fn cached_vectors(
    path: &Path,
    key: &str,
    count: usize,
    dimension: usize,
) -> Result<Option<Vec<Vec<f32>>>, Box<dyn Error + Send + Sync>> {
    if !path.exists() {
        return Ok(None);
    }
    let cache: VectorCache = serde_json::from_slice(&fs::read(path)?)?;
    if cache.key != key || cache.vectors.len() != count {
        return Err(
            "incompatible vector cache; choose a new external path or remove it explicitly".into(),
        );
    }
    check_vectors(&cache.vectors, dimension)?;
    Ok(Some(cache.vectors))
}

fn new_private_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn private_path(path: &Path, existing: bool) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let full = if existing {
        path.canonicalize()?
    } else {
        path.parent()
            .unwrap_or(Path::new("."))
            .canonicalize()?
            .join(path.file_name().ok_or("invalid path")?)
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    if full.starts_with(root) {
        return Err("model, cache and candidate output must be outside the checkout".into());
    }
    Ok(full)
}

fn hybrid_evidence(
    id: &str,
    lexical: Option<&serde_json::Value>,
    semantic: &[serde_json::Value],
) -> Option<serde_json::Value> {
    let native = lexical
        .and_then(|response| response["results"].as_array())
        .and_then(|results| results.iter().find(|hit| hit["documentId"] == id));
    let conceptual = semantic.iter().find(|hit| hit["documentId"] == id);
    let mut hit = native.or(conceptual)?.clone();
    if let (Some(_), Some(source)) = (native, conceptual) {
        // Keep conceptual context separate from literal counts, marks and navigation targets.
        hit["semanticEvidence"] = source.clone();
    }
    Some(hit)
}

fn fuse(lexical: &[String], semantic: &[String]) -> Vec<String> {
    let mut scores = HashMap::<&str, f32>::new();
    for channel in [lexical, semantic] {
        for (rank, id) in channel.iter().take(50).enumerate() {
            *scores.entry(id).or_default() += 1.0 / (60.0 + (rank + 1) as f32);
        }
    }
    let mut hits = scores.into_iter().collect::<Vec<_>>();
    hits.sort_by(|(left_id, left_score), (right_id, right_score)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left_id.cmp(right_id))
    });
    hits.into_iter().map(|(id, _)| id.to_owned()).collect()
}

fn hybrid_results(
    ids: &[String],
    lexical: Option<&serde_json::Value>,
    semantic: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>, Box<dyn Error + Send + Sync>> {
    ids.iter()
        .take(10)
        .map(|id| {
            hybrid_evidence(id, lexical, semantic)
                .ok_or("hybrid result lacks source evidence".into())
        })
        .collect()
}

fn relevant_at_ten(ids: &[String], relevant: &[Judgment]) -> (usize, usize) {
    let labeled = relevant
        .iter()
        .filter(|item| item.grade > 0)
        .collect::<Vec<_>>();
    (
        labeled
            .iter()
            .filter(|item| ids.iter().take(10).any(|id| id == &item.id))
            .count(),
        labeled.len(),
    )
}

fn check_dimension(dimension: usize) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !matches!(dimension, 384 | 768) {
        return Err("only E5 Small (384) and Base (768) dimensions are supported".into());
    }
    Ok(())
}

fn model_dimension(config: &[u8]) -> Result<usize, Box<dyn Error + Send + Sync>> {
    let value: serde_json::Value = serde_json::from_slice(config)?;
    let dimension = usize::try_from(
        value["hidden_size"]
            .as_u64()
            .ok_or("missing model dimension")?,
    )?;
    check_dimension(dimension)?;
    Ok(dimension)
}

fn check_vectors(
    vectors: &[Vec<f32>],
    dimension: usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    check_dimension(dimension)?;
    for vector in vectors {
        if vector.len() != dimension || vector.iter().any(|value| !value.is_finite()) {
            return Err("invalid embedding dimension or value".into());
        }
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if (norm - 1.0).abs() > 0.01 {
            return Err("embedding is not unit normalized".into());
        }
    }
    Ok(())
}

fn quantization_mode(value: Option<&str>) -> Result<QuantizationMode, &'static str> {
    match value {
        None | Some("none") => Ok(QuantizationMode::None),
        Some("dynamic-int8") => Ok(QuantizationMode::Dynamic),
        _ => Err("SEARCH_V2_MODEL_QUANTIZATION must be none or dynamic-int8"),
    }
}

fn embed_passages(
    model: &mut TextEmbedding,
    chunks: &[Passage],
    quantization: QuantizationMode,
    dimension: usize,
) -> Result<Vec<Vec<f32>>, Box<dyn Error + Send + Sync>> {
    // Dynamic activation ranges depend on batch peers. One passage per call keeps indexing repeatable.
    let batch_size = if matches!(quantization, QuantizationMode::Dynamic) {
        1
    } else {
        16
    };
    let mut vectors = Vec::with_capacity(chunks.len());
    for batch in chunks.chunks(batch_size) {
        // Bound retained token-level outputs too: FastEmbed otherwise accumulates every internal batch.
        let output = model.embed(
            batch
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            Some(batch.len()),
        )?;
        if output.len() != batch.len() {
            return Err("wrong passage vector count".into());
        }
        check_vectors(&output, dimension)?;
        vectors.extend(output);
    }
    Ok(vectors)
}

fn load_model(
    model_dir: &Path,
    quantization: QuantizationMode,
) -> Result<(TextEmbedding, String, usize), Box<dyn Error + Send + Sync>> {
    let onnx_file = fs::read(model_dir.join("onnx/model.onnx"))?;
    let files = TokenizerFiles {
        tokenizer_file: fs::read(model_dir.join("tokenizer.json"))?,
        config_file: fs::read(model_dir.join("config.json"))?,
        special_tokens_map_file: fs::read(model_dir.join("special_tokens_map.json"))?,
        tokenizer_config_file: fs::read(model_dir.join("tokenizer_config.json"))?,
    };
    let dimension = model_dimension(&files.config_file)?;
    let size = if dimension == 384 { "small" } else { "base" };
    let mut identity = Sha256::new();
    identity.update(format!(
        "e5-{size}/fastembed-7.1.0/mean/l2/dim{dimension}/chunks-v2-untruncated/prefixes-v1/320/40"
    ));
    if matches!(quantization, QuantizationMode::Dynamic) {
        identity.update(b"/dynamic-int8/per-text-v1");
    }
    for bytes in [
        &onnx_file,
        &files.tokenizer_file,
        &files.config_file,
        &files.special_tokens_map_file,
        &files.tokenizer_config_file,
    ] {
        identity.update(Sha256::digest(bytes));
    }
    let model_sha256 = format!("{:x}", identity.finalize());
    let model = TextEmbedding::try_new_from_user_defined(
        UserDefinedEmbeddingModel::new(onnx_file, files)
            .with_pooling(Pooling::Mean)
            .with_quantization(quantization),
        InitOptionsUserDefined::new()
            .with_max_length(512)
            .with_intra_threads(2),
    )?;
    Ok((model, model_sha256, dimension))
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = std::env::args().collect::<Vec<_>>();
    let query_only = args.get(1).is_some_and(|arg| arg == "--query-index");
    if args.len() != if query_only { 5 } else { 3 } {
        return Err("usage: search-v2-embeddings CORPUS_JSON LEXICAL_REPORT_JSON or --query-index INDEX_SQLITE QUERIES_JSON LEXICAL_REPORT_JSON".into());
    }
    let source_started = Instant::now();
    let corpus_bytes = fs::read(Path::new(&args[if query_only { 3 } else { 1 }]))?;
    let source_input_bytes = corpus_bytes.len();
    let query_index = if query_only {
        Some(private_path(Path::new(&args[2]), true)?)
    } else {
        None
    };
    let mut store = query_index
        .as_ref()
        .map(|path| storage::Store::read_only(path))
        .transpose()?;
    let (mut corpus, stored_chunks, stored_key, corpus_sha256) = if let Some(store) = &store {
        let input: QueryInput = serde_json::from_slice(&corpus_bytes)?;
        let mut snapshot = store.snapshot()?;
        if input.corpus_sha256 != snapshot.corpus_sha256
            || input.snapshot_sha256 != snapshot.corpus.snapshot_sha256
        {
            return Err("query input belongs to another indexed source snapshot".into());
        }
        snapshot.corpus.queries = input.queries;
        (
            snapshot.corpus,
            Some(snapshot.chunks),
            Some(snapshot.key),
            snapshot.corpus_sha256,
        )
    } else {
        (
            serde_json::from_slice::<Corpus>(&corpus_bytes)?,
            None,
            None,
            format!("{:x}", Sha256::digest(&corpus_bytes)),
        )
    };
    corpus.corpus_sha256 = Some(corpus_sha256.clone());
    drop(corpus_bytes);
    let source_secs = source_started.elapsed().as_secs_f64();
    let document_ids = corpus
        .documents
        .iter()
        .map(|doc| doc.id.as_str())
        .collect::<HashSet<_>>();
    if document_ids.len() != corpus.documents.len() || document_ids.contains("") {
        return Err("duplicate or empty document identity".into());
    }
    for query in &corpus.queries {
        if query
            .relevant
            .iter()
            .any(|item| !document_ids.contains(item.id.as_str()))
            || query
                .scope
                .iter()
                .any(|id| !document_ids.contains(id.as_str()))
        {
            return Err("unknown document in judgments or scope".into());
        }
    }
    let lexical_bytes = fs::read(Path::new(&args[if query_only { 4 } else { 2 }]))?;
    let report: LexicalReport = serde_json::from_slice(&lexical_bytes)?;
    if report
        .corpus_sha256
        .as_ref()
        .is_some_and(|hash| hash != &corpus_sha256)
    {
        return Err("lexical report belongs to another corpus".into());
    }
    if (query_only || corpus.documents.iter().any(|doc| doc.url.is_some()))
        && report.corpus_sha256.is_none()
    {
        return Err(
            "private source-linked comparison requires a corpus-bound lexical report".into(),
        );
    }
    let mut lexical = HashMap::new();
    for row in report.queries {
        let case = corpus
            .queries
            .iter()
            .find(|case| case.id == row.id)
            .ok_or("unknown lexical query")?;
        if row.query.as_ref().is_some_and(|query| query != &case.query) {
            return Err("lexical query text does not match corpus question".into());
        }
        if corpus.documents.iter().any(|doc| doc.url.is_some()) {
            let request = row
                .request
                .as_ref()
                .ok_or("missing production query constraints")?;
            let phrases: Vec<String> = serde_json::from_value(request["exactPhrases"].clone())?;
            let urls: Option<Vec<String>> =
                serde_json::from_value(request["documentUrls"].clone())?;
            let actual_scope = corpus
                .documents
                .iter()
                .filter(|doc| {
                    urls.as_ref()
                        .is_some_and(|urls| doc.url.as_ref().is_some_and(|url| urls.contains(url)))
                })
                .map(|doc| doc.id.as_str())
                .collect::<HashSet<_>>();
            if phrases != case.phrases
                || actual_scope != case.scope.iter().map(String::as_str).collect()
                || urls.as_ref().is_some_and(|urls| {
                    urls.iter().any(|url| {
                        !corpus
                            .documents
                            .iter()
                            .any(|doc| doc.url.as_ref() == Some(url))
                    })
                })
            {
                return Err("lexical phrase or document scope differs from semantic scope".into());
            }
            let result_ids: Vec<String> = serde_json::from_value(serde_json::Value::Array(
                row.response.as_ref().ok_or("missing native evidence")?["results"]
                    .as_array()
                    .ok_or("invalid native evidence")?
                    .iter()
                    .map(|hit| hit["documentId"].clone())
                    .collect(),
            ))?;
            if result_ids != row.ids {
                return Err("native evidence and lexical ranks differ".into());
            }
        }
        let allowed = if case.scope.is_empty() {
            &document_ids
        } else {
            &case.scope.iter().map(String::as_str).collect()
        };
        if row.ids.iter().any(|id| !allowed.contains(id.as_str()))
            || row.ids.iter().collect::<HashSet<_>>().len() != row.ids.len()
            || lexical.insert(row.id.clone(), row).is_some()
        {
            return Err("duplicate or out-of-scope lexical result".into());
        }
    }
    if lexical.len() != corpus.queries.len() {
        return Err("query pools do not match".into());
    }
    let candidate_path = std::env::var_os("SEARCH_V2_CANDIDATES_JSON")
        .map(|path| private_path(Path::new(&path), false))
        .transpose()?;
    if candidate_path.as_ref().is_some_and(|path| path.exists()) {
        return Err("candidate output must be new".into());
    }
    let vector_path = std::env::var_os("SEARCH_V2_VECTOR_CACHE")
        .map(|path| private_path(Path::new(&path), Path::new(&path).exists()))
        .transpose()?;
    let sqlite_path = std::env::var_os("SEARCH_V2_VECTOR_SQLITE")
        .map(|path| private_path(Path::new(&path), Path::new(&path).exists()))
        .transpose()?;
    if query_only && (vector_path.is_some() || sqlite_path.is_some()) {
        return Err(
            "query-only index is selected by its argument; do not also set vector store variables"
                .into(),
        );
    }
    let sqlite_path = query_index.or(sqlite_path);
    if vector_path.is_some() && sqlite_path.is_some() {
        return Err("choose one vector store: JSON cache or SQLite".into());
    }
    let cancel_path = std::env::var_os("SEARCH_V2_CANCEL_FILE")
        .map(|path| private_path(Path::new(&path), Path::new(&path).exists()))
        .transpose()?;
    for path in [&vector_path, &sqlite_path, &cancel_path]
        .into_iter()
        .flatten()
    {
        if candidate_path.as_ref() == Some(path) {
            return Err("candidate output must not share a store or cancellation path".into());
        }
    }
    if cancel_path.is_some() && (cancel_path == sqlite_path || cancel_path == vector_path) {
        return Err("cancellation marker must not share the vector store path".into());
    }
    check_cancelled(cancel_path.as_deref())?;
    let model_dir = private_path(
        Path::new(
            &std::env::var_os("FASTEMBED_MODEL_DIR")
                .ok_or("set FASTEMBED_MODEL_DIR to a cached model snapshot outside the checkout")?,
        ),
        true,
    )?;
    let quantization_value = std::env::var_os("SEARCH_V2_MODEL_QUANTIZATION")
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "invalid quantization policy encoding")
        })
        .transpose()?;
    let quantization = quantization_mode(quantization_value.as_deref())?;
    let load_started = Instant::now();
    let (mut model, model_sha256, dimension) = load_model(&model_dir, quantization)?;
    let load_secs = load_started.elapsed().as_secs_f64();
    let chunk_started = Instant::now();
    let chunks = if let Some(chunks) = stored_chunks {
        chunks
    } else {
        passages(&model, &corpus)?
    };
    let chunk_secs = chunk_started.elapsed().as_secs_f64();
    let mut digest = Sha256::new();
    digest.update(model_sha256.as_bytes());
    if !query_only {
        digest.update(serde_json::to_vec(&chunks)?);
    }
    let key = stored_key.unwrap_or_else(|| format!("{:x}", digest.finalize()));
    eprintln!(
        "Prepared {} structurally anchored passages; checking vector cache.",
        chunks.len()
    );
    let index_started = Instant::now();
    if let Some(store) = &store {
        store.check_model(&model_sha256, dimension)?;
    } else {
        store = sqlite_path
            .as_ref()
            .map(|path| storage::Store::open(path, &model_sha256, dimension))
            .transpose()?;
    }
    let index_stats = store
        .as_mut()
        .filter(|_| !query_only)
        .map(|store| {
            store.sync(
                &corpus,
                &chunks,
                if matches!(quantization, QuantizationMode::Dynamic) {
                    1
                } else {
                    16
                },
                |batch| embed_passages(&mut model, batch, quantization, dimension),
                || check_cancelled(cancel_path.as_deref()),
            )
        })
        .transpose()?;
    let cached = vector_path
        .as_ref()
        .map(|path| cached_vectors(path, &key, chunks.len(), dimension))
        .transpose()?
        .flatten();
    let reused = query_only
        || index_stats
            .as_ref()
            .map_or(cached.is_some(), |stats| stats.embedded == 0);
    let vectors = if store.is_some() {
        Vec::new()
    } else if let Some(vectors) = cached {
        vectors
    } else {
        let vectors = embed_passages(&mut model, &chunks, quantization, dimension)?;
        if let Some(path) = &vector_path {
            // ponytail: immutable whole-probe cache; incremental SQLite indexing remains a separate gate.
            let mut file = new_private_file(path)?;
            let data = serde_json::to_vec(&VectorCache {
                key: key.clone(),
                vectors: vectors.clone(),
            })?;
            file.write_all(&data)?;
        }
        vectors
    };
    let index_secs = index_started.elapsed().as_secs_f64();
    let mut totals = [[0usize; 2]; 3];
    let mut conceptual = [[0usize; 2]; 3];
    let mut top_one = [0usize; 3];
    let mut query_ms = Vec::new();
    let mut compared = 0;
    let mut candidates = Vec::new();
    let mut loaded_evidence_passages = 0;
    let mut loaded_evidence_bytes = 0;
    for case in &corpus.queries {
        check_cancelled(cancel_path.as_deref())?;
        if !case.phrases.is_empty() {
            let row = lexical
                .get(&case.id)
                .ok_or("missing quoted lexical query")?;
            candidates.push(serde_json::json!({"id": case.id, "query": case.query,
                "request": row.request, "response": row.response, "retry": row.retry, "semantic": null, "hybrid": null,
                "semanticFirstResponse": null,
                "excluded": "mandatory quotes: lexical control only"}));
            continue;
        }
        let row = lexical
            .get(&case.id)
            .ok_or("missing lexical result for comparison")?;
        let literal = &row.ids;
        let started = Instant::now();
        let query = model.embed([format!("query: {}", case.query)], None)?;
        check_vectors(&query, dimension)?;
        let embedding_ms = started.elapsed().as_secs_f64() * 1000.0;
        let scan_started = Instant::now();
        let query_vector = query.first().ok_or("model returned no query vector")?;
        let semantic_hits = if let Some(store) = &store {
            store.rank(&corpus, &chunks, query_vector, &case.scope, || {
                check_cancelled(cancel_path.as_deref())
            })?
        } else {
            rank_vectors(
                &corpus,
                &chunks,
                vectors.iter().enumerate().map(Ok),
                query_vector,
                &case.scope,
                || check_cancelled(cancel_path.as_deref()),
            )?
        };
        let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
        let evidence_started = Instant::now();
        let semantic = semantic_hits
            .iter()
            .map(|hit| corpus.documents[chunks[hit.passage].document].id.clone())
            .collect::<Vec<_>>();
        let hybrid = fuse(literal, &semantic);
        // Both channels supply context for displayed hybrid hits; source loading remains bounded.
        let needed = semantic
            .iter()
            .take(10)
            .chain(hybrid.iter().take(10))
            .collect::<HashSet<_>>();
        let source_hits = semantic_hits
            .iter()
            .take(50)
            .filter(|hit| needed.contains(&corpus.documents[chunks[hit.passage].document].id))
            .map(|hit| {
                semantic_evidence(hit, |index, score| {
                    check_cancelled(cancel_path.as_deref())?;
                    let passage = &chunks[index];
                    let source = if query_only {
                        store
                            .as_ref()
                            .ok_or("missing query index")?
                            .source_hit(&corpus, &chunks, index, score)?
                    } else {
                        source_hit(&corpus, passage, score)
                    };
                    loaded_evidence_passages += 1;
                    loaded_evidence_bytes += passage.end - passage.start;
                    Ok(source)
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error + Send + Sync>>>()?;
        let hybrid_hits = hybrid_results(&hybrid, row.response.as_ref(), &source_hits)?;
        let semantic_first_hits = hybrid_results(&semantic, row.response.as_ref(), &source_hits)?;
        candidates.push(serde_json::json!({
            "id": case.id, "query": case.query,
            "semanticMeasurements": {"queryEmbeddingMs": embedding_ms, "scanMs": scan_ms, "fusionEvidenceMs": evidence_started.elapsed().as_secs_f64() * 1000.0},
            "request": row.request, "response": row.response, "retry": row.retry,
            "semanticResponse": {"results": source_hits.iter().take(10).collect::<Vec<_>>()},
            "hybridResponse": {"results": hybrid_hits},
            "semanticFirstResponse": {"results": semantic_first_hits},
            "lexical": literal.iter().take(10).collect::<Vec<_>>(),
            "semantic": semantic.iter().take(10).collect::<Vec<_>>(),
            "hybrid": hybrid.iter().take(10).collect::<Vec<_>>(),
        }));
        query_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        if !case.relevant.iter().any(|item| item.grade > 0) {
            continue;
        }
        for (channel, ids) in [literal.as_slice(), semantic.as_slice(), hybrid.as_slice()]
            .iter()
            .enumerate()
        {
            let (found, labeled) = relevant_at_ten(ids, &case.relevant);
            totals[channel][0] += found;
            totals[channel][1] += labeled;
            if ids.first().is_some_and(|id| {
                case.relevant
                    .iter()
                    .any(|item| item.grade > 0 && item.id == *id)
            }) {
                top_one[channel] += 1;
            }
            if case.category == "concept-gap" {
                conceptual[channel][0] += found;
                conceptual[channel][1] += labeled;
            }
        }
        compared += 1;
    }
    if let Some(path) = candidate_path {
        check_cancelled(cancel_path.as_deref())?;
        let report = serde_json::json!({"corpusSha256": corpus_sha256,
            "snapshotSha256": corpus.snapshot_sha256,
            "lexicalSha256": format!("{:x}", Sha256::digest(&lexical_bytes)),
            "modelSha256": model_sha256, "dimension": dimension, "modelQuantization": quantization_value.as_deref().unwrap_or("none"), "vectorCacheKey": key, "reusedVectors": reused,
            "vectorStorage": if store.is_some() { "sqlite" } else { "json-or-memory" }, "indexChanges": index_stats,
            "documents": corpus.documents.len(), "passages": chunks.len(), "queries": candidates,
            "semanticEvidencePolicy": {"minimumContextBodyTokens": EVIDENCE_BODY_TOKENS, "retainScoreWinner": true, "maximumPassages": 2, "shortSourceFallback": true},
            "hybridEvidencePolicy": {"lexical": "unchanged-first", "semantic": "separate-verified-context", "maximumLexicalPassages": 3, "maximumSemanticPassages": 2},
            "sourceLoading": {"mode": if query_only { "winning-only" } else { "whole-corpus" }, "inputSeconds": source_secs, "inputBytes": source_input_bytes, "authoredBytesRetainedBeforeQueries": corpus.documents.iter().flat_map(|doc| &doc.sections).map(|section| section.text.len()).sum::<usize>(), "embeddedTextBytesRetainedBeforeQueries": chunks.iter().map(|chunk| chunk.text.len()).sum::<usize>(), "evidencePassages": loaded_evidence_passages, "evidenceBytes": loaded_evidence_bytes, "metadataPassages": chunks.len()},
            "loadSeconds": load_secs, "chunkSeconds": chunk_secs, "indexOrCacheSeconds": index_secs, "queryMilliseconds": query_ms});
        serde_json::to_writer_pretty(new_private_file(&path)?, &report)?;
    }
    query_ms.sort_by(|a, b| a.total_cmp(b));
    let p95 = query_ms
        .get((query_ms.len() * 95 / 100).min(query_ms.len().saturating_sub(1)))
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "n/a".into());
    println!("documents={} passages={} model_load_s={load_secs:.2} index_or_cache_s={index_secs:.2} reused={reused} query_p95_ms={p95} raw_vector_bytes={}", corpus.documents.len(), chunks.len(), chunks.len() * dimension * 4);
    println!("judged_unquoted_queries={compared} (quotes excluded; incomplete labels)");
    if compared > 0 {
        for (index, name) in ["lexical", "semantic", "hybrid"].iter().enumerate() {
            println!(
                "{name}: recall@10={}/{} top1={}/{} concept={}/{}",
                totals[index][0],
                totals[index][1],
                top_one[index],
                compared,
                conceptual[index][0],
                conceptual[index][1]
            );
        }
    }
    // Source spans remain attached to every vector; this experiment does not render them.
    debug_assert!(chunks.iter().all(|chunk| chunk.end > chunk.start
        && chunk.section < corpus.documents[chunk.document].sections.len()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fusion_deduplicates_and_prefers_agreement() {
        let lexical = vec!["a".into(), "b".into()];
        let semantic = vec!["b".into(), "c".into()];
        assert_eq!(fuse(&lexical, &semantic), ["b", "a", "c"]);
    }

    #[test]
    fn semantic_first_assembly_preserves_native_evidence_and_bounds() {
        let native = serde_json::json!({"results": [{
            "documentId": "record", "title": "Mira 1924", "excerpt": "Mira arrived in <mark>1924</mark>.",
            "sectionIndex": 2, "pageIndex": 7, "matchCount": null, "matchingSections": 1, "matchScope": "section",
            "termMatches": [{"term": "1924", "matchingSections": 1, "sectionIndex": 2, "pageIndex": 7, "text": "1924"}],
            "matchLocations": [{"binIndex": 0, "matchCount": 1, "sectionIndex": 2, "pageIndex": 7, "text": "1924"}],
            "passages": [{"sectionIndex": 2, "pageIndex": 7, "excerpt": "Mira arrived in 1924."}]
        }]});
        let context = serde_json::json!({"documentId": "record", "sectionIndex": 0, "byteStart": 0, "byteEnd": 8, "excerpt": "Context."});
        let other = serde_json::json!({"documentId": "other", "sectionIndex": 1, "byteStart": 0, "byteEnd": 7, "excerpt": "Source."});
        let order = vec!["other".into(), "record".into()];
        let mut hits =
            hybrid_results(&order, Some(&native), &[context.clone(), other.clone()]).unwrap();
        assert_eq!(
            hits[0], other,
            "semantic-only hits retain their source evidence"
        );
        assert_eq!(
            hits[1].as_object_mut().unwrap().remove("semanticEvidence"),
            Some(context)
        );
        assert_eq!(
            hits[1], native["results"][0],
            "names, dates, counts and native navigation are unchanged"
        );
        assert_eq!(
            hybrid_results(&["record".into()], Some(&native), &[]).unwrap(),
            [native["results"][0].clone()]
        );
        assert!(hybrid_results(&["missing".into()], Some(&native), &[]).is_err());
        assert!(hybrid_results(&[], Some(&native), &[]).unwrap().is_empty());
        let bounded = (0..10).map(|i| format!("record-{i}")).collect::<Vec<_>>();
        let evidence = bounded
            .iter()
            .map(|id| serde_json::json!({"documentId": id}))
            .collect::<Vec<_>>();
        let mut order = bounded;
        order.push("missing".into());
        assert_eq!(hybrid_results(&order, None, &evidence).unwrap().len(), 10);
    }
    #[test]
    fn cached_ranking_keeps_source_scope_and_rejects_incompatible_vectors() {
        let corpus: Corpus = serde_json::from_value(serde_json::json!({
            "documents": [
                {"id": "a", "title": "Compass guide", "sections": [{"text": "compass <source>", "page_index": 4}]},
                {"id": "b", "title": "Archive guide", "sections": [{"text": "archive source"}]}
            ], "queries": []
        })).unwrap();
        let chunks = vec![
            Passage {
                document: 0,
                section: 0,
                start: 0,
                end: 16,
                text: "passage: compass <source>".into(),
                body_tokens: 3,
            },
            Passage {
                document: 1,
                section: 0,
                start: 0,
                end: 14,
                text: "passage: archive source".into(),
                body_tokens: 2,
            },
        ];
        let mut vector = vec![0.0; 384];
        vector[0] = 1.0;
        let vectors = vec![vector.clone(), vector.clone()];
        assert_eq!(
            rank(&corpus, &chunks, &vectors, &vector, &[])
                .iter()
                .map(|h| (h.passage, h.score))
                .collect::<Vec<_>>(),
            [(0, 1.0), (1, 1.0)]
        );
        assert_eq!(
            rank(&corpus, &chunks, &vectors, &vector, &["b".into()])
                .iter()
                .map(|h| (h.passage, h.score))
                .collect::<Vec<_>>(),
            [(1, 1.0)]
        );
        let hit = source_hit(&corpus, &chunks[0], 1.0);
        assert_eq!(hit["pageIndex"], 4);
        assert_eq!(hit["excerpt"], "compass &lt;source&gt;");
        assert_eq!(hit["byteEnd"], 16);
        let lexical = serde_json::json!({"results": [{"documentId": "a", "excerpt": "Existing lexical evidence", "sectionIndex": 1, "pageIndex": 5}]});
        let mut combined =
            hybrid_evidence("a", Some(&lexical), std::slice::from_ref(&hit)).unwrap();
        assert_eq!(
            combined.as_object_mut().unwrap().remove("semanticEvidence"),
            Some(hit.clone())
        );
        assert_eq!(
            combined, lexical["results"][0],
            "literal evidence remains unchanged"
        );
        assert_eq!(
            hybrid_evidence("a", Some(&lexical), &[]).unwrap(),
            lexical["results"][0]
        );
        assert_eq!(
            hybrid_evidence("a", None, std::slice::from_ref(&hit)).unwrap(),
            hit
        );
        assert!(hybrid_evidence("unknown", Some(&lexical), &[hit]).is_none());

        let path =
            std::env::temp_dir().join(format!("papercut-vector-check-{}.json", std::process::id()));
        assert!(cached_vectors(&path, "missing", 2, 384).unwrap().is_none());
        fs::write(
            &path,
            serde_json::to_vec(&VectorCache {
                key: "model-and-content".into(),
                vectors: vectors.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            cached_vectors(&path, "model-and-content", 2, 384)
                .unwrap()
                .unwrap(),
            vectors
        );
        assert!(cached_vectors(&path, "changed-model-or-content", 2, 384).is_err());
        assert!(cached_vectors(&path, "model-and-content", 1, 384).is_err());
        let mut invalid = vectors;
        invalid[0][0] = 0.0;
        fs::write(
            &path,
            serde_json::to_vec(&VectorCache {
                key: "model-and-content".into(),
                vectors: invalid,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(cached_vectors(&path, "model-and-content", 2, 384).is_err());
        fs::remove_file(path).unwrap();
        assert!(check_vectors(&[vec![f32::NAN; 384]], 384).is_err());
        assert!(check_vectors(&[vec![1.0; 383]], 384).is_err());
        let mut base = vec![0.0; 768];
        base[0] = 1.0;
        check_vectors(&[base.clone()], 768).unwrap();
        assert!(check_vectors(&[base], 384).is_err());
        assert_eq!(model_dimension(br#"{"hidden_size":384}"#).unwrap(), 384);
        assert_eq!(model_dimension(br#"{"hidden_size":768}"#).unwrap(), 768);
        assert!(model_dimension(br#"{"hidden_size":1024}"#).is_err());
        assert!(model_dimension(b"{}").is_err());
    }

    #[test]
    fn body_evidence_preserves_document_order_and_short_source_fallback() {
        let corpus: Corpus = serde_json::from_value(serde_json::json!({
            "documents": [
                {"id": "a", "title": "Navigation", "sections": [{"text": "Index"}, {"text": "A substantive source passage.", "page_index": 3}]},
                {"id": "b", "title": "Brief note", "sections": [{"text": "Short answer."}]}
            ], "queries": []
        })).unwrap();
        let make = |document, section, body_tokens| Passage {
            document,
            section,
            start: 0,
            end: corpus.documents[document].sections[section].text.len(),
            text: corpus.documents[document].sections[section].text.clone(),
            body_tokens,
        };
        let mut chunks = vec![
            make(0, 0, 1),
            make(0, 1, EVIDENCE_BODY_TOKENS),
            make(1, 0, 2),
        ];
        let ranked = rank(
            &corpus,
            &chunks,
            &[vec![1.0], vec![0.8], vec![0.9]],
            &[1.0],
            &[],
        );
        assert_eq!(
            ranked
                .iter()
                .map(|h| (h.passage, h.score))
                .collect::<Vec<_>>(),
            [(0, 1.0), (2, 0.9)]
        );
        assert_eq!(ranked[0].evidence, (1, 0.8));
        assert_eq!(ranked[1].evidence, (2, 0.9));
        let hit = source_hit(&corpus, &chunks[ranked[0].evidence.0], ranked[0].evidence.1);
        assert_eq!(hit["excerpt"], "A substantive source passage.");
        assert_eq!(hit["sectionIndex"], 1);
        assert_eq!(hit["pageIndex"], 3);
        assert_eq!(hit["cosine"], serde_json::json!(0.8f32));
        let mut loaded = Vec::new();
        let combined = semantic_evidence(&ranked[0], |index, score| {
            loaded.push(index);
            Ok(source_hit(&corpus, &chunks[index], score))
        })
        .unwrap();
        assert_eq!(loaded, [1, 0]);
        assert_eq!(combined["excerpt"], hit["excerpt"]);
        assert_eq!(combined["passages"][0]["pageIndex"], 3);
        assert_eq!(combined["passages"][1]["excerpt"], "Index");
        assert_eq!(combined["passages"][1]["cosine"], 1.0);
        assert_eq!(combined["passages"][1]["sectionIndex"], 0);
        let brief = semantic_evidence(&ranked[1], |index, score| {
            assert_eq!(index, 2);
            Ok(source_hit(&corpus, &chunks[index], score))
        })
        .unwrap();
        assert!(
            brief.get("passages").is_none(),
            "do not duplicate one source"
        );
        assert!(semantic_evidence(&ranked[0], |index, score| {
            if index == ranked[0].passage {
                return Err("score-winning source failed validation".into());
            }
            Ok(source_hit(&corpus, &chunks[index], score))
        })
        .is_err());
        let scoped = rank(
            &corpus,
            &chunks,
            &[vec![1.0], vec![0.8], vec![0.9]],
            &[1.0],
            &["b".into()],
        );
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].evidence, (2, 0.9));
        let serialized = serde_json::to_vec(&chunks).unwrap();
        chunks[1].body_tokens = EVIDENCE_BODY_TOKENS - 1;
        assert_eq!(
            serde_json::to_vec(&chunks).unwrap(),
            serialized,
            "selection metadata must not invalidate unchanged passage vectors"
        );
        assert_eq!(
            rank(
                &corpus,
                &chunks,
                &[vec![1.0], vec![0.8], vec![0.9]],
                &[1.0],
                &[]
            )[0]
            .evidence,
            (0, 1.0)
        );
    }

    #[test]
    fn quantization_requires_an_explicit_supported_policy() {
        assert!(matches!(
            quantization_mode(None).unwrap(),
            QuantizationMode::None
        ));
        assert!(matches!(
            quantization_mode(Some("none")).unwrap(),
            QuantizationMode::None
        ));
        assert!(matches!(
            quantization_mode(Some("dynamic-int8")).unwrap(),
            QuantizationMode::Dynamic
        ));
        assert!(quantization_mode(Some("int4")).is_err());
        assert!(quantization_mode(Some("")).is_err());
    }

    #[test]
    #[ignore = "requires an explicit cached model snapshot; no downloads"]
    fn bounded_indexing_preserves_vectors_across_order_and_resume() {
        let directory = std::env::var_os("FASTEMBED_MODEL_DIR").expect("cached model snapshot");
        let mode = quantization_mode(
            std::env::var("SEARCH_V2_MODEL_QUANTIZATION")
                .ok()
                .as_deref(),
        )
        .unwrap();
        let (mut model, identity, dimension) = load_model(Path::new(&directory), mode).unwrap();
        let corpus: Corpus = serde_json::from_value(serde_json::json!({
            "documents": [
                {"id": "guide", "title": "Navigation", "sections": [
                    {"text": "A compass points north; a map locates the route."},
                    {"text": "تساعد الخريطة والبوصلة على تحديد الطريق والاتجاه."}
                ]},
                {"id": "shelves", "title": "Library", "sections": vec![serde_json::json!({"text": "A library stores books in numbered shelves."}); 16]}
            ], "queries": []
        }))
        .unwrap();
        let mut chunks = passages(&model, &corpus).unwrap();
        let first = embed_passages(&mut model, &chunks, mode, dimension).unwrap();
        for question in ["How can a walker find north?", "كيف أحدد اتجاه الشمال؟"]
        {
            let query = model.embed([format!("query: {question}")], None).unwrap();
            check_vectors(&query, dimension).unwrap();
            let ranked = rank(&corpus, &chunks, &first, &query[0], &[]);
            assert_eq!(
                corpus.documents[chunks[ranked[0].passage].document].id,
                "guide"
            );
        }
        let resumed = embed_passages(&mut model, &chunks[1..], mode, dimension).unwrap();
        if matches!(mode, QuantizationMode::Dynamic) {
            assert_eq!(
                first[1..],
                resumed,
                "dynamic passage vectors must not depend on indexing peers"
            );
        } else {
            let original = model
                .embed(
                    chunks.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
                    Some(16),
                )
                .unwrap();
            assert_eq!(
                first, original,
                "bounded calls must preserve the original float32 batching"
            );
        }
        chunks.reverse();
        let reversed = embed_passages(&mut model, &chunks, mode, dimension).unwrap();
        if matches!(mode, QuantizationMode::Dynamic) {
            assert_eq!(first.iter().rev().cloned().collect::<Vec<_>>(), reversed);
        }
        let path = std::env::temp_dir().join(format!(
            "papercut-model-sqlite-{}.sqlite3",
            std::process::id()
        ));
        let mut store = storage::Store::open(&path, &identity, dimension).unwrap();
        let stats = store
            .sync(
                &corpus,
                &chunks,
                if matches!(mode, QuantizationMode::Dynamic) {
                    1
                } else {
                    16
                },
                |batch| embed_passages(&mut model, batch, mode, dimension),
                || Ok(()),
            )
            .unwrap();
        assert_eq!(stats.embedded, chunks.len());
        let query = model
            .embed(["query: How can a walker find north?"], None)
            .unwrap();
        let disk = store
            .rank(&corpus, &chunks, &query[0], &[], || Ok(()))
            .unwrap();
        let memory = rank(&corpus, &chunks, &reversed, &query[0], &[]);
        assert_eq!(
            disk.iter()
                .map(|h| (h.passage, h.score, h.evidence))
                .collect::<Vec<_>>(),
            memory
                .iter()
                .map(|h| (h.passage, h.score, h.evidence))
                .collect::<Vec<_>>()
        );
        drop(store);
        let mut store = storage::Store::open(&path, &identity, dimension).unwrap();
        let stats = store
            .sync(
                &corpus,
                &chunks,
                1,
                |_| panic!("restart must reuse model vectors"),
                || Ok(()),
            )
            .unwrap();
        assert_eq!((stats.embedded, stats.reused), (0, chunks.len()));
        drop(store);
        fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires an explicit cached model snapshot; no downloads"]
    fn long_sections_keep_their_tail_and_utf8_source_spans() {
        let directory = std::env::var_os("FASTEMBED_MODEL_DIR").expect("cached model snapshot");
        let (model, _, _) = load_model(
            Path::new(&directory),
            quantization_mode(
                std::env::var("SEARCH_V2_MODEL_QUANTIZATION")
                    .ok()
                    .as_deref(),
            )
            .unwrap(),
        )
        .unwrap();
        let text = format!(
            "{} نهاية الدليل tail-marker",
            "compass archive navigation ".repeat(900)
        );
        let corpus: Corpus = serde_json::from_value(serde_json::json!({
            "documents": [{"id": "long", "title": "Navigation", "sections": [{"text": text, "page_index": 2}]}],
            "queries": []
        })).unwrap();
        let chunks = passages(&model, &corpus).unwrap();
        assert!(
            chunks.len() > 3,
            "whole section must be tokenized before chunking"
        );
        assert_eq!(chunks.first().unwrap().start, 0);
        assert_eq!(chunks.last().unwrap().end, text.len());
        assert!(chunks.last().unwrap().text.contains("tail-marker"));
        for pair in chunks.windows(2) {
            assert!(pair[0].end >= pair[1].start);
        }
        for chunk in chunks {
            assert!(text.get(chunk.start..chunk.end).is_some());
            assert!(
                model
                    .tokenizer
                    .encode(chunk.text.as_str(), true)
                    .unwrap()
                    .len()
                    <= MODEL_TOKENS
            );
            assert_eq!(source_hit(&corpus, &chunk, 1.0)["pageIndex"], 2);
        }
    }
}
