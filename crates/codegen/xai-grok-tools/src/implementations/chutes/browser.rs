//! Stateful local Chrome/Edge automation over the Chrome DevTools Protocol.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt as _;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::types::output::{DynamicOutput, ToolOutput};
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

type CdpSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate,
    Snapshot,
    Click,
    Type,
    Screenshot,
    Close,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BrowserInput {
    /// Browser operation. Sessions are reused across calls within the current agent session.
    pub action: BrowserAction,
    /// HTTP(S) URL for `navigate`.
    pub url: Option<String>,
    /// CSS selector for `click` or `type`.
    pub selector: Option<String>,
    /// Text for `type`.
    pub text: Option<String>,
    /// Submit the nearest form after typing.
    pub submit: Option<bool>,
    /// Workspace-relative PNG path for `screenshot`.
    pub path: Option<String>,
}

#[derive(Clone)]
pub struct BrowserClient {
    state: Arc<Mutex<Option<BrowserSession>>>,
    cwd: PathBuf,
}

struct BrowserSession {
    child: Child,
    _profile: tempfile::TempDir,
    socket: CdpSocket,
    next_id: u64,
}

impl BrowserClient {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            cwd,
        }
    }

    async fn execute(&self, input: BrowserInput) -> Result<serde_json::Value, String> {
        if input.action == BrowserAction::Close {
            let mut guard = self.state.lock().await;
            if let Some(mut session) = guard.take() {
                let _ = session.child.kill().await;
            }
            return Ok(serde_json::json!({"closed": true}));
        }

        let mut guard = self.state.lock().await;
        if guard.is_none() {
            *guard = Some(BrowserSession::launch().await?);
        }
        let session = guard.as_mut().expect("browser session initialized");
        // Wrapped in an async block so `?` inside each arm resolves this
        // block's Result instead of returning from `execute` directly --
        // that lets us inspect the outcome below and evict a session that
        // died mid-command instead of leaving a dead connection in `guard`
        // for every subsequent call to fail against.
        let result: Result<serde_json::Value, String> = async {
            match input.action {
            BrowserAction::Navigate => {
                let url = validate_navigation_url(required(input.url, "url")?)?;
                session
                    .command("Page.navigate", serde_json::json!({"url": url}))
                    .await?;
                session.wait_until_ready().await?;
                session.snapshot().await
            }
            BrowserAction::Snapshot => session.snapshot().await,
            BrowserAction::Click => {
                let selector = required(input.selector, "selector")?;
                let script = format!(
                    "(() => {{ const el = document.querySelector({}); if (!el) return {{ok:false,error:'selector not found'}}; el.scrollIntoView({{block:'center'}}); el.click(); return {{ok:true,tag:el.tagName}}; }})()",
                    serde_json::to_string(&selector).map_err(|error| error.to_string())?
                );
                let result = session.evaluate(&script).await?;
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(result)
            }
            BrowserAction::Type => {
                let selector = required(input.selector, "selector")?;
                let text = required(input.text, "text")?;
                let script = format!(
                    "(() => {{ const el = document.querySelector({selector}); if (!el) return {{ok:false,error:'selector not found'}}; el.focus(); const value={text}; const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype; const setter=Object.getOwnPropertyDescriptor(proto,'value')?.set; if (setter) setter.call(el,value); else el.value=value; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); if ({submit}) el.form?.requestSubmit(); return {{ok:true,tag:el.tagName}}; }})()",
                    selector =
                        serde_json::to_string(&selector).map_err(|error| error.to_string())?,
                    text = serde_json::to_string(&text).map_err(|error| error.to_string())?,
                    submit = input.submit.unwrap_or(false),
                );
                let result = session.evaluate(&script).await?;
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(result)
            }
            BrowserAction::Screenshot => {
                let relative = input.path.unwrap_or_else(|| {
                    format!(
                        ".chutes-build/browser/{}.png",
                        chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f")
                    )
                });
                let output = workspace_output_path(&self.cwd, &relative)?;
                let result = session
                    .command(
                        "Page.captureScreenshot",
                        serde_json::json!({"format": "png", "captureBeyondViewport": false}),
                    )
                    .await?;
                let encoded = result
                    .get("data")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "Chrome did not return screenshot data".to_owned())?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|error| format!("Invalid screenshot data: {error}"))?;
                if let Some(parent) = output.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| {
                        format!("Failed to create screenshot directory: {error}")
                    })?;
                }
                let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)
                    .await
                    .map_err(|error| format!("Failed to create screenshot: {error}"))?;
                file.write_all(&bytes)
                    .await
                    .map_err(|error| format!("Failed to write screenshot: {error}"))?;
                file.flush()
                    .await
                    .map_err(|error| format!("Failed to flush screenshot: {error}"))?;
                Ok(serde_json::json!({"path": output, "format": "png"}))
            }
            BrowserAction::Close => unreachable!(),
            }
        }
        .await;

        if let Err(ref message) = result
            && is_connection_error(message)
            && let Some(mut dead_session) = guard.take()
        {
            let _ = dead_session.child.kill().await;
        }

        result
    }
}

