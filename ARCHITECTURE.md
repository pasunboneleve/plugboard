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
* waiting for work in either persistent or reactive worker mode

Plugboard v1 does **not** need:

* network services
* distributed workers
* retries
* priorities
* dead-letter queues
* DAG orchestration
* rich filtering
* schema validation
* embedded business workflows

Plugboard v1 also does **not** treat polling as the preferred model
for passive tools. Persistent and reactive execution are distinct
worker modes and should be implemented as such.

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
* participant activation policy

Messages are routed by topic. Plugboard is agnostic to who or what
consumes them.

Agent-style workflows still use this same rule. A participant might
publish to `gemini.review.request` and read from
`gemini.review.done`, but those names are topic conventions rather
than built-in identity routing.

## 2. Worker host layer

A worker host is an adapter runtime. It:

* waits for work on one topic
* claims a message
* invokes a plugin
* captures result
* appends follow-up messages

The worker host is a client of the exchange.

A worker host may run in one of two modes:

* persistent mode
* reactive one-shot mode

These are lifecycle choices at the edge, not changes to the exchange
model.

## 3. Plugin layer

Plugins implement actual behaviour. A plugin may wrap a command-line
tool, call an API, or adapt a local tool into a non-interactive
contract that the worker host can run safely.

## Backend execution models

The worker host and plugin layer can support several practical backend
styles without changing Plugboard core:

* **simple stateless transforms**
  One claimed message starts one process. The worker writes the body to
  `stdin`, the plugin returns text on `stdout`, and the process exits.

* **local model plugins**
  A plugin talks to a local inference engine or service. This is useful
  for fast demos and day-to-day development when hosted agent cold
  start is too slow.

* **already-running agent or session-backed plugins**
  A plugin may talk to a warm backend or long-lived local agent. Any
  persistence stays in the plugin layer; the exchange still sees only
  topic-based messages, claims, and follow-ups.

* **API plugins**
  A plugin can call a hosted API directly and still return a textual
  result through the same worker lifecycle.

These are backend alternatives, not protocol changes. Plugboard still
does not manage identity, presence, routing, or sessions.

The CLI remains the user-facing entrypoint:

* `plugboard publish`
* `plugboard read`
* `plugboard inspect`
* `plugboard run`
* `plugboard request`

`plugboard run` should be understood as a worker host command.
`plugboard request` is a request/reply helper at the edge, not a new
core abstraction.

## Storage backend

V1 uses **SQLite**.

Reasons:

* local single-file state
* atomic transactions
* no separate service
* easy to inspect
* easy to test
* reliable enough for local coordination

The SQLite database file can default to something like:

```text
.plugboard/plugboard.db
```

The exact path can be configurable later.

SQLite remains the source of truth for:

* messages
* claims
* follow-up history

Wakeup signals, if present, are advisory only. They do not replace the
database and they do not grant ownership. Ownership still comes only
from a successful transactional claim.

## Core entities

V1 should model only two durable things:

* messages
* claims

Do not introduce jobs, workflows, agents, executions, subscriptions,
or notifier state as first-class database entities in v1.

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
* `worker_group`
* `worker_instance_id`
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
* `worker_group` is the stable logical worker class or configuration
* `worker_instance_id` is a fresh per-process identifier for one concrete worker instance
* a claim is live only while `status = 'active'` and `lease_until` is still in the future
* expired active claims are discarded in the claim path itself in v1
* terminal claims remain as processing history and continue to make a message non-claimable in v1
* a message may have zero or one live active claim in v1

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
    worker_group TEXT NOT NULL,
    worker_instance_id TEXT NOT NULL,
    claimed_at TEXT NOT NULL,
    lease_until TEXT NOT NULL,
    status TEXT NOT NULL,
    completed_at TEXT
);

CREATE UNIQUE INDEX idx_claims_active_message
    ON claims(message_id)
    WHERE status = 'active';

CREATE INDEX idx_claims_message_id
    ON claims(message_id);

