# Plugboard with a Local Model

This guide shows the full Plugboard workflow with a local model backend.

Current path:

* local runtime: Ollama
* default model: `gemma3:1b`
* Plugboard adapter: `ollama-plugin`

Use this when you want a real model-backed request/reply loop without
waiting on a hosted agent CLI cold start.

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
plugboard publish local.review.request "Explain Rust ownership in one short paragraph."
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
the same.

## Model selection

By default, `ollama-plugin` uses `gemma3:1b`.

To use a different locally available model:

```bash
OLLAMA_PLUGIN_MODEL=qwen2.5:1.5b \
timeout 30 plugboard run \
  --topic local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --timeout-seconds 20 \
  -- ollama-plugin
```

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
