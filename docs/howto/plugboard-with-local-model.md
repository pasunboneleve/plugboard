# Plugboard with a Local Model

This guide shows the full Plugboard workflow with a local model backend.

Current path:

* local runtime: Ollama
* default model: `gemma3:1b`
* Plugboard adapter: `ollama-plugin`

Use this when you want a real local model-backed workflow without
waiting on a hosted agent CLI cold start.

Treat small local models as useful bounded transforms, not as general
stand-ins for larger hosted systems. Good uses here include:

* short rewrites
* summaries
* classification or labeling
* bounded critique or transformation tasks

## Before you begin

Complete the setup in
[Install a Local Model Backend](install-local-model-backend.md).

You should already have:

* `ollama serve` running
* `gemma3:1b` pulled locally
* `ollama-plugin` built successfully

## Build the binaries

```bash
cargo build
export PATH="$PWD/target/debug:$PATH"
```

## Publish a request

```bash
plugboard publish local.review.request "Summarize this code review in one short paragraph."
```

## Run the worker

```bash
timeout 30 plugboard run \
  --topic local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --timeout-seconds 20 \
  -- ollama-plugin
```

This works because:

* Plugboard gives the claimed message body to `ollama-plugin` on `stdin`
* `ollama-plugin` posts that text to the local Ollama service
* the plugin prints the final model reply to `stdout`
* Plugboard publishes that stdout text to `local.review.done`

## Read the reply

```bash
plugboard read --topic local.review.done
```

Expected shape:

```text
2026-03-18T...Z	local.review.done	<local model reply>
```

The exact text depends on your local model, but the workflow should stay
the same. Keep prompts narrow enough that a small local model is likely
to produce something useful.

## Model selection

By default, `ollama-plugin` uses the official Ollama model tag
`gemma3:1b`.

To use a different locally available model:

```bash
OLLAMA_PLUGIN_MODEL=qwen2:1.5b \
timeout 30 plugboard run \
  --topic local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --timeout-seconds 20 \
  -- ollama-plugin
```

## Dogfooding with Ollama (model override)

Per-request metadata can override the model without changing plugin
stdin or restarting the worker.

Default request:

```bash
plugboard request local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --body "Rewrite this error message in calmer language."
```

Request with metadata override:

```bash
plugboard request local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --meta model=llama3.2:3b \
  --meta temperature=0.7 \
  --body "Classify this paragraph as bug report, feature request, or question."
```

How it works:

* request metadata is stored in `messages.metadata_json.meta`
* the worker passes `.meta` entries to plugins as `PLUGBOARD_META_*`
  environment variables
* plugins opt in to using those variables
* `ollama-plugin` prefers `PLUGBOARD_META_MODEL` over
  `OLLAMA_PLUGIN_MODEL`

## Troubleshooting

If the worker publishes to `local.review.failed`, check:

* `ollama serve` is still running
* the model is already pulled locally
* the selected model name is valid

If the worker publishes to `local.review.request.timed_out`, increase
`--timeout-seconds` and try again.

## Short walkthrough

For a more compact publish to worker to read example, see
[Local Model Workflow](local-model-workflow.md).
