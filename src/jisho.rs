use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordEntry {
    pub word: String,
    pub reading: String,
    pub definition: String,
    pub example: Option<ExampleSentence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleSentence {
    pub japanese: String,
    pub reading: String,
    pub english: String,
}

pub fn jlpt_search_url(jlpt_level: &str, page: u32) -> String {
    let level = jlpt_level.trim().to_ascii_lowercase();
    let level = level.strip_prefix("jlpt-").unwrap_or(&level);
    let level = level.strip_prefix("n").unwrap_or(&level);
    format!("https://jisho.org/search/%20%23words%20%23jlpt-n{level}?page={page}")
}

pub fn fetch_jlpt_page_html(client: &Client, jlpt_level: &str, page: u32) -> Result<String> {
    let url = jlpt_search_url(jlpt_level, page);
    let response = client
        .get(&url)
        .header(
            header::USER_AGENT,
            "jisho-scraper/0.1 (+https://github.com/; educational use)",
        )
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} (status)"))?;

    response.text().with_context(|| format!("GET {url} (body)"))
}

pub fn parse_words_from_html(html: &str) -> Result<Vec<WordEntry>> {
    let document = Html::parse_document(html);

    let concept_selector = Selector::parse("div.concepts div.concept_light")
        .map_err(|e| anyhow::anyhow!("invalid selector (concept): {e:?}"))?;
    let word_selector = Selector::parse("div.concept_light-representation span.text")
        .map_err(|e| anyhow::anyhow!("invalid selector (word): {e:?}"))?;
    let furigana_selector = Selector::parse("div.concept_light-representation span.furigana")
        .map_err(|e| anyhow::anyhow!("invalid selector (furigana): {e:?}"))?;
    let meaning_selector = Selector::parse("div.concept_light-meanings span.meaning-meaning")
        .map_err(|e| anyhow::anyhow!("invalid selector (meaning): {e:?}"))?;
    let sentence_selector = Selector::parse("div.concept_light-meanings div.sentence")
        .map_err(|e| anyhow::anyhow!("invalid selector (sentence): {e:?}"))?;
    let sentence_japanese_selector = Selector::parse("ul.japanese")
        .map_err(|e| anyhow::anyhow!("invalid selector (sentence_japanese): {e:?}"))?;
    let sentence_english_selector = Selector::parse("span.english")
        .map_err(|e| anyhow::anyhow!("invalid selector (sentence_english): {e:?}"))?;
    let sentence_furigana_selector = Selector::parse("span.furigana")
        .map_err(|e| anyhow::anyhow!("invalid selector (sentence_furigana): {e:?}"))?;
    let sentence_unlinked_selector = Selector::parse("span.unlinked")
        .map_err(|e| anyhow::anyhow!("invalid selector (sentence_unlinked): {e:?}"))?;

    let mut out = Vec::new();
    for concept in document.select(&concept_selector) {
        let word = concept
            .select(&word_selector)
            .next()
            .map(element_text_compact)
            .unwrap_or_default();

        let mut reading = concept
            .select(&furigana_selector)
            .next()
            .map(element_text_compact_no_spaces)
            .unwrap_or_default();

        if reading.is_empty() {
            reading = word.clone();
        }

        let definition = concept
            .select(&meaning_selector)
            .next()
            .map(element_text_compact)
            .unwrap_or_default();

        if word.is_empty() || definition.is_empty() {
            continue;
        }

        let example = concept
            .select(&sentence_selector)
            .next()
            .and_then(|sentence| {
                parse_example_sentence(
                    sentence,
                    &sentence_japanese_selector,
                    &sentence_english_selector,
                    &sentence_furigana_selector,
                    &sentence_unlinked_selector,
                )
            });

        out.push(WordEntry {
            word,
            reading,
            definition,
            example,
        });
    }

    Ok(out)
}

