# jisho-scraper

Small Rust CLI + library that scrapes `jisho.org` JLPT word lists.

It extracts, per entry:

- `word` (kanji / surface form)
- `reading` (kana reading)
- `definition` (first English definition)
- `example` (optional): one example sentence with Japanese surface, Japanese reading, and English translation

## Install / Build

```bash
cargo build --release
```

## Usage

Fetch a fixed number of pages and return *all* entries as JSON:

```bash
cargo run -- --level n5 --pages 1 --format entries
```

Collect *only* entries that have an example sentence, until you reach a target count:

```bash
cargo run -- --level n5 --target 50 --format entries
```

### Sensei cloze deck export

Emit a Sensei cloze deck JSON (ready to drop into `sensei/content/cloze/decks/<deck_id>.json`):

```bash
cargo run -- \
  --level n5 \
  --format cloze-deck \
  --deck-id n5-vocab-1 \
  --target 50
```

Useful flags:

- `--level`: `n5|n4|n3|n2|n1` (also accepts `jlpt-n5`, etc.)
- `--page-start`: start page (1-indexed)
- `--pages`: number of pages to fetch (conflicts with `--target`)
- `--target`: keep scraping until at least this many entries with `example != null`
- `--max-pages`: safety cap when using `--target` (default: `100`)
- `--delay-ms`: delay between requests (default: `500`)
- `--seed`: deterministic sampling/order (for `--format cloze-deck`)
- `--max-cards`: cap cards output (for `--format cloze-deck`)
- `--bank-from`: `examples|all` (for `--format cloze-deck`)
- `--require-word-in-example`: only keep entries where the word appears in the example sentence (default: `true`)

## Output

The CLI prints a JSON array of objects like:

```json
[
  {
    "word": "川",
    "reading": "かわ",
    "definition": "river; stream",
    "example": {
      "japanese": "この川はあの川の３倍長い。",
      "reading": "このかわはあのかわのさんばいながい。",
      "english": "This river is three times longer than that one."
    }
  }
]
```

## Notes

- This is HTML scraping; selectors may break if Jisho changes their markup.
- Be polite: keep a delay between requests and avoid scraping more than you need.
