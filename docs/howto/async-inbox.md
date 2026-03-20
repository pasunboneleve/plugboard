# Async Inbox Workflow

This is the default Plugboard usage pattern:

1. enqueue work
2. do something else
3. check replies later

For the underlying model, see [Design](../design.md) and
[Architecture](../architecture.md).

## Start a worker

Keep the Ollama worker running in a separate terminal:

```bash
./scripts/run-ollama-worker
```

## Send work

Non-blocking send:

```bash
./target/debug/plugboard publish \
  ollama.request \
  "Summarize Rust ownership in one short paragraph." \
  --meta model=llama3.2:latest \
  --json
```

Capture `conversation_id` from the publish event. That is the primary
handle for later checks.

If you want the blocking convenience path instead, use:

```bash
./target/debug/plugboard request \
  ollama.request \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --body "Summarize Rust ownership in one short paragraph."
```

## Check later

Read the reply topic:

```bash
./target/debug/plugboard read --topic ollama.done
```

Or read one conversation:

```bash
./target/debug/plugboard read --conversation-id <conversation-id>
```

Or do a compact terminal-state check:

```bash
./target/debug/plugboard check \
  --conversation-id <conversation-id> \
  --success-topic ollama.done \
  --failure-topic ollama.failed
```

For natural-language Ollama workflows, prefer:

```bash
./scripts/check-ollama-conversation <conversation-id>
```

## Notes

* `publish` and `request` both enqueue work.
* `read` is the normal consumption path.
* `inspect` is for debugging and forensics.
* If identifiers are available, prefer `conversation_id` over request
  body matching.
