//! Text cleanup and English-only synthesis normalization for native TTS.
//!
//! This module owns transformations applied only to the text copy sent into
//! sherpa-onnx. Source chunks, search/highlighting, cache identity, and bundle
//! metadata keep the original text.

use num2words::{Currency, Num2Words};

/// Clean Unicode text before model-specific tokenization: normalize
/// smart quotes/dashes/ellipses to ASCII, fold all whitespace to single spaces,
/// drop zero-width and control characters, and drop non-BMP symbols (emoji) that
/// can trip native tokenization. Language-specific normalization (e.g. English
/// year expansion) is applied separately by the caller, not here.
pub(super) fn sanitize_tts_text(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut previous_was_space = false;

    for ch in text.chars() {
        let mapped = match ch {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{2026}' => {
                cleaned.push_str("...");
                previous_was_space = false;
                continue;
            }
            '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}' => ' ',
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => continue,
            _ => ch,
        };

        // Collapse consecutive whitespace into one space.
        if mapped.is_whitespace() {
            if !previous_was_space {
                cleaned.push(' ');
                previous_was_space = true;
            }
            continue;
        }

        // Native espeak-backed paths are most reliable with BMP text.
        // Emoji and other non-BMP symbols can trip native tokenization, so
        // drop them before handing text to sherpa-onnx.
        if mapped.is_control() || mapped as u32 > 0xFFFF {
            continue;
        }

        cleaned.push(mapped);
        previous_was_space = false;
    }

    cleaned.trim().to_string()
}

/// Compose the English-only synthesis-text rewrites in the order their guards
/// expect: roman-numeral expansion first (operates on letters), then abbreviation
/// expansion (turns `Dr.`/`U.S.A.` into spoken words before any dot handling),
/// then clock-time expansion (consumes `H:MM` before the pause pass would see a
/// bare colon), then decimal-dot repair (so the following year pass never mistakes
/// a decimal fraction for a year), then year expansion, then currency expansion
/// (after year so a `$1984`-style amount is never read as a year), then
/// pause-punctuation softening last (it depends on abbreviations/initialisms/times
/// having already shed their dots and colons).
///
/// Like [`normalize_year_like_numbers`], every step runs only on the synthesis
/// copy, leaving source chunks, highlighting, search, bundle metadata, and cache
/// identity untouched. It is gated per-model by `english_text_normalization()`.
pub(super) fn normalize_english_synthesis_text(text: &str) -> String {
    let with_romans = expand_section_roman_numerals(text);
    let with_abbreviations = expand_abbreviations(&with_romans);
    let with_times = expand_clock_times(&with_abbreviations);
    let collapsed = collapse_decimal_dots(&with_times);
    let with_years = normalize_year_like_numbers(&collapsed);
    let with_currency = expand_currency(&with_years);
    soften_pause_punctuation(&with_currency)
}

/// Section keywords whose following uppercase roman numeral reads as a number.
/// Restricting expansion to these prefixes avoids rewriting the pronoun "I",
/// the grade "C", "X marks the spot", and other legitimate standalone letters.
const SECTION_WORDS: [&str; 10] = [
    "chapter", "part", "section", "book", "act", "volume", "appendix", "article", "canto", "scene",
];

/// Expand an uppercase roman numeral that directly follows a section keyword into
/// its cardinal digits, e.g. `Chapter IV` -> `Chapter 4`, `Part VII` -> `Part 7`.
/// eSpeak's own roman handling is inconsistent (it can announce "roman one"), so
/// we feed it plain digits instead. Post-`sanitize_tts_text` text is single-spaced
/// with no newlines, so splitting on a single space is sufficient and lossless.
///
/// The keyword must be capitalized to match. Several keywords (`book`, `act`,
/// `part`, `section`) are also common words, and a lowercase one is usually prose
/// followed by the pronoun "I" (`the book I read`); requiring a capital keeps the
/// match to genuine section references and avoids rewriting that "I" to "1". The
/// trade-off is that a lowercase reference (`in chapter IV`) is left for eSpeak.
fn expand_section_roman_numerals(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut previous_is_section = false;

    for word in text.split(' ') {
        let (lead, core, trail) = split_word_affixes(word);
        let is_section = core.starts_with(|ch: char| ch.is_ascii_uppercase())
            && SECTION_WORDS.contains(&core.to_ascii_lowercase().as_str());

        if previous_is_section {
            if let Some(value) = roman_to_u16(core) {
                out.push(format!("{lead}{value}{trail}"));
                // A numeral is never itself a section keyword for the next word.
                previous_is_section = false;
                continue;
            }
        }

        out.push(word.to_string());
        previous_is_section = is_section;
    }

    out.join(" ")
}

