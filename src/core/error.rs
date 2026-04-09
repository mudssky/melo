use thiserror::Error;

/// Melo 的统一错误类型。
#[derive(Debug, Error)]
pub enum MeloError {
    /// 兜底字符串错误，后续再按领域细分。
    #[error("{0}")]
    Message(String),
}

/// 项目级 Result 别名。
pub type MeloResult<T> = Result<T, MeloError>;
