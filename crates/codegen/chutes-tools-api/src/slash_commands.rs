//! Canonical slash-command wording (`/loop`, `/imagine`, `/imagine-video`, `/goal`),
//! shared by every front-end (Chutes Build shell/pager and other hosts) so
//! expansions cannot drift.

/// Canonical tool name advertised by the scheduler create tool. Gating code
/// (shell `CommandAvailability`, pager `required_tools`, host command lists)
/// keys `/loop` availability on this name.
pub const SCHEDULER_CREATE_TOOL_NAME: &str = "scheduler_create";

/// Usage hint shown when `/loop` is invoked with no arguments.
pub fn loop_usage_message() -> &'static str {
    "Usage: /loop [interval] <prompt>\n\
     Example: /loop 30m check deploy status\n\
     Example: /loop check deploy status every hour\n\n\
     Tell me how often it should run (e.g. 30m, 1 hour, every 2 days)."
}

/// Build the model instruction that `/loop` expands into for `args`. The model, not brittle host
/// parsing, turns the request into the `scheduler_create` interval, accepting every natural
/// phrasing and erroring on bad input rather than silently defaulting. See [`loop_usage_message`].
pub fn loop_schedule_instruction(args: &str) -> String {
    let fire_context = "Each fire runs in a detached background subagent, not in this conversation,\n\
         so the prompt you store must stand on its own.\n\n\
         ## Writing a prompt that survives a fresh fire\n\
         - Inline the state a fire needs: paths, job/PR/branch ids, the command that checks\n\
           status, and what \"healthy\" looks like. A fire cannot see this conversation, and\n\
           a long-running task restarts from a short summary every few iterations.\n\
         - Only a short status comes back here, so say what that status must contain.";
    format!(
        "# /loop -- schedule a recurring prompt\n\n\
         Turn the input below into a scheduler_create call. {fire_context}\n\
         - Say what one fire does and when it bails: \"if still pending, report one line and\n\
           stop.\" A fire must not poll inline.\n\
         - Give it a stop condition and an exit: \"when <condition> holds, report it and call\n\
           scheduler_delete <task_id>.\" Without that the loop runs until it expires.\n\
         - Keep it short and concrete -- the stored prompt is re-sent on every fire.\n\n\
         ## Deriving the interval\n\
         Convert the user's cadence -- however phrased, at either end of the request -- into a\n\
         compact `<number><unit>` string (`s`/`m`/`h`/`d`); the remaining text is the prompt.\n\
         The minimum is 60 seconds and shorter values are raised, so say so when it applies.\n\
         If no cadence is given, ask the user how often it should run -- never invent one.\n\n\
         ## Action\n\
         Schedule from what the user already gave you \u{2014} do not explore the workspace or run\n\
         checks before scheduling; the first fire does that.\n\
         1. Call scheduler_create with the interval, the prompt, and fire_immediately: true.\n\
            If the interval is rejected, fix the string rather than guessing.\n\
         2. Confirm what's scheduled, the cadence, its stop condition, that it auto-expires\n\
            after 7 days, and the task_id to cancel with scheduler_delete.\n\
         3. Do NOT execute the prompt inline. The scheduler fires it immediately.\n\n\
         ## Wrong tool for the job\n\
         - \"Tell me when X finishes\" -> a background command or watch tool that wakes you on\n\
           the event, not a recurring loop that re-checks on a timer.\n\
         - \"Do X once in N minutes\" -> background `sleep <secs> && <command>`; scheduling is\n\
           recurring-only.\n\n\
         ## Changing an existing loop\n\
         Call scheduler_create with its task_id and only the changed fields; do not\n\
         delete and recreate. If later work changes what a loop should do, update its\n\
         prompt the same way.\n\n\
         ## Input\n\
         {args}"
    )
}

/// Canonical name of the image generation tool; gates `/imagine`.
pub const IMAGE_GEN_TOOL_NAME: &str = "image_gen";

/// Advertised name of the /imagine command.
pub const IMAGINE_COMMAND_NAME: &str = "imagine";

/// Canonical name of the image-to-video tool; gates `/imagine-video`.
pub const IMAGE_TO_VIDEO_TOOL_NAME: &str = "image_to_video";

/// Advertised name of the /imagine-video command.
pub const IMAGINE_VIDEO_COMMAND_NAME: &str = "imagine-video";

/// Usage hint shown by `/imagine` when the model needs an explicit reminder.
pub fn imagine_usage_message() -> &'static str {
    "Usage: /imagine [model] <description>\n\
     `/imagine <description>` generates with an available catalog model.\n\
     `/imagine <model> <description>` picks a specific model.\n\
     `/imagine` alone lists the image models that are available right now."
}