/// Split a whitespace-delimited word into leading punctuation, an alphanumeric
/// core, and trailing punctuation so `(VII),` keeps its wrappers while only the
/// core is tested as a numeral. An all-punctuation word yields an empty core.
fn split_word_affixes(word: &str) -> (&str, &str, &str) {
    let start = word
        .find(|ch: char| ch.is_ascii_alphanumeric())
        .unwrap_or(word.len());
    let end = word
        .rfind(|ch: char| ch.is_ascii_alphanumeric())
        .map(|index| index + 1)
        .unwrap_or(start);
    (&word[..start], &word[start..end], &word[end..])
}

/// Parse an uppercase roman numeral token into its value using the standard
/// right-to-left subtractive scan. Returns `None` for an empty token, any
/// non-roman or lowercase character, or a zero result, so only deliberate
/// uppercase numerals (`I`, `IV`, `Xii` is rejected) are ever expanded.
fn roman_to_u16(token: &str) -> Option<u16> {
    if token.is_empty() {
        return None;
    }
    let mut total: u16 = 0;
    let mut highest: u16 = 0;
    for ch in token.chars().rev() {
        let value = match ch {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if value < highest {
            total = total.checked_sub(value)?;
        } else {
            total = total.checked_add(value)?;
            highest = value;
        }
    }
    (total > 0).then_some(total)
}

/// Map a common abbreviation or unit core to its spoken form. The trailing period
/// that eSpeak/Kokoro otherwise reads as a sentence stop (the awkward pause after
/// `Dr. Smith`) disappears because the replacement carries no period. Keys are
/// matched case-sensitively so `Mr.` matches but a lowercased `mr` does not.
/// `St.` and `etc.` are handled separately (context-dependent Saint/Street, and a
/// sentence-final stop respectively).
fn abbreviation_expansion(core: &str) -> Option<&'static str> {
    Some(match core {
        "Mr." => "Mister",
        "Mrs." => "Missus",
        "Ms." => "Miss",
        "Dr." => "Doctor",
        "Drs." => "Doctors",
        "Prof." => "Professor",
        "Jr." => "Junior",
        "Sr." => "Senior",
        "Mt." => "Mount",
        "Ave." => "Avenue",
        "vs." => "versus",
        "e.g." => "for example",
        "i.e." => "that is",
        // Units (unambiguous multi-character tokens only).
        "mph" => "miles per hour",
        "km/h" | "kmh" | "kph" => "kilometers per hour",
        _ => return None,
    })
}

/// Expand common abbreviations on the synthesis copy so their trailing period is
/// not read as a sentence stop. Three cases, in priority order: a curated map of
/// titles/Latin forms (`Dr.` -> `Doctor`), the context-sensitive `St.` (Saint
/// before a capitalized word, Street otherwise), and dotted initialisms
/// (`U.S.A.` -> `USA`, `a.m.` -> `am`) whose dots are dropped so eSpeak spells the
/// letters without an inter-letter pause. Outer wrappers and trailing commas are
/// preserved around the replacement. Post-`sanitize_tts_text` text is
/// single-spaced, so splitting on a single space is sufficient.
fn expand_abbreviations(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());

    for (index, word) in words.iter().enumerate() {
        let (lead, core, trail) = strip_outer_punctuation(word);

        if let Some(expansion) = abbreviation_expansion(core) {
            out.push(format!("{lead}{expansion}{trail}"));
            continue;
        }

        if core == "etc." {
            // Unlike a title, `etc.` often ends a sentence. Keep the full stop
            // when nothing or a capitalized word follows and no other trailing
            // mark is already present; otherwise expand without a terminator.
            let next_is_capital_or_end = words
                .get(index + 1)
                .map(|next| strip_outer_punctuation(next).1)
                .map_or(true, |next_core| {
                    next_core.starts_with(|ch: char| ch.is_ascii_uppercase())
                });
            let expansion = if trail.is_empty() && next_is_capital_or_end {
                "et cetera."
            } else {
                "et cetera"
            };
            out.push(format!("{lead}{expansion}{trail}"));
            continue;
        }

        if core == "St." {
            // Saint before a capitalized name (`St. John`), Street otherwise
            // (`Main St.`). Looks at the next word's core for the capital.
            let next_is_capitalized = words
                .get(index + 1)
                .map(|next| strip_outer_punctuation(next).1)
                .is_some_and(|next_core| next_core.starts_with(|ch: char| ch.is_ascii_uppercase()));
            let expansion = if next_is_capitalized {
                "Saint"
            } else {
                "Street"
            };
            out.push(format!("{lead}{expansion}{trail}"));
            continue;
        }

        if let Some(collapsed) = collapse_initialism(core) {
            out.push(format!("{lead}{collapsed}{trail}"));
            continue;
        }

        out.push((*word).to_string());
    }

    out.join(" ")
}

