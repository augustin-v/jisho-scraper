use jisho_scraper::jisho::parse_words_from_html;
use jisho_scraper::sensei_cloze::{BankFrom, ClozeBuildOptions, build_vocab_cloze_deck};

#[test]
fn exports_cloze_deck_with_blank_and_tags() {
    let html = std::fs::read_to_string("tests/fixtures/jlpt-n5-page1.html")
        .expect("fixture should be readable");
    let entries = parse_words_from_html(&html).expect("fixture should parse");

    let deck = build_vocab_cloze_deck(
        "n5-vocab-1".to_string(),
        "N5 — Vocab (cloze)".to_string(),
        "Complète avec le bon mot.".to_string(),
        entries,
        ClozeBuildOptions {
            seed: Some(0),
            max_cards: None,
            bank_from: BankFrom::Examples,
            require_word_in_example: true,
            tag: "jlpt-n5".to_string(),
        },
    );

    let kawa = deck
        .cards
        .iter()
        .find(|c| c.expected.first().is_some_and(|w| w == "川"))
        .expect("川 card should exist");

    assert_eq!(kawa.prompt_jp, "この＿はあの川の３倍長い。");
    assert_eq!(kawa.context_fr, "river; stream");
    assert_eq!(kawa.tags, ["jlpt-n5"]);
    assert!(deck.bank.contains(&"川".to_string()));
}

#[test]
fn bank_from_all_includes_words_without_examples() {
    let html = std::fs::read_to_string("tests/fixtures/jlpt-n5-page1.html")
        .expect("fixture should be readable");
    let entries = parse_words_from_html(&html).expect("fixture should parse");

    let deck = build_vocab_cloze_deck(
        "n5-vocab-1".to_string(),
        "N5 — Vocab (cloze)".to_string(),
        "Complète avec le bon mot.".to_string(),
        entries,
        ClozeBuildOptions {
            seed: None,
            max_cards: Some(1),
            bank_from: BankFrom::All,
            require_word_in_example: true,
            tag: "jlpt-n5".to_string(),
        },
    );

    assert!(deck.bank.contains(&"学校".to_string()));
}
