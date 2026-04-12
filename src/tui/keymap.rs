use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::core::error::{MeloError, MeloResult};
use crate::tui::event::ActionId;

const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(700);

/// 按键解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Matched(ActionId),
    Pending,
    NoMatch,
}

/// 归一化后的单次按键输入。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: String,
    pub modifiers: String,
}

/// TUI keymap 解析器。
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<ActionId, Vec<Vec<KeyStroke>>>,
    pending: Vec<KeyStroke>,
    pending_since: Option<Instant>,
}

impl Default for Keymap {
    /// 返回内置默认 keymap。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：带默认绑定的解析器
    fn default() -> Self {
        let mut bindings = HashMap::new();
        bindings.insert(ActionId::FocusNext, vec![vec![KeyStroke::named("tab")]]);
        bindings.insert(
            ActionId::FocusPrev,
            vec![vec![KeyStroke::modified("tab", "shift")]],
        );
        bindings.insert(
            ActionId::MoveUp,
            vec![vec![KeyStroke::named("up")], vec![KeyStroke::char('k')]],
        );
        bindings.insert(
            ActionId::MoveDown,
            vec![vec![KeyStroke::named("down")], vec![KeyStroke::char('j')]],
        );
        bindings.insert(
            ActionId::JumpTop,
            vec![
                vec![KeyStroke::named("home")],
                vec![KeyStroke::char('g'), KeyStroke::char('g')],
            ],
        );
        bindings.insert(
            ActionId::JumpBottom,
            vec![
                vec![KeyStroke::named("end")],
                vec![KeyStroke::modified("g", "shift")],
            ],
        );
        bindings.insert(ActionId::PageUp, vec![vec![KeyStroke::named("pageup")]]);
        bindings.insert(ActionId::PageDown, vec![vec![KeyStroke::named("pagedown")]]);
        bindings.insert(ActionId::Activate, vec![vec![KeyStroke::named("enter")]]);
        bindings.insert(
            ActionId::TogglePlayback,
            vec![vec![KeyStroke::named("space")]],
        );
        bindings.insert(ActionId::Next, vec![vec![KeyStroke::char('>')]]);
        bindings.insert(ActionId::Prev, vec![vec![KeyStroke::char('<')]]);
        bindings.insert(ActionId::CycleRepeatMode, vec![vec![KeyStroke::char('r')]]);
        bindings.insert(ActionId::ToggleShuffle, vec![vec![KeyStroke::char('s')]]);
        bindings.insert(ActionId::OpenHelp, vec![vec![KeyStroke::char('?')]]);
        bindings.insert(ActionId::Quit, vec![vec![KeyStroke::char('q')]]);

        Self {
            bindings,
            pending: Vec::new(),
            pending_since: None,
        }
    }
}

impl KeyStroke {
    /// 构造一个字符按键。
    ///
    /// # 参数
    /// - `ch`：字符按键
    ///
    /// # 返回值
    /// - `Self`：归一化后的按键描述
    pub fn char(ch: char) -> Self {
        Self {
            code: ch.to_ascii_lowercase().to_string(),
            modifiers: "none".to_string(),
        }
    }

    /// 构造一个无修饰键的命名按键。
    ///
    /// # 参数
    /// - `code`：按键代码名
    ///
    /// # 返回值
    /// - `Self`：归一化后的按键描述
    pub fn named(code: &str) -> Self {
        Self {
            code: code.to_ascii_lowercase(),
            modifiers: "none".to_string(),
        }
    }

    /// 构造一个带修饰键的命名按键。
    ///
    /// # 参数
    /// - `code`：按键代码名
    /// - `modifiers`：修饰键文本
    ///
    /// # 返回值
    /// - `Self`：归一化后的按键描述
    pub fn modified(code: &str, modifiers: &str) -> Self {
        Self {
            code: code.to_ascii_lowercase(),
            modifiers: modifiers.to_ascii_lowercase(),
        }
    }
}

impl Keymap {
    /// 根据配置覆盖构造 keymap。
    ///
    /// # 参数
    /// - `overrides`：动作到绑定列表的配置覆盖
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：构造成功时返回解析器
    pub fn from_settings(
        overrides: &BTreeMap<String, Vec<crate::core::config::settings::TuiBindingSpec>>,
    ) -> MeloResult<Self> {
        let mut keymap = Self::default();
        for (action_name, specs) in overrides {
            let action = ActionId::from_config_name(action_name)?;
            keymap.bindings.insert(action, parse_specs(specs)?);
        }
        Ok(keymap)
    }

