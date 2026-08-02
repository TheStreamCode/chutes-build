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
    /// Scroll the page, or bring an element into view.
    Scroll,
    /// Dispatch a named key (`Enter`, `Tab`, `Escape`, arrows, ...).
    Key,
    /// Block until a selector appears, or page text contains a string.
    Wait,
    /// Choose an option in a `<select>` by value or visible label.
    Select,
    /// Go back in history.
    Back,
    /// Reload the current page.
    Reload,
    /// Full visible text of the page, for reading rather than clicking.
    Text,
    /// Console output and uncaught errors collected since the session started.
    Console,
    /// Network requests observed since the session started.
    Network,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BrowserInput {
    /// Browser operation. Sessions are reused across calls within the current agent session.
    pub action: BrowserAction,
    /// HTTP(S) URL for `navigate`.
    pub url: Option<String>,
    /// CSS selector for `click`, `type`, `select`, `scroll`, or `wait`.
    pub selector: Option<String>,
    /// Element index returned by `snapshot`, as an alternative to `selector`.
    pub index: Option<usize>,
    /// Text for `type`, the substring to await for `wait`, or the option label
    /// for `select`.
    pub text: Option<String>,
    /// Submit the nearest form after typing.
    pub submit: Option<bool>,
    /// Workspace-relative PNG path for `screenshot`.
    pub path: Option<String>,
    /// Vertical scroll distance in pixels for `scroll`; negative scrolls up.
    /// Ignored when `selector`/`index` names an element to scroll into view.
    pub delta_y: Option<i64>,
    /// Key name for `key`, e.g. `Enter`, `Tab`, `Escape`, `ArrowDown`.
    pub key: Option<String>,
    /// Option value for `select`, as an alternative to `text`.
    pub value: Option<String>,
    /// Deadline in milliseconds for `wait` (default 10000, max 60000).
    pub timeout_ms: Option<u64>,
    /// Maximum entries returned by `console` and `network` (default 50).
    pub limit: Option<usize>,
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
    /// Console output and uncaught errors, oldest first.
    console: Vec<serde_json::Value>,
    /// Network activity, oldest first.
    network: Vec<serde_json::Value>,
}

/// Cap on each retained event log. CDP pushes events for the session's whole
/// lifetime, so an unbounded buffer would grow with every page the agent
/// visits; the oldest entries are dropped once the cap is reached.
const EVENT_LOG_CAP: usize = 200;

