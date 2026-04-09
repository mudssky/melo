use crate::core::error::{MeloError, MeloResult};

/// Smart playlist 与库搜索共用的查询对象。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartQuery {
    /// 艺术家过滤。
    pub artist: Option<String>,
    /// 专辑过滤。
    pub album: Option<String>,
    /// 流派过滤。
    pub genre: Option<String>,
    /// 起始年份。
    pub year_start: Option<i32>,
    /// 结束年份。
    pub year_end: Option<i32>,
    /// 自由文本词项。
    pub free_text: Vec<String>,
}

impl SmartQuery {
    /// 将用户输入的查询字符串解析成结构化查询。
    ///
    /// # 参数
    /// - `input`：原始查询字符串
    ///
    /// # 返回
    /// - `MeloResult<Self>`：解析后的结构化查询
    pub fn parse(input: &str) -> MeloResult<Self> {
        let mut query = Self::default();

        for token in shell_words::split(input).map_err(|err| MeloError::Message(err.to_string()))? {
            if let Some(value) = token.strip_prefix("artist:") {
                query.artist = Some(value.to_string());
            } else if let Some(value) = token.strip_prefix("album:") {
                query.album = Some(value.to_string());
            } else if let Some(value) = token.strip_prefix("genre:") {
                query.genre = Some(value.to_string());
            } else if let Some(value) = token.strip_prefix("year:") {
                let mut parts = value.splitn(2, "..");
                query.year_start = parts.next().and_then(|part| part.parse::<i32>().ok());
                query.year_end = parts.next().and_then(|part| part.parse::<i32>().ok());
            } else {
                query.free_text.push(token);
            }
        }

        Ok(query)
    }
}
