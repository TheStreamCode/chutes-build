use crate::session::user_message::user_query;
use agent_client_protocol::{self as acp, ImageContent};
use base64::Engine as _;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use xai_grok_workspace::file_system::{
    FileReference, render_embedded_resource, render_file_reference,
};
/// Parsed prompt with context and query kept separate.
///
/// Some templates put `<user_query>` last (context first); Grok puts it first.
/// Keeping them separate lets the caller truncate context without
/// searching for the query boundary in a flat string.
#[derive(Debug, Clone)]
pub struct ParsedPrompt {
    /// Context blocks: `<attached_files>` payloads and resource-link sections.
    /// Grok mode may include editor open/focus metadata; the compat mode does not.
    /// Empty string when there is no context.
    pub context: String,
    /// The user's query, already wrapped in `<user_query>` tags
    /// (or raw when verbatim).
    pub query: String,
    /// Skill information block: `<skill_information>` envelope with expanded
    /// skill content. Empty string when no skills were invoked.
    pub skill_information: String,
    /// Extracted images from the prompt.
    pub images: Vec<ImageContent>,
    /// Whether the prompt was parsed in query-last mode.
    pub is_cursor: bool,
}
impl ParsedPrompt {
    /// Assemble into the final message string with correct ordering.
    pub fn assemble(&self) -> String {
        Self::assemble_parts_with_skills(
            &self.context,
            &self.query,
            &self.skill_information,
            self.is_cursor,
        )
    }
    /// Assemble context and query into the final message string.
    ///
    /// Legacy entry point — delegates to [`assemble_parts_with_skills`] with
    /// no skill information.
    pub fn assemble_parts(context: &str, query: &str, is_cursor: bool) -> String {
        Self::assemble_parts_with_skills(context, query, "", is_cursor)
    }
    /// Assemble context, query, and skill information into the final message string.
    ///
    /// Layout:
    /// - **Grok mode:** `<user_query>` + `<skill_information>` + context
    /// - **Query-last mode:** context + `<user_query>` + `<skill_information>`
    ///
    /// The `<skill_information>` block always follows `<user_query>` immediately
    /// so the model sees the user's request and skill instructions together.
    pub fn assemble_parts_with_skills(
        context: &str,
        query: &str,
        skill_information: &str,
        is_cursor: bool,
    ) -> String {
        let query_block = if skill_information.is_empty() {
            query.to_string()
        } else {
            format!("{query}\n{skill_information}")
        };
        if context.is_empty() {
            return query_block;
        }
        let _ = is_cursor;
        format!("{query_block}\n\n{context}")
    }
}
/// Parses ACP prompt content blocks into a [`ParsedPrompt`] with context
/// and query kept separate.
///
/// When `is_cursor` is true, produces query-last format output:
/// - `<attached_files>` (bare), resource links, then `<user_query>` last
/// - File references use `<code_selection>` tags
///
/// When `is_cursor` is false, produces original Grok-format output:
/// - `<user_query>` first, then `<system-reminder>` wrapped `<attached_files>` and resource links
/// - File references use `<file_contents>` tags
pub async fn parse_prompt(
    prompt: &[acp::ContentBlock],
    working_directory: PathBuf,
    _session_info: &crate::session::info::Info,
    verbatim: bool,
    is_cursor: bool,
) -> Result<ParsedPrompt, acp::Error> {
    parse_prompt_with_skills(
        prompt,
        working_directory,
        _session_info,
        verbatim,
        is_cursor,
        String::new(),
    )
    .await
}
/// Parse prompt with optional pre-built skill information block.
///
/// This is the full-featured entry point. `parse_prompt` delegates here with
/// an empty `skill_information` string for backward compatibility.
pub async fn parse_prompt_with_skills(
    prompt: &[acp::ContentBlock],
    working_directory: PathBuf,
    _session_info: &crate::session::info::Info,
    verbatim: bool,
    is_cursor: bool,
    skill_information: String,
) -> Result<ParsedPrompt, acp::Error> {
    let mut message_parts: Vec<String> = Vec::new();
    let mut image_parts = Vec::new();
    let mut resource_links = Vec::new();
    let mut embedded_resources = Vec::new();
    for block in prompt {
        match block {
            acp::ContentBlock::Text(text) => message_parts.push(text.text.clone()),
            acp::ContentBlock::Image(image_content) => image_parts.push(image_content.clone()),
            acp::ContentBlock::ResourceLink(link) => {
                let video_resource = is_video_resource(link);
                if video_resource {
                    let path =
                        resolve_local_resource_path(link, &working_directory).map_err(|e| {
                            acp::Error::invalid_params().data(format!(
                                "video attachment `{}` could not be resolved: {e}",
                                link.name
                            ))
                        })?;
                    let frames = extract_video_frames(&path).await.map_err(|e| {
                        acp::Error::invalid_params().data(format!(
                            "video attachment `{}` could not be inspected: {e}",
                            link.name
                        ))
                    })?;
                    message_parts.push(format!(
                        "[Video attachment: {}. {} representative frames were extracted locally for visual analysis.]",
                        link.name,
                        frames.len()
                    ));
                    image_parts.extend(frames);
                }
                resource_links.push(link.clone());
                if link.meta.is_none() && !video_resource {
                    let path = extract_path_from_uri(link);
                    message_parts.push(format!("@{path}"));
                }
            }
            acp::ContentBlock::Resource(resource) => embedded_resources.push(resource.clone()),
            other => {
                return Err(acp::Error::invalid_params()
                    .data(format!("unsupported content block in prompt: {other:?}")));
            }
        }
    }
    let message = message_parts.join(" ");
    let file_ref_tokens = collect_file_references(&message);
    let mut file_ref_contents = Vec::new();
    for token in file_ref_tokens {
        let Some(mut file_ref) = FileReference::parse(&token) else {
            continue;
        };
        file_ref.path = working_directory.join(&file_ref.path);
        let rendered_file = render_file_reference(file_ref, is_cursor).await;
        let success = rendered_file.is_some();
        tracing::info_span!("at_mention", mention_type = "file", success).in_scope(|| {});
        if let Some(rendered_file) = rendered_file {
            file_ref_contents.push(rendered_file);
        }
    }
    let mut embedded_contents = Vec::new();
    for resource in &embedded_resources {
        if let Some(rendered) = render_embedded_resource(resource, is_cursor).await {
            embedded_contents.push(rendered);
        }
    }
    let parsed = render_message(
        message,
        embedded_contents,
        file_ref_contents,
        &resource_links,
        verbatim,
        is_cursor,
    );
    Ok(ParsedPrompt {
        context: parsed.0,
        query: parsed.1,
        skill_information,
        images: image_parts,
        is_cursor,
    })
}

