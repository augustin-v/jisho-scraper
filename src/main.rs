use anyhow::{Context, Result};
use clap::Parser;
use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Entries,
    ClozeDeck,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum BankFromArg {
    Examples,
    All,
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// JLPT level: n5, n4, n3, n2, n1 (accepts also jlpt-n5, jlpt-n4, ...)
    #[arg(long)]
    level: String,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Entries)]
    format: OutputFormat,

    /// Deck id (used by `--format cloze-deck`)
    #[arg(long)]
    deck_id: Option<String>,

    /// Deck title (used by `--format cloze-deck`)
    #[arg(long)]
    title: Option<String>,

    /// Deck description (used by `--format cloze-deck`)
    #[arg(long)]
    description: Option<String>,

    /// First page to fetch (1-indexed)
    #[arg(long, default_value_t = 1)]
    page_start: u32,

    /// Number of pages to fetch
    #[arg(long, default_value_t = 1)]
    #[arg(conflicts_with = "target")]
    pages: u32,

    /// Collect entries with a non-null example until at least this many are gathered
    ///
    /// When set, scraping continues page-by-page starting at `--page-start`, filtering out entries
    /// that have no example sentence, until `--target` is reached or `--max-pages` is hit.
    #[arg(long)]
    target: Option<usize>,

    /// Safety cap on how many pages to fetch when using `--target`
    #[arg(long, default_value_t = 100)]
    max_pages: u32,

    /// Deterministic sampling/order seed (used by `--format cloze-deck`)
    #[arg(long)]
    seed: Option<u64>,

    /// Cap cards output (used by `--format cloze-deck`)
    #[arg(long)]
    max_cards: Option<usize>,

    /// Where to build the word bank from (used by `--format cloze-deck`)
    #[arg(long, value_enum, default_value_t = BankFromArg::Examples)]
    bank_from: BankFromArg,

    /// Require the word to appear in the example sentence (used by `--format cloze-deck`)
    #[arg(long, default_value_t = true)]
    require_word_in_example: bool,

    /// Delay between page requests
    #[arg(long, default_value_t = 500)]
    delay_ms: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let client = Client::builder().build().context("building HTTP client")?;

    let (level_num, level_tag) = normalize_level(&args.level)?;
    let default_title = format!("N{level_num} — Vocab (cloze)");
    let default_description = "Complète avec le bon mot.".to_string();

    let qualify_for_target = |e: &jisho_scraper::jisho::WordEntry| match args.format {
        OutputFormat::Entries => e.example.is_some(),
        OutputFormat::ClozeDeck => {
            if e.example.is_none() {
                return false;
            }
            if !args.require_word_in_example {
                return true;
            }
            e.example
                .as_ref()
                .is_some_and(|ex| ex.japanese.contains(&e.word))
        }
    };

    let entries = if let Some(target) = args.target {
        collect_until_target(
            &client,
            &args.level,
            args.page_start,
            args.max_pages,
            target,
            args.delay_ms,
            qualify_for_target,
        )?
    } else {
        fetch_pages(
            &client,
            &args.level,
            args.page_start,
            args.pages,
            args.delay_ms,
        )?
    };

    match args.format {
        OutputFormat::Entries => {
            println!("{}", serde_json::to_string_pretty(&entries)?);
        }
        OutputFormat::ClozeDeck => {
            let deck_id = args
                .deck_id
                .clone()
                .context("`--deck-id` is required when `--format cloze-deck`")?;

            let title = args.title.clone().unwrap_or(default_title);
            let description = args.description.clone().unwrap_or(default_description);

            let bank_from = match args.bank_from {
                BankFromArg::Examples => jisho_scraper::sensei_cloze::BankFrom::Examples,
                BankFromArg::All => jisho_scraper::sensei_cloze::BankFrom::All,
            };

            let deck = jisho_scraper::sensei_cloze::build_vocab_cloze_deck(
                deck_id,
                title,
                description,
                entries,
                jisho_scraper::sensei_cloze::ClozeBuildOptions {
                    seed: args.seed,
                    max_cards: args.max_cards,
                    bank_from,
                    require_word_in_example: args.require_word_in_example,
                    tag: level_tag,
                },
            );

            println!("{}", serde_json::to_string_pretty(&deck)?);
        }
    }

    Ok(())
}

fn normalize_level(level: &str) -> Result<(u8, String)> {
    let raw = level.trim().to_ascii_lowercase();
    let raw = raw.strip_prefix("jlpt-").unwrap_or(&raw);
    let raw = raw.strip_prefix("n").unwrap_or(&raw);
    let num: u8 = raw
        .parse()
        .with_context(|| format!("invalid JLPT level `{level}` (expected n5..n1)"))?;
    if !(1..=5).contains(&num) {
        anyhow::bail!("invalid JLPT level `{level}` (expected n5..n1)");
    }
    Ok((num, format!("jlpt-n{num}")))
}

fn fetch_pages(
    client: &Client,
    level: &str,
    page_start: u32,
    pages: u32,
    delay_ms: u64,
) -> Result<Vec<jisho_scraper::jisho::WordEntry>> {
    let mut all = Vec::new();
    for i in 0..pages {
        let page = page_start + i;
        let html = jisho_scraper::jisho::fetch_jlpt_page_html(client, level, page)
            .with_context(|| format!("fetching page {page}"))?;
        let mut entries = jisho_scraper::jisho::parse_words_from_html(&html)
            .with_context(|| format!("parsing page {page}"))?;
        all.append(&mut entries);

        if i + 1 < pages && delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
    }
    Ok(all)
}

fn collect_until_target(
    client: &Client,
    level: &str,
    page_start: u32,
    max_pages: u32,
    target: usize,
    delay_ms: u64,
    qualify: impl Fn(&jisho_scraper::jisho::WordEntry) -> bool,
) -> Result<Vec<jisho_scraper::jisho::WordEntry>> {
    let mut out = Vec::new();

    for i in 0..max_pages {
        if out.len() >= target {
            break;
        }

        let page = page_start + i;
        let html = jisho_scraper::jisho::fetch_jlpt_page_html(client, level, page)
            .with_context(|| format!("fetching page {page}"))?;
        let entries = jisho_scraper::jisho::parse_words_from_html(&html)
            .with_context(|| format!("parsing page {page}"))?;

        if entries.is_empty() {
            break;
        }

        out.extend(entries.into_iter().filter(|e| qualify(e)));

        if i + 1 < max_pages && delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    if out.len() > target {
        out.truncate(target);
    }

    Ok(out)
}
