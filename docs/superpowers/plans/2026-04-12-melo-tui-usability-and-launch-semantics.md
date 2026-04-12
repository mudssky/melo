# Melo TUI Usability and Launch Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Melo’s TUI genuinely usable by fixing dev-wrapper launch cwd semantics, preserving active playback on bare `melo`, defaulting idle launches to the caller’s current directory, adding visible focus/highlight states plus mouse support, and introducing a configurable action-driven keymap.

**Architecture:** Keep the current `playlist`-first TUI information architecture, but insert one formal interaction layer between raw input and UI/daemon effects. Split the work into five slices: launch semantics, keymap configuration, mouse intent normalization, stateful Ratatui rendering, and final runtime wiring with verbose/TUI boundary cleanup. Use TDD for each slice and keep commits narrow so each commit leaves the app working.

**Tech Stack:** Rust, Tokio, Axum, Ratatui, Crossterm, reqwest, serde, Clap, Node.js, Vitest, pnpm

---

## File Structure

### Launch semantics and development wrapper

- Modify: `bin/melo-dev.cjs`
  - Responsibility: keep injecting `config.dev.toml`, but stop overriding the caller’s working directory.
- Modify: `tests/dev-cli/melo-dev-wrapper.test.ts`
  - Responsibility: assert the wrapper preserves the caller’s cwd while still injecting `MELO_CONFIG`.
- Create: `src/cli/launch.rs`
  - Responsibility: define `DefaultLaunchDecision` and encapsulate “preserve current session vs open launch cwd vs open explicit target”.
- Create: `src/cli/launch/tests.rs`
  - Responsibility: unit-test launch decision rules without needing a live daemon.
- Modify: `src/cli/mod.rs`
  - Responsibility: export the new launch module.
- Modify: `src/cli/run.rs`
  - Responsibility: use `DefaultLaunchDecision` for bare launches and drop the verbose mirror before entering TUI.
- Modify: `src/cli/run/tests.rs`
  - Responsibility: cover default launch context assembly and verbose mirror scoping.

### Configurable keymap foundation

- Modify: `src/core/config/settings.rs`
  - Responsibility: add `tui.mouse_enabled`, `tui.keymap`, and typed settings models for action-driven bindings.
- Modify: `config.example.toml`
  - Responsibility: document the default keymap and mouse toggle.
- Modify: `tests/config_loading.rs`
  - Responsibility: verify keymap/mouse settings parse correctly.
- Modify: `src/tui/event.rs`
  - Responsibility: define stable `ActionId` values and richer `Intent` types shared by keyboard and mouse.
- Create: `src/tui/keymap.rs`
  - Responsibility: parse/validate binding specs, provide defaults, and resolve single keys, chords, and sequences.
- Create: `src/tui/keymap/tests.rs`
  - Responsibility: unit-test single binding, multi-binding, sequence prefix, timeout, and invalid action/binding handling.
- Modify: `src/tui/mod.rs`
  - Responsibility: export the new keymap module.

### Mouse intent normalization

- Create: `src/tui/mouse.rs`
  - Responsibility: map `crossterm` mouse events into typed `Intent`s, including software double-click detection because current `crossterm` docs only expose down/up/drag/moved/scroll events plus mouse capture commands.
- Create: `src/tui/mouse/tests.rs`
  - Responsibility: unit-test click classification, double-click timing, and wheel-to-scroll conversion.
- Modify: `src/tui/mod.rs`
  - Responsibility: export the new mouse module.
- Modify: `src/tui/app.rs`
  - Responsibility: expose narrow selection helpers that mouse intents can call.

### Stateful TUI rendering and visible interaction feedback

- Modify: `src/tui/app.rs`
  - Responsibility: hold launch cwd context, list states, preview list state, and intent application helpers.
- Modify: `src/tui/app/tests.rs`
  - Responsibility: cover focus shifts, selection persistence, `Esc`, page jumps, and activate semantics.
- Modify: `src/tui/theme.rs`
  - Responsibility: define focused-border, selected-row, playing-source, and combined visual styles.
- Modify: `src/tui/ui/layout.rs`
  - Responsibility: keep the current 4-region layout but expose exact pane rectangles for hit testing.
- Modify: `src/tui/ui/playlist.rs`
  - Responsibility: replace string-only rendering with stateful `List` widgets, visible focus borders, current-playing badges, and row hit-test helpers.
- Modify: `src/tui/ui/popup.rs`
  - Responsibility: render help lines from the resolved keymap instead of hardcoded text.
- Modify: `src/tui/ui/mod.rs`
  - Responsibility: export any new playlist rendering helpers.
- Modify: `tests/tui_app.rs`
  - Responsibility: integration coverage for visible focus/highlight semantics and footer/help content.

### Runtime wiring, current-play preservation, and verbose boundary

- Modify: `src/tui/run.rs`
  - Responsibility: enable/disable mouse capture, construct the keymap resolver, route keyboard+mouse events through `Intent`, and keep launch cwd/current source visible in the status pane.
- Modify: `src/tui/run/tests.rs`
  - Responsibility: cover repeat cycling, keymap-driven help generation, and intent dispatch helpers.
- Modify: `tests/tui_home.rs`
  - Responsibility: cover idle bare-launch cwd selection and active-session preservation defaults.
- Modify: `tests/cli_remote.rs`
  - Responsibility: verify bare launch startup semantics and the “verbose before TUI, not during TUI” boundary with scoped startup helpers.

## Task 1: Preserve Caller CWD and Introduce Bare-Launch Decision Logic

