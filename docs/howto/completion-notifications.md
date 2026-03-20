# Completion Notifications

Plugboard notifications are a local convenience layer on top of the
exchange. They do not change delivery or correctness.

## What exists today

`plugboard notify` reads tracked conversations from:

```text
.plugboard/tracked-conversations.json
```

For each conversation that has not been marked notified yet, it checks
for a terminal reply on that conversation's configured success or
failure topic.

If it finds one, it emits one local notification and marks that
conversation as notified in the tracking file.

## How conversations get tracked

`plugboard publish` automatically tracks messages whose topic ends in
`.request`.

That tracking record stores:

* `conversation_id`
* success topic
* failure topic
* whether notification has already been emitted

## Run it once

```bash
plugboard notify --once
```

## Run it continuously

```bash
plugboard notify --poll-seconds 2
```

## Notification behavior

Notifications are advisory only.

If notification delivery fails, replies are still available through the
exchange:

* `plugboard read --conversation-id <conversation-id>`
* `plugboard check --conversation-id <conversation-id> --success-topic ... --failure-topic ...`

The notification backend order is:

* `notify-send`, when available
* terminal bell plus stderr fallback

You can also force:

* `PLUGBOARD_NOTIFY_BACKEND=stderr`
* `PLUGBOARD_NOTIFY_BACKEND=bell`

## Notes

The label is currently specialized for `ollama.done`; other success
topics use the generic `Reply ready`.

For the underlying async model, see
[Async inbox workflow](async-inbox.md).
