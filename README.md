Plugboard
=========

[![cargo-test](https://github.com/pasunboneleve/plugboard/actions/workflows/cargo-test.yml/badge.svg)](https://github.com/pasunboneleve/plugboard/actions/workflows/cargo-test.yml)

**Plugboard** is a local textual exchange for cooperating programs.

It keeps the core small and Unix-like: publish text, read text, claim
work, and append follow-up messages. Plugboard does not define agents,
workflow graphs, or identity-based delivery. It routes interest by
topic and leaves behaviour to processes outside the core.

The main operating model is asynchronous:

1. publish or request work now
2. continue doing other work
3. later read replies from the relevant topic or conversation

That durable, inspectable exchange is the product. Blocking
request/reply is available for quick experiments, but it is not the
main thing Plugboard is for.

<figure style="margin: 1.5rem 0;">
  <a href="https://blog.sciencemuseum.org.uk/life-on-the-exchange-stories-from-the-hello-girls/" target="_blank" rel="noopener noreferrer" style="text-decoration:none;border:0;">
    <img src="docs/images/Archive-picture-of-operators-on-the-Enfield-switchboard-3-credit-Science-Museum-SSPL-low-res.jpg" alt="Archive photograph of operators on the Enfield switchboard" loading="lazy" style="display: block; width: 80%; height: auto; margin: 0 auto;" />
  </a>
  <figcaption style="width: 80%; margin: 0.5rem auto 0; text-align: center;">
    <strong>Figure 1.</strong> Archive photograph of operators on the Enfield switchboard.
    Source: Science Museum / SSPL.
  </figcaption>
</figure>

Getting Started
---------------

Plugboard is a textual exchange; workers listen on topics and process
messages using simple stdin/stdout contracts.

For a first local Ollama run, use two terminals.

Worker (must stay running):

```bash
./scripts/run-ollama-worker
```

Client:

```bash
./target/debug/plugboard request ollama.request \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --meta model=llama3.2:latest \
  --body "1+3"
```

`plugboard request` only publishes a message and waits for a reply; it
does not execute the backend itself.

For the non-blocking path, publish and capture identifiers immediately:

```bash
./target/debug/plugboard publish ollama.request "1+3" \
  --meta model=llama3.2:latest \
  --json
```

Then later check:

```bash
./target/debug/plugboard check \
  --conversation-id <conversation-id> \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --json
```

For prompt-level async usage, that conversation-based `check` path is
the intended meaning of “check Ollama.”

`./scripts/run-ollama-worker` starts a long-lived worker for
`ollama.request`. Keep it running in a separate terminal while sending
requests.

In the async path, structured output is for the agent or tool to parse.
The human-facing response should stay plain text, for example:

* `Sent to Ollama.`
* `Conversation ID: <id>`

For a single-message experiment, you can use the one-shot helper
instead:

```bash
./scripts/run-ollama-worker-once
```

That worker processes one matching message and exits.

WARNING: If no worker is running for a topic, requests will not be
processed and may appear to hang.

- [Quickstart](docs/quickstart.md)
- [Plugin Backend Options](docs/plugin-backends.md)
- [Install a Local Model Backend](docs/howto/install-local-model-backend.md)
- [Plugboard with a Local Model](docs/howto/plugboard-with-local-model.md)
- [Async Inbox Workflow](docs/howto/async-inbox.md)
- [Completion Notifications](docs/howto/completion-notifications.md)
- [Local Model Workflow](docs/howto/local-model-workflow.md)
- [Measure Local Latency](docs/howto/measure-latency.md)
- [Write a Worker Plugin](docs/howto/write-a-worker-plugin.md)
- [Codex to Gemini Workflow](docs/howto/codex-to-gemini.md)

The repository includes a deterministic demo plugin, a real Gemini
adapter, and a local-model adapter built around Ollama for low-latency
bounded text transforms on a developer machine.

Operationally, blocking worker and request/reply paths use advisory
local wakeups plus bounded SQLite re-checks. The default notifier wait
timeout is 250 ms, and the no-notifier periodic fallback interval is
also 250 ms. Targeted debug visibility is available with
`RUST_LOG=debug`. The main tuning knobs are
`plugboard run --wait-timeout-ms --idle-sleep-ms` and
`plugboard request --wait-timeout-ms --recheck-ms`.

Plugboard can be useful with several backend styles: simple stateless
transforms, low-latency local model plugins, plugins that talk to
already-running agents or warm backends, and direct API plugins for
hosted services. Those are plugin-layer choices rather than protocol
changes in the core exchange.

Plugboard also includes a thin request/reply helper for quick manual
experiments:

```text
plugboard request --topic ... --success-topic ... --failure-topic ... --body ...
```

It publishes a request, waits for the first correlated follow-up in the
same conversation, prints the reply body, and exits. Treat that as a
convenience wrapper around the asynchronous exchange, not as the main
workflow to optimize your mental model around.

That gives Plugboard two explicit operator paths:

* blocking: `request`
* non-blocking: `publish` now, `check` later

On publish, `plugboard request` also emits the request `message_id` and
`conversation_id` on `stderr` in a stable format. Agents should capture
those identifiers and later use `conversation_id` to check the exact
request/reply thread. When parse reliability matters, prefer
`plugboard request --json`.

Worker lifecycle
----------------

Plugboard uses a queue/worker model.

`plugboard run` starts a worker that listens on a topic, claims
matching messages, runs a backend, and publishes follow-up messages.
Workers are typically long-lived processes.

If no worker is running for a topic, requests will queue and
`plugboard request` will wait indefinitely.

Persistent worker mode (recommended):

* start a worker once
* keep it running in a separate terminal
* reuse it for many requests

One-shot worker mode:

* start a worker with `plugboard run --once`
* or use `./scripts/run-ollama-worker-once` for the Ollama demo topic
* process a single matching message
* exit after that one message

Async-First Usage
-----------------

The most useful Plugboard workflow is:

1. enqueue work on a topic
2. leave the worker alone to process it
3. come back later and read replies

For humans, that means you can send work, move on, and check your inbox
later.

For agent/tool use, that means the safe default is also non-blocking:
enqueue now, keep working, and only wait in the foreground when the user
explicitly asks for it.

`plugboard request` exists because it is handy during dogfooding and
small demos. But the more characteristic Plugboard flow is:

```text
publish/request -> do other work -> read replies later
```

That is different from foreground-parallel tools that fan out work but
still expect the user to sit in the same shell waiting for completion.
Plugboard's durable exchange and later inspection are part of the
product value.

Foreground parallel tools are still useful. They are just solving a
different problem: do more work at once while the user waits. Plugboard
is for leaving work in a durable local exchange and coming back later.

Read vs Inspect
---------------

Use `plugboard read` for normal usage. It is the routine way to consume
messages from a topic or conversation, especially when you are checking
for replies later.

Use `plugboard inspect` when you need forensic detail about raw message
history or claim state. It can print a lot of historical output on a
non-empty database, so it is best treated as a debugging command. For
experiments, prefer using a temporary database so the output stays
focused.

Use `plugboard request` or `plugboard publish` to enqueue work. Use
`plugboard read` to come back and see what happened. Use
`plugboard inspect` only when the normal story is not enough.

For the common Ollama demo path, `./scripts/check-ollama` is a separate
inbox helper that shows recent replies from `ollama.done` and
`ollama.failed` together. By default it shows the 10 most recent
replies, formats timestamps for local human reading, and is safe to run
repeatedly.

That is different from prompt-level `check ollama`, which should use
the stored `conversation_id` from the most recent async send and run:

```bash
./scripts/check-ollama-conversation <conversation-id>
```

If no stored async conversation is available, say so plainly rather
than falling back to recent replies by default.

For agent use, the preferred async tracking path is:

1. send a request
2. capture `message_id` and `conversation_id`
3. later check:

   ```bash
   ./scripts/check-ollama-conversation <conversation-id>
   ```

`conversation_id` is the primary retrieval handle. Topics are for
routing; the conversation is the natural unit of meaning for one
request/reply exchange.

For Ollama prompt-level usage, prefer the helper-layer script above.
Reserve raw `plugboard check` for low-level/manual/debugging workflows.

`plugboard check` itself is a thin helper over conversation-based reads.
It tells you whether the conversation is still pending, has a terminal
success reply, or has a terminal failure reply.

When using `--json`, the intended pattern is:

* parse JSON internally
* return plain text to the human

For example:

* `Not yet.`
* `Yes — Albert Einstein.`
* `It failed: <failure body>`

If those identifiers are unavailable, fall back to matching the request
body text, preferring the latest plausible request, but that is a
heuristic and less reliable than using the IDs Plugboard already
returns.

Troubleshooting
---------------

Problem: `plugboard request` hangs

Likely causes:

* No worker is running
* The worker is listening on a different topic

How to verify:

* Start the worker in a separate terminal:

  ```bash
  ./scripts/run-ollama-worker
  ```

* Retry the request

Old messages on `ollama.done` do not prove that the current request was
processed. `plugboard request` waits for a reply in the same
conversation, not just any older success message on the topic.

Three-layer model
-----------------

Plugboard is easiest to understand as three layers:

1. **Plugboard core exchange**
   Stores messages, supports topic-based reads and claims, and records
   follow-up messages and claim outcomes.

2. **Worker host**
   A long-running adapter process that polls topics, claims one
   message at a time, executes work, and publishes success, failure,
   or timeout follow-ups.

3. **Plugins**
   Execution backends used by the worker host. A plugin may wrap a
   command-line tool, call an API, or adapt an awkward local tool into
   a simple non-interactive contract.

Those plugins can take several practical forms:

* **simple stateless transforms**
  One message triggers one process. Best for shell filters,
  deterministic tools, and small adapters.

* **local model plugins**
  Good for fast local demos and development when hosted agent cold
  start is too slow. Small local models are best for bounded,
  low-risk text transforms rather than ambitious open-ended tasks.

* **already-running agent or session-backed plugins**
  Useful when the plugin needs to talk to a warm backend. Any
  persistence belongs in the plugin layer, not Plugboard core.

* **API plugins**
  Call hosted models or external services directly while preserving
  the same topic-based exchange pattern.

The core remains agnostic to who reads a message. Delivery is
topic-based, not identity-based. Agent behaviour lives entirely in the
worker host and its plugins.

## Why
Many automation and AI systems rely on tightly coupled frameworks:

- shared SDKs
- rigid schemas
- centralized orchestration
- strongly typed RPC between services

These approaches couple tools together and make systems harder to
evolve.

Unix took a different path: small programs communicating through text.
Plugboard applies that idea to asynchronous coordination between
independent tools.

## Design goals

- **Local-first**
  Designed to run on a single machine.

- **Text-first**
  Messages are textual. Participants decide how to interpret them.

- **Minimal core**
  Plugboard manages message lifecycle, not agent semantics.

- **Decoupled participants**
  Tools do not need to share a framework or runtime.

- **Inspectable system**
  Users should be able to understand activity by reading messages and
  claims.

## Non-goals

Plugboard is not:

- an agent framework
- a workflow orchestration engine
- a typed RPC system
- a vendor-specific AI runtime
- a distributed task scheduler

Programs remain ordinary processes outside the exchange.

## Conceptual model

Messages are published into a local exchange. Worker hosts watch
topics, claim matching messages, run plugins, and append follow-up
messages.

```text
publisher -> Plugboard topic -> worker host -> plugin -> follow-up topic
```

The exchange manages:

- message storage
- topic-based visibility
- atomic claiming
- claim completion, failure, and timeout state
- follow-up message history

It does not define what a reviewer, coder, planner, or agent is.

## Worker Model

The worker host is a long-running process.

Its loop is intentionally small:

1. claim one message for a configured topic
2. if none exists, sleep briefly and try again
3. execute a plugin for that message
4. publish a success, failure, or timeout follow-up
5. update the claim state

V1 should process one message at a time.

For simple command plugins, the worker uses a stdin/stdout contract:

- message body is written to plugin stdin
- stdin is then closed
- stdout is captured as success output
- stderr is captured for diagnostics
- non-zero exit is treated as failure

Each worker configuration also defines:

- the watched topic
- success topic
- failure topic
- timeout duration
- optional worker name

Timeouts publish to `<topic>.timed_out`.

## Plugin Model

A plugin defines the actual execution behaviour behind a worker.

Conceptually, a plugin receives:

- the input message body
- selected message metadata
- worker execution context such as timeout or plugin name

And returns either:

- a success result
- a failure result
- or a timeout outcome enforced by the worker host

Plugins in v1 should be:

- non-interactive
- bounded
- terminating

Example plugin types:

- **command**
  Wraps a local CLI using stdin/stdout.

- **API**
  Calls an SDK or remote LLM API and returns textual output.

- **wrapper**
  Adapts an awkward CLI such as `gemini` into a clean
  non-interactive contract, while still accepting the worker's message
  body on `stdin`.

- **session**
  A future stateful plugin model, not required in v1.

## End-to-end example

The most representative flow is asynchronous:

1. Publish a request:

```text
topic: review.request
body:
Review this patch for correctness and missing tests.
```

2. Ensure a worker host is running for that topic:

```text
plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- some-review-plugin
```

3. Continue doing other work, then later read the reply topic:

```text
plugboard read --topic review.done
```

4. The worker claims the message, runs the plugin, and publishes:

```text
topic: review.done
body:
Found one regression risk in timeout handling.
```

The follow-up keeps the conversation linked through `parent_id` and
`conversation_id`. That durable history is why Plugboard is useful even
when nobody is sitting in the shell waiting.

## CLI sketch

```text
plugboard publish TOPIC BODY
plugboard read --topic TOPIC
plugboard inspect
plugboard run --topic TOPIC --success-topic OK --failure-topic FAIL -- plugin
```

`plugboard run` should be understood as a worker host entrypoint. It
continuously claims matching messages, invokes its configured plugin,
and publishes follow-up messages.

## Status

Early implementation stage.

## Licence

MIT
