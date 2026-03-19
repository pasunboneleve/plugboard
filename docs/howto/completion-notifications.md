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

## Likely shape of a future solution

The most plausible next step is a thin edge helper that notices new
reply messages and surfaces a local notification.

That should remain:

* optional
* local to the machine
* downstream of normal message publication

It should not replace `read`, `inspect`, or the durable exchange model.