**Files:**
- Modify: `bin/melo-dev.cjs`
- Modify: `tests/dev-cli/melo-dev-wrapper.test.ts`
- Create: `src/cli/launch.rs`
- Create: `src/cli/launch/tests.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/cli/run/tests.rs`

- [ ] **Step 1: Write the failing wrapper and launch-decision tests**

Update `tests/dev-cli/melo-dev-wrapper.test.ts` to assert the wrapper preserves the caller’s cwd:

```ts
it('spawns the installed binary from the caller cwd with forwarded args', () => {
  const spawnSyncImpl = vi.fn(() => ({ status: 0 }))

  const exitCode = wrapper.run(['status'], {
    cwd: 'D:/Music/Aimer',
    env: {},
    homeDir: 'C:/Users/dev',
    platform: 'win32',
    repoRoot,
    spawnSyncImpl,
  })

  expect(exitCode).toBe(0)
  expect(spawnSyncImpl).toHaveBeenCalledWith(
    path.join('C:/Users/dev', '.cargo', 'bin', 'melo.exe'),
    ['status'],
    expect.objectContaining({
      cwd: 'D:/Music/Aimer',
      stdio: 'inherit',
      env: expect.objectContaining({
        MELO_CONFIG: path.join(repoRoot, 'config.dev.toml'),
      }),
    }),
  )
})
```

Create `src/cli/launch/tests.rs` with:

```rust
use std::path::Path;

use crate::cli::launch::{DefaultLaunchDecision, choose_default_launch_decision};

fn playing_snapshot() -> crate::core::model::tui::TuiSnapshot {
    crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot {
            playback_state: crate::core::model::player::PlaybackState::Playing,
            ..crate::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot {
            default_view: crate::core::model::tui::TuiViewKind::Playlist,
            default_selected_playlist: Some("Favorites".to_string()),
            current_playing_playlist: Some(crate::core::model::tui::PlaylistListItem {
                name: "Favorites".to_string(),
                kind: "static".to_string(),
                count: 3,
                is_current_playing_source: true,
                is_ephemeral: false,
            }),
            visible_playlists: Vec::new(),
        },
    }
}

#[test]
fn choose_default_launch_decision_preserves_active_playback_session() {
    let decision =
        choose_default_launch_decision(Path::new("D:/Music/Aimer"), &playing_snapshot());

    assert_eq!(
        decision,
        DefaultLaunchDecision::PreserveCurrentSession {
            launch_cwd: "D:/Music/Aimer".to_string(),
            playlist_name: "Favorites".to_string(),
        }
    );
}

#[test]
fn choose_default_launch_decision_opens_launch_cwd_when_not_playing() {
    let mut snapshot = playing_snapshot();
    snapshot.player.playback_state = crate::core::model::player::PlaybackState::Stopped;

    let decision = choose_default_launch_decision(Path::new("D:/Music/Aimer"), &snapshot);

    assert_eq!(
        decision,
        DefaultLaunchDecision::OpenLaunchCwd {
            launch_cwd: "D:/Music/Aimer".to_string(),
        }
    );
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test choose_default_launch_decision_ --lib -- --nocapture`
Expected: FAIL because `src/cli/launch.rs` does not exist yet.

Run: `pnpm exec vitest run tests/dev-cli/melo-dev-wrapper.test.ts`
Expected: FAIL because the wrapper still forces `cwd` to `repoRoot`.

- [ ] **Step 3: Implement the wrapper cwd fix**

Update `bin/melo-dev.cjs` so `run()` respects the caller cwd:

```js
function run(argv = process.argv.slice(2), options = {}) {
  const repoRoot =
    options.repoRoot ?? resolveRepoRoot(options.scriptPath ?? __filename)
  const env = buildChildEnv(options.env ?? process.env, repoRoot)
  const binaryPath = resolveBinaryPath(
    env,
    options.platform ?? process.platform,
    options.homeDir ?? os.homedir(),
  )
  const childCwd = options.cwd ?? process.cwd()
  const result = (options.spawnSyncImpl ?? spawnSync)(binaryPath, argv, {
    cwd: childCwd,
    env,
    stdio: 'inherit',
  })

  if (result.error) {
    throw result.error
  }

  return typeof result.status === 'number' ? result.status : 1
}
```

- [ ] **Step 4: Add `DefaultLaunchDecision` and route bare launch through it**

Create `src/cli/launch.rs`:

```rust
/// 裸 `melo` 启动时的默认决策。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultLaunchDecision {
    PreserveCurrentSession {
        launch_cwd: String,
        playlist_name: String,
    },
    OpenLaunchCwd {
        launch_cwd: String,
    },
}

/// 根据当前播放快照和调用目录决定裸启动语义。
///
/// # 参数
/// - `launch_cwd`：调用方 shell 当前目录
/// - `snapshot`：daemon 当前 TUI 首页聚合快照
///
/// # 返回值
/// - `DefaultLaunchDecision`：裸启动的默认行为
pub fn choose_default_launch_decision(
    launch_cwd: &std::path::Path,
    snapshot: &crate::core::model::tui::TuiSnapshot,
) -> DefaultLaunchDecision {
    let launch_cwd = launch_cwd.to_string_lossy().into_owned();
    let is_playing = matches!(
        snapshot.player.playback_state,
        crate::core::model::player::PlaybackState::Playing
    );

    if is_playing
        && let Some(current) = snapshot.playlist_browser.current_playing_playlist.as_ref()
    {
        return DefaultLaunchDecision::PreserveCurrentSession {
            launch_cwd,
            playlist_name: current.name.clone(),
        };
    }

    DefaultLaunchDecision::OpenLaunchCwd { launch_cwd }
}

#[cfg(test)]
mod tests;
```