CREATE INDEX idx_claims_message_status_lease
    ON claims(message_id, status, lease_until);

CREATE INDEX idx_claims_worker_group_status
    ON claims(worker_group, status);
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

After a successful publish commit, the implementation may emit a local
wakeup signal so waiting workers can re-attempt claim promptly.

That wakeup is a hint only. The durable system of record remains
SQLite.

## Claim

A worker host asks the exchange for one unclaimed message matching a
topic.

The exchange should atomically:

* find a matching message
* ensure it has no live active claim and no terminal processing record
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
2. verify it has no live active claim and has not already completed, failed, or timed out
3. insert an active claim
4. commit

Whether this is implemented with a subquery, CTE, or simple
transaction logic is not important yet. What matters is the semantics.

For v1, “eligible” can simply mean:

* message topic matches
* no live active claim exists
* no terminal claim row exists

Do not add priority, scheduling, or fairness rules yet.

The claim query should treat expired active claims as stale ownership
records and discard them transactionally before deciding eligibility.
Terminal claims remain as processing history and still block replay in
v1.

## Activation and wakeup model

This is the main architectural distinction in v1.

Plugboard separates:

* durable exchange state
* worker lifecycle
* wakeup mechanics

For a clarification of how blocking waits relate to asynchronous
exchange semantics, see “Asynchronous vs blocking” in [DESIGN.md](./DESIGN.md).

### Exchange state

The exchange stores messages and claims in SQLite.

### Worker lifecycle

Workers are external processes. The exchange does not own their
lifetime.

### Wakeup mechanics

Wakeups are advisory signals that tell workers it is worth retrying a
claim. They do not assign messages and they do not replace the claim
transaction.

## Local wakeup mechanism

V1 should support a local wakeup mechanism for workers that want to
block without polling.

The wakeup mechanism should satisfy these rules:

* it is local to one machine
* it is tied to one exchange database
* a publish may coincide with a local wakeup hint, but correctness must not depend on it
* waiting workers wake and retry `claim_next()`
* correctness does not depend on delivery of the wakeup itself

A suitable initial abstraction is:

* `Notifier::notify(topic)`
* `Waiter::wait(topic)`

The implementation may begin with a simple local mechanism and change
later without changing the exchange model.

Possible implementations include:

* Unix domain socket notifier
* filesystem notification on database or WAL changes

For v1, the preferred direction is a local notifier abstraction plus a
bounded periodic re-check, rather than trusting filesystem
notifications as a correctness mechanism.

## Worker modes

V1 should support two worker modes.

## Persistent mode

Persistent mode is for long-running workers.

A persistent worker:

* starts once
* waits for matching work repeatedly
* handles messages one at a time
* remains alive after processing

Persistent mode may use polling or blocking internally.

Polling is acceptable here because the process is intentionally
resident.

## Reactive one-shot mode

Reactive mode is for passive tools and request/reply workflows.

A reactive worker:

* starts
* waits for one matching message without polling
* claims exactly one message
* runs one plugin execution
* publishes follow-up
* exits immediately

Reactive mode should use blocking wait plus transactional claim.

Blocking is an optimization over polling, not a correctness
mechanism. Reactive mode may still use bounded re-check intervals
around the blocking wait so missed advisory wakeups do not strand a
worker forever.

## Worker host API shape

The worker host should expose two distinct execution paths:

* `run_forever()`
* `run_once_blocking()`

Suggested semantics:

### `run_forever()`

* wait for matching work
* claim one message
* execute plugin
* record outcome
* publish follow-up
* repeat

### `run_once_blocking()`

* try immediate claim once
* if none, block waiting for wakeup
* retry claim after wake
* execute plugin
* record outcome
* publish follow-up
* exit

The second path exists specifically so passive tools do not need to
pretend to be daemons.

## Worker model

A worker host should be intentionally boring.

The stateless stdin/stdout contract remains the baseline because it is
easy to understand and test. But it is not the only useful plugin
shape. A plugin may also hide a local service, a warm session-backed
backend, or a hosted API as long as the worker host still receives a
bounded per-message result.

