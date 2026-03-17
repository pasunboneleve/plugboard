# Plugboard Architecture

## Purpose

This document narrows the first implementation of Plugboard.

`DESIGN.md` explains the philosophy and boundaries of the project.
This document defines a small concrete architecture for v1 so
implementation stays aligned with that design.

The goal is not to design the final system. The goal is to keep the
first version small, legible, and hard to overengineer.

## v1 scope

Plugboard v1 should support:

* publishing a textual message with a topic
* listing and reading messages
* claiming a message atomically for processing
* recording completion, failure, or timeout
* publishing follow-up messages
* running a configured plugin against claimed messages
* publishing the plugin output as a new message

Plugboard v1 does **not** need:

* network services
* push delivery
* distributed workers
* retries
* priorities
* dead-letter queues
* DAG orchestration
* rich filtering
* schema validation
* embedded business workflows

## High-level structure

Plugboard v1 has three layers:

## 1. Plugboard core exchange

The core owns:

* message storage
* claims
* state inspection
* atomic transitions

The core does **not** own:

* agent semantics
* workflow logic
* message interpretation
* plugin-specific policy
* identity-based delivery

Messages are routed by topic. Plugboard is agnostic to who or what
consumes them.

## 2. Worker host layer

A worker host is a long-running adapter runtime. It:

* polls for work on one topic
* claims a message
* invokes a plugin
* captures result
* appends follow-up messages

The worker host is a client of the exchange.

## 3. Plugin layer

Plugins implement actual behaviour. A plugin may wrap a command-line
tool, call an API, or adapt a local tool into a non-interactive
contract that the worker host can run safely.

The CLI remains the user-facing entrypoint:

* `plugboard publish`
* `plugboard read`
* `plugboard inspect`
* `plugboard run`

`plugboard run` should be understood as a worker host command.

## Storage backend

V1 uses **SQLite**.

Reasons:

* local single-file state
* atomic transactions
* no separate daemon
* easy to inspect
* easy to test
* reliable enough for local coordination

The SQLite database file can default to something like:

```text
.plugboard/plugboard.db
```

The exact path can be configurable later.

## Core entities

V1 should model only two durable things:

* messages
* claims

Do not introduce jobs, workflows, agents, executions, or subscriptions
as first-class database entities in v1.

## Message model

A message is an immutable textual record.

Suggested fields:

* `id`
* `topic`
* `body`
* `created_at`
* `parent_id`
* `conversation_id`
* `producer`
* `metadata_json`

Notes:

* `body` is the main payload.
* `parent_id` links a follow-up message to an earlier one.
* `conversation_id` groups related messages.
* `metadata_json` is optional and should remain shallow in v1.

Do not make metadata central to the model.

## Claim model

A claim is an operational record saying a worker host is processing a
message.

Suggested fields:

* `id`
* `message_id`
* `runner_name`
* `claimed_at`
* `lease_until`
* `status`
* `completed_at`

Where `status` is one of:

* `active`
* `completed`
* `failed`
* `timed_out`

Notes:

* claim state is separate from message content
* claims are about processing, not communication
* a message may have zero or one active claim in v1

If later you need richer execution state, add it later. Do not
prebuild it now.

## Suggested schema sketch

This is intentionally small.

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    topic TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    parent_id TEXT REFERENCES messages(id),
    conversation_id TEXT,
    producer TEXT,
    metadata_json TEXT
);

CREATE INDEX idx_messages_topic_created_at
    ON messages(topic, created_at);

CREATE INDEX idx_messages_conversation_id
    ON messages(conversation_id);

CREATE TABLE claims (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id),
    runner_name TEXT NOT NULL,
    claimed_at TEXT NOT NULL,
    lease_until TEXT NOT NULL,
    status TEXT NOT NULL,
    completed_at TEXT
);

CREATE UNIQUE INDEX idx_claims_active_message
    ON claims(message_id)
    WHERE status = 'active';

CREATE INDEX idx_claims_runner_status
    ON claims(runner_name, status);