/// Split a word into leading wrappers, a core, and trailing punctuation, stripping
/// only quotes/brackets at the front and commas/closing punctuation at the back.
/// Unlike [`split_word_affixes`], a `.` is never stripped, so an abbreviation's
/// own period stays part of its core (`(e.g.,` -> lead `(`, core `e.g.`, trail `,`).
fn strip_outer_punctuation(word: &str) -> (&str, &str, &str) {
    const LEAD: &[char] = &['(', '[', '{', '"', '\''];
    const TRAIL: &[char] = &[',', ';', ':', ')', ']', '}', '"', '\''];
    let start = word
        .char_indices()
        .find(|(_, ch)| !LEAD.contains(ch))
        .map(|(index, _)| index)
        .unwrap_or(word.len());
    let end = word
        .char_indices()
        .rev()
        .find(|(_, ch)| !TRAIL.contains(ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(start);
    if end < start {
        return ("", word, "");
    }
    (&word[..start], &word[start..end], &word[end..])
}

/// Split a word into its body and any trailing punctuation (`million.` ->
/// `("million", ".")`). Used so a unit voiced after a magnitude word lands before
/// that word's sentence mark (`$5 million.` -> `5 million dollars.`).
fn split_trailing_marks(word: &str) -> (&str, &str) {
    let body_len = word
        .trim_end_matches(|ch: char| ".,;:!?)]}\"'".contains(ch))
        .len();
    word.split_at(body_len)
}

/// Collapse a dotted initialism (`U.S.A.`, `N.Y.`, `a.m.`) to its bare letters so
/// eSpeak spells them without pausing between each one. Requires at least two
/// dots and a strict letter/dot alternation, so a sentence-final single token like
/// `A.` keeps its period (and thus its full stop) and a decimal like `3.14` or a
/// word are rejected. Returns the letters only when two or more were found.
fn collapse_initialism(core: &str) -> Option<String> {
    if core.matches('.').count() < 2 {
        return None;
    }
    let mut letters = String::new();
    let mut expect_letter = true;
    for ch in core.chars() {
        if expect_letter {
            if !ch.is_ascii_alphabetic() {
                return None;
            }
            letters.push(ch);
            expect_letter = false;
        } else {
            if ch != '.' {
                return None;
            }
            expect_letter = true;
        }
    }
    (letters.len() >= 2).then_some(letters)
}

/// Expand a clock time (`9:00`, `3:30`, `10:45`) into spoken words so eSpeak reads
/// it as a time rather than "nine zero zero". A trailing sentence mark is peeled
/// off first so `at 9:00.` still matches. Requires a two-digit minute, which keeps
/// ratios and references with a single-digit right side (`2:1`, `16:9`, `1:1`) out
/// of scope so the pause pass preserves their colon.
fn expand_clock_times(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();

    for word in text.split(' ') {
        let (lead, core_with_marks, trail) = strip_outer_punctuation(word);
        let mut core = core_with_marks;
        let mut sentence_tail = String::new();
        while let Some(rest) = core.strip_suffix(['.', '!', '?']) {
            sentence_tail.insert(0, core.as_bytes()[rest.len()] as char);
            core = rest;
        }

        if let Some(spoken) = clock_time_to_words(core) {
            out.push(format!("{lead}{spoken}{sentence_tail}{trail}"));
        } else {
            out.push(word.to_string());
        }
    }

    out.join(" ")
}

/// Convert a single `H:MM` or `HH:MM` token to spoken words, or `None` when it is
/// not a valid 24-hour time. `9:00` reads "nine o'clock", `9:05` reads "nine oh
/// five", and `10:45` reads "ten forty-five". The minute must be exactly two
/// digits, so a ratio like `2:1` is rejected and left for the colon to survive.
fn clock_time_to_words(core: &str) -> Option<String> {
    let (hour_part, minute_part) = core.split_once(':')?;
    if hour_part.is_empty()
        || hour_part.len() > 2
        || minute_part.len() != 2
        || !hour_part.chars().all(|ch| ch.is_ascii_digit())
        || !minute_part.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }

    let hour: u16 = hour_part.parse().ok()?;
    let minute: u16 = minute_part.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }

    let hour_words = cardinal_words(hour)?;
    Some(if minute == 0 {
        format!("{hour_words} o'clock")
    } else if minute < 10 {
        format!("{hour_words} oh {}", cardinal_words(minute)?)
    } else {
        format!("{hour_words} {}", cardinal_words(minute)?)
    })
}

