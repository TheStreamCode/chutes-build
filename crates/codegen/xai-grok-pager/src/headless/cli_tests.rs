use super::*;

#[test]
fn blank_text_blocks_are_an_empty_prompt() {
    let err = HeadlessPrompt::from_args(None, Some(r#"[{"type": "text", "text": "   "}]"#), None)
        .expect_err("all-blank text blocks must not prompt");
    assert!(
        err.to_string().contains("prompt is empty"),
        "unexpected error: {err}"
    );
}

#[test]
fn non_text_blocks_make_a_prompt_without_words() {
    let prompt = HeadlessPrompt::from_args(
        None,
        Some(r#"[{"type": "image", "data": "aGk=", "mimeType": "image/png"}]"#),
        None,
    )
    .expect("image-only prompt must parse");
    assert!(matches!(prompt, Some(HeadlessPrompt::Blocks(_))));

    let prompt = HeadlessPrompt::from_args(
        None,
        Some(
            r#"[{"type": "text", "text": "  "}, {"type": "image", "data": "aGk=", "mimeType": "image/png"}]"#,
        ),
        None,
    )
    .expect("blank caption next to an image must parse");
    assert!(matches!(prompt, Some(HeadlessPrompt::Blocks(_))));
}