```

This is enough for v1.

## Message lifecycle

V1 should keep the lifecycle simple.

## Publish

A producer appends a message to the exchange.

Required inputs:

* topic
* body

Optional inputs:

* parent id
* conversation id
* producer
* metadata

Publishing a message never modifies an existing message.

## Claim

A worker host asks the exchange for one unclaimed message matching a
topic.

The exchange should atomically:

* find a matching message
* ensure it has no active claim
* create an active claim row

This operation must be transactional.

## Complete

When processing succeeds, the worker host:

* marks the claim as `completed`
* may append a follow-up success message

Completion does not mutate the original message.

## Fail

When processing fails, the worker host:

* marks the claim as `failed`
* may append a follow-up failure message

Again, the original message remains unchanged.

## Timeout

If plugin execution exceeds the configured timeout, the worker host:

* terminates the command if possible
* marks the claim as `timed_out`
* may append a timeout message

Lease and timeout policy should remain simple in v1.

## How claiming should work

This is the most important bit to get right.

A worker host should not:

* list messages
* choose one in process memory
* then try to mark it later

That creates races.

Instead, claiming should happen in one transaction.

Conceptually:

1. select one eligible message for the topic
2. verify it has no active claim
3. insert an active claim
4. commit

Whether this is implemented with a subquery, CTE, or simple
transaction logic is not important yet. What matters is the semantics.

For v1, “eligible” can simply mean:

* message topic matches
* no active claim exists

Do not add priority, scheduling, or fairness rules yet.

## Worker Model

A worker host should be intentionally boring.

## Inputs

The worker host needs:

* a topic to watch
* a plugin to invoke
* a success topic
* a failure topic
* a timeout
* optionally a worker name

## Loop

The loop is:

1. try to claim one message for the configured topic
2. if none exists, sleep briefly and try again
3. run the configured plugin
4. pass the message body and selected metadata to the plugin
5. collect stdout, stderr, exit code
6. record claim outcome
7. append follow-up message

That is all.

Do not add concurrency to the worker host initially. One worker, one
message at a time is fine for v1.

## Simple plugin contract

V1 should choose one simple contract for how simple plugins receive
input and return output.

Recommended initial contract:

* pass message body to the plugin on `stdin`
* close stdin after writing
* treat `stdout` as success output
* treat non-zero exit as failure
* capture `stderr` for diagnostics

This is simple and Unix-like.

Later, alternative modes could be added, such as:

* file input
* arguments
* structured metadata passing

But not in v1.

## Plugin Model

A plugin conceptually receives:

* the claimed message body
* selected message metadata
* execution context from the worker host

And returns:

* success output
* failure output
* or a process state that the worker maps to timeout

Plugins must be:

* non-interactive
* terminating
* usable without the core understanding agent identity

Example plugin types:

* command plugin using stdin/stdout
* API-backed plugin using an SDK
* wrapper plugin for awkward CLIs such as `gemini`
* future session plugin for stateful backends

## Follow-up messages

Worker hosts should publish follow-up messages rather than mutating prior
ones.

Recommended default pattern:

Original message:

* topic: `code.generate`

Success follow-up:

* topic: `code.generated`
* parent_id: original message id

Failure follow-up:

* topic: `code.generate.failed`
* parent_id: original message id

Timeout follow-up:

* topic: `code.generate.timed_out`
* parent_id: original message id

This keeps the conversation visible in the message log.

## Conversation model

V1 should support lightweight message threading.

Recommended rule:

* if a published message has no `conversation_id`, assign its own id
  as the conversation id
* follow-up messages inherit the parent conversation id

This makes it easy to inspect all messages in one chain.

Do not introduce a separate conversations table in v1 unless it
becomes necessary.

## CLI sketch

This is only a sketch, but it is useful to anchor implementation.

## Publish

```text
plugboard publish TOPIC BODY
```

Optional flags could later include:

* `--parent`
* `--conversation`
* `--producer`
* `--meta`

## Read

```text
plugboard read --topic TOPIC
plugboard read --conversation ID
```

For v1, keep output simple and human-readable.

## Inspect

```text
plugboard inspect
plugboard inspect --message ID
plugboard inspect --conversation ID
```

This command should help debugging.

## Run

```text
plugboard run \
  --topic code.generate \
  --success-topic code.generated \
  --failure-topic code.generate.failed \
  --timeout-seconds 120 \
  -- codex exec
```

The command after `--` identifies the initial command plugin to invoke.

This command is one of the most important parts of the project because
it demonstrates how passive tools join the exchange through a worker
host and plugin boundary.

## Config shape for workers

V1 can support CLI-only worker configuration at first.

If file-based configuration is added, keep it very small.

Example TOML shape:

```toml
[[worker]]
name = "codex-codegen"
topic = "code.generate"
success_topic = "code.generated"
failure_topic = "code.generate.failed"
timeout_seconds = 120
plugin = { type = "command", command = ["codex", "exec"] }
```

This is enough. Do not invent a worker DSL.

## Inspection and debugging

Plugboard should be easy to inspect during development.

V1 should make it easy to answer:

* what messages exist?
* which messages are unclaimed?
* which claims are active?
* what happened in this conversation?
* what did this worker produce?

This is why:

* messages are immutable
* follow-ups reference parents
* claims are separate
* SQLite is used

A useful principle for v1:

**If something goes wrong, a user should be able to understand it by
reading messages and claims.**

## Testing strategy

The implementation should make testing easy by separating logic from
loops.

## Good units to test

* message creation
* conversation id propagation
* atomic claim logic
* claim completion and failure transitions
* follow-up message creation
* plugin result mapping into success/failure messages

## Keep thin wrappers thin

The CLI and worker loop should mostly do I/O:

* parse args
* call exchange methods
* sleep/poll
* run plugin
* print results

This means most behaviour can be tested below the event-loop level.

That is a feature of this architecture.

## Important constraints for implementation

When implementing v1, keep these guardrails in mind:

* do not introduce agent abstractions
* do not introduce workflow graphs
* do not parse message body for routing
* do not make metadata central
* do not add retries automatically
* do not add scheduling or priorities
* do not add network protocols
* do not make the worker host clever

Plugboard should feel smaller after implementation, not bigger.

## Recommended implementation order

1. create SQLite schema
2. implement `publish`
3. implement `read`
4. implement atomic `claim`
5. implement `complete` and `fail`
6. implement follow-up message creation
7. implement `run` as a worker host entrypoint
8. implement `inspect`

This order proves the model early and reduces the risk of abstraction
drift.

## Summary

Plugboard v1 should be a very small local system with:

* immutable textual messages
* separate operational claims
* topic-based routing
* a SQLite backend
* a minimal polling worker host
* follow-up messages for outcomes
* thin CLI commands over a small exchange API

## End-to-end example

```text
plugboard publish review.request "Review this code"

plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- my-review-plugin
```

The worker host claims one `review.request` message, runs the plugin,
and publishes a `review.done` follow-up if the plugin succeeds.

That is enough to prove the project’s core idea:

**independent programs can coordinate asynchronously through a local
textual exchange without sharing a framework or rigid typed
protocol.**
