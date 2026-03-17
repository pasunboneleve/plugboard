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
stdin to stdout contract intact, and exits after each response.

## Prerequisites

Before running this workflow, you need:

* Gemini CLI installed on `PATH` as `gemini`, or set
  `GEMINI_PLUGIN_CLI` to its path
* one working Gemini auth method:
  * `GEMINI_API_KEY`
  * `GOOGLE_GENAI_USE_VERTEXAI=true`
  * `GOOGLE_GENAI_USE_GCA=true`
  * or an authenticated Gemini CLI config in `~/.gemini/settings.json`
* optional model override via `GEMINI_PLUGIN_MODEL`

If the workflow fails or hangs, verify that the Gemini CLI itself can
complete a non-interactive request with your current auth and network
setup before debugging Plugboard.

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
* the worker can forward the text to a passive backend over stdin
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
once per message and extracts the `response` field for Plugboard to
publish.

If your preferred Gemini setup is interactive or long-lived, wrap it
so the worker still sees the same non-interactive stdin to stdout
boundary.