/// Whether `message` (an `execute()`/`BrowserSession::command()` error
/// string) indicates the CDP WebSocket transport itself died, as opposed to
/// a command-level or validation failure that leaves the connection usable
/// for the next call.
fn is_connection_error(message: &str) -> bool {
    message.starts_with("Failed to send browser command:")
        || message.starts_with("Browser connection failed:")
        || message == "Browser connection closed"
        || message == "Browser connection ended before the command completed"
}

impl BrowserSession {
    async fn launch() -> Result<Self, String> {
        let executable = find_browser_executable().ok_or_else(|| {
            "Chrome or Edge was not found. Set CHUTES_BROWSER_EXECUTABLE to the browser path."
                .to_owned()
        })?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("Failed to reserve browser port: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("Failed to read browser port: {error}"))?
            .port();
        drop(listener);

        let profile = tempfile::tempdir()
            .map_err(|error| format!("Failed to create isolated browser profile: {error}"))?;
        let mut command = Command::new(executable);
        command
            .arg(format!("--remote-debugging-port={port}"))
            .arg("--remote-debugging-address=127.0.0.1")
            .arg(format!("--remote-allow-origins=http://127.0.0.1:{port}"))
            .arg(format!("--user-data-dir={}", profile.path().display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-sync")
            .arg("--metrics-recording-only")
            .arg("--disable-breakpad")
            .arg("--disable-features=OptimizationHints,MediaRouter")
            .arg("about:blank")
            .kill_on_drop(true);
        if !env_flag("CHUTES_BROWSER_HEADFUL") {
            command.arg("--headless=new");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.as_std_mut().creation_flags(0x0800_0000);
        }
        let child = command
            .spawn()
            .map_err(|error| format!("Failed to start browser: {error}"))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| format!("Failed to build local browser client: {error}"))?;
        let base = format!("http://127.0.0.1:{port}");
        let mut target = None;
        for _ in 0..100 {
            if let Ok(response) = http
                .put(format!("{base}/json/new?about%3Ablank"))
                .send()
                .await
                && response.status().is_success()
                && let Ok(value) = response.json::<serde_json::Value>().await
            {
                target = Some(value);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let target =
            target.ok_or_else(|| "Browser DevTools endpoint did not become ready".to_owned())?;
        let websocket_url = target
            .get("webSocketDebuggerUrl")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Browser target did not expose a DevTools WebSocket".to_owned())?;
        let (socket, _) = connect_async(websocket_url)
            .await
            .map_err(|error| format!("Failed to connect to browser DevTools: {error}"))?;
        let mut session = Self {
            child,
            _profile: profile,
            socket,
            next_id: 0,
        };
        session
            .command("Page.enable", serde_json::json!({}))
            .await?;
        session
            .command("Runtime.enable", serde_json::json!({}))
            .await?;
        Ok(session)
    }

    async fn command(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let request = serde_json::json!({"id": id, "method": method, "params": params});
        self.socket
            .send(Message::Text(request.to_string().into()))
            .await
            .map_err(|error| format!("Failed to send browser command: {error}"))?;
        while let Some(message) = self.socket.next().await {
            let message = message.map_err(|error| format!("Browser connection failed: {error}"))?;
            let Message::Text(text) = message else {
                if matches!(message, Message::Close(_)) {
                    return Err("Browser connection closed".to_owned());
                }
                continue;
            };
            let value: serde_json::Value = serde_json::from_str(text.as_ref())
                .map_err(|error| format!("Invalid browser response: {error}"))?;
            if value.get("id").and_then(serde_json::Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("Browser command {method} failed: {error}"));
            }
            return Ok(value.get("result").cloned().unwrap_or_default());
        }
        Err("Browser connection ended before the command completed".to_owned())
    }

    async fn evaluate(&mut self, expression: &str) -> Result<serde_json::Value, String> {
        let result = self
            .command(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("Browser script failed: {exception}"));
        }
        Ok(result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    async fn wait_until_ready(&mut self) -> Result<(), String> {
        for _ in 0..100 {
            let state = self.evaluate("document.readyState").await?;
            if matches!(state.as_str(), Some("interactive" | "complete")) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("Timed out waiting for the page to become ready".to_owned())
    }

    async fn snapshot(&mut self) -> Result<serde_json::Value, String> {
        self.evaluate(
            r#"(() => {
                const visible = el => { const s=getComputedStyle(el), r=el.getBoundingClientRect(); return s.display!=='none' && s.visibility!=='hidden' && r.width>0 && r.height>0; };
                const nodes=[...document.querySelectorAll('a,button,input,textarea,select,[role],h1,h2,h3,p,li')].filter(visible).slice(0,300);
                return {url:location.href,title:document.title,elements:nodes.map((el,index)=>{const password=el instanceof HTMLInputElement&&el.type.toLowerCase()==='password';return {index,tag:el.tagName.toLowerCase(),role:el.getAttribute('role'),text:password?'[redacted]':(el.innerText||el.value||el.getAttribute('aria-label')||'').trim().slice(0,500),selector:el.id?'#'+CSS.escape(el.id):null,href:el.href||null,type:el.type||null};})};
            })()"#,
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct BrowserTool;

impl crate::types::tool_metadata::ToolMetadata for BrowserTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Control an isolated local Chrome or Edge session: navigate, inspect a structured page snapshot, click, type, submit forms, capture workspace screenshots, or close the session. Browser actions can affect external sites; inspect before mutating."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for BrowserTool {
    type Args = BrowserInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("browser").expect("valid tool id")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "browser",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            max_concurrency: Some(1),
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            timeout_ms: Some(60_000),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: BrowserInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;
        let client = {
            let resources = resources.lock().await;
            resources.require::<BrowserClient>()?.clone()
        };
        let output = client.execute(input).await.map_err(|error| {
            xai_tool_runtime::ToolError::execution(
                xai_tool_protocol::ToolId::new("browser").expect("valid tool id"),
                error,
            )
        })?;
        Ok(ToolOutput::Dynamic(DynamicOutput::from(output)))
    }
}

fn required(value: Option<String>, field: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("'{field}' is required for this browser action"))
}

fn validate_navigation_url(raw: String) -> Result<String, String> {
    let url = reqwest::Url::parse(&raw).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Browser navigation supports only HTTP and HTTPS URLs".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Credentials embedded in browser URLs are not allowed".to_owned());
    }
    Ok(url.to_string())
}

fn workspace_output_path(cwd: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Screenshot path must stay inside the current workspace".to_owned());
    }
    let canonical_cwd = dunce::canonicalize(cwd)
        .map_err(|error| format!("Failed to resolve the current workspace: {error}"))?;
    let output = canonical_cwd.join(path);
    if output.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err("Screenshot path must use a .png extension".to_owned());
    }
    let mut existing_ancestor = output.as_path();
    while !existing_ancestor.exists() {
        existing_ancestor = existing_ancestor
            .parent()
            .ok_or_else(|| "Screenshot path has no existing workspace ancestor".to_owned())?;
    }
    let canonical_ancestor = dunce::canonicalize(existing_ancestor)
        .map_err(|error| format!("Failed to resolve screenshot directory: {error}"))?;
    if !canonical_ancestor.starts_with(&canonical_cwd) {
        return Err("Screenshot path must stay inside the current workspace".to_owned());
    }
    Ok(output)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn find_browser_executable() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CHUTES_BROWSER_EXECUTABLE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    for name in [
        "chrome",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "msedge",
    ] {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }
    #[cfg(windows)]
    {
        let roots = [
            std::env::var_os("PROGRAMFILES"),
            std::env::var_os("PROGRAMFILES(X86)"),
            std::env::var_os("LOCALAPPDATA"),
        ];
        for root in roots.into_iter().flatten() {
            for suffix in [
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
            ] {
                let path = PathBuf::from(&root).join(suffix);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    for path in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ] {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_paths_cannot_escape_workspace() {
        let cwd = tempfile::tempdir().unwrap();
        assert!(workspace_output_path(cwd.path(), "shots/page.png").is_ok());
        assert!(workspace_output_path(cwd.path(), "../outside.png").is_err());
        assert!(workspace_output_path(cwd.path(), "shots/page.jpg").is_err());
    }

    #[test]
    fn navigation_rejects_credentials_and_non_http_protocols() {
        assert!(validate_navigation_url("https://example.com".into()).is_ok());
        assert!(validate_navigation_url("file:///etc/passwd".into()).is_err());
        assert!(validate_navigation_url("https://user:pass@example.com".into()).is_err());
    }
}