    /// 解析一次按键输入。
    ///
    /// # 参数
    /// - `event`：原始键盘事件
    /// - `now`：当前时间，用于序列超时判断
    ///
    /// # 返回值
    /// - `Resolution`：命中、等待后续按键或未命中
    pub fn resolve_key(&mut self, event: KeyEvent, now: Instant) -> Resolution {
        if self
            .pending_since
            .is_some_and(|started| now.duration_since(started) > SEQUENCE_TIMEOUT)
        {
            self.pending.clear();
            self.pending_since = None;
        }

        let stroke = normalize_key_event(event);
        self.pending.push(stroke);
        self.pending_since.get_or_insert(now);

        let mut saw_prefix = false;
        for (action, bindings) in &self.bindings {
            for binding in bindings {
                if binding == &self.pending {
                    self.pending.clear();
                    self.pending_since = None;
                    return Resolution::Matched(*action);
                }
                if binding.starts_with(&self.pending) {
                    saw_prefix = true;
                }
            }
        }

        if saw_prefix {
            Resolution::Pending
        } else {
            self.pending.clear();
            self.pending_since = None;
            Resolution::NoMatch
        }
    }

    /// 返回某个动作的首选绑定描述。
    ///
    /// # 参数
    /// - `action`：目标动作
    ///
    /// # 返回值
    /// - `String`：首选绑定文本；未绑定时返回 `unbound`
    pub fn describe(&self, action: ActionId) -> String {
        self.bindings
            .get(&action)
            .and_then(|bindings| bindings.first())
            .map(|binding| format_binding(binding))
            .unwrap_or_else(|| "unbound".to_string())
    }
}

impl ActionId {
    /// 将配置中的动作名转换为稳定动作 ID。
    ///
    /// # 参数
    /// - `name`：配置中的动作名
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：转换成功时返回动作 ID
    pub fn from_config_name(name: &str) -> MeloResult<Self> {
        match name {
            "focus_next" => Ok(ActionId::FocusNext),
            "focus_prev" => Ok(ActionId::FocusPrev),
            "move_up" => Ok(ActionId::MoveUp),
            "move_down" => Ok(ActionId::MoveDown),
            "jump_top" => Ok(ActionId::JumpTop),
            "jump_bottom" => Ok(ActionId::JumpBottom),
            "page_up" => Ok(ActionId::PageUp),
            "page_down" => Ok(ActionId::PageDown),
            "activate" => Ok(ActionId::Activate),
            "toggle_playback" => Ok(ActionId::TogglePlayback),
            "next" => Ok(ActionId::Next),
            "prev" => Ok(ActionId::Prev),
            "cycle_repeat_mode" => Ok(ActionId::CycleRepeatMode),
            "toggle_shuffle" => Ok(ActionId::ToggleShuffle),
            "open_help" => Ok(ActionId::OpenHelp),
            "quit" => Ok(ActionId::Quit),
            other => Err(MeloError::Message(format!("unknown_tui_action:{other}"))),
        }
    }
}

/// 将配置中的绑定规格解析为运行时绑定。
///
/// # 参数
/// - `specs`：配置中的绑定规格列表
///
/// # 返回值
/// - `MeloResult<Vec<Vec<KeyStroke>>>`：解析后的绑定序列集合
fn parse_specs(
    specs: &[crate::core::config::settings::TuiBindingSpec],
) -> MeloResult<Vec<Vec<KeyStroke>>> {
    specs
        .iter()
        .map(|spec| match spec {
            crate::core::config::settings::TuiBindingSpec::Chord(text) => {
                Ok(vec![parse_stroke(text)?])
            }
            crate::core::config::settings::TuiBindingSpec::Sequence(items) => {
                items.iter().map(|item| parse_stroke(item)).collect()
            }
        })
        .collect()
}

/// 将单条绑定文本解析为归一化按键。
///
/// # 参数
/// - `text`：例如 `tab`、`shift+tab`、`g`
///
/// # 返回值
/// - `MeloResult<KeyStroke>`：解析后的按键描述
fn parse_stroke(text: &str) -> MeloResult<KeyStroke> {
    let parts = text
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [code] => Ok(KeyStroke::named(code)),
        [modifier, code] => Ok(KeyStroke::modified(code, modifier)),
        _ => Err(MeloError::Message(format!("invalid_tui_binding:{text}"))),
    }
}

/// 将 crossterm 键盘事件归一化为稳定按键描述。
///
/// # 参数
/// - `event`：原始键盘事件
///
/// # 返回值
/// - `KeyStroke`：归一化后的按键描述
fn normalize_key_event(event: KeyEvent) -> KeyStroke {
    let modifiers = if event.modifiers.contains(KeyModifiers::SHIFT) {
        "shift"
    } else if event.modifiers.contains(KeyModifiers::CONTROL) {
        "ctrl"
    } else if event.modifiers.contains(KeyModifiers::ALT) {
        "alt"
    } else {
        "none"
    };

    let code = match event.code {
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "tab".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(ch) => ch.to_ascii_lowercase().to_string(),
        other => format!("{other:?}").to_ascii_lowercase(),
    };

    KeyStroke::modified(&code, modifiers)
}

/// 将单个绑定序列格式化为帮助文案。
///
/// # 参数
/// - `binding`：按键序列
///
/// # 返回值
/// - `String`：可显示的绑定文本
fn format_binding(binding: &[KeyStroke]) -> String {
    binding
        .iter()
        .map(|stroke| {
            if stroke.modifiers == "none" {
                stroke.code.clone()
            } else {
                format!("{}+{}", stroke.modifiers, stroke.code)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
