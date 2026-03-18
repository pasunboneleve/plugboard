# Write a Worker Plugin

## Overview

Plugboard is the core exchange. It stores messages, claims work, and
records follow-up messages. Worker hosts are the long-running
processes that listen on topics and execute plugins. Plugins define
how a claimed message is handled.

This keeps Plugboard unaware of the backend. It does not know whether
the work is handled by a shell command, an API call, or a wrapper
around another tool.

## Minimal Plugin Model

Conceptually, a plugin receives:

* the message body
* selected message metadata
* worker context such as timeout or worker name

And returns one of:

* success output
* failure output
* timeout, enforced by the worker host

Topic selection and message claiming stay in the worker host. The
plugin only handles execution.

## Execution Model

Workers are stateless per-message executors. Each claimed message
starts a fresh backend process, so there is no shared memory between
runs and no persistent session managed by Plugboard.

That makes the model a good fit for deterministic commands and
API-based adapters that can read one input, write one result, and
exit.

## Command Plugin

The current baseline plugin is the command plugin:

* spawn a subprocess
* write the message body to `stdin`
* close `stdin`
* capture `stdout` and `stderr`
* map the exit code into success or failure

This stdin to stdout contract is the default execution model because
it keeps the worker simple and Unix-like.

The repository includes a small example command-style plugin at
`src/bin/example-review-plugin.rs`. It is intentionally deterministic:
it reads the message body from `stdin`, prints a review-style response
to `stdout`, and exits successfully. That makes it a good reference
for the minimum useful worker plugin shape.

That binary is a demo plugin only. It proves the worker contract but
does not call Gemini or any other external backend.

## When You Need an Adapter

Some tools are not good worker plugins by themselves:

* interactive REPLs or TUIs
* commands that do not terminate
* commands that ignore `stdin` or expect prompts

In those cases, wrap the tool in a small adapter that presents a
non-interactive interface to Plugboard. The wrapper should accept the
message on `stdin`, run the awkward tool in a controlled way, emit a
single result, and exit.

## Future Plugin Types

Other plugin types can fit the same worker model:

* API-based plugins using an LLM SDK or HTTP client
* session-based plugins for long-lived tools
* wrappers around external tools that need adaptation

## Local Example Workflow

After `cargo build`, you can run the example plugin through Plugboard:

```bash
export PATH="$PWD/target/debug:$PATH"

plugboard publish review.request "Check timeout handling"

timeout 2 plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- example-review-plugin

plugboard read --topic review.done
```

This proves the full path: topic publication, worker claim, plugin
execution, and success follow-up publication.

For an agent-style request/reply narrative built on the same pattern,
see [Codex to Gemini Workflow](codex-to-gemini.md).

## Real Gemini Adapter

The repository also includes a real Gemini adapter binary at
`src/bin/gemini-plugin.rs`.

It uses the installed Gemini CLI in non-interactive JSON mode:

* reads the claimed message body from `stdin`
* invokes Gemini as `gemini --prompt <message body> --output-format json --approval-mode plan`
* does not forward plugin `stdin` to the Gemini subprocess
* extracts the response text from Gemini's JSON output
* writes the final response to `stdout`
* exits nonzero if Gemini returns an error

The worker host still enforces a per-message timeout around that plugin.
`plugboard run` defaults to 60 seconds, which may be too short for real
Gemini calls. Use `--timeout-seconds` when validating live workflows.

Prerequisites for the real adapter:

* Gemini CLI installed on `PATH` as `gemini`, or set
  `GEMINI_PLUGIN_CLI` to the executable path
* one working Gemini auth method:
  * `GEMINI_API_KEY`
  * `GOOGLE_GENAI_USE_VERTEXAI=true`
  * `GOOGLE_GENAI_USE_GCA=true`
  * or an authenticated Gemini CLI config in `~/.gemini/settings.json`
* optional model override via `GEMINI_PLUGIN_MODEL`

The simplest headless Gemini check is:

`gemini --prompt 'how much is 5+4?' --output-format json --approval-mode plan`

That confirms your local CLI can run the same one-shot Gemini mode used
by `gemini-plugin`.

## Local Ollama Adapter

The repository also includes a local-model adapter at
`src/bin/ollama-plugin.rs`.

It uses a local Ollama service rather than a hosted agent CLI:

* reads the claimed message body from `stdin`
* posts that text to `POST /api/generate` on a local Ollama service
* defaults to model `gemma3:1b`
* writes the final generated text to `stdout`
* exits nonzero if the local model backend returns an error

This is useful when you want Plugboard to feel responsive in local demos
or development loops.

Use these docs for the full workflow:

* [Install a Local Model Backend](install-local-model-backend.md)
* [Plugboard with a Local Model](plugboard-with-local-model.md)
* [Local Model Workflow](local-model-workflow.md)
