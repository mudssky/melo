use serde_json::json;

use crate::core::config::settings::Settings;
use crate::core::runtime_templates::{RuntimeTemplateKey, RuntimeTemplateRenderer};

#[test]
fn runtime_template_renderer_prefers_override_for_scan_messages() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start =
        Some("Start {{ source_label|basename }}".to_string());

    let rendered = RuntimeTemplateRenderer::default().render(
        &settings,
        RuntimeTemplateKey::CliScanStart,
        json!({ "source_label": "D:/Music/Aimer" }),
    );

    assert_eq!(rendered, "Start Aimer");
}

#[test]
fn runtime_template_renderer_falls_back_to_builtin_when_override_is_invalid() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start = Some("{{ source_label ".to_string());

    let rendered = RuntimeTemplateRenderer::default().render(
        &settings,
        RuntimeTemplateKey::CliScanStart,
        json!({ "source_label": "D:/Music/Aimer" }),
    );

    assert_eq!(rendered, "Scanning D:/Music/Aimer...");
}