Expose it from `src/cli/mod.rs`:

```rust
pub mod launch;
```

Update the default branch in `src/cli/run.rs`:

```rust
let launch_cwd = std::env::current_dir()
    .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
let home = crate::cli::client::ApiClient::new(base_url.clone()).tui_home().await?;
let decision = crate::cli::launch::choose_default_launch_decision(&launch_cwd, &home);
let (source_label, startup_notice) = match decision {
    crate::cli::launch::DefaultLaunchDecision::PreserveCurrentSession { .. } => {
        (None, Some("Continuing current playback".to_string()))
    }
    crate::cli::launch::DefaultLaunchDecision::OpenLaunchCwd { launch_cwd } => {
        if let Some(line) = render_scan_cli_lines(&renderer, &settings, &launch_cwd).first() {
            println!("{line}");
        }
        let opened = crate::cli::client::ApiClient::new(base_url.clone())
            .open_target(launch_cwd.clone(), "cwd_dir")
            .await?;
        if let Some(line) = render_scan_cli_lines(&renderer, &settings, &opened.source_label).get(1)
        {
            println!("{line}");
        }
        (Some(opened.source_label), None)
    }
};
```

- [ ] **Step 5: Scope the verbose log mirror to startup only and test the helper**

Add a helper to `src/cli/run.rs` and test it in `src/cli/run/tests.rs`:

```rust
pub(crate) fn launch_cwd_text(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
```

Update the default launch flow so the mirror guard is dropped before `tui::run::start(...)`:

```rust
let launch = {
    let _mirror = if prepared.logging.verbose {
        Some(crate::core::logging::attach_daemon_log_mirror(
            crate::core::logging::daemon_log_path(&settings),
            resolved_cli.prefix_enabled,
            settings.logging.daemon_prefix.clone(),
        ))
    } else {
        None
    };

    let ensured =
        crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging).await?;
    // prepare launch context here, then leave the scope
    (ensured, source_label, startup_notice)
};

crate::tui::run::start(
    base_url,
    crate::tui::run::LaunchContext {
        launch_cwd: Some(launch_cwd_text(&launch_cwd)),
        source_label,
        startup_notice,
        footer_hints_enabled: settings.tui.show_footer_hints,
    },
)
```

Add this test to `src/cli/run/tests.rs`:

```rust
#[test]
fn launch_cwd_text_preserves_runtime_directory() {
    let text = super::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}
```

- [ ] **Step 6: Re-run focused verification and commit**

Run: `cargo test choose_default_launch_decision_ launch_cwd_text_preserves_runtime_directory --lib -- --nocapture`
Expected: PASS.

Run: `pnpm exec vitest run tests/dev-cli/melo-dev-wrapper.test.ts`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add bin/melo-dev.cjs tests/dev-cli/melo-dev-wrapper.test.ts src/cli/launch.rs src/cli/launch/tests.rs src/cli/mod.rs src/cli/run.rs src/cli/run/tests.rs
git commit -m "feat(cli): preserve caller cwd and bare launch semantics"
```

## Task 2: Add Action-Driven TUI Keymap Configuration and Resolver

**Files:**
- Modify: `src/core/config/settings.rs`
- Modify: `config.example.toml`
- Modify: `tests/config_loading.rs`
- Modify: `src/tui/event.rs`
- Create: `src/tui/keymap.rs`
- Create: `src/tui/keymap/tests.rs`
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Write the failing config-loading and resolver tests**

Add this test to `tests/config_loading.rs`:

```rust
#[test]
fn settings_load_tui_keymap_and_mouse_toggle() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[tui]
show_footer_hints = true
mouse_enabled = true

[tui.keymap]
focus_next = ["tab", "l"]
focus_prev = ["shift+tab", "h"]
jump_top = ["home", ["g", "g"]]
"#,
    )
    .unwrap();

    let settings = melo::core::config::settings::Settings::load_from_path(&path).unwrap();
    assert!(settings.tui.mouse_enabled);
    assert_eq!(
        settings.tui.keymap.get("focus_next").unwrap()[0],
        melo::core::config::settings::TuiBindingSpec::Chord("tab".to_string())
    );
}
```

Create `src/tui/keymap/tests.rs`:

```rust
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::event::ActionId;
use crate::tui::keymap::{Keymap, Resolution};

#[test]
fn keymap_matches_single_binding() {
    let mut keymap = Keymap::default();
    let resolution = keymap.resolve_key(
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        Instant::now(),
    );

    assert_eq!(resolution, Resolution::Matched(ActionId::FocusNext));
}