/// The model selection part of a media slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MediaRequest {
    model: Option<String>,
    prompt: Option<String>,
}

/// Parse `/imagine` / `/imagine-video` arguments.
///
/// Accepted forms:
/// - empty: the caller lists the live catalog instead of generating;
/// - `<prompt>`: `generate_media` picks an available model (no model name is
///   guessed or hardcoded);
/// - `<model> <prompt>`: a model id/slug followed by the prompt, as long as the
///   first token looks like an id (`vendor-model-name`, two hyphens or more,
///   `vendor/name`, or a UUID) — otherwise the whole text stays the prompt;
/// - `--model <model> <prompt>` / `model=<model> <prompt>`: unambiguous forms.
fn parse_media_request(args: &str) -> MediaRequest {
    let args = args.trim();
    if args.is_empty() {
        return MediaRequest {
            model: None,
            prompt: None,
        };
    }
    let mut rest = args;
    let mut model = None;
    for prefix in ["--model=", "model="] {
        if let Some(value) = rest.strip_prefix(prefix) {
            let (candidate, tail) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
            model = Some(candidate.trim().to_owned()).filter(|value| !value.is_empty());
            rest = tail.trim();
            break;
        }
    }
    if model.is_none() {
        for prefix in ["--model ", "model "] {
            if let Some(value) = rest.strip_prefix(prefix) {
                let (candidate, tail) =
                    value.split_once(char::is_whitespace).unwrap_or((value, ""));
                model = Some(candidate.trim().to_owned()).filter(|value| !value.is_empty());
                rest = tail.trim();
                break;
            }
        }
    }
    if model.is_none()
        && let Some((first, tail)) = rest.split_once(char::is_whitespace)
        && looks_like_media_model_id(first)
        && !tail.trim().is_empty()
    {
        model = Some(first.to_owned());
        rest = tail.trim();
    }
    let prompt = (!rest.is_empty()).then(|| rest.to_owned());
    match (model, prompt) {
        // `--model x` with no prompt: ask the caller to add one.
        (Some(_), None) => MediaRequest {
            model: None,
            prompt: None,
        },
        other => MediaRequest {
            model: other.0,
            prompt: other.1,
        },
    }
}

/// A media model id looks like a catalog slug, a vendor-scoped name, or a chute id.
fn looks_like_media_model_id(token: &str) -> bool {
    let token = token.trim_matches(|c: char| c == '"' || c == '\'');
    if token.is_empty() {
        return false;
    }
    // `vonkaiser-ltx-25-video`, `z-image-turbo`: two or more hyphenated segments.
    let hyphen_segments = token.split('-').count();
    let uuid = token.len() == 36 && token.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    let vendor_scoped = token.contains('/');
    let explicit_id = token.contains("--");
    (hyphen_segments >= 3
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        || uuid
        || vendor_scoped
        || explicit_id
}

fn imagine_model_list_instruction(kind: &str) -> String {
    let command = if kind == "image" {
        "imagine"
    } else {
        "imagine-video"
    };
    format!(
        "Call list_media_models with kind \"{kind}\" and limit 20, then present the available {kind} models as a compact list: slug (or name), id, and a one-line reason to pick it. Do not generate anything yet. Close with the three forms the user can run: `/{command} <description>` lets the tool pick an available model automatically, `/{command} <model> <description>` chooses one of the listed models, and `/{command} --model <model> <description>` is the unambiguous form. Only list models that appear in the tool result; never invent one."
    )
}

fn image_model_clause(model: &Option<String>) -> String {
    match model {
        Some(model) => format!(
            "Pass model \"{model}\" as `model`; if that model is gone, generate_media substitutes an \
             available image model automatically. "
        ),
        None => "Omit `model`: generate_media picks a currently available image model from the \
                 live Chutes catalog, so do not guess or hardcode a model name. "
            .to_owned(),
    }
}

/// Build the model instruction that `/imagine` expands into for its arguments.
/// Routes through the Chutes-native `generate_media` tool: the legacy
/// `image_gen` tool targets the legacy Imagine API, which Chutes does not serve.
pub fn imagine_instruction(args: &str) -> String {
    let request = parse_media_request(args);
    let Some(prompt) = request.prompt else {
        return imagine_model_list_instruction("image");
    };
    format!(
        "Call the generate_media tool immediately with kind \"image\", passing the user's prompt below \
         verbatim as `prompt` — do not rewrite, embellish, or expand it. {model_clause}\
         If you need alternatives, call list_media_models with kind \"image\" first. \
         Report generate_media's final error verbatim instead of retrying blindly. \
         After the tool completes, briefly acknowledge and mention where the image was saved.\n\n\
         Prompt: {prompt}",
        model_clause = image_model_clause(&request.model),
    )
}

/// Usage hint shown by `/imagine-video` when the model needs an explicit reminder.
pub fn imagine_video_usage_message() -> &'static str {
    "Usage: /imagine-video [model] <description>\n\
     `/imagine-video <description>` generates with an available catalog model.\n\
     `/imagine-video <model> <description>` picks a specific model.\n\
     `/imagine-video` alone lists the video models that are available right now."
}