fn is_video_resource(link: &acp::ResourceLink) -> bool {
    link.mime_type
        .as_deref()
        .is_some_and(|mime| mime.to_ascii_lowercase().starts_with("video/"))
        || Path::new(&link.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "mp4" | "mov" | "mkv" | "webm" | "m4v" | "avi"
                )
            })
}

fn resolve_local_resource_path(
    link: &acp::ResourceLink,
    working_directory: &Path,
) -> Result<PathBuf, String> {
    let path = if link.uri.starts_with("file:") {
        url::Url::parse(&link.uri)
            .map_err(|error| error.to_string())?
            .to_file_path()
            .map_err(|_| "invalid local file URI".to_owned())?
    } else {
        let named = PathBuf::from(&link.name);
        if named.is_absolute() {
            named
        } else {
            working_directory.join(named)
        }
    };
    let canonical_root = dunce::canonicalize(working_directory).map_err(|e| e.to_string())?;
    let canonical_path = dunce::canonicalize(&path).map_err(|e| e.to_string())?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err("video attachments must stay inside the active workspace".to_owned());
    }
    if !canonical_path.is_file() {
        return Err("video attachment is not a file".to_owned());
    }
    Ok(canonical_path)
}

async fn extract_video_frames(path: &Path) -> Result<Vec<ImageContent>, String> {
    const MAX_FRAMES: usize = 8;
    let temp_dir = std::env::temp_dir().join(format!(
        "chutes-build-video-frames-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|error| error.to_string())?;

    let ffmpeg = std::env::var_os("CHUTES_FFMPEG_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let duration = probe_video_duration(path, &ffmpeg).await.unwrap_or(80.0);
    let interval = (duration / (MAX_FRAMES as f64 + 1.0)).max(0.5);
    let output_pattern = temp_dir.join("frame-%03d.jpg");
    let filter = format!("fps=1/{interval:.3},scale=1280:-2:force_original_aspect_ratio=decrease");
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        tokio::process::Command::new(&ffmpeg)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(path)
            .arg("-vf")
            .arg(filter)
            .arg("-frames:v")
            .arg(MAX_FRAMES.to_string())
            .arg("-q:v")
            .arg("3")
            .arg("-y")
            .arg(&output_pattern)
            .output(),
    )
    .await;

    let result = match output {
        Ok(Ok(output)) if output.status.success() => {
            let mut paths = Vec::new();
            let mut entries = tokio::fs::read_dir(&temp_dir)
                .await
                .map_err(|error| error.to_string())?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|error| error.to_string())?
            {
                if entry.path().extension().and_then(|ext| ext.to_str()) == Some("jpg") {
                    paths.push(entry.path());
                }
            }
            paths.sort();
            let mut frames = Vec::with_capacity(paths.len());
            for frame in paths {
                let bytes = tokio::fs::read(frame)
                    .await
                    .map_err(|error| error.to_string())?;
                frames.push(ImageContent::new(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                    "image/jpeg",
                ));
            }
            if frames.is_empty() {
                Err("FFmpeg produced no readable video frames".to_owned())
            } else {
                Ok(frames)
            }
        }
        Ok(Ok(output)) => Err(format!(
            "FFmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(1000)
                .collect::<String>()
        )),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("FFmpeg was not found; install it or set CHUTES_FFMPEG_EXECUTABLE".to_owned())
        }
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("FFmpeg timed out after 120 seconds".to_owned()),
    };
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

async fn probe_video_duration(path: &Path, ffmpeg: &Path) -> Option<f64> {
    let ffprobe = if ffmpeg.is_absolute() {
        ffmpeg.with_file_name(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        })
    } else {
        PathBuf::from("ffprobe")
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        tokio::process::Command::new(ffprobe)
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(path)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration > 0.0)
}
/// Returns `(context, query)` — the two halves of the prompt kept separate
/// so the caller can truncate context without searching for the query boundary.
fn render_message(
    message: String,
    embedded_contents: Vec<String>,
    file_ref_contents: Vec<String>,
    resource_links: &[acp::ResourceLink],
    verbatim: bool,
    is_cursor: bool,
) -> (String, String) {
    let all_attached_contents: Vec<String> = embedded_contents
        .into_iter()
        .chain(file_ref_contents)
        .collect();
    let wrap = |msg: String| -> String { if verbatim { msg } else { user_query(msg) } };
    let query = wrap(message);
    let _ = is_cursor;
    let mut context = String::new();
    if !all_attached_contents.is_empty() {
        context.push_str(&format!(
            r#"<system-reminder>
Below are some potentially helpful/relevant pieces of information for figuring out how to respond

<attached_files>

{}

</attached_files>

</system-reminder>"#,
            all_attached_contents.join("\n\n"),
        ));
    }
    if !resource_links.is_empty() {
        if !context.is_empty() {
            context.push_str("\n\n");
        }
        context.push_str(&render_resource_links_grok(resource_links));
    }
    (context, query)
}
fn collect_file_references(message: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut i = 0;
    while i < message.len() {
        if !message.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let Some(at_symbol_offset) = message[i..].find('@') else {
            break;
        };
        let start = i + at_symbol_offset + 1;
        if start >= message.len() || !message.is_char_boundary(start) {
            break;
        }
        let rest = &message[start..];
        let token = rest.split_whitespace().next().unwrap_or("");
        if !token.is_empty() {
            paths.push(token.to_string());
        }
        i = start + token.len().max(1);
    }
    paths
}
#[derive(Debug, Deserialize)]
struct CursorPosition {
    line: u64,
    column: u64,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "fileState")]