#[test]
fn keymap_waits_for_sequence_prefix() {
    let mut keymap = Keymap::default();
    let now = Instant::now();

    assert_eq!(
        keymap.resolve_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), now),
        Resolution::Pending
    );
    assert_eq!(
        keymap.resolve_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            now + Duration::from_millis(100)
        ),
        Resolution::Matched(ActionId::JumpTop)
    );
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test settings_load_tui_keymap_and_mouse_toggle keymap_matches_single_binding keymap_waits_for_sequence_prefix -- --nocapture`
Expected: FAIL because `mouse_enabled`, `TuiBindingSpec`, `ActionId`, and `Keymap` do not exist yet.

- [ ] **Step 3: Add keymap settings and stable `ActionId` values**

Update `src/core/config/settings.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum TuiBindingSpec {
    Chord(String),
    Sequence(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    pub show_footer_hints: bool,
    pub mouse_enabled: bool,
    pub keymap: std::collections::BTreeMap<String, Vec<TuiBindingSpec>>,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            show_footer_hints: true,
            mouse_enabled: true,
            keymap: std::collections::BTreeMap::new(),
        }
    }
}
```

Add defaults in `Settings::load_from_path(...)`:

```rust
.set_default("tui.show_footer_hints", true)?
.set_default("tui.mouse_enabled", true)?
```

Update `src/tui/event.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    FocusNext,
    FocusPrev,
    MoveUp,
    MoveDown,
    JumpTop,
    JumpBottom,
    PageUp,
    PageDown,
    Activate,
    PlaySelection,
    PlayPreviewSelection,
    LoadPreview,
    TogglePlayback,
    Next,
    Prev,
    CycleRepeatMode,
    ToggleShuffle,
    OpenHelp,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Action(ActionId),
    SelectPlaylist { index: usize, focus: bool },
    SelectPreview { index: usize, focus: bool },
    ScrollPreview(isize),
    ScrollPlaylist(isize),
}
```

- [ ] **Step 4: Implement the resolver with defaults, sequence prefix handling, and timeout**

Create `src/tui/keymap.rs`:

```rust
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::core::error::{MeloError, MeloResult};
use crate::tui::event::ActionId;

const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Matched(ActionId),
    Pending,
    NoMatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: String,
    pub modifiers: String,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<ActionId, Vec<Vec<KeyStroke>>>,
    pending: Vec<KeyStroke>,
    pending_since: Option<Instant>,
}

impl Default for Keymap {
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
        bindings.insert(ActionId::TogglePlayback, vec![vec![KeyStroke::named("space")]]);
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
    pub fn char(ch: char) -> Self {
        Self {
            code: ch.to_string(),
            modifiers: "none".to_string(),
        }
    }

    pub fn named(code: &str) -> Self {
        Self {
            code: code.to_string(),
            modifiers: "none".to_string(),
        }
    }

    pub fn modified(code: &str, modifiers: &str) -> Self {
        Self {
            code: code.to_string(),
            modifiers: modifiers.to_string(),
        }
    }
}

impl Keymap {
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

    pub fn describe(&self, action: ActionId) -> String {
        self.bindings
            .get(&action)
            .and_then(|bindings| bindings.first())
            .map(format_binding)
            .unwrap_or_else(|| "unbound".to_string())
    }
}

impl ActionId {
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

fn parse_stroke(text: &str) -> MeloResult<KeyStroke> {
    let parts = text.split('+').collect::<Vec<_>>();
    match parts.as_slice() {
        [code] => Ok(KeyStroke::named(code)),
        [modifier, code] => Ok(KeyStroke::modified(code, modifier)),
        _ => Err(MeloError::Message(format!("invalid_tui_binding:{text}"))),
    }
}

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
        other => format!("{other:?}").to_lowercase(),
    };

    KeyStroke::modified(&code, modifiers)
}

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
```

Expose the module from `src/tui/mod.rs`:

```rust
pub mod keymap;
```

- [ ] **Step 5: Re-run focused verification and commit**

Run: `cargo test settings_load_tui_keymap_and_mouse_toggle keymap_matches_single_binding keymap_waits_for_sequence_prefix -- --nocapture`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add src/core/config/settings.rs config.example.toml tests/config_loading.rs src/tui/event.rs src/tui/keymap.rs src/tui/keymap/tests.rs src/tui/mod.rs
git commit -m "feat(tui): add configurable action-driven keymap"
```

## Task 3: Normalize Mouse Input and Add Software Double-Click Detection

**Files:**
- Create: `src/tui/mouse.rs`
- Create: `src/tui/mouse/tests.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/app/tests.rs`
- Modify: `src/tui/ui/layout.rs`
- Modify: `src/tui/ui/playlist.rs`

- [ ] **Step 1: Write the failing mouse tests**

Create `src/tui/mouse/tests.rs`:

```rust
use std::time::{Duration, Instant};

use crate::tui::mouse::{ClickKind, ClickTracker, MouseTarget};

#[test]
fn click_tracker_promotes_second_click_on_same_target_to_double_click() {
    let mut tracker = ClickTracker::default();
    let now = Instant::now();

    assert_eq!(
        tracker.classify(MouseTarget::PlaylistRow(3), now),
        ClickKind::Single
    );
    assert_eq!(
        tracker.classify(MouseTarget::PlaylistRow(3), now + Duration::from_millis(200)),
        ClickKind::Double
    );
}

#[test]
fn click_tracker_resets_when_target_changes() {
    let mut tracker = ClickTracker::default();
    let now = Instant::now();

    assert_eq!(tracker.classify(MouseTarget::PlaylistRow(1), now), ClickKind::Single);
    assert_eq!(
        tracker.classify(MouseTarget::PreviewRow(1), now + Duration::from_millis(200)),
        ClickKind::Single
    );
}
```

Add this to `src/tui/app/tests.rs`:

```rust
#[test]
fn selecting_playlist_index_updates_highlight_without_playing() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
    });

    let effect = app.select_playlist_index(1);

    assert_eq!(
        effect,
        Some(crate::tui::event::Intent::Action(
            crate::tui::event::ActionId::LoadPreview
        ))
    );
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));
}
```

- [ ] **Step 2: Run the mouse tests to confirm they fail**

Run: `cargo test click_tracker_promotes_second_click_on_same_target_to_double_click click_tracker_resets_when_target_changes selecting_playlist_index_updates_highlight_without_playing -- --nocapture`
Expected: FAIL because the mouse module and selection helpers do not exist yet.

