# Plugboard Architecture

This document covers the current system model and mechanics. It answers
how Plugboard works today. For intent and project boundaries, see
[Design](design.md).

## System shape

Plugboard has three layers:

### 1. Exchange

The exchange stores messages and claims in SQLite and exposes the core
operations:

* publish a message
* read by topic or conversation
* claim the next message for a topic
* transition claims to completed, failed, or timed out

Messages are immutable records. Claims are separate operational state.

### 2. Worker host

`plugboard run` is the worker host. It:

* waits for work on one topic
* claims one message
* invokes a plugin
* publishes a follow-up on success, failure, or timeout

Workers record both:

* `worker_group`: the stable logical worker class
* `worker_instance_id`: the concrete running process

### 3. Plugin

The baseline plugin contract is passive and Unix-like:

* read the claimed message body from `stdin`
* write a result to `stdout`
* exit

The worker maps the outcome into follow-up topics and claim status.

## Storage model

Plugboard uses SQLite at `.plugboard/plugboard.db` by default.

SQLite is the source of truth for:

* messages
* claims
* conversation history

The schema has two durable entities:

### Messages

Fields in use today:

* `id`
* `topic`
* `body`
* `created_at`
* `parent_id`
* `conversation_id`
* `producer`
* `metadata_json`

If a message does not specify `conversation_id`, the exchange uses the
message id. Follow-up messages inherit the parent conversation.

### Claims

Fields in use today:

* `id`
* `message_id`
* `worker_group`
* `worker_instance_id`
* `claimed_at`
* `lease_until`
* `status`
* `completed_at`

Claim status is one of:

* `active`
* `completed`
* `failed`
* `timed_out`

A claim is live only while it is `active` and `lease_until` is still in
the future.

## Message flow

### Publish

`plugboard publish` appends one message to a topic. If the topic ends in
`.request`, the CLI also records local tracking metadata for
notifications.

### Claim

Workers claim the oldest claimable message for their topic in a SQLite
transaction. Before claiming, the exchange deletes expired active claims
so stale ownership does not block recovery.

### Execute

The worker runs the plugin once for that message. There is no
persistent backend session in the core worker model.

### Follow-up

The worker transitions the claim, then publishes a follow-up message:

* success: configured success topic
* failure: configured failure topic
* timeout: `<request-topic>.timed_out`

Follow-ups keep `parent_id` and `conversation_id`, so later reads can
inspect the whole thread.

## Async and blocking paths

The default operating model is asynchronous:

1. publish work
2. keep going
3. read replies later

Plugboard also has a thin blocking helper:

### `plugboard request`

This command publishes a request, emits `message_id` and
`conversation_id`, then waits for the first correlated reply on the
configured success or failure topic.

It does not introduce a new request entity. It still uses the normal
message log and conversation correlation.

### `plugboard check`

This command reads one conversation and reports whether a terminal
success or failure reply exists yet.

### `plugboard read`

This is the normal consumption command. It reads messages by topic or by
conversation.

### `plugboard inspect`

This is the forensic command. It is for raw history and claim-state
debugging rather than routine use.

## Wakeups and waiting

Workers and request waiters use advisory wakeups plus bounded SQLite
re-checks.

Current defaults:

* worker wait timeout: 250 ms
* worker fallback re-check: 250 ms
* request wait timeout: 250 ms
* request fallback re-check: 250 ms

The wakeup path improves responsiveness, but correctness still depends
on re-checking SQLite.

## Worker modes

### Persistent worker

`plugboard run` waits for work, handles a message, drains any immediately
claimable backlog, then waits again.

### Reactive one-shot worker

`plugboard run --once` blocks until one matching message exists, handles
it, publishes the follow-up, and exits.

## Notification layer

`plugboard notify` is a local helper on top of the exchange. It reads
tracked conversations from `.plugboard/tracked-conversations.json`,
checks them for terminal replies, emits one advisory notification, and
marks them as notified.

This does not affect message correctness. The exchange remains the
source of truth.

## Current CLI surface

* `plugboard publish`
* `plugboard read`
* `plugboard check`
* `plugboard notify`
* `plugboard request`
* `plugboard inspect`
* `plugboard run`

## Backend variants

The current architecture supports several plugin styles without changing
the core:

* command-style transforms
* local model adapters such as `ollama-plugin`
* API adapters such as `gemini-plugin`
* wrappers around warm or session-backed tools

Those variants all fit the same message, claim, execute, follow-up
cycle.