fn parse_example_sentence(
    sentence: scraper::ElementRef<'_>,
    japanese_selector: &Selector,
    english_selector: &Selector,
    furigana_selector: &Selector,
    unlinked_selector: &Selector,
) -> Option<ExampleSentence> {
    let ul = sentence.select(japanese_selector).next()?;
    let english = sentence
        .select(english_selector)
        .next()
        .map(element_text_compact)?;
    if english.is_empty() {
        return None;
    }

    let japanese = sentence_surface_from_ul(ul, unlinked_selector);
    let reading = sentence_reading_from_ul(ul, furigana_selector, unlinked_selector);
    if japanese.is_empty() || reading.is_empty() {
        return None;
    }

    Some(ExampleSentence {
        japanese,
        reading,
        english,
    })
}

fn sentence_surface_from_ul(ul: scraper::ElementRef<'_>, unlinked_selector: &Selector) -> String {
    sentence_from_ul(
        ul,
        SentenceMode::Surface,
        unlinked_selector,
        unlinked_selector,
    )
}

fn sentence_reading_from_ul(
    ul: scraper::ElementRef<'_>,
    furigana_selector: &Selector,
    unlinked_selector: &Selector,
) -> String {
    sentence_from_ul(
        ul,
        SentenceMode::Reading,
        furigana_selector,
        unlinked_selector,
    )
}

#[derive(Debug, Clone, Copy)]
enum SentenceMode {
    Surface,
    Reading,
}

fn sentence_from_ul(
    ul: scraper::ElementRef<'_>,
    mode: SentenceMode,
    furigana_selector: &Selector,
    unlinked_selector: &Selector,
) -> String {
    let mut out = String::new();
    for child in ul.children() {
        match child.value() {
            scraper::Node::Text(t) => {
                let text = t.text.trim();
                if !text.is_empty() {
                    out.push_str(&text.split_whitespace().collect::<String>());
                }
            }
            scraper::Node::Element(_) => {
                if let Some(li) = scraper::ElementRef::wrap(child) {
                    if li.value().name() != "li" {
                        continue;
                    }

                    match mode {
                        SentenceMode::Surface => {
                            if let Some(unlinked) = li.select(unlinked_selector).next() {
                                out.push_str(&element_text_compact_no_spaces(unlinked));
                            } else {
                                out.push_str(&element_text_compact_no_spaces(li));
                            }
                        }
                        SentenceMode::Reading => {
                            if let Some(furigana) = li.select(furigana_selector).next() {
                                let base = element_text_compact_no_spaces(furigana);
                                if let Some(unlinked) = li.select(unlinked_selector).next() {
                                    let kana = kana_only(&element_text_compact_no_spaces(unlinked));
                                    let remainder = strip_common_prefix(&base, &kana);
                                    out.push_str(&base);
                                    out.push_str(remainder);
                                } else {
                                    out.push_str(&base);
                                }
                            } else if let Some(unlinked) = li.select(unlinked_selector).next() {
                                out.push_str(&element_text_compact_no_spaces(unlinked));
                            } else {
                                out.push_str(&element_text_compact_no_spaces(li));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    out
}

fn element_text_compact(element: scraper::ElementRef<'_>) -> String {
    collapse_whitespace(&element.text().collect::<String>())
}

fn element_text_compact_no_spaces(element: scraper::ElementRef<'_>) -> String {
    element.text().flat_map(|t| t.split_whitespace()).collect()
}

fn kana_only(s: &str) -> String {
    s.chars().filter(|&ch| is_kana(ch) || ch == 'ー').collect()
}

fn is_kana(ch: char) -> bool {
    matches!(ch,
        '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana Phonetic Extensions
    )
}

fn strip_common_prefix<'a>(base: &str, kana: &'a str) -> &'a str {
    let mut base_iter = base.chars();
    let mut prefix_chars = 0usize;
    for ch in kana.chars() {
        match base_iter.next() {
            Some(bch) if bch == ch => prefix_chars += 1,
            _ => break,
        }
    }
    if prefix_chars == 0 {
        return kana;
    }
    let end = kana
        .char_indices()
        .nth(prefix_chars)
        .map(|(i, _)| i)
        .unwrap_or(kana.len());
    &kana[end..]
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_was_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            prev_was_space = true;
            continue;
        }

        if prev_was_space && !out.is_empty() {
            out.push(' ');
        }
        prev_was_space = false;
        out.push(ch);
    }
    out.trim().to_string()
}
