#[test]
fn detect_cover_protocol_prefers_kitty_then_iterm_then_sixel() {
    let kitty =
        super::detect_cover_protocol_from_env(&[("TERM".to_string(), "xterm-kitty".to_string())]);
    assert_eq!(kitty, super::CoverProtocol::Kitty);

    let iterm = super::detect_cover_protocol_from_env(&[(
        "TERM_PROGRAM".to_string(),
        "iTerm.app".to_string(),
    )]);
    assert_eq!(iterm, super::CoverProtocol::Iterm2);
}

#[test]
fn unsupported_terminal_returns_text_fallback() {
    let protocol = super::detect_cover_protocol_from_env(&[]);
    assert_eq!(protocol, super::CoverProtocol::Unsupported);
}
