use anyhow::{Context, Result};
use clap::Parser;
use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// JLPT level: n5, n4, n3, n2, n1 (accepts also jlpt-n5, jlpt-n4, ...)
    #[arg(long)]
    level: String,

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

    /// Delay between page requests
    #[arg(long, default_value_t = 500)]
    delay_ms: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let client = Client::builder().build().context("building HTTP client")?;

    let entries = if let Some(target) = args.target {
        collect_until_target(
            &client,
            &args.level,
            args.page_start,
            args.max_pages,
            target,
            args.delay_ms,
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

    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
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
) -> Result<Vec<jisho_scraper::jisho::WordEntry>> {
    let mut with_examples = Vec::new();

    for i in 0..max_pages {
        if with_examples.len() >= target {
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

        with_examples.extend(entries.into_iter().filter(|e| e.example.is_some()));

        if i + 1 < max_pages && delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    if with_examples.len() > target {
        with_examples.truncate(target);
    }

    Ok(with_examples)
}
