use std::ffi::OsString;

use crate::core::config::settings::LoggingLevel;
use crate::core::error::{MeloError, MeloResult};
use crate::core::logging::CliLogOverrides;

/// 预解析后的参数集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedArgs {
    /// 保留给 Clap 的参数数组。
    pub clap_args: Vec<OsString>,
    /// 保留给预分发逻辑的参数数组。
    pub dispatch_args: Vec<OsString>,
    /// 从原始参数提取出的日志覆盖项。
    pub logging: CliLogOverrides,
}

/// 在进入 Clap 与预分发前提取全局日志参数。
///
/// # 参数
/// - `raw_args`：原始命令行参数
///
/// # 返回值
/// - `MeloResult<PreparedArgs>`：剥离日志参数后的参数集合
pub fn prepare_args(raw_args: &[OsString]) -> MeloResult<PreparedArgs> {
    let mut clap_args = Vec::with_capacity(raw_args.len());
    let mut dispatch_args = Vec::with_capacity(raw_args.len());
    let mut logging = CliLogOverrides::default();

    if let Some(program) = raw_args.first() {
        clap_args.push(program.clone());
        dispatch_args.push(program.clone());
    }

    let mut index = 1usize;
    let mut parsing_global_flags = true;
    while index < raw_args.len() {
        if !parsing_global_flags {
            clap_args.push(raw_args[index].clone());
            dispatch_args.push(raw_args[index].clone());
            index += 1;
            continue;
        }

        let Some(current) = raw_args[index].to_str() else {
            clap_args.push(raw_args[index].clone());
            dispatch_args.push(raw_args[index].clone());
            parsing_global_flags = false;
            index += 1;
            continue;
        };

        match current {
            "--verbose" => {
                logging.verbose = true;
                index += 1;
            }
            "--no-log-prefix" => {
                logging.no_log_prefix = true;
                index += 1;
            }
            "--log-level" => {
                let value = raw_args
                    .get(index + 1)
                    .and_then(|item| item.to_str())
                    .ok_or_else(|| MeloError::Message("missing_log_level_value".to_string()))?;
                logging.log_level = Some(parse_level(value)?);
                index += 2;
            }
            "--daemon-log-level" => {
                let value = raw_args
                    .get(index + 1)
                    .and_then(|item| item.to_str())
                    .ok_or_else(|| {
                        MeloError::Message("missing_daemon_log_level_value".to_string())
                    })?;
                logging.daemon_log_level = Some(parse_level(value)?);
                index += 2;
            }
            _ => {
                clap_args.push(raw_args[index].clone());
                dispatch_args.push(raw_args[index].clone());
                parsing_global_flags = false;
                index += 1;
            }
        }
    }

    Ok(PreparedArgs {
        clap_args,
        dispatch_args,
        logging,
    })
}

/// 解析日志等级文本。
///
/// # 参数
/// - `value`：命令行中的等级字符串
///
/// # 返回值
/// - `MeloResult<LoggingLevel>`：解析后的日志等级
fn parse_level(value: &str) -> MeloResult<LoggingLevel> {
    value
        .parse()
        .map_err(|_| MeloError::Message(format!("unsupported_log_level:{value}")))
}

#[cfg(test)]
mod tests;
