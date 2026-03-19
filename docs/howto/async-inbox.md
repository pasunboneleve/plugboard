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

When `request` publishes, it also emits stable identifiers on `stderr`:

```text
published message_id=<message-id> conversation_id=<conversation-id> topic=ollama.request
```

Or, for agent use:

```bash
./target/debug/plugboard request \
  ollama.request \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --json \
  --body "Summarize Rust ownership in one short paragraph."
```

which emits:

```json
{"event":"published","message_id":"...","conversation_id":"...","topic":"ollama.request"}
```

Capture `conversation_id`. That is the primary async tracking key.

For a truly non-blocking send, use `publish` instead:

```bash
./target/debug/plugboard publish \
  ollama.request \
  "Summarize Rust ownership in one short paragraph." \
  --meta model=llama3.2:latest \
  --json
```

That returns immediately. Capture `conversation_id`, then check later.

For humans, do not surface the raw JSON directly. Parse it internally
and answer in plain text, for example:

* `Sent to Ollama.`
* `Conversation ID: <conversation-id>`

For agents and tools, prefer the JSON form when parse reliability
matters. The intended pattern is:

1. send request
2. capture `conversation_id`
3. store it in agent working memory
4. later read by that `conversation_id`

## Do something else

At this point, leave the worker alone and continue with other work.

## Later: read replies

Check the reply topic:

```bash
./target/debug/plugboard read --topic ollama.done
```

For the common Ollama flow, you can use the higher-level helper instead:

```bash
./scripts/check-ollama
```

That shows recent replies from `ollama.done` and `ollama.failed`
together. It is meant for normal consumption, not debugging. By default
it shows the 10 most recent replies; pass a number to change that.

If you want to narrow the view to one correlated exchange, read by
conversation id instead:

```bash
./target/debug/plugboard read --conversation-id <conversation-id>
```

That is the preferred way for agents and tools to check a specific
request later.

For a compact terminal-state check, use:

```bash
./target/debug/plugboard check \
  --conversation-id <conversation-id> \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --json
```

A terminal reply is any message in that conversation whose topic is the
configured success or failure topic.

Use `inspect` only when the normal topic or conversation view is not
enough.

## Mental model

`publish` and `request` enqueue work.

`read` is the normal way to consume replies later.

`inspect` is for debugging and forensics.

For the Ollama demo path:

* `ask ollama` maps to sending work
* `check ollama` maps to checking recent replies later

If IDs are unavailable, the fallback is to match the original request
body text, preferring the latest plausible request, but that is
heuristic and less reliable than using `message_id` or
`conversation_id`.

## Agent-oriented example

1. Blocking path if you want the answer now:

   ```bash
   ./target/debug/plugboard request \
     ollama.request \
     --success-topic ollama.done \
     --failure-topic ollama.failed \
     --json \
     --body "Rewrite this status update in calmer language."
   ```

2. Non-blocking path if you want to continue working:

   ```bash
   ./target/debug/plugboard publish \
     ollama.request \
     "Rewrite this status update in calmer language." \
     --meta model=llama3.2:latest \
     --json
   ```

3. Capture the publish event from `stderr` and store:

   * `message_id`
   * `conversation_id`

4. Later, check the exact exchange:

   ```bash
   ./target/debug/plugboard check \
     --conversation-id <conversation-id> \
     --success-topic ollama.done \
     --failure-topic ollama.failed \
     --json
   ```

5. Report one of:

   * `Yes, it replied ...` if `state` is `success` or `failure`
   * `No reply yet.` if `state` is `pending`

The JSON output is for machine parsing. The user-facing answer should be
short plain text, not the JSON wrapper.

If more detail is needed after that compact check, then read the full
conversation:

```bash
./target/debug/plugboard read --conversation-id <conversation-id>
```
