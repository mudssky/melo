use melo::domain::playlist::query::SmartQuery;

#[test]
fn smart_query_parses_field_filters_and_year_ranges() {
    let query = SmartQuery::parse(r#"artist:"Aimer" year:2020..2021 brave shine"#).unwrap();

    assert_eq!(query.artist.as_deref(), Some("Aimer"));
    assert_eq!(query.year_start, Some(2020));
    assert_eq!(query.year_end, Some(2021));
    assert_eq!(
        query.free_text,
        vec!["brave".to_string(), "shine".to_string()]
    );
}
