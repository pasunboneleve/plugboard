# Codex to Gemini Workflow

This guide shows a topic-based request/reply flow using the bundled
`gemini-plugin`.

For the core system model, see [Architecture](../architecture.md). This
page stays at the workflow level.

## Prerequisites

You need:

* `cargo build` completed
* `target/debug` on your `PATH`
* Gemini CLI available as `gemini`, or `GEMINI_PLUGIN_CLI` set
* one working Gemini auth path

A useful baseline check is:

```bash
gemini --prompt 'how much is 5+4?' --output-format json --approval-mode plan
```

That is the same non-interactive mode used by `gemini-plugin`.

## Run the flow

Publish the request:

```bash
plugboard publish gemini.review.request "Review this Rust code for timeout bugs"
```

Start the worker:

```bash
timeout 320 plugboard run \
  --topic gemini.review.request \
  --success-topic gemini.review.done \
  --failure-topic gemini.review.failed \
  --timeout-seconds 300 \
  -- gemini-plugin
```

Read the reply:

```bash
plugboard read --topic gemini.review.done
```

If the worker times out first, Plugboard publishes a follow-up on
`gemini.review.request.timed_out`. Raise `--timeout-seconds` and try
again.

## What the adapter does

`gemini-plugin`:

* reads the Plugboard message body from `stdin`
* invokes Gemini CLI with `--prompt`, `--output-format json`, and
  `--approval-mode plan`
* extracts the `response` field from Gemini's JSON output
* writes the final text to `stdout`

Each claimed message starts one fresh Gemini process. There is no
persistent session managed by Plugboard.