- [ ] **Step 3: Implement mouse targets, click tracking, and wheel normalization**

Create `src/tui/mouse.rs`:

```rust
use std::time::{Duration, Instant};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    PlaylistRow(usize),
    PreviewRow(usize),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    Single,
    Double,
}

#[derive(Debug, Default)]
pub struct ClickTracker {
    last_click: Option<(MouseTarget, Instant)>,
}

impl ClickTracker {
    pub fn classify(&mut self, target: MouseTarget, now: Instant) -> ClickKind {
        let kind = match self.last_click {
            Some((previous_target, previous_at))
                if previous_target == target
                    && now.duration_since(previous_at) <= DOUBLE_CLICK_WINDOW =>
            {
                ClickKind::Double
            }
            _ => ClickKind::Single,
        };

        self.last_click = Some((target, now));
        kind
    }
}
```

- [ ] **Step 4: Add app selection helpers and row hit-test helpers**

Add this to `src/tui/app.rs`:

```rust
pub fn select_playlist_index(
    &mut self,
    index: usize,
) -> Option<crate::tui::event::Intent> {
    let next_name = self
        .playlist_browser
        .visible_playlists
        .get(index)
        .map(|playlist| playlist.name.clone())?;

    self.focus = FocusArea::PlaylistList;
    if self.selected_playlist_name.as_deref() == Some(next_name.as_str()) {
        return None;
    }

    self.selected_playlist_name = Some(next_name);
    self.preview_error = None;
    self.preview_loading = false;
    self.preview_titles.clear();
    self.selected_preview_index = 0;
    Some(crate::tui::event::Intent::Action(
        crate::tui::event::ActionId::LoadPreview,
    ))
}

pub fn select_preview_index(&mut self, index: usize) {
    if index < self.preview_titles.len() {
        self.focus = FocusArea::PlaylistPreview;
        self.selected_preview_index = index;
    }
}
```

Add row hit-test helpers to `src/tui/ui/playlist.rs`:

```rust
pub fn playlist_index_at(
    area: ratatui::layout::Rect,
    row: u16,
    item_count: usize,
) -> Option<usize> {
    let start = area.y.saturating_add(2);
    if row < start {
        return None;
    }
    let index = (row - start) as usize;
    (index < item_count).then_some(index)
}

pub fn preview_index_at(
    area: ratatui::layout::Rect,
    row: u16,
    item_count: usize,
) -> Option<usize> {
    let start = area.y.saturating_add(1);
    if row < start {
        return None;
    }
    let index = (row - start) as usize;
    (index < item_count).then_some(index)
}
```

Expose the module from `src/tui/mod.rs`:

```rust
pub mod mouse;
```

- [ ] **Step 5: Re-run focused verification and commit**

Run: `cargo test click_tracker_promotes_second_click_on_same_target_to_double_click click_tracker_resets_when_target_changes selecting_playlist_index_updates_highlight_without_playing -- --nocapture`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add src/tui/mouse.rs src/tui/mouse/tests.rs src/tui/mod.rs src/tui/app.rs src/tui/app/tests.rs src/tui/ui/layout.rs src/tui/ui/playlist.rs
git commit -m "feat(tui): normalize mouse input and double-click activation"
```

## Task 4: Rebuild the Playlist-First TUI Rendering Around Stateful Lists and Visible Feedback

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/app/tests.rs`
- Modify: `src/tui/theme.rs`
- Modify: `src/tui/ui/playlist.rs`
- Modify: `src/tui/ui/popup.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing focus/highlight/help tests**

Update `src/tui/app/tests.rs` with:

```rust
#[test]
fn esc_returns_focus_to_playlist_list() {
    let mut app = crate::tui::app::App::new_for_test();
    app.focus = crate::tui::app::FocusArea::PlaylistPreview;

    let intent = app.handle_action(crate::tui::event::ActionId::FocusPrev);

    assert_eq!(intent, None);
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistList);
}
```

Update `tests/tui_app.rs` with:

```rust
#[test]
fn render_playlist_rows_marks_selected_and_current_source_separately() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
    });
    app.select_playlist_index(1);

    let rows = melo::tui::ui::playlist::playlist_row_models(&app);
    assert!(rows[0].is_current_source);
    assert!(!rows[0].is_selected);
    assert!(rows[1].is_selected);
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test esc_returns_focus_to_playlist_list render_playlist_rows_marks_selected_and_current_source_separately -- --nocapture`
Expected: FAIL because there is no stateful row model or `handle_action` helper yet.

- [ ] **Step 3: Add stateful list state, launch cwd context, and richer row models**

Update `src/tui/app.rs`:

```rust
use ratatui::widgets::ListState;

pub struct App {
    pub player: PlayerSnapshot,
    pub active_task: Option<crate::core::model::runtime_task::RuntimeTaskSnapshot>,
    pub active_view: ActiveView,
    pub focus: FocusArea,
    pub launch_cwd: Option<String>,
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
    pub show_help: bool,
    pub playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot,
    pub playlist_state: ListState,
    pub preview_state: ListState,
    pub selected_playlist_name: Option<String>,
    pub preview_name: Option<String>,
    pub preview_titles: Vec<String>,
    pub selected_preview_index: usize,
    pub preview_loading: bool,
    pub preview_error: Option<String>,
}

pub fn set_launch_cwd(&mut self, launch_cwd: impl Into<String>) {
    self.launch_cwd = Some(launch_cwd.into());
}

