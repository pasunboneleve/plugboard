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

This repository does not depend on a real Gemini integration. The
commands below use the local `example-review-plugin` binary to
demonstrate the same request/reply pattern that a real Gemini adapter
would follow.

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
  -- example-review-plugin
```

Read the reply:

```bash
plugboard read --topic gemini.review.done
```

The response proves the pattern:

* Codex can send a request by publishing plain text to a topic
* a Gemini-oriented worker can listen on that topic
* the worker can forward the text to a passive backend over stdin
* the backend can write a result to stdout
* Plugboard can publish the reply to a follow-up topic for Codex to read

## Choosing Topics

Plugboard does not route by built-in agent identity. Topic names
express intent and operating conventions.

For example:

* `gemini.review.request`
* `gemini.review.done`
* `gemini.review.failed`

These names are just conventions between participants. Plugboard
itself remains topic-based and agent-agnostic.

## Real Gemini Adapters

A real Gemini adapter would fit the same worker contract:

* receive the claimed message body
* transform it through Gemini or another backend
* write the final text result to stdout
* exit so the worker can publish the follow-up message

If the backend is interactive or long-lived, wrap it so the worker
still sees the same non-interactive stdin to stdout boundary.