/// Magnitude words that may sit between a currency symbol and its unit so the unit
/// is voiced after them (`$5 million` -> `5 million dollars`).
const CURRENCY_MAGNITUDES: [&str; 5] = ["hundred", "thousand", "million", "billion", "trillion"];

/// A recognized currency symbol and how to voice it.
#[derive(Clone, Copy)]
enum CurrencyKind {
    Dollar,
    Euro,
    Pound,
    Yen,
    Cent,
}

impl CurrencyKind {
    fn from_symbol(symbol: char) -> Option<Self> {
        Some(match symbol {
            '$' => Self::Dollar,
            '€' => Self::Euro,
            '£' => Self::Pound,
            '¥' => Self::Yen,
            '¢' => Self::Cent,
            _ => return None,
        })
    }

    /// num2words ISO currency for the symbols it pluralizes correctly. `¥`/`¢`
    /// return `None` (num2words says "yens"); those use [`Self::unit`] instead.
    fn num2words_currency(self) -> Option<Currency> {
        Some(match self {
            Self::Dollar => Currency::DOLLAR,
            Self::Euro => Currency::EUR,
            Self::Pound => Currency::GBP,
            Self::Yen | Self::Cent => return None,
        })
    }

    /// Spoken unit word for the magnitude path and the `¥`/`¢` fallback.
    fn unit(self, singular: bool) -> &'static str {
        match self {
            Self::Dollar => {
                if singular {
                    "dollar"
                } else {
                    "dollars"
                }
            }
            Self::Euro => {
                if singular {
                    "euro"
                } else {
                    "euros"
                }
            }
            Self::Pound => {
                if singular {
                    "pound"
                } else {
                    "pounds"
                }
            }
            Self::Yen => "yen",
            Self::Cent => {
                if singular {
                    "cent"
                } else {
                    "cents"
                }
            }
        }
    }
}

/// Move a leading currency symbol to a spoken unit after the amount. `$`/`€`/`£`
/// are voiced through num2words for correct words, plurals, and cents (`$5` ->
/// `five dollars`, `$5.50` -> `five dollars and fifty cents`, `$1984` -> `one
/// thousand nine hundred and eighty-four dollars`). When a magnitude word follows,
/// the digits are kept and the unit voiced after it (`$5 million` -> `5 million
/// dollars`). `¥`/`¢` and any unparseable amount fall back to `<digits> <unit>`.
fn expand_currency(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut pending_unit: Option<&'static str> = None;

    for (index, word) in words.iter().enumerate() {
        let (lead, core, trail) = strip_outer_punctuation(word);

        if let Some((amount, kind)) = parse_currency_amount(core) {
            // Peel a trailing sentence mark off the next word so `$5 million.`
            // still resolves the magnitude (the unit is reordered before the mark
            // when the magnitude word is emitted below).
            let next_is_magnitude = words
                .get(index + 1)
                .map(|next| {
                    strip_outer_punctuation(next)
                        .1
                        .trim_end_matches(['.', '!', '?'])
                        .to_ascii_lowercase()
                })
                .is_some_and(|next_core| CURRENCY_MAGNITUDES.contains(&next_core.as_str()));

            if next_is_magnitude {
                // Keep the digits; the plural unit is voiced after the magnitude.
                out.push(format!("{lead}{amount}{trail}"));
                pending_unit = Some(kind.unit(false));
            } else if let Some(words) = kind
                .num2words_currency()
                .and_then(|currency| currency_amount_words(amount, currency))
            {
                out.push(format!("{lead}{words}{trail}"));
            } else {
                let amount_is_one = matches!(amount, "1" | "1.0" | "1.00");
                out.push(format!(
                    "{lead}{amount} {}{trail}",
                    kind.unit(amount_is_one)
                ));
            }
            continue;
        }

        if let Some(unit) = pending_unit.take() {
            // This word is the magnitude just consumed; voice the unit after it,
            // moving any trailing sentence mark past the unit so `$5 million.`
            // reads `5 million dollars.`.
            let (body, marks) = split_trailing_marks(word);
            out.push(body.to_string());
            out.push(format!("{unit}{marks}"));
        } else {
            out.push((*word).to_string());
        }
    }

    if let Some(unit) = pending_unit.take() {
        out.push(unit.to_string());
    }

    out.join(" ")
}

