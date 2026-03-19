# Async Inbox Workflow

This is the simplest way to use Plugboard as an asynchronous exchange:

1. send work now
2. do something else
3. later read replies

Use this when you want Plugboard's real value: durable message history
and the ability to come back later without keeping the foreground shell
blocked.

## Terminal A: worker

Keep the worker running in a separate terminal:

```bash
./scripts/run-ollama-worker
```

This is a long-lived worker for `ollama.request`.

## Terminal B: enqueue work

Send work without waiting for an immediate reply:

```bash
./target/debug/plugboard publish \
  ollama.request \
  "Summarize Rust ownership in one short paragraph."
```

Or use `request` when you want Plugboard to create a correlated
conversation while still thinking in terms of queued work:

```bash
./target/debug/plugboard request \
  ollama.request \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --meta model=llama3.2:latest \
  --body "Summarize Rust ownership in one short paragraph."
```

If you do not want to block, prefer `publish` and come back later with
`read`.

## Do something else

At this point, leave the worker alone and continue with other work.

## Later: read replies

Check the reply topic:

```bash
./target/debug/plugboard read --topic ollama.done
```

If you want to narrow the view to one correlated exchange, read by
conversation id instead:

```bash
./target/debug/plugboard read --conversation-id <conversation-id>
```

Use `inspect` only when the normal topic or conversation view is not
enough.

## Mental model

`publish` and `request` enqueue work.

`read` is the normal way to consume replies later.

`inspect` is for debugging and forensics.
