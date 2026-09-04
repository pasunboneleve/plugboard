# Incremental conversation reads

Use a bounded [Plugboard](https://github.com/pasunboneleve/plugboard)
conversation read when a local service or interface needs to follow new
messages without loading the full history each time.

Start at position zero and retain the largest `position` returned:

```bash
plugboard read \
  --conversation-id <conversation-id> \
  --after-position 0 \
  --limit 100 \
  --json
```

The command emits newline-delimited JSON: one complete message object per
line. Each object includes the message fields and its database-local
`position`. Pass the last position back as `--after-position` to request the
next page.

`--after-position` and `--limit` require `--json`. Human reads without these
flags retain their existing tab-separated format.

The cursor is exclusive. An empty result means no later message is currently
stored; it does not mean the conversation is complete. Wait for a Plugboard
wakeup or retry according to the consumer's own bounded polling policy.

Limits must be between 1 and 1,000. Plugboard rejects negative positions and
out-of-range limits instead of silently changing them.

Positions order all messages in one database. Do not persist a cursor for use
with another database, infer elapsed time from it, or assume positions within
one conversation are consecutive.
