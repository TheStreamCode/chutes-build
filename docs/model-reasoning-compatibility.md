# Model reasoning compatibility

Chutes Build exposes reasoning controls only when the deployed model's published
chat template supports them. A model advertising `reasoning` does not imply that
its effort can be changed.

Verified against the Chutes catalog and upstream Hugging Face artifacts on
2026-07-19.

| Chutes model | Upstream control | Chutes Build choices |
| --- | --- | --- |
| `deepseek-ai/DeepSeek-V3.2-TEE` | The [official encoder](https://huggingface.co/deepseek-ai/DeepSeek-V3.2/tree/main/encoding) defines a binary `thinking_mode`; Chutes accepts the template switch. | Instant, Thinking (default) |
| `google/gemma-4-31B-turbo-TEE` | The [Gemma 4 template](https://huggingface.co/google/gemma-4-31b-it/blob/main/chat_template.jinja) uses `enable_thinking` and defaults it off. | Instant (default), Thinking |
| `MiniMaxAI/MiniMax-M2.5-TEE` | The [MiniMax M2.5 template](https://huggingface.co/MiniMaxAI/MiniMax-M2.5/blob/main/chat_template.jinja) always opens a thinking block for generation and publishes no disable switch. | Fixed thinking; no selector |
| `moonshotai/Kimi-K2.5-TEE` | The [Kimi K2.5 template](https://huggingface.co/moonshotai/Kimi-K2.5/blob/main/chat_template.jinja) uses the binary `thinking` switch. | Instant, Thinking (default) |
| `moonshotai/Kimi-K2.6-TEE` | The [Kimi K2.6 template](https://huggingface.co/moonshotai/Kimi-K2.6/blob/main/chat_template.jinja) uses `thinking` and also supports preserving prior thinking. | Instant, Thinking (default) |
| `Qwen/Qwen3-235B-A22B-Thinking-2507-TEE` | The [official model card](https://huggingface.co/Qwen/Qwen3-235B-A22B-Thinking-2507) identifies a thinking-only release. | Fixed thinking; no selector |
| `Qwen/Qwen3-32B-TEE` | The [official model card](https://huggingface.co/Qwen/Qwen3-32B#switching-between-thinking-and-non-thinking-mode) documents `enable_thinking`, on by default. | Instant, Thinking (default) |
| `Qwen/Qwen3.5-397B-A17B-TEE` | The [Qwen3.5 template](https://huggingface.co/Qwen/Qwen3.5-397B-A17B/blob/main/chat_template.jinja) implements `enable_thinking`. | Instant, Thinking (default) |
| `Qwen/Qwen3.6-27B-TEE` | The [Qwen3.6 model card](https://huggingface.co/Qwen/Qwen3.6-27B#instruct-or-non-thinking-mode) documents `chat_template_kwargs.enable_thinking`; thinking is the default. | Instant, Thinking (default) |
| `unsloth/Mistral-Nemo-Instruct-2407-TEE` | The [Mistral Nemo Instruct card](https://huggingface.co/mistralai/Mistral-Nemo-Instruct-2407) does not publish a reasoning mode. | No selector |
| `zai-org/GLM-5-TEE` | The [GLM-5 template](https://huggingface.co/zai-org/GLM-5/blob/main/chat_template.jinja) uses `enable_thinking`. | Instant, Thinking (default) |
| `zai-org/GLM-5.1-TEE` | The [GLM-5.1 template](https://huggingface.co/zai-org/GLM-5.1/blob/main/chat_template.jinja) uses `enable_thinking`. | Instant, Thinking (default) |
| `zai-org/GLM-5.2-TEE` | The [GLM-5.2 template](https://huggingface.co/zai-org/GLM-5.2/blob/main/chat_template.jinja) supports `enable_thinking` plus `high`/`max` effective effort. | Instant, Fast reasoning (default), Maximum reasoning |
| `model-router` | The target model varies per task, so a model-specific wire control would be unsafe. | No selector; Chutes routes the task |

## Compatibility precedence

1. An explicit `reasoning_efforts` menu returned by Chutes or configured for a
   model wins over bundled defaults.
2. Otherwise the centralized registry in
   `crates/chutes-build-core/src/reasoning.rs` supplies controls verified
   against the exact published generation.
3. Unknown future generations do not inherit controls from a broad provider
   prefix. They keep explicit catalog values when present and otherwise hide
   the selector, preventing invalid or silently ignored request fields.
4. The sampler translates the UI vocabulary into the model's native template
   key. In particular, GLM-5.2 Maximum is sent with the gateway-compatible
   scalar that its template maps to `max`; the rejected literal `xhigh` is never
   sent.

## Auto routing

`Auto (Chutes Router)` is a virtual entry at the top of the model picker. It
sends `model-router` requests to the Chutes native router endpoint, which owns
task classification, model selection, and cold/unavailable fallback. Selecting
a concrete model still pins that model. Auto intentionally exposes no reasoning
selector because the routed target can vary between requests.

## User controls

- Run `/model` to choose Auto or a concrete model. Models with configurable
  reasoning present a second, model-specific choice.
- Run `/effort` to change the active concrete model without reopening the model
  picker.
- In headless mode, use `--model <model-id>` and `--effort <option-id>`. Chutes
  model option IDs are `none`/`high` for binary modes and
  `none`/`high`/`xhigh` for GLM-5.2.

`Instant` is always explicit. Defaults track the published model behavior so a
latency optimization never silently disables reasoning.