/// Default number of entries returned by `console`/`network`.
const DEFAULT_EVENT_LIMIT: usize = 50;

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
                let element = element_expression(input.selector, input.index)?;
                // Focus before clicking: a real pointer click focuses the
                // element, but `el.click()` does not, and `key` dispatches to
                // whatever holds focus — so without this a click on a field
                // followed by a keystroke would silently go to the body.
                let script = format!(
                    "(() => {{ const el = {element}; if (!el) return {{ok:false,error:'element not found'}}; el.scrollIntoView({{block:'center'}}); if (typeof el.focus === 'function') el.focus(); el.click(); return {{ok:true,tag:el.tagName}}; }})()",
                );
                let result = session.evaluate(&script).await?;
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(result)
            }
            BrowserAction::Type => {
                let element = element_expression(input.selector, input.index)?;
                let text = required(input.text, "text")?;
                let script = format!(
                    "(() => {{ const el = {element}; if (!el) return {{ok:false,error:'element not found'}}; el.focus(); const value={text}; const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype; const setter=Object.getOwnPropertyDescriptor(proto,'value')?.set; if (setter) setter.call(el,value); else el.value=value; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); if ({submit}) el.form?.requestSubmit(); return {{ok:true,tag:el.tagName}}; }})()",
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
                write_new_screenshot(&output, &bytes).await?;
                Ok(serde_json::json!({"path": output, "format": "png"}))
            }
            BrowserAction::Scroll => {
                let target = match (input.selector.clone(), input.index) {
                    (None, None) => None,
                    (selector, index) => Some(element_expression(selector, index)?),
                };
                let script = match target {
                    Some(element) => format!(
                        "(() => {{ const el = {element}; if (!el) return {{ok:false,error:'element not found'}}; el.scrollIntoView({{block:'center'}}); return {{ok:true,x:window.scrollX,y:window.scrollY}}; }})()"
                    ),
                    None => format!(
                        "(() => {{ window.scrollBy(0, {delta}); return {{ok:true,x:window.scrollX,y:window.scrollY}}; }})()",
                        delta = input.delta_y.unwrap_or(600),
                    ),
                };
                let result = session.evaluate(&script).await?;
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(result)
            }
            BrowserAction::Key => {
                let name = required(input.key, "key")?;
                let key = KeySpec::lookup(&name)?;
                session.dispatch_key(key).await?;
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(serde_json::json!({"ok": true, "key": key.key}))
            }
            BrowserAction::Wait => {
                let timeout = Duration::from_millis(
                    input.timeout_ms.unwrap_or(10_000).clamp(100, 60_000),
                );
                let condition = wait_condition(
                    input.selector.clone(),
                    input.index,
                    input.text.clone(),
                )?;
                session.wait_for(&condition, timeout).await
            }
            BrowserAction::Select => {
                let element = element_expression(input.selector, input.index)?;
                let (field, wanted) = match (input.value, input.text) {
                    (Some(value), _) => ("value", value),
                    (None, Some(label)) => ("label", label),
                    (None, None) => {
                        return Err("'value' or 'text' is required to select an option".to_owned());
                    }
                };
                let script = format!(
                    "(() => {{ const el = {element}; if (!el) return {{ok:false,error:'element not found'}}; if (!(el instanceof HTMLSelectElement)) return {{ok:false,error:'element is not a <select>'}}; const wanted={wanted}; const option=[...el.options].find(o => {by}); if (!option) return {{ok:false,error:'option not found'}}; el.value=option.value; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); return {{ok:true,value:option.value,label:option.text}}; }})()",
                    wanted = serde_json::to_string(&wanted).map_err(|error| error.to_string())?,
                    by = if field == "value" {
                        "o.value === wanted"
                    } else {
                        "o.text.trim() === wanted"
                    },
                );
                let result = session.evaluate(&script).await?;
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(result)
            }
            BrowserAction::Back => {
                session.evaluate("history.back()").await?;
                session.wait_until_ready().await?;
                session.snapshot().await
            }
            BrowserAction::Reload => {
                session.command("Page.reload", serde_json::json!({})).await?;
                session.wait_until_ready().await?;
                session.snapshot().await
            }
            BrowserAction::Text => {
                session
                    .evaluate(
                        "(() => ({url:location.href,title:document.title,text:(document.body?.innerText||'').slice(0,20000)}))()",
                    )
                    .await
            }
            BrowserAction::Console => {
                session.drain_events().await?;
                Ok(recent_events(&session.console, input.limit))
            }
            BrowserAction::Network => {
                session.drain_events().await?;
                Ok(recent_events(&session.network, input.limit))
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
            console: Vec::new(),
            network: Vec::new(),
        };
        session
            .command("Page.enable", serde_json::json!({}))
            .await?;
        session
            .command("Runtime.enable", serde_json::json!({}))
            .await?;
        // Feed the console/network logs. Both are best-effort: an older browser
        // that rejects one domain must not make the whole session unusable.
        let _ = session.command("Log.enable", serde_json::json!({})).await;
        let _ = session
            .command("Network.enable", serde_json::json!({}))
            .await;
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
                // Anything that is not this command's reply is either an event
                // or a stale reply. Events only ever reach us while a command
                // is in flight, so this is the one place they can be captured.
                self.record_event(&value);
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("Browser command {method} failed: {error}"));
            }
            return Ok(value.get("result").cloned().unwrap_or_default());
        }
        Err("Browser connection ended before the command completed".to_owned())
    }

    /// File a CDP event into the console or network log, ignoring everything
    /// else. Both logs are capped at [`EVENT_LOG_CAP`], dropping oldest-first.
    fn record_event(&mut self, value: &serde_json::Value) {
        let Some((log_kind, entry)) = classify_event(value) else {
            return;
        };
        let log = match log_kind {
            EventLog::Console => &mut self.console,
            EventLog::Network => &mut self.network,
        };
        if log.len() >= EVENT_LOG_CAP {
            log.remove(0);
        }
        log.push(entry);
    }

    /// Read pending events off the socket without changing page state.
    ///
    /// Events are only drained while a command awaits its reply, so reporting
    /// the logs requires a round-trip first — otherwise everything since the
    /// previous action would still be sitting in the socket.
    async fn drain_events(&mut self) -> Result<(), String> {
        self.evaluate("1").await.map(|_| ())
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

    /// Send one key as a `keyDown`/`keyUp` pair through CDP, so the page sees
    /// real key events (a JS-dispatched `KeyboardEvent` is `isTrusted: false`
    /// and many form/editor handlers ignore it).
    async fn dispatch_key(&mut self, key: &KeySpec) -> Result<(), String> {
        for kind in ["keyDown", "keyUp"] {
            let mut params = serde_json::json!({
                "type": kind,
                "key": key.key,
                "code": key.code,
                "windowsVirtualKeyCode": key.virtual_key_code,
                "nativeVirtualKeyCode": key.virtual_key_code,
            });
            // `text` turns Enter into an actual newline for editors; omitted
            // for navigation keys, which must not insert anything.
            if kind == "keyDown"
                && let Some(text) = key.text
                && let Some(map) = params.as_object_mut()
            {
                map.insert(
                    "text".to_owned(),
                    serde_json::Value::String(text.to_owned()),
                );
            }
            self.command("Input.dispatchKeyEvent", params).await?;
        }
        Ok(())
    }

    /// Poll `condition` (a JS expression returning a boolean) until it holds or
    /// `timeout` elapses.
    async fn wait_for(
        &mut self,
        condition: &str,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.evaluate(condition).await?.as_bool() == Some(true) {
                return Ok(serde_json::json!({"ok": true}));
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(format!(
                    "Timed out after {}ms waiting for the page condition",
                    timeout.as_millis()
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
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
        "Control an isolated local Chrome or Edge session. Read the page: `snapshot` (structured, clickable elements with indices), `text` (full visible text), `screenshot` (PNG into the workspace), `console` (logs and uncaught errors), `network` (requests and failures). Act on it: `navigate`, `click`, `type`, `select`, `key` (Enter/Tab/Escape/arrows), `scroll`, `wait` (until a selector appears or text shows up), `back`, `reload`, `close`. Target elements by CSS selector or by the index from `snapshot`. Prefer `wait` over retrying a failed click. Browser actions can affect external sites; inspect before mutating."
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

/// Which log a CDP event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventLog {
    Console,
    Network,
}

/// Reduce a CDP event to the log it belongs to and the entry to retain.
///
/// `None` for everything else: CDP emits far more events than these two logs
/// are meant to surface, and keeping the projection here makes the retained
/// shape testable without a live browser.
fn classify_event(value: &serde_json::Value) -> Option<(EventLog, serde_json::Value)> {
    let method = value.get("method").and_then(serde_json::Value::as_str)?;
    let params = value.get("params");
    let entry = match method {
        "Runtime.consoleAPICalled" => (
            EventLog::Console,
            serde_json::json!({
                "kind": "console",
                "level": params.and_then(|p| p.get("type")).cloned(),
                "text": console_args_text(params),
            }),
        ),
        "Runtime.exceptionThrown" => (
            EventLog::Console,
            serde_json::json!({
                "kind": "exception",
                "level": "error",
                "text": params
                    .and_then(|p| p.pointer("/exceptionDetails/exception/description"))
                    .or_else(|| params.and_then(|p| p.pointer("/exceptionDetails/text")))
                    .cloned(),
            }),
        ),
        "Log.entryAdded" => (
            EventLog::Console,
            serde_json::json!({
                "kind": "log",
                "level": params.and_then(|p| p.pointer("/entry/level")).cloned(),
                "text": params.and_then(|p| p.pointer("/entry/text")).cloned(),
                "url": params.and_then(|p| p.pointer("/entry/url")).cloned(),
            }),
        ),
        "Network.requestWillBeSent" => (
            EventLog::Network,
            serde_json::json!({
                "kind": "request",
                "method": params.and_then(|p| p.pointer("/request/method")).cloned(),
                "url": params.and_then(|p| p.pointer("/request/url")).cloned(),
            }),
        ),
        "Network.responseReceived" => (
            EventLog::Network,
            serde_json::json!({
                "kind": "response",
                "status": params.and_then(|p| p.pointer("/response/status")).cloned(),
                "url": params.and_then(|p| p.pointer("/response/url")).cloned(),
                "mime_type": params.and_then(|p| p.pointer("/response/mimeType")).cloned(),
            }),
        ),
        "Network.loadingFailed" => (
            EventLog::Network,
            serde_json::json!({
                "kind": "failed",
                "error": params.and_then(|p| p.get("errorText")).cloned(),
                "type": params.and_then(|p| p.get("type")).cloned(),
            }),
        ),
        _ => return None,
    };
    Some(entry)
}

/// A key the `key` action can dispatch, with the codes CDP expects.
#[derive(Debug)]
struct KeySpec {
    key: &'static str,
    code: &'static str,
    virtual_key_code: u32,
    /// Character inserted on `keyDown`; `None` for keys that only navigate.
    text: Option<&'static str>,
}

/// Named keys, matched case-insensitively. Deliberately a closed set: free-form
/// text belongs in the `type` action, and an unknown name here would otherwise
/// be dispatched as a key the page silently ignores.
const KEYS: &[KeySpec] = &[
    KeySpec {
        key: "Enter",
        code: "Enter",
        virtual_key_code: 13,
        text: Some("\r"),
    },
    KeySpec {
        key: "Tab",
        code: "Tab",
        virtual_key_code: 9,
        text: Some("\t"),
    },
    KeySpec {
        key: "Escape",
        code: "Escape",
        virtual_key_code: 27,
        text: None,
    },
    KeySpec {
        key: "Backspace",
        code: "Backspace",
        virtual_key_code: 8,
        text: None,
    },
    KeySpec {
        key: "Delete",
        code: "Delete",
        virtual_key_code: 46,
        text: None,
    },
    KeySpec {
        key: "ArrowUp",
        code: "ArrowUp",
        virtual_key_code: 38,
        text: None,
    },
    KeySpec {
        key: "ArrowDown",
        code: "ArrowDown",
        virtual_key_code: 40,
        text: None,
    },
    KeySpec {
        key: "ArrowLeft",
        code: "ArrowLeft",
        virtual_key_code: 37,
        text: None,
    },
    KeySpec {
        key: "ArrowRight",
        code: "ArrowRight",
        virtual_key_code: 39,
        text: None,
    },
    KeySpec {
        key: "Home",
        code: "Home",
        virtual_key_code: 36,
        text: None,
    },
    KeySpec {
        key: "End",
        code: "End",
        virtual_key_code: 35,
        text: None,
    },
    KeySpec {
        key: "PageUp",
        code: "PageUp",
        virtual_key_code: 33,
        text: None,
    },
    KeySpec {
        key: "PageDown",
        code: "PageDown",
        virtual_key_code: 34,
        text: None,
    },
];

impl KeySpec {
    fn lookup(name: &str) -> Result<&'static KeySpec, String> {
        let wanted = name.trim();
        KEYS.iter()
            .find(|key| key.key.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| {
                let known: Vec<&str> = KEYS.iter().map(|key| key.key).collect();
                format!(
                    "Unsupported key `{wanted}`. Supported: {}",
                    known.join(", ")
                )
            })
    }
}

/// JS expression for the `wait` action: a selector becoming visible, or the
/// page text containing a string.
fn wait_condition(
    selector: Option<String>,
    index: Option<usize>,
    text: Option<String>,
) -> Result<String, String> {
    let has_target = selector.as_ref().is_some_and(|s| !s.trim().is_empty()) || index.is_some();
    match (has_target, text.filter(|value| !value.trim().is_empty())) {
        (true, _) => {
            let element = element_expression(selector, index)?;
            Ok(format!(
                "(() => {{ const el = {element}; if (!el) return false; const s=getComputedStyle(el), r=el.getBoundingClientRect(); return s.display!=='none' && s.visibility!=='hidden' && r.width>0 && r.height>0; }})()"
            ))
        }
        (false, Some(text)) => Ok(format!(
            "(document.body?.innerText || '').includes({})",
            serde_json::to_string(&text).map_err(|error| error.to_string())?
        )),
        (false, None) => Err("'selector', 'index', or 'text' is required to wait".to_owned()),
    }
}

/// Newest `limit` entries of an event log, oldest first, with the total kept so
/// the caller can tell truncation from an empty tail.
fn recent_events(log: &[serde_json::Value], limit: Option<usize>) -> serde_json::Value {
    let limit = limit.unwrap_or(DEFAULT_EVENT_LIMIT).clamp(1, EVENT_LOG_CAP);
    let start = log.len().saturating_sub(limit);
    serde_json::json!({
        "total": log.len(),
        "entries": &log[start..],
    })
}

/// Flatten `Runtime.consoleAPICalled` arguments into one readable line.
fn console_args_text(params: Option<&serde_json::Value>) -> String {
    let Some(args) = params
        .and_then(|p| p.get("args"))
        .and_then(serde_json::Value::as_array)
    else {
        return String::new();
    };
    args.iter()
        .map(|arg| {
            arg.get("value")
                .map(|value| match value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .or_else(|| {
                    arg.get("description")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn required(value: Option<String>, field: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("'{field}' is required for this browser action"))
}

fn element_expression(selector: Option<String>, index: Option<usize>) -> Result<String, String> {
    match (selector.filter(|value| !value.trim().is_empty()), index) {
        (Some(selector), None) => Ok(format!(
            "document.querySelector({})",
            serde_json::to_string(&selector).map_err(|error| error.to_string())?
        )),
        (None, Some(index)) => Ok(format!(
            "[...document.querySelectorAll('a,button,input,textarea,select,[role],h1,h2,h3,p,li')].filter(el => {{ const s=getComputedStyle(el), r=el.getBoundingClientRect(); return s.display!=='none' && s.visibility!=='hidden' && r.width>0 && r.height>0; }})[{index}]"
        )),
        (Some(_), Some(_)) => Err("Use either 'selector' or 'index', not both".to_owned()),
        (None, None) => Err("'selector' or 'index' is required for this browser action".to_owned()),
    }
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

async fn write_new_screenshot(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(|error| format!("Failed to create screenshot: {error}"))?;
    let write_result = async {
        file.write_all(bytes)
            .await
            .map_err(|error| format!("Failed to write screenshot: {error}"))?;
        file.flush()
            .await
            .map_err(|error| format!("Failed to flush screenshot: {error}"))
    }
    .await;
    drop(file);
    if let Err(error) = write_result {
        if let Err(cleanup_error) = tokio::fs::remove_file(path).await {
            return Err(format!(
                "{error}; failed to remove partial screenshot: {cleanup_error}"
            ));
        }
        return Err(error);
    }
    Ok(())
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

    /// Page exercising what only a live browser can prove: a console log, a
    /// failing request, a `<select>`, a keydown handler, and enough height to
    /// scroll. Served by [`serve_fixture`].
    const LIVE_FIXTURE: &str = r#"<!doctype html>
<html><head><title>cdp fixture</title></head><body>
<h1 id="heading">CDP fixture</h1>
<select id="picker"><option value="a">Alpha</option><option value="b">Beta</option></select>
<input id="typed" type="text">
<div id="key-result">no-key</div>
<div style="height:2000px"></div>
<div id="bottom">bottom marker</div>
<script>
  console.log('fixture ready', 42);
  document.getElementById('typed').addEventListener('keydown', e => {
    document.getElementById('key-result').textContent = 'key:' + e.key;
  });
  fetch('/missing-on-purpose').catch(() => {});
  setTimeout(() => {
    const late = document.createElement('div');
    late.id = 'late';
    late.textContent = 'late content';
    document.body.appendChild(late);
  }, 400);
</script>
</body></html>"#;

    /// Serve [`LIVE_FIXTURE`] on a loopback port. Hand-rolled so the test needs
    /// no HTTP server dependency; `/missing-on-purpose` answers 404 so the
    /// network log has a failure to report.
    fn serve_fixture() -> u16 {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fixture port");
        let port = listener.local_addr().expect("fixture addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                std::thread::spawn(move || {
                    let mut probe = [0u8; 1024];
                    let _ = stream.read(&mut probe);
                    let request = String::from_utf8_lossy(&probe);
                    let response = if request.contains("/missing-on-purpose") {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_owned()
                    } else {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{LIVE_FIXTURE}",
                            LIVE_FIXTURE.len()
                        )
                    };
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                });
            }
        });
        port
    }

    fn live_input(action: BrowserAction) -> BrowserInput {
        BrowserInput {
            action,
            url: None,
            selector: None,
            index: None,
            text: None,
            submit: None,
            path: None,
            delta_y: None,
            key: None,
            value: None,
            timeout_ms: None,
            limit: None,
        }
    }

    /// End-to-end check of the CDP actions. Ignored by default because it
    /// launches a real Chrome/Edge; run with
    /// `cargo test -p xai-grok-tools --lib browser -- --ignored`.
    #[tokio::test]
    #[ignore = "launches a real Chrome/Edge"]
    async fn cdp_actions_work_against_a_live_page() {
        let port = serve_fixture();
        let workspace = tempfile::tempdir().expect("workspace");
        let client = BrowserClient::new(workspace.path().to_path_buf());

        let mut navigate = live_input(BrowserAction::Navigate);
        navigate.url = Some(format!("http://127.0.0.1:{port}/"));
        let snapshot = client.execute(navigate).await.expect("navigate");
        assert_eq!(snapshot["title"], serde_json::json!("cdp fixture"));

        let text = client
            .execute(live_input(BrowserAction::Text))
            .await
            .expect("text");
        let body = text["text"].as_str().unwrap_or_default();
        assert!(body.contains("CDP fixture"), "text missing heading: {body}");
        assert!(body.contains("bottom marker"), "text missing tail: {body}");

        // The late element only appears after 400ms.
        let mut wait_late = live_input(BrowserAction::Wait);
        wait_late.selector = Some("#late".to_owned());
        wait_late.timeout_ms = Some(5_000);
        client.execute(wait_late).await.expect("wait for selector");

        let mut wait_text = live_input(BrowserAction::Wait);
        wait_text.text = Some("late content".to_owned());
        client.execute(wait_text).await.expect("wait for text");

        let mut select = live_input(BrowserAction::Select);
        select.selector = Some("#picker".to_owned());
        select.text = Some("Beta".to_owned());
        let selected = client.execute(select).await.expect("select");
        assert_eq!(selected["ok"], serde_json::json!(true), "{selected}");
        assert_eq!(selected["value"], serde_json::json!("b"), "{selected}");

        let mut focus = live_input(BrowserAction::Click);
        focus.selector = Some("#typed".to_owned());
        client.execute(focus).await.expect("focus input");
        let mut key = live_input(BrowserAction::Key);
        key.key = Some("ArrowDown".to_owned());
        client.execute(key).await.expect("key");
        let after_key = client
            .execute(live_input(BrowserAction::Text))
            .await
            .expect("text after key");
        assert!(
            after_key["text"]
                .as_str()
                .unwrap_or_default()
                .contains("key:ArrowDown"),
            "the page's keydown handler did not see the key: {after_key}"
        );

        let mut scroll = live_input(BrowserAction::Scroll);
        scroll.delta_y = Some(900);
        let scrolled = client.execute(scroll).await.expect("scroll");
        assert!(
            scrolled["y"].as_f64().unwrap_or_default() > 0.0,
            "page did not scroll: {scrolled}"
        );

        let console = client
            .execute(live_input(BrowserAction::Console))
            .await
            .expect("console");
        let entries = console["entries"].as_array().cloned().unwrap_or_default();
        assert!(
            entries.iter().any(|entry| entry["text"]
                .as_str()
                .is_some_and(|text| text.contains("fixture ready"))),
            "console log not captured: {console}"
        );

        let network = client
            .execute(live_input(BrowserAction::Network))
            .await
            .expect("network");
        let entries = network["entries"].as_array().cloned().unwrap_or_default();
        assert!(
            entries.iter().any(|entry| entry["url"]
                .as_str()
                .is_some_and(|url| url.contains("missing-on-purpose"))),
            "network activity not captured: {network}"
        );

        let reloaded = client
            .execute(live_input(BrowserAction::Reload))
            .await
            .expect("reload");
        assert_eq!(reloaded["title"], serde_json::json!("cdp fixture"));

        client
            .execute(live_input(BrowserAction::Close))
            .await
            .expect("close");
    }

    #[test]
    fn known_keys_resolve_and_unknown_ones_list_the_alternatives() {
        let enter = KeySpec::lookup("enter").expect("case-insensitive lookup");
        assert_eq!(enter.key, "Enter");
        assert_eq!(enter.virtual_key_code, 13);
        // Navigation keys must not insert a character.
        assert!(KeySpec::lookup("Escape").unwrap().text.is_none());
        let error = KeySpec::lookup("F13").expect_err("unknown key is rejected");
        assert!(
            error.contains("Enter"),
            "error should list known keys: {error}"
        );
    }

    #[test]
    fn wait_accepts_a_target_or_a_text_probe() {
        let by_selector = wait_condition(Some("#ready".into()), None, None).unwrap();
        assert!(by_selector.contains("querySelector"));
        assert!(by_selector.contains("getBoundingClientRect"));

        let by_text = wait_condition(None, None, Some("Welcome".into())).unwrap();
        assert!(by_text.contains("innerText"));
        assert!(by_text.contains("\"Welcome\""), "text must be JSON-escaped");

        assert!(wait_condition(None, None, None).is_err());
        assert!(wait_condition(None, None, Some("  ".into())).is_err());
    }

    /// A quoted string in the awaited text must not be able to close the
    /// expression and append script of its own.
    #[test]
    fn wait_text_is_escaped_into_the_expression() {
        let condition = wait_condition(None, None, Some("\");alert(1)//".into())).unwrap();
        assert!(condition.contains(r#"\");alert(1)//"#), "got: {condition}");
    }

    #[test]
    fn event_logs_report_the_newest_entries_and_the_true_total() {
        let log: Vec<serde_json::Value> = (0..5).map(|i| serde_json::json!({"n": i})).collect();
        let recent = recent_events(&log, Some(2));
        assert_eq!(recent["total"], serde_json::json!(5));
        assert_eq!(recent["entries"], serde_json::json!([{"n": 3}, {"n": 4}]));
        // An empty log is reported as such rather than erroring.
        assert_eq!(recent_events(&[], None)["total"], serde_json::json!(0));
    }

    #[test]
    fn console_arguments_flatten_to_one_line() {
        let params = serde_json::json!({
            "args": [
                {"type": "string", "value": "count"},
                {"type": "number", "value": 42},
                {"type": "object", "description": "Error: boom"},
            ]
        });
        assert_eq!(console_args_text(Some(&params)), "count 42 Error: boom");
        assert_eq!(console_args_text(None), "");
    }

    #[test]
    fn console_and_network_events_are_routed_to_their_own_logs() {
        for method in [
            "Runtime.consoleAPICalled",
            "Runtime.exceptionThrown",
            "Log.entryAdded",
        ] {
            let (log, _) = classify_event(&serde_json::json!({"method": method}))
                .unwrap_or_else(|| panic!("{method} should be captured"));
            assert_eq!(log, EventLog::Console, "{method}");
        }
        for method in [
            "Network.requestWillBeSent",
            "Network.responseReceived",
            "Network.loadingFailed",
        ] {
            let (log, _) = classify_event(&serde_json::json!({"method": method}))
                .unwrap_or_else(|| panic!("{method} should be captured"));
            assert_eq!(log, EventLog::Network, "{method}");
        }
        // CDP emits a flood of other events; they must not enter either log.
        assert!(classify_event(&serde_json::json!({"method": "Page.frameNavigated"})).is_none());
        assert!(classify_event(&serde_json::json!({"id": 7, "result": {}})).is_none());
    }

    #[test]
    fn captured_events_keep_the_fields_worth_reporting() {
        let (_, console) = classify_event(&serde_json::json!({
            "method": "Runtime.consoleAPICalled",
            "params": {"type": "error", "args": [{"value": "boom"}]},
        }))
        .unwrap();
        assert_eq!(console["level"], serde_json::json!("error"));
        assert_eq!(console["text"], serde_json::json!("boom"));

        let (_, response) = classify_event(&serde_json::json!({
            "method": "Network.responseReceived",
            "params": {"response": {"status": 404, "url": "https://example.com/x"}},
        }))
        .unwrap();
        assert_eq!(response["status"], serde_json::json!(404));
        assert_eq!(response["url"], serde_json::json!("https://example.com/x"));
    }

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

    #[test]
    fn browser_elements_can_be_selected_by_css_or_snapshot_index() {
        let selector = element_expression(Some("#submit".into()), None).unwrap();
        assert!(selector.contains("querySelector"));
        let index = element_expression(None, Some(7)).unwrap();
        assert!(index.ends_with("[7]"));
        assert!(element_expression(None, None).is_err());
        assert!(element_expression(Some("#x".into()), Some(1)).is_err());
    }

    #[tokio::test]
    async fn screenshot_writer_never_overwrites_an_existing_file() {
        let cwd = tempfile::tempdir().unwrap();
        let path = cwd.path().join("page.png");
        std::fs::write(&path, b"original").unwrap();
        assert!(write_new_screenshot(&path, b"replacement").await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }
}
