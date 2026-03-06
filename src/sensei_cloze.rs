use crate::jisho::WordEntry;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BankFrom {
    Examples,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClozeDeck {
    pub deck_id: String,
    pub deck_type: String,
    pub cloze_subtype: String,
    pub title: String,
    pub description: String,
    pub bank: Vec<String>,
    pub cards: Vec<ClozeCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClozeCard {
    pub card_local_id: String,
    pub prompt_jp: String,
    pub expected: Vec<String>,
    pub context_fr: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ClozeBuildOptions {
    pub seed: Option<u64>,
    pub max_cards: Option<usize>,
    pub bank_from: BankFrom,
    pub require_word_in_example: bool,
    pub tag: String,
}

pub fn build_vocab_cloze_deck(
    deck_id: String,
    title: String,
    description: String,
    all_entries: Vec<WordEntry>,
    opts: ClozeBuildOptions,
) -> ClozeDeck {
    let bank_all = unique_preserve_order(all_entries.iter().map(|e| e.word.as_str()));

    let mut candidates: Vec<WordEntry> = all_entries
        .into_iter()
        .filter(|e| e.example.is_some())
        .filter(|e| {
            if !opts.require_word_in_example {
                return true;
            }
            e.example
                .as_ref()
                .is_some_and(|ex| ex.japanese.contains(&e.word))
        })
        .collect();

    if let Some(seed) = opts.seed {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        candidates.shuffle(&mut rng);
    }

    if let Some(max) = opts.max_cards {
        if candidates.len() > max {
            candidates.truncate(max);
        }
    }

    let mut cards = Vec::with_capacity(candidates.len());
    for (idx, entry) in candidates.into_iter().enumerate() {
        let Some(example) = entry.example else {
            continue;
        };

        let Some(prompt_jp) = blank_first_occurrence(&example.japanese, &entry.word) else {
            continue;
        };

        let card_local_id = format!("{}-{:03}", entry.word, idx + 1);
        cards.push(ClozeCard {
            card_local_id,
            prompt_jp,
            expected: vec![entry.word.clone()],
            context_fr: entry.definition,
            tags: vec![opts.tag.clone()],
        });
    }

    let bank = match opts.bank_from {
        BankFrom::Examples => unique_preserve_order(cards.iter().map(|c| c.expected[0].as_str())),
        BankFrom::All => bank_all,
    };

    ClozeDeck {
        deck_id,
        deck_type: "cloze".to_string(),
        cloze_subtype: "vocab".to_string(),
        title,
        description,
        bank,
        cards,
    }
}

fn unique_preserve_order<'a, I>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item) {
            out.push(item.to_string());
        }
    }
    out
}

fn blank_first_occurrence(haystack: &str, needle: &str) -> Option<String> {
    if needle.is_empty() {
        return None;
    }
    let idx = haystack.find(needle)?;
    let mut out = String::with_capacity(haystack.len());
    out.push_str(&haystack[..idx]);
    out.push('＿');
    out.push_str(&haystack[idx + needle.len()..]);
    Some(out)
}
