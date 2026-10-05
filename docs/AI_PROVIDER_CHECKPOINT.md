# AI provider checkpoint

Revalidated against official provider documentation on 2026-10-05. Model IDs, endpoints, structured-output behavior, retention, and pricing are time-sensitive and must be checked again before a release.

## Shared boundary

- Every remote call originates in Rust through `reqwest`; no provider SDK or key is bundled into React.
- Every adapter receives the same minimized observation: at most 200 filtered on-screen elements, password names replaced with `[protected]`, and one PNG only when the user enabled screenshot transmission and the provider profile permits it.
- Every response must deserialize into the same deny-unknown-fields action schema and pass local target, field, verification, risk, and step-limit validation. Provider-side schema enforcement is never sufficient by itself.
- HTTP errors expose only provider name and status code. Response bodies, credentials, prompts, and target content are not logged.

## Implemented adapters

| Provider | Endpoint shape | Fast profile | Reasoning profile | Structured output |
| --- | --- | --- | --- | --- |
| Google Gemini | Interactions v1 | `gemini-3.8-flash` | `gemini-3.1-pro-preview` | Gemini JSON Schema response format |
| OpenCode Zen | `/zen/v1/responses` | `gpt-5.6-luna` | `gpt-6-astra` | OpenAI Responses `text.format` |
| OpenCode Go | `/zen/go/v1/chat/completions` or `/responses` | `glm-5.3-flash` | `gpt-5.6-luna` | JSON Schema response format; local validation remains authoritative |
| OpenRouter | `/api/v1/chat/completions` | `openai/gpt-5.6-luna` | `openai/gpt-5.6-sol` | `response_format.json_schema` with `require_parameters=true` |
| NVIDIA NIM | `/v1/chat/completions` | Qwen 3.5 122B, thinking off | Qwen 3.5 122B, thinking on | top-level `guided_json` |
| OpenAI | `/v1/responses` | `gpt-5.6-luna` | `gpt-5.6-sol` | Responses `text.format` JSON Schema |
| Anthropic | `/v1/messages` | `claude-haiku-4-5` | `claude-sonnet-5-5` | `output_config.format` JSON Schema |

OpenCode Go receives a stable `x-opencode-session` value for the captured process/window. Its documentation says clients should send coding-agent traffic, so the UI exposes that restriction and DeskFlow disables screenshot transmission for the current Go profiles.

Official references:

- [OpenCode Zen](https://dev.opencode.ai/docs/zen/) and [OpenCode Go](https://dev.opencode.ai/docs/go/)
- [OpenRouter structured outputs](https://openrouter.ai/docs/guides/features/structured-outputs)
- [NVIDIA NIM structured generation](https://docs.nvidia.com/nim/large-language-models/1.14.0/structured-generation.html)
- [OpenAI models](https://platform.openai.com/docs/models) and [Responses API](https://platform.openai.com/docs/api-reference/responses)
- [Anthropic models](https://platform.claude.com/docs/en/models/overview), [Messages API](https://platform.claude.com/docs/en/api/messages/create), and [structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)
- [Gemini models](https://ai.google.dev/gemini-api/docs/models) and [structured outputs](https://ai.google.dev/gemini-api/docs/structured-output)

## Credential boundary

- Each provider key is stored under `DeskFlow AI/provider/<provider_id>` as a Windows generic credential with local-machine persistence.
- Credential Manager takes precedence over an allowlisted native-process environment variable.
- The settings JSON stores only the selected provider/profile. Provider status returns only configured state and source.
- The masked frontend field is never prefilled and is cleared after every save attempt. Browser-only preview refuses save/delete commands.
- Saved keys are read only when constructing the selected native adapter and are never sent to another provider.

Do not put keys in localStorage, repository files, JavaScript bundles, logs, command output, tests, or documentation. Do not accept custom base URLs or arbitrary model IDs without a separate allowlist and SSRF review.
