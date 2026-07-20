//! Canonical slash-command wording (`/loop`, `/imagine`, `/imagine-video`, `/goal`),
//! shared by every front-end (Grok Build shell/pager and other hosts) so
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

/// Build the model instruction that `/loop` expands into for `args`.
///
/// The model, not brittle host parsing, turns the request into the
/// `scheduler_create` interval, accepting every natural phrasing and erroring
/// on bad input rather than silently defaulting. See [`loop_usage_message`].
pub fn loop_schedule_instruction(args: &str) -> String {
    format!(
        "# /loop -- schedule a recurring prompt\n\n\
         Parse the input below into an interval and a prompt, then schedule it with scheduler_create.\n\n\
         ## Deriving the interval\n\
         Read how often to run from the user's request — however they phrase it — and convert it\n\
         to a compact `<number><unit>` string, where unit is one of `s` (seconds), `m` (minutes),\n\
         `h` (hours), or `d` (days). The interval may appear at the start or end of the request;\n\
         extract it and use the remaining text as the prompt.\n\n\
         The minimum interval is 60 seconds; shorter values are raised to 60s, so tell the user if that applies.\n\n\
         If the request contains no interval at all, ask the user how often it should run before\n\
         scheduling. Do NOT invent or assume a default interval.\n\n\
         ## Action\n\
         1. Call scheduler_create with: interval (the compact string you derived), prompt,\n\
            recurring: true, fire_immediately: true. If the interval is unparseable, the tool\n\
            returns an error — fix the interval string rather than guessing.\n\
         2. Confirm: what's scheduled, the cadence, that it auto-expires after 7 days,\n\
            and that they can cancel with scheduler_delete (include the job ID).\n\
         3. Do NOT execute the prompt inline. The scheduler will fire it immediately.\n\n\
         ## Input\n\
         {args}"
    )
}

/// Canonical Chutes media tool name; gates `/imagine`.
pub const IMAGE_GEN_TOOL_NAME: &str = "generate_media";

/// Advertised name of the /imagine command.
pub const IMAGINE_COMMAND_NAME: &str = "imagine";

/// Canonical Chutes media tool name; gates `/imagine-video`.
pub const IMAGE_TO_VIDEO_TOOL_NAME: &str = "generate_media";

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
        "Use the Chutes media workflow for the user's image request. \
         Call list_media_models with kind=image, choose the best matching model, \
         call describe_media_model to obtain its exact schema, then call \
         generate_media with kind=image. Preserve the user's prompt verbatim in \
         the model's prompt field and do not invent unsupported parameters. After \
         generation, briefly acknowledge the result and mention the saved path.\n\n\
         Prompt: {prompt}"
    )
}

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
const IMAGINE_VIDEO_SKILL: &str = "\
# Chutes Video

## Default: single clip

Unless the user asks for a long video, multiple scenes, or a multi-shot sequence, \
generate **one** video:

1. Call `list_media_models` with `kind=video` and choose the best model for the request.
2. Call `describe_media_model` and follow its exact cord schema.
3. Call `generate_media` with `kind=video`, preserving the user's prompt and attaching \
workspace image references only when the selected schema supports them.
4. After completion, mention the saved file path.

## Longer / multi-shot videos

When the user requests a longer video, multiple scenes, or a narrative sequence:

1. **Plan the story as shots** — break the idea into distinct shots, one beat each.
2. **Favor frequent, short shots** — prefer more 6s clips over fewer long ones; more cuts keep it dynamic.
3. **Generate each shot** with the selected Chutes video model, using a shared visual bible and compatible parameters for consistency.
4. **Use reference images when supported** by the model's described schema.
5. **Assemble with FFmpeg** using stream copy (`ffmpeg -f concat ... -c copy` — never re-encode). \
Keep every shot at the same resolution and frame rate so the concat works. \
After assembly, mention the final output path.

## Shot guidance

- **Prompt-craft:** one short, vivid moment in present tense with a clear camera movement, in 1–2 sentences.
- **Minimal but interesting:** one clear subject, one simple motion or camera move per shot. Avoid complex multi-action animation; make the shot compelling through composition, lighting, and a strong moment.
- **Complex source image?** Intricate frames (busy geometry, fine detail, heavy reflections) warp when animated. Keep the subject fixed and move only the camera (slow push-in, orbit, or parallax), or break into simpler shots. For new shots, generate a simpler, animation-friendly base image rather than animating a busy one.
- **Schema first:** never guess cord names, duration ranges, aspect ratios, or reference fields.
- **Aspect ratio and duration:** follow the selected model's schema and keep them consistent across shots.
- **Real people:** reference-first — drive the video from a verified reference image; never animate a named person without one.
- Don't loop the same clip unless asked.";

pub const UPDATE_GOAL_TOOL_NAME: &str = "update_goal";

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
        assert!(text.contains("verbatim"));
    }

    #[test]
    fn imagine_video_instruction_carries_prompt_and_workflow() {
        let text = imagine_video_instruction("a cat playing piano");
        assert!(text.contains("a cat playing piano"));
        assert!(text.contains("generate_media"));
        assert!(text.contains("FFmpeg"));
    }

    #[test]
    fn instruction_carries_args_and_contract_tokens() {
        let text = loop_schedule_instruction("every 30 minutes do x");
        assert!(text.contains("every 30 minutes do x"));
        assert!(text.contains("<number><unit>"));
        assert!(text.contains("ask the user how often"));
        assert!(!text.contains("10m"), "no host-side default interval");
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
