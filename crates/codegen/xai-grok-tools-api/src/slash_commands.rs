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

/// Where a scheduled fire runs, which decides what the stored prompt can rely on.
///
/// Resolved from `[scheduler] background_loops` (env, config, managed policy and
/// remote settings all feed it), so `/loop` describes the runtime the user
/// actually has rather than hedging across both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopFireMode {
    /// Each fire runs in a detached background subagent that cannot see this
    /// conversation. The default.
    Detached,
    /// Each fire runs as a turn in this conversation, where earlier results from
    /// the same task may still be visible.
    InSession,
}

/// Build the model instruction that `/loop` expands into for `args`.
///
/// The model, not brittle host parsing, turns the request into the
/// `scheduler_create` interval, accepting every natural phrasing and erroring
/// on bad input rather than silently defaulting. See [`loop_usage_message`].
///
/// Only the framing differs by `mode`; the stop condition and length guidance
/// are identical, because both hold wherever the fire runs.
pub fn loop_schedule_instruction(args: &str, mode: LoopFireMode) -> String {
    let fire_context = match mode {
        LoopFireMode::Detached => {
            "Each fire runs in a detached background subagent, not in this conversation,\n\
             so the prompt you store must stand on its own.\n\n\
             ## Writing a prompt that survives a fresh fire\n\
             - Inline the state a fire needs: paths, job/PR/branch ids, the command that checks\n\
               status, and what \"healthy\" looks like. A fire cannot see this conversation, and\n\
               a long-running task restarts from a short summary every few iterations.\n\
             - Only a short status comes back here, so say what that status must contain."
        }
        LoopFireMode::InSession => {
            "Each fire arrives as a new turn in this conversation, and earlier results from\n\
             the same task may still be above it. The stored prompt is re-sent verbatim every\n\
             time, so write a standing order rather than a one-off request.\n\n\
             ## Writing a prompt that reads well on every fire\n\
             - Name the state that must not be guessed: paths, job/PR/branch ids, the command\n\
               that checks status, and what \"healthy\" looks like. This conversation is\n\
               compacted as it grows, so do not rely on details staying visible.\n\
             - Earlier fires may be above you: continue from them instead of restarting."
        }
    };
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

/// Canonical name of the image generation tool; upstream's `/imagine` gate.
///
/// Retained because the config resolver and the tool implementation still refer
/// to it, but it no longer gates any command: the tool calls an xAI endpoint
/// Chutes has no equivalent for. See [`GENERATE_MEDIA_TOOL_NAME`].
pub const IMAGE_GEN_TOOL_NAME: &str = "image_gen";

/// Canonical name of the Chutes media tool; gates `/imagine` and
/// `/imagine-video`.
///
/// One tool covers image, video, music and speech, because on Chutes the model
/// is a catalog entry rather than a fixed endpoint — which is also why the
/// instructions below teach a workflow instead of naming a model.
pub const GENERATE_MEDIA_TOOL_NAME: &str = "generate_media";

/// Advertised name of the /imagine command.
pub const IMAGINE_COMMAND_NAME: &str = "imagine";

/// Canonical name of the image-to-video tool; upstream's `/imagine-video` gate.
/// Kept for the same reason as [`IMAGE_GEN_TOOL_NAME`].
pub const IMAGE_TO_VIDEO_TOOL_NAME: &str = "image_to_video";

/// Advertised name of the /imagine-video command.
pub const IMAGINE_VIDEO_COMMAND_NAME: &str = "imagine-video";

/// Usage hint shown when `/imagine` is invoked with no arguments.
pub fn imagine_usage_message() -> &'static str {
    "Usage: /imagine <description>\n\
     Provide a text description to generate an image."
}

/// Build the model instruction that `/imagine` expands into for `prompt`.
pub fn imagine_instruction(prompt: &str) -> String {
    format!(
        "{IMAGINE_SKILL}\n\n\
         Prompt (pass verbatim — do not rewrite, embellish, or expand it): {prompt}"
    )
}

/// Image workflow injected by `/imagine`.
///
/// Chutes publishes a catalog, not one endpoint, and every model declares its own
/// invocation schema — so the payload cannot be guessed. `describe_media_model`
/// returns a ready-to-fill `example` per cord; starting from it is the difference
/// between one call and a series of validation errors.
const IMAGINE_SKILL: &str = "\
# Imagine

Generate the image with `generate_media`. Do not ask which model to use — pick one \
and go.

1. **Pick a model.** If one is already known to work for this kind of image, reuse \
it. Otherwise call `list_media_models` with `kind: \"image\"` and choose from the \
result.
2. **Describe it** with `describe_media_model` unless this session already did, \
for this exact model. Read the generation cord's `required` fields and its \
`example`.
3. **Generate** with `generate_media { model, kind: \"image\", params }`, building \
`params` from that example and placing the user's prompt in the field the cord \
declares for it. Keep the payload flat.
4. **Say where it landed** — report the returned `path` so the user can open it.

## Notes

- **Never invent field names.** Model schemas differ (FLUX, Qwen-Image and the \
rest share nothing); a guessed field is a wasted GPU call. If a call reports a \
schema mismatch, re-read the cord rather than trying variants.
- **Draft cheap, then finalise.** For an iteration, lower `num_inference_steps` \
and the dimensions; raise them once the prompt is right.
- **Cold start.** A `503 \"No instances available\"` means the model is scaling \
from zero; the tool already re-warms and retries. If it stays cold, pick another \
model of the same kind instead of retrying.
- **Editing** an existing image means passing its workspace path in the field the \
edit cord declares (often `image`, sometimes an array). The tool reads the file \
and encodes it, so an image generated moments ago can be edited by its path.";