/// Build the model instruction that `/imagine-video` expands into for its arguments.
pub fn imagine_video_instruction(args: &str) -> String {
    let request = parse_media_request(args);
    let Some(prompt) = request.prompt else {
        return imagine_model_list_instruction("video");
    };
    let model_clause = match &request.model {
        Some(model) => format!(
            "Pass model \"{model}\" as `model`; if that model is gone, generate_media substitutes an \
             available video model automatically."
        ),
        None => {
            "Omit `model`: generate_media picks a currently available video model from the live \
                 Chutes catalog, so do not guess or hardcode a model name."
                .to_owned()
        }
    };
    format!(
        "{IMAGINE_VIDEO_SKILL}\n\n\
         Model selection: {model_clause}\n\n\
         User prompt: {prompt}"
    )
}

/// Video workflow guidance injected by `/imagine-video`.
/// Routes through the Chutes-native `generate_media` tool: the legacy
/// `image_to_video`/`reference_to_video` tools target the legacy Imagine API,
/// which Chutes does not serve.
const IMAGINE_VIDEO_SKILL: &str = "\
# Imagine Video

Generate video with the `generate_media` tool (kind \"video\"). \
If you need alternatives, call `list_media_models` with kind \"video\" first. \
`generate_media` substitutes an available model automatically when the requested \
one is gone; report its final error verbatim instead of retrying blindly.

Some video models animate from a source image, others take a bare text \
prompt — call `describe_media_model` on the model first when you are unsure \
which inputs its cord expects. To stage a first frame explicitly, generate \
a source image with `generate_media` (kind \"image\") and pass it as an \
input asset via `params`.

If a video tool fails with a zero-data-retention (ZDR) storage error, relay \
that error verbatim and stop the workflow — do not generate more source \
images or retry.

## Default: single clip

Unless the user asks for a long video, multiple scenes, or a multi-shot sequence, \
generate **one** video with a short prompt describing the motion \
or camera move (1–2 sentences, present tense). \
After the tool completes, mention the saved file path so the user can find it.

## Longer / multi-shot videos

When the user requests a longer video, multiple scenes, or a narrative sequence:

1. **Plan the story as shots** — break the idea into distinct shots, one beat each.
2. **Favor frequent, short shots** — prefer more short clips over fewer long ones; more cuts keep it dynamic.
3. **Create each shot's source image** with `generate_media` (kind \"image\") when the video model needs one, keeping characters and settings consistent across shots.
4. **Animate each shot with `generate_media`** (kind \"video\").
5. **Assemble with FFmpeg** using stream copy (`ffmpeg -f concat ... -c copy` — never re-encode). \
Keep every shot at the same resolution and frame rate so the concat works. \
After assembly, mention the final output path.

## Shot guidance

- **Prompt-craft:** one short, vivid moment in present tense with a clear camera movement, in 1–2 sentences.
- **Minimal but interesting:** one clear subject, one simple motion or camera move per shot. Avoid complex multi-action animation; make the shot compelling through composition, lighting, and a strong moment.
- **Complex source image?** Intricate frames (busy geometry, fine detail, heavy reflections) warp when animated. Keep the subject fixed and move only the camera (slow push-in, orbit, or parallax), or break into simpler shots. For new shots, generate a simpler, animation-friendly base image rather than animating a busy one.
- **Real people:** reference-first — drive the video from a verified reference image; never animate a named person without one.
- Don't loop the same clip unless asked.";

pub const UPDATE_GOAL_TOOL_NAME: &str = "update_goal";

pub const WORKFLOW_TOOL_NAME: &str = "workflow";

pub const GOAL_COMMAND_NAME: &str = "goal";

/// Bare subcommand tokens reserved for goal lifecycle control rather than
/// being treated as an objective, matching the shell's /goal grammar.
pub const GOAL_RESERVED_SUBCOMMANDS: &[&str] = &["status", "pause", "resume", "clear", "edit"];

pub fn goal_usage_message() -> &'static str {
    "Usage: /goal <objective>\n\
     Set an objective to work toward until it is complete."
}

pub fn goal_instruction(objective: &str) -> String {
    format!(
        "# /goal -- pursue an objective\n\n\
         A goal has been set: {objective}\n\n\
         Work directly on this goal and carry it as far as you can. Deliver \
         everything the user asked for yourself: no follow-up questions, no \
         manual steps left for the user. If the conversation continues, keep \
         pursuing the goal until it is complete.\n\n\
         TRACKING: break the objective into concrete steps and track them \
         (use your todo tool if one is available), marking each done as you \
         finish it.\n\n\
         VERIFY AS YOU GO: test each change on the real path before moving on. \
         A completion claim must be backed by evidence produced in this \
         session, not assumptions.\n\n\
         Call update_goal(completed: true, message: \"summary\") ONLY when the \
         goal is fully achieved. Call update_goal(blocked_reason: \"reason\") \
         only when truly stuck after 3+ consecutive failed attempts at the \
         same problem. Call update_goal(message: \"status note\") to log \
         progress along the way. If update_goal returns an error, continue \
         working the goal and report status in your reply instead.\n\n\
         Start now."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagine_instruction_carries_prompt_verbatim() {
        let text = imagine_instruction("a golden sunset");
        assert!(text.contains("a golden sunset"));
        assert!(text.contains("generate_media"));
        assert!(text.contains("Omit `model`"));
        assert!(!text.contains("image_gen"));
    }

    #[test]
    fn imagine_instruction_lists_models_when_empty() {
        let text = imagine_instruction("");
        assert!(text.contains("list_media_models"));
        assert!(!text.contains("generate_media"));
    }

    #[test]
    fn imagine_instruction_passes_a_named_model() {
        let text = imagine_instruction("z-image-turbo a spaceship in space");
        assert!(text.contains("z-image-turbo"));
        assert!(text.contains("a spaceship in space"));
        assert!(!text.contains("Omit `model`"));
    }

    #[test]
    fn imagine_instruction_treats_a_hyphenated_prompt_prefix_as_prompt() {
        // `8-bit` is not a catalog slug: one hyphen keeps the whole text a prompt.
        let text = imagine_instruction("8-bit pixel art of a castle");
        assert!(text.contains("8-bit pixel art of a castle"));
        assert!(text.contains("Omit `model`"));
    }

    #[test]
    fn imagine_video_instruction_carries_prompt_and_workflow() {
        let text = imagine_video_instruction("a cat playing piano");
        assert!(text.contains("a cat playing piano"));
        assert!(text.contains("generate_media"));
        assert!(text.contains("list_media_models"));
        assert!(text.contains("Omit `model`"));
        assert!(!text.contains("image_to_video"));
    }

    #[test]
    fn imagine_video_instruction_lists_models_when_empty() {
        let text = imagine_video_instruction("");
        assert!(text.contains("list_media_models"));
        assert!(text.contains("/imagine-video <model> <description>"));
    }

    #[test]
    fn imagine_video_instruction_passes_a_named_model() {
        let text = imagine_video_instruction("ltx-25-video a cat playing piano");
        assert!(text.contains("ltx-25-video"));
        assert!(text.contains("a cat playing piano"));
        assert!(!text.contains("Omit `model`"));
    }

    #[test]
    fn instruction_carries_args_and_contract_tokens() {
        let text = loop_schedule_instruction("every 30 minutes do x");
        assert!(text.contains("every 30 minutes do x"));
        assert!(text.contains("<number><unit>"));
        assert!(!text.contains("10m"), "no host-side default interval");
        assert!(
            !text.contains("recurring:"),
            "the retired one-shot flag must not be referenced"
        );
        assert!(
            text.contains("task_id"),
            "must teach in-place updates via task_id"
        );
        assert!(
            text.contains("scheduler_delete <task_id>"),
            "the fire must be authorized to end the task"
        );
        assert!(
            text.contains("detached background subagent"),
            "every fire is detached, and the stored prompt must be told so"
        );
    }

    #[test]
    fn goal_instruction_carries_objective_and_contract_tokens() {
        let text = goal_instruction("ship the widget");
        assert!(text.contains("ship the widget"));
        assert!(text.contains("update_goal(completed: true"));
        assert!(text.contains("blocked_reason"));
        assert!(
            !text.contains("system-reminder"),
            "expansions ride as user messages and must not claim reminder authority"
        );
        assert!(goal_usage_message().contains("Usage: /goal"));
    }

    #[test]
    fn usage_message_has_no_default_claim() {
        assert!(loop_usage_message().contains("Usage: /loop"));
        assert!(!loop_usage_message().contains("10m"));
    }
}