/// Split a currency token into its numeric amount and currency kind, or `None`
/// when it is not `<symbol><number>`. The number may carry digit-grouping commas
/// and a decimal point but must start with a digit.
fn parse_currency_amount(core: &str) -> Option<(&str, CurrencyKind)> {
    let symbol = core.chars().next()?;
    let kind = CurrencyKind::from_symbol(symbol)?;
    let amount = &core[symbol.len_utf8()..];
    if !amount.starts_with(|ch: char| ch.is_ascii_digit())
        || !amount
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == '.')
    {
        return None;
    }
    Some((amount, kind))
}

/// Voice a currency amount through num2words, e.g. `5` -> `five dollars`, `5.50`
/// -> `five dollars and fifty cents`. Digit-grouping commas are stripped first.
/// Returns `None` if the amount does not parse, so the caller can fall back.
fn currency_amount_words(amount: &str, currency: Currency) -> Option<String> {
    let cleaned = amount.replace(',', "");
    if cleaned.contains('.') {
        let value: f64 = cleaned.parse().ok()?;
        Num2Words::new(value).currency(currency).to_words().ok()
    } else {
        let value: i64 = cleaned.parse().ok()?;
        Num2Words::new(value).currency(currency).to_words().ok()
    }
}

/// Repair a decimal fraction the segment chunker may have split and rejoined with
/// a spurious space (`3.14` -> `3. 14`) by collapsing `digit. digit` back to
/// `digit.digit`. Only fires between two digits, so sentence-ending periods like
/// `... ends. 4 remain` are untouched.
fn collapse_decimal_dots(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index].is_ascii_digit()
            && chars.get(index + 1) == Some(&'.')
            && chars.get(index + 2) == Some(&' ')
            && chars.get(index + 3).is_some_and(|ch| ch.is_ascii_digit())
        {
            out.push(chars[index]);
            out.push('.');
            // Skip the dot and the spurious space; the trailing digit is emitted
            // by the next iteration.
            index += 3;
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }

    out
}

/// Convert punctuation that should pause but otherwise gets dropped or rushed by
/// Kokoro into commas, which reliably produce a medium pause:
///
/// - Semicolons and colons (clause separators), unless flanked by digits on both
///   sides, which keeps ratios intact (`2:1`); real clock times are already gone
///   by this point (see [`expand_clock_times`]).
/// - A space-flanked hyphen, which is what `sanitize_tts_text` leaves an em or en
///   dash used as a clause break (` - `). A glued hyphen (`well-known`) or numeric
///   range (`10-20`) has no surrounding spaces and is left as-is.
/// - Brackets `(` `)` `[` `]`, which Kokoro does not pause on, so a parenthetical
///   aside otherwise runs into the surrounding words. The bracket is dropped and a
///   comma marks the aside boundary.
/// - An ellipsis: a run of two or more dots. A lone period stays a full stop.
///
/// Each insertion goes through [`push_pause_comma`], which collapses an adjacent
/// space or comma so neighbouring markers (`);`, ` - `) never stack into a double
/// pause.
fn soften_pause_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        match ch {
            '.' => {
                let mut run_end = index;
                while run_end < chars.len() && chars[run_end] == '.' {
                    run_end += 1;
                }
                if run_end - index >= 2 {
                    push_pause_comma(&mut out);
                    index = run_end;
                    continue;
                }
                out.push('.');
                index += 1;
            }
            ';' | ':' => {
                let previous_is_digit = index
                    .checked_sub(1)
                    .and_then(|prev| chars.get(prev))
                    .is_some_and(|ch| ch.is_ascii_digit());
                let next_is_digit = chars.get(index + 1).is_some_and(|ch| ch.is_ascii_digit());
                if previous_is_digit && next_is_digit {
                    out.push(ch);
                } else {
                    push_pause_comma(&mut out);
                }
                index += 1;
            }
            '-' => {
                let previous_is_space = index
                    .checked_sub(1)
                    .and_then(|prev| chars.get(prev))
                    .is_some_and(|ch| *ch == ' ');
                let next_is_space = chars.get(index + 1).is_some_and(|ch| *ch == ' ');
                if previous_is_space && next_is_space {
                    push_pause_comma(&mut out);
                } else {
                    out.push('-');
                }
                index += 1;
            }
            '(' | '[' => {
                // A bracket at the very start has no preceding word to pause after,
                // so skip the separator space to avoid a leading space.
                push_pause_comma(&mut out);
                if !out.is_empty() {
                    out.push(' ');
                }
                index += 1;
            }
            ')' | ']' => {
                push_pause_comma(&mut out);
                index += 1;
            }
            _ => {
                out.push(ch);
                index += 1;
            }
        }
    }

    out
}

