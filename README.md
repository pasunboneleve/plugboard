Plugboard
=========

**Plugboard** is a local textual exchange for cooperating programs.

It keeps the core small and Unix-like: publish text, read text, claim
work, and append follow-up messages. Plugboard does not define agents,
workflow graphs, or identity-based delivery. It routes interest by
topic and leaves behaviour to processes outside the core.

Getting Started
---------------

Plugboard is a textual exchange; workers listen on topics and process
messages using simple stdin/stdout contracts.

- [Quickstart](docs/quickstart.md)
- [Write a Worker Plugin](docs/howto/write-a-worker-plugin.md)

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
  non-interactive contract.

- **session**
  A future stateful plugin model, not required in v1.

## End-to-end example

1. Publish a request:

```text
topic: review.request
body:
Review this patch for correctness and missing tests.
```

2. Start a worker host for that topic:

```text
plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- some-review-plugin
```

3. The worker claims the message, runs the plugin, and publishes:

```text
topic: review.done
body:
Found one regression risk in timeout handling.
```

The follow-up keeps the conversation linked through `parent_id` and
`conversation_id`.

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