enum FileState {
    /// The file is currently visible in the editor
    #[serde(rename = "focused")]
    Focused { cursor: CursorPosition },
    /// The file is open in a tab but not currently visible
    #[serde(rename = "open")]
    Open,
}
#[derive(Debug, Deserialize)]
struct EditorMeta {
    source: String,
    #[serde(flatten)]
    file_state: FileState,
}
fn parse_editor_meta(link: &acp::ResourceLink) -> Option<EditorMeta> {
    let meta_value = link.meta.as_ref()?;
    let editor_meta: EditorMeta =
        serde_json::from_value(serde_json::Value::Object(meta_value.clone())).ok()?;
    if editor_meta.source != "editor" {
        return None;
    }
    Some(editor_meta)
}
fn extract_path_from_uri(link: &acp::ResourceLink) -> String {
    if let Some(path) = link.uri.strip_prefix("file://") {
        path.to_string()
    } else {
        link.name.clone()
    }
}
fn render_regular_links(links: &[&acp::ResourceLink]) -> String {
    let mut s =
        String::from("Below is data for the files mentioned by the user\nReferenced resources:\n");
    for (idx, link) in links.iter().enumerate() {
        let label = link
            .title
            .as_deref()
            .or(link.description.as_deref())
            .unwrap_or(&link.name);
        if let Some(size) = link.size {
            s.push_str(&format!("{idx}. {label} -> {} (~{size} bytes)\n", link.uri));
        } else {
            s.push_str(&format!("{idx}. {label} -> {}\n", link.uri));
        }
    }
    s.trim_end_matches('\n').to_string()
}
/// Grok-format resource links: `<focused_files>` / `<open_files>` with
/// metadata inside a `<system-reminder>` wrapper.
fn render_resource_links_grok(resource_links: &[acp::ResourceLink]) -> String {
    let mut regular_links = Vec::new();
    let mut focused_files = Vec::new();
    let mut open_files = Vec::new();
    for link in resource_links {
        match parse_editor_meta(link) {
            Some(EditorMeta {
                file_state: FileState::Focused { cursor },
                ..
            }) => {
                focused_files.push((extract_path_from_uri(link), cursor));
            }
            Some(EditorMeta {
                file_state: FileState::Open,
                ..
            }) => {
                open_files.push(extract_path_from_uri(link));
            }
            None => {
                regular_links.push(link);
            }
        }
    }
    let mut sections: Vec<String> = Vec::new();
    if !regular_links.is_empty() {
        sections.push(render_regular_links(&regular_links));
    }
    if !focused_files.is_empty() {
        sections.push(render_focused_files(&focused_files));
    }
    if !open_files.is_empty() {
        sections.push(render_open_files(&open_files));
    }
    format!(
        "<system-reminder>\n{}\n</system-reminder>",
        sections.join("\n\n")
    )
}
fn render_focused_files(files: &[(String, CursorPosition)]) -> String {
    let mut s = String::from(
        "Below is data for the file(s) the user is currently actively looking at while making their query\n<focused_files>\n",
    );
    for (path, cursor) in files {
        s.push_str(&format!(
            "<file path=\"{path}\" cursor_line=\"{}\" cursor_column=\"{}\"/>\n",
            cursor.line, cursor.column
        ));
    }
    s.push_str("</focused_files>");
    s
}
fn render_open_files(paths: &[String]) -> String {
    let mut s = String::from(
        "Below is data for the file(s) the user has previously opened but are not currently visible to the user\n<open_files>\n",
    );
    for path in paths {
        s.push_str(&format!("<file path=\"{path}\"/>\n"));
    }
    s.push_str("</open_files>");
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Assemble a `render_message` result into a flat string for test assertions.
    fn assemble(parts: (String, String), is_cursor: bool) -> String {
        let (context, query) = parts;
        if context.is_empty() {
            return query;
        }
        if is_cursor {
            format!("{context}\n\n{query}")
        } else {
            format!("{query}\n\n{context}")
        }
    }
    /// Shorthand: render + assemble for grok mode.
    fn render_grok(
        message: &str,
        embedded: Vec<String>,
        file_refs: Vec<String>,
        links: &[acp::ResourceLink],
        verbatim: bool,
    ) -> String {
        assemble(
            render_message(message.into(), embedded, file_refs, links, verbatim, false),
            false,
        )
    }
    #[test]
    fn test_collect_single_reference() {
        let tokens = collect_file_references("look at @src/main.rs please");
        assert_eq!(tokens, vec!["src/main.rs"]);
    }
    #[test]
    fn test_collect_multiple_references() {
        let tokens = collect_file_references("check @foo.rs and @bar/baz.rs");
        assert_eq!(tokens, vec!["foo.rs", "bar/baz.rs"]);
    }
    #[test]
    fn test_collect_reference_with_line_range() {
        let tokens = collect_file_references("see @lib.rs:10-20 for details");
        assert_eq!(tokens, vec!["lib.rs:10-20"]);
    }
    #[test]
    fn test_collect_no_references() {
        let tokens = collect_file_references("no file references here");
        assert!(tokens.is_empty());
    }
    #[test]
    fn test_collect_at_end_of_message() {
        let tokens = collect_file_references("check @README.md");
        assert_eq!(tokens, vec!["README.md"]);
    }
    #[test]
    fn test_collect_trailing_at_ignored() {
        let tokens = collect_file_references("trailing @");
        assert!(tokens.is_empty());
    }
    #[test]
    fn test_collect_at_with_space() {
        let tokens = collect_file_references("email me @ work");
        assert_eq!(tokens, vec!["work"]);
    }
    #[test]
    fn test_collect_adjacent_references() {
        let tokens = collect_file_references("@a.rs @b.rs");
        assert_eq!(tokens, vec!["a.rs", "b.rs"]);
    }
    fn make_link(meta: Option<serde_json::Value>) -> acp::ResourceLink {
        let mut link = acp::ResourceLink::new("test.rs", "file:///project/test.rs");
        if let Some(m) = meta.and_then(|v| v.as_object().cloned()) {
            link = link.meta(m);
        }
        link
    }
    #[test]
    fn detects_video_resources_by_mime_or_extension() {
        let mime = acp::ResourceLink::new("clip.bin", "file:///clip.bin")
            .mime_type(Some("video/mp4".to_owned()));
        let extension = acp::ResourceLink::new("clip.webm", "file:///clip.webm");
        let image = acp::ResourceLink::new("still.png", "file:///still.png")
            .mime_type(Some("image/png".to_owned()));
        assert!(is_video_resource(&mime));
        assert!(is_video_resource(&extension));
        assert!(!is_video_resource(&image));
    }
    #[tokio::test]
    async fn extracts_representative_video_frames_when_ffmpeg_is_available() {
        let temp = tempfile::tempdir().unwrap();
        let video = temp.path().join("sample.mp4");
        let status = tokio::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg("color=c=blue:s=320x240:d=2")
            .args(["-pix_fmt", "yuv420p", "-y"])
            .arg(&video)
            .status()
            .await;
        let status = match status {
            Ok(status) => status,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("failed to start FFmpeg: {error}"),
        };
        assert!(status.success());

        let frames = extract_video_frames(&video).await.unwrap();
        assert!(!frames.is_empty());
        assert!(frames.len() <= 8);
        assert!(frames.iter().all(|frame| frame.mime_type == "image/jpeg"));
    }
    #[test]
    fn test_parse_editor_meta_focused_with_cursor() {
        let link = make_link(Some(serde_json::json!(
            { "source" : "editor", "fileState" : "focused", "cursor" : { "line" :
            10, "column" : 3 } }
        )));
        let meta = parse_editor_meta(&link).expect("should parse");
        assert!(matches!(
            meta.file_state,
            FileState::Focused {
                cursor: CursorPosition {
                    line: 10,
                    column: 3
                }
            }
        ));
    }
    #[test]
    fn test_parse_editor_meta_focused_without_cursor_fails() {
        let link = make_link(Some(
            serde_json::json!({ "source" : "editor", "fileState" : "focused" }),
        ));
        assert!(parse_editor_meta(&link).is_none());
    }
    #[test]
    fn test_parse_editor_meta_open() {
        let link = make_link(Some(
            serde_json::json!({ "source" : "editor", "fileState" : "open" }),
        ));
        let meta = parse_editor_meta(&link).expect("should parse");
        assert!(matches!(meta.file_state, FileState::Open));
    }
    #[test]
    fn test_parse_editor_meta_non_editor_source_returns_none() {
        let link = make_link(Some(serde_json::json!(
            { "source" : "something_else", "fileState" : "focused" }
        )));
        assert!(parse_editor_meta(&link).is_none());
    }
    #[test]
    fn test_parse_editor_meta_no_meta_returns_none() {
        let link = make_link(None);
        assert!(parse_editor_meta(&link).is_none());
    }
    #[test]
    fn test_parse_editor_meta_unknown_file_state_returns_none() {
        let link = make_link(Some(
            serde_json::json!({ "source" : "editor", "fileState" : "minimized" }),
        ));
        assert!(parse_editor_meta(&link).is_none());
    }
    #[test]
    fn test_grok_render_plain_message() {
        let result = render_grok("hello", vec![], vec![], &[], false);
        assert_eq!(result, "<user_query>\nhello\n</user_query>");
    }
    #[test]
    fn test_grok_render_with_attachments_uses_system_reminder_wrapper() {
        let result = render_grok(
            "check this",
            vec!["embedded content".into()],
            vec![],
            &[],
            false,
        );
        assert!(
            result.contains("<system-reminder>"),
            "expected system-reminder wrapper, got: {result}"
        );
        assert!(result.contains("<attached_files>"));
        assert!(result.contains("embedded content"));
        assert!(
            result.starts_with("<user_query>"),
            "Grok should start with <user_query>, got: {result}"
        );
    }
    #[test]
    fn test_grok_render_user_query_first() {
        let link = acp::ResourceLink::new("doc.md", "file:///doc.md")
            .title(Some("My Doc".into()))
            .size(Some(1024));
        let result = render_grok("hello", vec![], vec![], &[link], false);
        let uq_pos = result.find("<user_query>").unwrap();
        let rr_pos = result.find("Referenced resources:").unwrap();
        assert!(
            uq_pos < rr_pos,
            "Grok: <user_query> ({uq_pos}) should come before resource links ({rr_pos})\ngot: {result}"
        );
        assert!(result.contains("<system-reminder>"));
    }
    #[test]
    fn test_grok_render_resource_links_use_focused_files_format() {
        let links = vec![
            acp::ResourceLink::new("main.rs", "file:///project/src/main.rs").meta(
                serde_json::json!({ "source" : "editor", "fileState" : "focused",
            "cursor" : { "line" : 42, "column" : 10 } })
                .as_object()
                .cloned(),
            ),
            acp::ResourceLink::new("Cargo.toml", "file:///project/Cargo.toml").meta(
                serde_json::json!({ "source" : "editor", "fileState" : "open" })
                    .as_object()
                    .cloned(),
            ),
        ];
        let result = render_grok("hello", vec![], vec![], &links, false);
        assert!(result.contains("<system-reminder>"), "got: {result}");
        assert!(result.contains("<focused_files>"), "got: {result}");
        assert!(result.contains("<open_files>"), "got: {result}");
        assert!(
            !result.contains("<open_and_recently_viewed_files>"),
            "got: {result}"
        );
    }
}