/// Usage hint shown when `/imagine-video` is invoked with no arguments.
pub fn imagine_video_usage_message() -> &'static str {
    "Usage: /imagine-video <description>\n\
     Provide a text description to generate a video."
}

/// Build the model instruction that `/imagine-video` expands into for `prompt`.
pub fn imagine_video_instruction(prompt: &str) -> String {
    format!(
        "{IMAGINE_VIDEO_SKILL}\n\n\
         User prompt: {prompt}"
    )
}

/// Video workflow guidance injected by `/imagine-video`.
///
/// Deliberately different from upstream's, which is built on there being no
/// text-to-video tool. On Chutes that depends on the model you pick, so the
/// workflow starts by finding out.
const IMAGINE_VIDEO_SKILL: &str = "\
# Imagine Video

Generate video with `generate_media`. Do not ask which model to use — pick one and go.

## Find out what the model wants

1. `list_media_models` with `kind: \"video\"`. Some models take a prompt directly; \
others animate a starting image. Both exist — do not assume.
2. `describe_media_model` on the one you chose. The cord's `required` fields answer \
the question: a `prompt` field means text-to-video, an image field (`image`, \
`image_b64s`, `start_image`) means you must supply frame 1.
3. If it needs a starting frame, generate one first with `generate_media` \
`kind: \"image\"`, then pass that returned path in the field the video cord declares.

## Default: one clip

Unless the user asks for a longer or multi-shot video, make **one** clip: compose the \
payload from the cord's example, describe the motion or camera move in one or two \
present-tense sentences, and report the returned `path`.

## Longer / multi-shot videos

1. **Plan the story as shots** — break the idea into distinct shots, one beat each.
2. **Favor frequent, short shots** — more short clips over fewer long ones; cuts keep \
it dynamic.
3. **Keep continuity** — reuse the same model and the same style language for every \
shot, and keep characters and settings consistent.
4. **Assemble with FFmpeg** using stream copy (`ffmpeg -f concat ... -c copy` — never \
re-encode). Every shot must share resolution and frame rate for the concat to work. \
Report the final path.

## Shot guidance

- **Prompt-craft:** one short, vivid moment in present tense with a clear camera \
movement, in 1–2 sentences.
- **Minimal but interesting:** one subject, one motion or camera move per shot. Make \
it compelling through composition, lighting and timing rather than complex action.
- **Busy starting frame?** Intricate frames (fine detail, heavy reflections) warp when \
animated. Hold the subject and move only the camera, or generate a simpler base frame.
- **Duration** is whatever the cord's field allows — read it rather than assuming; \
values outside its range are rejected before the call is spent.
- **Draft cheap:** short duration and small dimensions while iterating, then re-run \
at quality.
- **Real people:** reference-first — drive the video from a verified reference image; \
never animate a named person without one.
- **Cold start:** a `503 \"No instances available\"` means the model is scaling from \
zero and the tool retries. If it stays cold, choose another video model.
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
        assert!(text.contains("image_gen"));
        assert!(text.contains("verbatim"));
    }

    #[test]
    fn imagine_video_instruction_carries_prompt_and_workflow() {
        let text = imagine_video_instruction("a cat playing piano");
        assert!(text.contains("a cat playing piano"));
        assert!(text.contains("image_to_video"));
        assert!(text.contains("FFmpeg"));
    }

    #[test]
    fn instruction_carries_args_and_contract_tokens() {
        for mode in [LoopFireMode::Detached, LoopFireMode::InSession] {
            let text = loop_schedule_instruction("every 30 minutes do x", mode);
            assert!(text.contains("every 30 minutes do x"), "{mode:?}");
            assert!(text.contains("<number><unit>"), "{mode:?}");
            assert!(text.contains("ask the user how often"), "{mode:?}");
            assert!(
                !text.contains("10m"),
                "no host-side default interval: {mode:?}"
            );
            assert!(
                !text.contains("recurring:"),
                "the retired one-shot flag must not be referenced: {mode:?}"
            );
            assert!(
                text.contains("task_id"),
                "must teach in-place updates via task_id: {mode:?}"
            );
            assert!(
                text.contains("delete and recreate"),
                "must steer away from delete+recreate: {mode:?}"
            );
            assert!(
                text.contains("scheduler_delete <task_id>"),
                "every mode must authorize the fire to end the task: {mode:?}"
            );
        }
    }

    #[test]
    fn each_fire_mode_describes_its_own_runtime() {
        let detached = loop_schedule_instruction("5m check ci", LoopFireMode::Detached);
        let in_session = loop_schedule_instruction("5m check ci", LoopFireMode::InSession);

        assert!(detached.contains("cannot see this conversation"));
        assert!(!detached.contains("arrives as a new turn in this conversation"));

        assert!(in_session.contains("arrives as a new turn in this conversation"));
        assert!(!in_session.contains("cannot see this conversation"));

        // The two levers the A/B showed carry the behavior are mode-independent.
        for text in [&detached, &in_session] {
            assert!(text.contains("report it and call"));
            assert!(text.contains("Keep it short and concrete"));
        }
    }

    #[test]
    fn goal_instruction_carries_objective_and_contract_tokens() {
        let text = goal_instruction("ship the widget");
        assert!(text.contains("ship the widget"));
        assert!(text.contains("update_goal(completed: true"));
        assert!(text.contains("blocked_reason"));
        assert!(text.contains("If update_goal returns an error"));
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