/// Append a pause comma to `out`, first dropping any trailing space and skipping
/// the comma entirely if one already ends the buffer. This keeps adjacent softened
/// markers (a closing bracket then a semicolon, a dash after a word) from
/// producing `,,` or ` ,` and gives the next token a single clean separator.
fn push_pause_comma(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
    if !out.is_empty() && !out.ends_with(',') {
        out.push(',');
    }
}

/// Expand standalone four-digit years into the phrasing eSpeak/Kokoro usually
/// needs for natural historical dates: `1984` becomes `nineteen eighty-four`.
/// This intentionally runs only on the synthesis copy, leaving source chunks,
/// highlighting, search, bundle metadata, and cache identity unchanged.
fn normalize_year_like_numbers(text: &str) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(text.len());
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if !ch.is_ascii_digit() {
            normalized.push(ch);
            index += 1;
            continue;
        }

        let start = index;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        let token = chars[start..index].iter().collect::<String>();
        let previous = start.checked_sub(1).map(|value| chars[value]);
        let next = chars.get(index).copied();
        let after_next = chars.get(index + 1).copied();
        let year_words = (token.len() == 4
            && is_prose_year_left(previous)
            && is_prose_year_right(next, after_next))
        .then(|| token.parse::<u16>().ok().and_then(year_to_words))
        .flatten();

        if let Some(words) = year_words {
            normalized.push_str(&words);
        } else {
            normalized.push_str(&token);
        }
    }

    normalized
}

/// A four-digit run reads as a spoken year only when its left side looks like
/// prose. Letters/digits (identifiers), a decimal point or currency/math symbol
/// (a measured amount), or a hyphen (a page/number range) all suppress it.
fn is_prose_year_left(previous: Option<char>) -> bool {
    match previous {
        None => true,
        Some(ch) if ch.is_whitespace() => true,
        Some('(') | Some('[') | Some('{') | Some('"') | Some('\'') => true,
        _ => false,
    }
}

/// Mirror of [`is_prose_year_left`] for the right side. A following decimal or
/// digit-grouping comma (`1984.5`, `1984,000`) marks a number, not a year, so
/// only whitespace or closing/sentence punctuation keeps the year reading.
fn is_prose_year_right(next: Option<char>, after_next: Option<char>) -> bool {
    let followed_by_digit = matches!(after_next, Some(ch) if ch.is_ascii_digit());
    match next {
        None => true,
        Some(ch) if ch.is_whitespace() => true,
        // A decimal point or thousands comma only counts as a boundary when it
        // is not gluing the run to more digits.
        Some('.') | Some(',') => !followed_by_digit,
        Some(')') | Some(']') | Some('}') | Some('"') | Some('\'') | Some(';') | Some(':')
        | Some('!') | Some('?') => true,
        _ => false,
    }
}

fn year_to_words(year: u16) -> Option<String> {
    // Keep the same 1000-2099 gate as before; delegate the actual phrasing to the
    // num2words `year` formatter (e.g. 1984 -> "nineteen eighty-four", 2026 ->
    // "twenty twenty-six"), which matches the historical-date style we want.
    if !(1000..=2099).contains(&year) {
        return None;
    }
    Num2Words::new(year).year().to_words().ok()
}

/// Spell a small non-negative integer as cardinal words via num2words (`45` ->
/// `"forty-five"`). Used for clock hours and minutes; returns `None` only if the
/// formatter fails.
fn cardinal_words(value: u16) -> Option<String> {
    Num2Words::new(value).to_words().ok()
}

