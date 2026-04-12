use crate::core::config::settings::Settings;
use crate::core::runtime_templates::RuntimeTemplateRenderer;

#[test]
fn render_scan_cli_lines_uses_runtime_template_overrides() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start =
        Some("Start {{ source_label|basename }}".to_string());
    settings.templates.runtime.scan.cli_handoff = Some("Into TUI".to_string());

    let renderer = RuntimeTemplateRenderer::default();
    let lines = super::render_scan_cli_lines(&renderer, &settings, "D:/Music/Aimer");

    assert_eq!(
        lines,
        vec!["Start Aimer".to_string(), "Into TUI".to_string()]
    );
}

#[test]
fn launch_cwd_text_preserves_runtime_directory() {
    let text = super::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}
