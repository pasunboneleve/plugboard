# Completion Notifications

Plugboard is already useful without push notifications:

1. enqueue work
2. continue doing other work
3. later read replies from the exchange

That is the current recommended model.

## Current state

Today, Plugboard does not actively notify a human when a job finishes.

The normal way to see completed work is still:

```bash
./target/debug/plugboard read --topic <reply-topic>
```

or:

```bash
./target/debug/plugboard read --conversation-id <conversation-id>
```

This keeps the core simple and local-first. The exchange remains the
source of truth.

## Why this is still a product gap

The async model is sound, but it is easier to forget about queued work
without a completion signal.

A good future notification path should:

* stay local-first
* treat the message log as authoritative
* avoid turning Plugboard into a general event bus
* help a human notice that a reply is ready

## Proposed v1 semantics

V1 should stay narrow and only cover tracked async send/check workflows.

The intended flow is:

1. a user or agent sends work asynchronously
2. the helper layer records the returned `conversation_id` as tracked
3. a local notifier process watches those tracked conversations
4. when one tracked conversation reaches a terminal state, it emits one
   advisory notification

### Trigger event

A notification is triggered when a tracked conversation first gains a
terminal reply:

* success topic reply
* failure topic reply

The trigger is tied to tracked `conversation_id` values, not to global
topics and not to fuzzy body matching.

### Matching

Matching is by:

* `conversation_id`
* configured success/failure topic pair for that tracked send

This keeps notification aligned with the same request/reply tracking
model already used by async `send` and `check`.

### Scope

V1 should cover explicit tracked async sends only.

That means:

* no arbitrary topic subscriptions
* no global reply watching
* no attempt to notify for work that was never tracked by the helper

### Success and failure

V1 should notify on both:

* success
* failure

Both are useful operator outcomes, and both should be surfaced once.

### Once-only behavior

Each tracked conversation should produce at most one notification.

Once a terminal success or failure notification has been emitted, the
local notifier should mark that tracked item as completed in its own
local state and stop notifying for it again.

This local tracking state is advisory only and must not affect message
correctness or exchange behavior.

### Notification content

A v1 notification should be brief and readable. It should include:

* status: success or failure
* a short label, such as `Ollama reply ready`
* `conversation_id`
* a short body preview if available

It should also make the useful follow-up action obvious, for example:

```text
Ollama reply ready
conversation_id=<id>
Run: ./scripts/check-ollama-conversation <id>
```

### Human and agent split

Structured internal state may be used for tracked conversations and
completion bookkeeping, but the human-facing notification should stay
brief and readable.

The durable exchange remains the source of truth. The notifier is only
an ergonomic layer on top.

### Failure handling

Notifier failure must not affect correctness.

If the notifier process crashes, the notification backend is missing, or
local state is lost:

* replies are still in the exchange
* `read` still works
* `check` still works
* the user can still recover manually by conversation id

Notification delivery is advisory only.

## Proposed minimal implementation shape

The most practical v1 is a thin local helper process that:

* reads a local tracked-conversation state file
* checks each tracked conversation for terminal success/failure
* emits one local notification when a terminal reply appears
* marks the tracked conversation as already notified

Likely local notification backends, in preference order:

* `notify-send` on Linux when available
* terminal bell
* stderr/plain terminal notice fallback

This stays:

* optional
* local to the machine
* downstream of normal message publication

It does not replace `read`, `inspect`, `check`, or the durable exchange
model.