/// Build a short, sanitized preview (≤140 chars, "..." if truncated) of a
/// chunk's text for progress/diagnostics messages.
pub(super) fn text_preview(text: &str) -> String {
    let sanitized = sanitize_tts_text(text);
    let mut preview = sanitized.chars().take(140).collect::<String>();
    if sanitized.chars().count() > 140 {
        preview.push_str("...");
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_expansion_fires_for_prose_years() {
        // We assert that our gate fires and preserves surrounding text, including
        // for years wrapped in parens or followed by a comma. The exact spoken
        // phrasing is num2words' contract, so it is intentionally not pinned here.
        for (input, prefix, suffix) in [
            ("Written in 1984.", "Written in ", "."),
            ("Released (1984) widely.", "Released (", ") widely."),
            ("In 1984, sales rose.", "In ", ", sales rose."),
        ] {
            let out = normalize_year_like_numbers(input);
            assert!(!out.contains("1984"), "year not expanded: {out:?}");
            assert!(
                out.starts_with(prefix) && out.ends_with(suffix),
                "context lost: {out:?}"
            );
        }
    }

    #[test]
    fn year_expansion_leaves_non_year_numbers_alone() {
        assert_eq!(
            normalize_year_like_numbers("See pp. 123-124 and item A1984."),
            "See pp. 123-124 and item A1984."
        );
        assert_eq!(
            normalize_year_like_numbers("The code 9876 is not a supported year."),
            "The code 9876 is not a supported year."
        );
    }

    #[test]
    fn year_expansion_skips_quantities_and_ranges() {
        assert_eq!(
            normalize_year_like_numbers("It cost 3.1984 per unit."),
            "It cost 3.1984 per unit."
        );
        assert_eq!(
            normalize_year_like_numbers("A 1984.50 balance and 1984,000 total."),
            "A 1984.50 balance and 1984,000 total."
        );
        assert_eq!(
            normalize_year_like_numbers("Paid $1984 at 1984% over pages 1900-2000."),
            "Paid $1984 at 1984% over pages 1900-2000."
        );
    }

    #[test]
    fn sanitizer_does_not_expand_years_by_itself() {
        assert_eq!(sanitize_tts_text("Written in 1984."), "Written in 1984.");
    }

    #[test]
    fn roman_numerals_expand_only_after_section_words() {
        assert_eq!(
            expand_section_roman_numerals("Chapter IV begins"),
            "Chapter 4 begins"
        );
        assert_eq!(
            expand_section_roman_numerals("See Part VII."),
            "See Part 7."
        );
        assert_eq!(
            expand_section_roman_numerals("Act III, Scene II:"),
            "Act 3, Scene 2:"
        );
        assert_eq!(expand_section_roman_numerals("Book (XII),"), "Book (12),");
    }

    #[test]
    fn roman_numerals_leave_bare_letters_alone() {
        assert_eq!(expand_section_roman_numerals("I went home"), "I went home");
        assert_eq!(
            expand_section_roman_numerals("grade C work"),
            "grade C work"
        );
        assert_eq!(expand_section_roman_numerals("X marks it"), "X marks it");
        assert_eq!(
            expand_section_roman_numerals("Part iv note"),
            "Part iv note"
        );
    }

    #[test]
    fn roman_numerals_require_capitalized_keyword() {
        assert_eq!(
            expand_section_roman_numerals("the book I read last week"),
            "the book I read last week"
        );
        assert_eq!(
            expand_section_roman_numerals("they act I think"),
            "they act I think"
        );
        assert_eq!(
            expand_section_roman_numerals("Book I cover"),
            "Book 1 cover"
        );
    }

    #[test]
    fn decimal_dots_recollapse_after_chunk_split() {
        assert_eq!(
            collapse_decimal_dots("It is 3. 14 today"),
            "It is 3.14 today"
        );
        assert_eq!(
            collapse_decimal_dots("version 1. 2 ships"),
            "version 1.2 ships"
        );
        assert_eq!(
            collapse_decimal_dots("The talk ends. 4 people left."),
            "The talk ends. 4 people left."
        );
    }

    #[test]
    fn pause_punctuation_softens_to_commas() {
        assert_eq!(
            soften_pause_punctuation("First; then second"),
            "First, then second"
        );
        assert_eq!(
            soften_pause_punctuation("Note: read this"),
            "Note, read this"
        );
        assert_eq!(
            soften_pause_punctuation("Meet at 3:30 for a 2:1 split"),
            "Meet at 3:30 for a 2:1 split"
        );
        assert_eq!(soften_pause_punctuation("wait - then go"), "wait, then go");
        assert_eq!(
            soften_pause_punctuation("a well-known 10-20 range"),
            "a well-known 10-20 range"
        );
        assert_eq!(
            soften_pause_punctuation("He paused... then spoke. Done."),
            "He paused, then spoke. Done."
        );
        assert_eq!(
            soften_pause_punctuation("sounds natural (smart text) here"),
            "sounds natural, smart text, here"
        );
        assert_eq!(
            soften_pause_punctuation("done (see note); next"),
            "done, see note, next"
        );
        assert_eq!(
            soften_pause_punctuation("(aside) follows"),
            "aside, follows"
        );
    }

    #[test]
    fn clock_times_expand_but_ratios_do_not() {
        assert_eq!(
            expand_clock_times("arriving at 9:00 sharp"),
            "arriving at nine o'clock sharp"
        );
        assert_eq!(
            expand_clock_times("the 3:30 train"),
            "the three thirty train"
        );
        assert_eq!(
            expand_clock_times("at 10:45 and 9:05"),
            "at ten forty-five and nine oh five"
        );
        assert_eq!(
            expand_clock_times("leaves at 9:00."),
            "leaves at nine o'clock."
        );
        assert_eq!(
            expand_clock_times("ratio 2:1 aspect 16:9 verse 1:1"),
            "ratio 2:1 aspect 16:9 verse 1:1"
        );
    }

    #[test]
    fn abbreviations_expand_to_spoken_words() {
        assert_eq!(expand_abbreviations("Dr. Smith"), "Doctor Smith");
        assert_eq!(
            expand_abbreviations("Mr. Jones and Mrs. Lee"),
            "Mister Jones and Missus Lee"
        );
        assert_eq!(expand_abbreviations("cats vs. dogs"), "cats versus dogs");
        assert_eq!(
            expand_abbreviations("on Tuesday, e.g. mid-week, etc."),
            "on Tuesday, for example mid-week, et cetera."
        );
        assert_eq!(expand_abbreviations("(i.e. that)"), "(that is that)");
    }

    #[test]
    fn initialisms_collapse_and_st_uses_context() {
        assert_eq!(
            expand_abbreviations("the U.S.A. and N.Y. at 9 a.m."),
            "the USA and NY at 9 am"
        );
        assert_eq!(
            expand_abbreviations("St. Louis office"),
            "Saint Louis office"
        );
        assert_eq!(
            expand_abbreviations("on Main St. today"),
            "on Main Street today"
        );
        assert_eq!(expand_abbreviations("Plan A. Next"), "Plan A. Next");
    }

    #[test]
    fn etc_keeps_a_stop_only_at_sentence_end() {
        assert_eq!(
            expand_abbreviations("apples, etc. They left"),
            "apples, et cetera. They left"
        );
        assert_eq!(
            expand_abbreviations("and so on, etc."),
            "and so on, et cetera."
        );
        assert_eq!(
            expand_abbreviations("apples, etc. and more"),
            "apples, et cetera and more"
        );
        assert_eq!(
            expand_abbreviations("apples, etc., and more"),
            "apples, et cetera, and more"
        );
    }

    #[test]
    fn units_expand_to_spoken_words() {
        assert_eq!(
            expand_abbreviations("60 mph limit"),
            "60 miles per hour limit"
        );
        assert_eq!(
            expand_abbreviations("at 90 km/h now"),
            "at 90 kilometers per hour now"
        );
    }

    #[test]
    fn currency_symbols_move_to_spoken_units() {
        assert_eq!(
            expand_currency("it cost $5 today"),
            "it cost five dollars today"
        );
        assert_eq!(
            expand_currency("about €10 and £20"),
            "about ten euros and twenty pounds"
        );
        assert_eq!(
            expand_currency("a $5 million deal"),
            "a 5 million dollars deal"
        );
        assert_eq!(
            expand_currency("worth $5 million."),
            "worth 5 million dollars."
        );
        assert_eq!(
            expand_currency("$5 million, plus"),
            "5 million dollars, plus"
        );
        assert_eq!(
            expand_currency("worth $1 billion"),
            "worth 1 billion dollars"
        );
        assert_eq!(expand_currency("($5) each,"), "(five dollars) each,");
        assert_eq!(expand_currency("¥100 fee"), "100 yen fee");
    }

    #[test]
    fn english_normalizer_composes_all_passes() {
        assert_eq!(
            normalize_english_synthesis_text(
                "Dr. Smith; see e.g. 3. 14 units - wait... done in 1984"
            ),
            "Doctor Smith, see for example 3.14 units, wait, done in nineteen eighty-four"
        );
        assert_eq!(
            normalize_english_synthesis_text("Met at 9:00 (sharp) then left"),
            "Met at nine o'clock, sharp, then left"
        );
    }
}
