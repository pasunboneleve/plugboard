# Codex to Gemini Workflow

This guide shows a request/reply workflow using topic conventions.
Plugboard stays agent-agnostic: it only stores messages on topics.
Workers listen on those topics and connect them to a backend.

The example narrative is:

1. Codex publishes a request to `gemini.review.request`
2. a Gemini-oriented worker listens on that topic
3. the worker processes the message
4. the result is published to `gemini.review.done`
5. Codex reads the result from that topic

This guide uses the real `gemini-plugin` adapter in this repository.
That adapter shells out to the Gemini CLI once per message, keeps the
stdin to stdout contract intact, and exits after each response. It
reads the Plugboard message body from `stdin`, then invokes Gemini with
that body as `--prompt` plus `--output-format json` and
`--approval-mode plan`. It does not forward the plugin's stdin stream to
the Gemini subprocess.

## Prerequisites

Use the same prerequisites documented in
[Write a Worker Plugin](write-a-worker-plugin.md#real-gemini-adapter).
That section is the canonical checklist for the real `gemini-plugin`
adapter.

If the workflow fails or hangs, verify that the Gemini CLI itself can
complete a non-interactive request with your current auth and network
setup before debugging Plugboard. A useful baseline is:

```bash
gemini --prompt 'how much is 5+4?' --output-format json --approval-mode plan
```

That is the same one-shot Gemini mode used by `gemini-plugin` and
should return JSON with a `response` field.

## Runnable Example

Build the binaries and put them on your `PATH`:

```bash
cargo build
export PATH="$PWD/target/debug:$PATH"
```

Publish the request:

```bash
plugboard publish gemini.review.request "Review this Rust code for timeout bugs"
```

Start the worker host:

```bash
timeout 2 plugboard run \
  --topic gemini.review.request \
  --success-topic gemini.review.done \
  --failure-topic gemini.review.failed \
  -- gemini-plugin
```

Read the reply:

```bash
plugboard read --topic gemini.review.done
```

The exact reply text depends on Gemini, the configured model, and the
prompt you publish.

The response proves the pattern:

* Codex can send a request by publishing plain text to a topic
* a Gemini-oriented worker can listen on that topic
* the worker can hand the text to the Gemini adapter over stdin
* the adapter can turn that message into
  `gemini --prompt <message> --output-format json --approval-mode plan`
* the Gemini adapter can write the result to stdout
* Plugboard can publish the reply to a follow-up topic for Codex to read

## Execution Model

This workflow is stateless. Each message triggers a fresh backend
process, with no shared memory between runs and no persistent session
managed by Plugboard.

That is why the adapter uses a passive backend contract: read stdin,
write stdout, and exit. The repository's `gemini-plugin` preserves
that same per-message execution model.

## Choosing Topics

Plugboard does not route by built-in agent identity. Topic names
express intent and operating conventions.

For example:

* `gemini.review.request`
* `gemini.review.done`
* `gemini.review.failed`

These names are just conventions between participants. Plugboard
itself remains topic-based and agent-agnostic.

## Adapter Notes

The `gemini-plugin` binary invokes Gemini in non-interactive JSON mode
once per message, using the Plugboard message body itself as the
`--prompt` value. It then extracts the `response` field from Gemini's
JSON output for Plugboard to publish.

If your preferred Gemini setup is interactive or long-lived, wrap it
so the worker still sees the same non-interactive stdin to stdout
boundary.
