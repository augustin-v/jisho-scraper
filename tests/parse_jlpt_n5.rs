use jisho_scraper::jisho::parse_words_from_html;

#[test]
fn parses_known_words_from_fixture() {
    let html = std::fs::read_to_string("tests/fixtures/jlpt-n5-page1.html")
        .expect("fixture should be readable");

    let entries = parse_words_from_html(&html).expect("fixture should parse");
    assert!(!entries.is_empty());

    let gakkou = entries.iter().find(|e| e.word == "学校").expect("学校");
    assert_eq!(gakkou.reading, "がっこう");
    assert_eq!(gakkou.definition, "school");

    let kawa = entries.iter().find(|e| e.word == "川").expect("川");
    assert_eq!(kawa.reading, "かわ");
    assert_eq!(kawa.definition, "river; stream");
    let example = kawa.example.as_ref().expect("川 should have an example");
    assert_eq!(example.japanese, "この川はあの川の３倍長い。");
    assert_eq!(example.reading, "このかわはあのかわのさんばいながい。");
    assert_eq!(
        example.english,
        "This river is three times longer than that one."
    );
}