pub fn handle_action(
    &mut self,
    action: crate::tui::event::ActionId,
) -> Option<crate::tui::event::Intent> {
    match action {
        crate::tui::event::ActionId::FocusNext => {
            self.focus = FocusArea::PlaylistPreview;
            None
        }
        crate::tui::event::ActionId::FocusPrev => {
            self.focus = FocusArea::PlaylistList;
            None
        }
        crate::tui::event::ActionId::Activate => match self.focus {
            FocusArea::PlaylistList => Some(crate::tui::event::Intent::Action(
                crate::tui::event::ActionId::PlaySelection,
            )),
            FocusArea::PlaylistPreview => Some(crate::tui::event::Intent::Action(
                crate::tui::event::ActionId::PlayPreviewSelection,
            )),
        },
        _ => None,
    }
}
```

- [ ] **Step 4: Upgrade theme and playlist rendering to actual Ratatui widgets**

Update `src/tui/theme.rs`:

```rust
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub title_prefix: &'static str,
    pub muted_prefix: &'static str,
    pub pane_border: Style,
    pub focused_border: Style,
    pub selected_row: Style,
    pub current_source_row: Style,
    pub selected_current_source_row: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            title_prefix: "Melo",
            muted_prefix: "Remote",
            pane_border: Style::default().fg(Color::DarkGray),
            focused_border: Style::default().fg(Color::Cyan),
            selected_row: Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD),
            current_source_row: Style::default().fg(Color::Yellow),
            selected_current_source_row: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}
```

Update `src/tui/ui/playlist.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistRowModel {
    pub text: String,
    pub is_selected: bool,
    pub is_current_source: bool,
}

pub fn playlist_row_models(app: &crate::tui::app::App) -> Vec<PlaylistRowModel> {
    app.playlist_browser
        .visible_playlists
        .iter()
        .map(|playlist| PlaylistRowModel {
            text: format!("{} ({})", playlist.name, playlist.count),
            is_selected: app.selected_playlist_name() == Some(playlist.name.as_str()),
            is_current_source: playlist.is_current_playing_source,
        })
        .collect()
}

pub fn render_playlist_widget(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    app: &mut crate::tui::app::App,
    theme: crate::tui::theme::Theme,
) {
    let items = playlist_row_models(app)
        .into_iter()
        .map(|row| {
            let style = match (row.is_selected, row.is_current_source) {
                (true, true) => theme.selected_current_source_row,
                (true, false) => theme.selected_row,
                (false, true) => theme.current_source_row,
                (false, false) => ratatui::style::Style::default(),
            };
            ratatui::widgets::ListItem::new(row.text).style(style)
        })
        .collect::<Vec<_>>();

    let border_style = if app.focus == crate::tui::app::FocusArea::PlaylistList {
        theme.focused_border
    } else {
        theme.pane_border
    };

    let list = ratatui::widgets::List::new(items)
        .block(ratatui::widgets::Block::bordered().title("播放列表").border_style(border_style))
        .highlight_symbol(">> ")
        .repeat_highlight_symbol(true);

    frame.render_stateful_widget(list, area, &mut app.playlist_state);
}
```

Update `src/tui/ui/popup.rs` so help text is keymap-driven:

```rust
pub fn help_lines(keymap: &crate::tui::keymap::Keymap) -> Vec<String> {
    vec![
        format!("{} 切换焦点", keymap.describe(crate::tui::event::ActionId::FocusNext)),
        format!("{} 播放当前选择", keymap.describe(crate::tui::event::ActionId::Activate)),
        format!("{} 切换循环模式", keymap.describe(crate::tui::event::ActionId::CycleRepeatMode)),
        format!("{} 切换随机播放", keymap.describe(crate::tui::event::ActionId::ToggleShuffle)),
        format!("{} 播放/暂停", keymap.describe(crate::tui::event::ActionId::TogglePlayback)),
        format!("{} 打开帮助", keymap.describe(crate::tui::event::ActionId::OpenHelp)),
        format!("{} 退出", keymap.describe(crate::tui::event::ActionId::Quit)),
    ]
}
```

- [ ] **Step 5: Re-run focused verification and commit**

Run: `cargo test esc_returns_focus_to_playlist_list render_playlist_rows_marks_selected_and_current_source_separately -- --nocapture`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add src/tui/app.rs src/tui/app/tests.rs src/tui/theme.rs src/tui/ui/playlist.rs src/tui/ui/popup.rs tests/tui_app.rs
git commit -m "feat(tui): add visible focus and highlight feedback"
```

## Task 5: Wire Keyboard/Mouse Runtime Input, Launch Context, and Verbose Boundary

**Files:**
- Modify: `src/tui/run.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `tests/tui_home.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing runtime and startup-boundary tests**

Update `src/tui/run/tests.rs`:

```rust
#[test]
fn repeat_mode_cycles_off_all_one_off() {
    assert_eq!(crate::tui::run::next_repeat_mode("off"), "all");
    assert_eq!(crate::tui::run::next_repeat_mode("all"), "one");
    assert_eq!(crate::tui::run::next_repeat_mode("one"), "off");
}
```

Update `tests/tui_home.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn active_playback_session_keeps_current_playlist_as_default_selection() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("Favorites", "static");
    state.player.play().await.unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(
        snapshot.playlist_browser.default_selected_playlist.as_deref(),
        Some("Favorites")
    );
}
```

Update `tests/cli_remote.rs` with a scoped helper regression:

```rust
#[test]
fn verbose_default_launch_stops_terminal_log_mirror_before_tui_scope() {
    let text = melo::cli::run::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test repeat_mode_cycles_off_all_one_off active_playback_session_keeps_current_playlist_as_default_selection verbose_default_launch_stops_terminal_log_mirror_before_tui_scope -- --nocapture`
Expected: FAIL because runtime wiring still uses direct key matching and the new helper/test paths are not fully connected.

- [ ] **Step 3: Enable mouse capture, keymap resolution, and launch cwd context in the runtime loop**

Update `src/tui/run.rs`:

```rust
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind};

pub struct LaunchContext {
    pub launch_cwd: Option<String>,
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
}

pub(crate) fn next_repeat_mode(current: &str) -> &'static str {
    match current {
        "off" => "all",
        "all" => "one",
        _ => "off",
    }
}

pub async fn start(base_url: String, context: LaunchContext) -> MeloResult<()> {
    crossterm::terminal::enable_raw_mode().map_err(|err| MeloError::Message(err.to_string()))?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|err| MeloError::Message(err.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| MeloError::Message(err.to_string()))?;

    let result = run_loop(&mut terminal, base_url, context).await;

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal.show_cursor();
    result
}
```

Inside `run_loop(...)`:

```rust
let settings = crate::core::config::settings::Settings::load().unwrap_or_default();
let mut keymap = crate::tui::keymap::Keymap::from_settings(&settings.tui.keymap)?;
let mut click_tracker = crate::tui::mouse::ClickTracker::default();
let theme = crate::tui::theme::Theme::default();

app.footer_hints_enabled = context.footer_hints_enabled;
app.startup_notice = context.startup_notice;
if let Some(launch_cwd) = context.launch_cwd {
    app.set_launch_cwd(launch_cwd);
}
if let Some(source_label) = context.source_label {
    app.set_source_label(source_label);
}
```

And in the event loop:

```rust
match event {
    Event::Key(key) if key.kind == KeyEventKind::Press => {
        match keymap.resolve_key(key, std::time::Instant::now()) {
            crate::tui::keymap::Resolution::Matched(action) => {
                if let Some(intent) = app.handle_action(action) {
                    dispatch_intent(&mut app, intent, &api_client).await?;
                }
            }
            crate::tui::keymap::Resolution::Pending => {}
            crate::tui::keymap::Resolution::NoMatch => {}
        }
    }
    Event::Mouse(mouse) if settings.tui.mouse_enabled => {
        let target = match mouse.kind {
            MouseEventKind::Down(_) => hit_test_mouse_target(layout, &app, mouse.column, mouse.row),
            MouseEventKind::ScrollUp => {
                dispatch_intent(&mut app, crate::tui::event::Intent::ScrollPlaylist(-1), &api_client).await?;
                crate::tui::mouse::MouseTarget::None
            }
            MouseEventKind::ScrollDown => {
                dispatch_intent(&mut app, crate::tui::event::Intent::ScrollPlaylist(1), &api_client).await?;
                crate::tui::mouse::MouseTarget::None
            }
            _ => crate::tui::mouse::MouseTarget::None,
        };

        match click_tracker.classify(target, std::time::Instant::now()) {
            crate::tui::mouse::ClickKind::Single => apply_mouse_selection(&mut app, target, &api_client).await?,
            crate::tui::mouse::ClickKind::Double => {
                if let Some(intent) = app.handle_action(crate::tui::event::ActionId::Activate) {
                    dispatch_intent(&mut app, intent, &api_client).await?;
                }
            }
        }
    }
    _ => {}
}
```

Add the small runtime helpers to the same file:

```rust
async fn dispatch_intent(
    app: &mut crate::tui::app::App,
    intent: crate::tui::event::Intent,
    api_client: &crate::cli::client::ApiClient,
) -> crate::core::error::MeloResult<()> {
    match intent {
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::LoadPreview) => {
            if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                app.set_playlist_preview_loading();
                match api_client.playlist_preview(&name).await {
                    Ok(preview) => app.set_playlist_preview(&preview),
                    Err(err) => app.set_playlist_preview_error(err.to_string()),
                }
            }
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::PlaySelection) => {
            if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                let snapshot = api_client.playlist_play(&name, 0).await?;
                app.apply_tui_snapshot(snapshot);
            }
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::PlayPreviewSelection) => {
            if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                let snapshot = api_client
                    .playlist_play(&name, app.selected_preview_index())
                    .await?;
                app.apply_tui_snapshot(snapshot);
            }
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::TogglePlayback) => {
            app.apply_snapshot(api_client.post_json("/api/player/toggle").await?);
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::Next) => {
            app.apply_snapshot(api_client.post_json("/api/player/next").await?);
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::Prev) => {
            app.apply_snapshot(api_client.post_json("/api/player/prev").await?);
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::CycleRepeatMode) => {
            app.apply_snapshot(
                api_client
                    .player_mode_repeat(next_repeat_mode(&app.player.repeat_mode))
                    .await?,
            );
        }
        crate::tui::event::Intent::Action(crate::tui::event::ActionId::ToggleShuffle) => {
            app.apply_snapshot(
                api_client
                    .player_mode_shuffle(!app.player.shuffle_enabled)
                    .await?,
            );
        }
        crate::tui::event::Intent::ScrollPlaylist(delta) => app.scroll_playlist(delta),
        crate::tui::event::Intent::ScrollPreview(delta) => app.scroll_preview(delta),
        crate::tui::event::Intent::SelectPlaylist { index, .. } => {
            if let Some(next) = app.select_playlist_index(index) {
                dispatch_intent(app, next, api_client).await?;
            }
        }
        crate::tui::event::Intent::SelectPreview { index, .. } => app.select_preview_index(index),
        _ => {}
    }

    Ok(())
}

fn hit_test_mouse_target(
    layout: crate::tui::ui::layout::AppLayout,
    app: &crate::tui::app::App,
    column: u16,
    row: u16,
) -> crate::tui::mouse::MouseTarget {
    if layout.sidebar.contains((column, row))
        && let Some(index) = crate::tui::ui::playlist::playlist_index_at(
            layout.sidebar,
            row,
            app.playlist_browser.visible_playlists.len(),
        )
    {
        return crate::tui::mouse::MouseTarget::PlaylistRow(index);
    }

    if layout.content_body.contains((column, row))
        && let Some(index) = crate::tui::ui::playlist::preview_index_at(
            layout.content_body,
            row,
            app.preview_titles.len(),
        )
    {
        return crate::tui::mouse::MouseTarget::PreviewRow(index);
    }

    crate::tui::mouse::MouseTarget::None
}

async fn apply_mouse_selection(
    app: &mut crate::tui::app::App,
    target: crate::tui::mouse::MouseTarget,
    api_client: &crate::cli::client::ApiClient,
) -> crate::core::error::MeloResult<()> {
    match target {
        crate::tui::mouse::MouseTarget::PlaylistRow(index) => {
            if let Some(intent) = app.select_playlist_index(index) {
                dispatch_intent(app, intent, api_client).await?;
            }
        }
        crate::tui::mouse::MouseTarget::PreviewRow(index) => app.select_preview_index(index),
        crate::tui::mouse::MouseTarget::None => {}
    }

    Ok(())
}
```