## Inputs

The worker host needs:

* a topic to watch
* a plugin to invoke
* a success topic
* a failure topic
* a timeout
* optionally a worker name

## Persistent loop

The persistent loop is:

1. wait for or look for one message for the configured topic
2. if none exists, keep waiting according to the chosen wait strategy
3. run the configured plugin
4. pass the message body and selected metadata to the plugin
5. collect stdout, stderr, exit code
6. record claim outcome
7. append follow-up message

## Reactive one-shot flow

The reactive flow is:

1. try to claim one message for the configured topic immediately
2. if none exists, block waiting for wakeup
3. retry claim after wake
4. run the configured plugin
5. collect stdout, stderr, exit code
6. record claim outcome
7. append follow-up message
8. exit

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

## Plugin model

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
* local-model plugin
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

## Request/reply helper

V1 should allow a small helper command for the common case:

* publish a request
* wait for a correlated follow-up
* print the result
* exit with meaningful status

This is not a new core abstraction. It is a CLI convenience built on:

* publish
* conversation id propagation
* topic filtering
* blocking wait for follow-up messages

Example shape:

```text
plugboard request \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  --body "Review this code"
```

This command exists to make passive request/reply flows legible without
changing the exchange model.

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

Persistent mode:

```text
plugboard run \
  --topic code.generate \
  --success-topic code.generated \
  --failure-topic code.generate.failed \
  --timeout-seconds 120 \
  -- codex exec
```

Reactive one-shot mode:

```text
plugboard run --once \
  --topic code.generate \
  --success-topic code.generated \
  --failure-topic code.generate.failed \
  --timeout-seconds 120 \
  -- codex exec
```

The command after `--` identifies the initial command plugin to invoke.

These commands are important because they demonstrate how both
persistent and passive tools join the exchange through a worker host
and plugin boundary.

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
mode = "persistent"
```

A reactive worker would differ only by mode:

```toml
[[worker]]
name = "codex-codegen-once"
topic = "code.generate"
success_topic = "code.generated"
failure_topic = "code.generate.failed"
timeout_seconds = 120
plugin = { type = "command", command = ["codex", "exec"] }
mode = "reactive"
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
loops and separating durable state from wakeup mechanics.

## Good units to test

* message creation
* conversation id propagation
* atomic claim logic
* claim completion and failure transitions
* follow-up message creation
* plugin result mapping into success/failure messages
* notifier wakeup behaviour
* `run_once_blocking()` semantics
* request/reply wait semantics

## Keep thin wrappers thin

The CLI and worker loop should mostly do I/O:

* parse args
* call exchange methods
* wait for wakeup or poll, depending on mode
* run plugin
* print results

This means most behaviour can be tested below the loop level.

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
* do not make reactive mode depend on polling sleeps
* do not make notifier delivery authoritative

Plugboard should feel smaller after implementation, not bigger.

## Recommended implementation order

1. create SQLite schema
2. implement `publish`
3. implement `read`
4. implement atomic `claim`
5. implement `complete` and `fail`
6. implement follow-up message creation
7. implement a notifier abstraction
8. implement `run --once` with `run_once_blocking()`
9. implement persistent `run`
10. implement `request`
11. implement `inspect`

This order proves the model early and reduces the risk of abstraction
drift.

## Summary

Plugboard v1 should be a very small local system with:

* immutable textual messages
* separate operational claims
* topic-based routing
* a SQLite backend
* advisory local wakeups
* a persistent worker mode
* a reactive one-shot worker mode
* follow-up messages for outcomes
* thin CLI commands over a small exchange API

## End-to-end examples

Persistent worker:

```text
plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- my-review-plugin
```

Reactive worker:

```text
plugboard run --once \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- my-review-plugin
```

Request/reply helper:

```text
plugboard request \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  --body "Review this code"
```

That is enough to prove the project’s core idea:

**independent programs can coordinate asynchronously through a local
textual exchange without sharing a framework or rigid typed
protocol.**
