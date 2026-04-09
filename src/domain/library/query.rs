use crate::domain::playlist::query::SmartQuery;

/// 将结构化查询转换成 SQLite where 子句和参数列表。
///
/// # 参数
/// - `query`：结构化 smart query
///
/// # 返回
/// - `(String, Vec<String>)`：where 子句与参数
pub fn build_song_search_sql(query: &SmartQuery) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();

    if let Some(artist) = &query.artist {
        clauses.push("artists.name LIKE ?".to_string());
        params.push(format!("%{artist}%"));
    }
    if let Some(album) = &query.album {
        clauses.push("albums.title LIKE ?".to_string());
        params.push(format!("%{album}%"));
    }
    if let Some(genre) = &query.genre {
        clauses.push("songs.genre LIKE ?".to_string());
        params.push(format!("%{genre}%"));
    }
    if let Some(start) = query.year_start {
        clauses.push("albums.year >= ?".to_string());
        params.push(start.to_string());
    }
    if let Some(end) = query.year_end {
        clauses.push("albums.year <= ?".to_string());
        params.push(end.to_string());
    }
    for term in &query.free_text {
        clauses.push(
            "(songs.title LIKE ? OR artists.name LIKE ? OR albums.title LIKE ? OR COALESCE(songs.lyrics, '') LIKE ?)"
                .to_string(),
        );
        for _ in 0..4 {
            params.push(format!("%{term}%"));
        }
    }

    let where_sql = if clauses.is_empty() {
        "1 = 1".to_string()
    } else {
        clauses.join(" AND ")
    };

    (where_sql, params)
}
