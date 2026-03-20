# Write a Worker Plugin

This guide focuses on the plugin boundary. For the core system model,
see [Architecture](../architecture.md).

## Baseline contract

The default worker plugin contract is:

* read the claimed message body from `stdin`
* write the result to `stdout`
* exit with `0` for success or non-zero for failure

`plugboard run` executes the backend once per claimed message.

## Minimal example

After `cargo build`, run the bundled demo plugin:

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

That proves the full path:

* publish to a topic
* claim the message
* run the plugin
* publish a follow-up

## When a wrapper is required

You usually need an adapter when the backend is:

* interactive
* long-lived and non-terminating
* not willing to read `stdin`
* not able to produce one bounded result and exit

The wrapper should preserve the Plugboard-side contract even if the
backend behind it is awkward.

## Repository examples

### `example-review-plugin`

Deterministic demo plugin for local testing.

### `gemini-plugin`

Reads the message body, invokes Gemini CLI in non-interactive JSON mode,
extracts the response text, and writes it to `stdout`.

Use [Codex to Gemini workflow](codex-to-gemini.md) for the end-to-end
path and its prerequisites.

### `ollama-plugin`

Reads the message body, calls a local Ollama service, and writes the
generated text to `stdout`.

Use these guides for the local-model path:

* [Install a local model backend](install-local-model-backend.md)
* [Plugboard with a local model](plugboard-with-local-model.md)
* [Local model workflow](local-model-workflow.md)