- [ ] **Step 4: Render launch cwd and keep verbose outside the TUI lifetime**

Update the status panel in `src/tui/ui/playlist.rs`:

```rust
pub fn render_status_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = vec![
        format!(
            "当前播放来源：{}",
            app.playlist_browser
                .current_playing_playlist
                .as_ref()
                .map(|playlist| playlist.name.as_str())
                .unwrap_or("无")
        ),
        format!("repeat={}", app.player.repeat_mode),
        format!("shuffle={}", app.player.shuffle_enabled),
    ];

    if let Some(launch_cwd) = &app.launch_cwd {
        lines.push(format!("当前运行目录：{launch_cwd}"));
    }

    lines
}
```

Keep the verbose mirror scoped to the startup phase in `src/cli/run.rs`:

```rust
let (base_url, source_label, startup_notice, launch_cwd_text) = {
    let _mirror = if prepared.logging.verbose {
        Some(crate::core::logging::attach_daemon_log_mirror(
            crate::core::logging::daemon_log_path(&settings),
            resolved_cli.prefix_enabled,
            settings.logging.daemon_prefix.clone(),
        ))
    } else {
        None
    };

    let ensured =
        crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging).await?;
    let base_url = ensured.base_url;
    let launch_cwd_text = launch_cwd_text(&launch_cwd);
    let home = crate::cli::client::ApiClient::new(base_url.clone()).tui_home().await?;
    let decision = crate::cli::launch::choose_default_launch_decision(&launch_cwd, &home);
    let (source_label, startup_notice) = match decision {
        crate::cli::launch::DefaultLaunchDecision::PreserveCurrentSession { .. } => {
            (None, Some("Continuing current playback".to_string()))
        }
        crate::cli::launch::DefaultLaunchDecision::OpenLaunchCwd { launch_cwd } => {
            let opened = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(launch_cwd, "cwd_dir")
                .await?;
            (Some(opened.source_label), None)
        }
    };
    (base_url, source_label, startup_notice, launch_cwd_text)
};

crate::tui::run::start(
    base_url,
    crate::tui::run::LaunchContext {
        launch_cwd: Some(launch_cwd_text),
        source_label,
        startup_notice,
        footer_hints_enabled: settings.tui.show_footer_hints,
    },
)
```

- [ ] **Step 5: Re-run focused verification and commit the finished feature**

Run: `cargo test repeat_mode_cycles_off_all_one_off active_playback_session_keeps_current_playlist_as_default_selection verbose_default_launch_stops_terminal_log_mirror_before_tui_scope -- --nocapture`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add src/tui/run.rs src/tui/run/tests.rs tests/tui_home.rs tests/cli_remote.rs src/cli/run.rs src/tui/ui/playlist.rs
git commit -m "feat(tui): wire launch context mouse and verbose boundary"
```

## Self-Review

### Spec coverage

- 开发包装器不再把 cwd 固定到 repo root：Task 1
- 裸 `melo` 保留活动播放、空闲时打开当前运行目录：Task 1 + Task 5
- Playlist 首页补焦点、高亮、当前播放来源提示：Task 4 + Task 5
- 鼠标单击选中、双击播放：Task 3 + Task 5
- 快捷键可配置，支持单键、组合键、序列、Vim 风格别名：Task 2
- `--verbose` 在进入 TUI 后停止污染终端：Task 1 + Task 5

### Placeholder scan

- 没有 `TBD`、`TODO`、`implement later`
- 每个代码步骤都给出具体结构体、函数签名、测试代码或命令
- 每个任务都有明确验证命令和提交命令

### Type consistency

- 统一使用 `DefaultLaunchDecision`
- 统一使用 `ActionId` 作为可配置动作 id
- 统一使用 `Intent` 作为输入归一化层
- 统一使用 `Keymap` / `Resolution`
- 统一使用 `ClickTracker` / `ClickKind` / `MouseTarget`
