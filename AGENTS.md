# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

Use 'bd' for task tracking


## Ollama via Plugboard (interactive use)

Use the local binary:

./target/debug/plugboard

Topics:
- request: ollama.request
- success: ollama.done
- failure: ollama.failed

### Worker

Start the Ollama worker with:

./scripts/run-ollama-worker

For one-shot debugging or a single local experiment, you can use:

./scripts/run-ollama-worker-once

This starts a long-lived worker. For normal Plugboard usage, prefer:

1. enqueue work now
2. keep doing other work
3. read replies later

Only block in the foreground when the user explicitly asks to wait for
the reply.

### Ask ollama

When I say:

ask ollama: <prompt>
ask ollama with <model>: <prompt>
check ollama

You must execute this flow exactly:

1. Ensure the worker is running.
   - If unsure, start it using:
     ./scripts/run-ollama-worker
   - Run it in a background terminal/session.
   - Say "Starting Ollama worker." only if you actually start it.

2. Send exactly one request by running exactly once:

   ./target/debug/plugboard request ollama.request \
     --success-topic ollama.done \
     --failure-topic ollama.failed \
     [--meta model=<model>] \
     --body "<prompt>"

   - If a model is specified, include:
     --meta model=<model>
   - Otherwise omit it.
   - Prefer `--json` when tooling reliability matters.

3. Say: "Request published."

4. Wait for the request command to finish.
   - The output of that command is the final result.
   - Return that result exactly once.

For async tracking, prefer the request identifiers that Plugboard
already returns at publish time:

- `message_id`
- `conversation_id`

Treat `conversation_id` as the primary handle for later async lookup.
Use `message_id` as supporting detail.

Later checks should prefer:

./target/debug/plugboard check \
  --conversation-id <conversation-id> \
  --success-topic ollama.done \
  --failure-topic ollama.failed \
  --json

This is the preferred path over request-body matching.

If more detail is needed, then use:

./target/debug/plugboard read --conversation-id <conversation-id>

Only fall back to matching request body text if those identifiers are
not available. When doing that, prefer the latest plausible request and
say explicitly that the result is heuristic.

### Preferred Plugboard pattern

Unless the user explicitly asks to block and wait, prefer the
asynchronous model:

1. publish or request work
2. leave the worker running
3. later use:

   ./target/debug/plugboard read --topic ollama.done

`read` is normal consumption. `inspect` is for debugging.

For agents and tools, the default async pattern is:

1. send request
2. capture `conversation_id`
3. later check by `conversation_id`
4. determine whether a terminal success or failure reply exists

### Check ollama

When I say:

check ollama

Run:

./scripts/check-ollama

Return the output directly.

This is the friendly Ollama-specific inbox view. It sits on top of:

./target/debug/plugboard read --topic ollama.done
./target/debug/plugboard read --topic ollama.failed

It shows the 10 most recent replies by default. An optional numeric
argument overrides that count.

This helper is friendly, but it is not the primary async tracking key.
For reliable agent follow-up, prefer stored `conversation_id` values and
`plugboard check --conversation-id ...`.

### Strict rules

- Never run the request command a second time unless I explicitly ask.
- Never restate, summarize, expand, or reinterpret the request result.
- Never mix:
  - request command output
  - topic inspection output
  - your own explanation

- Do not run `plugboard read` while the request command is still running.
- Topic inspection is allowed only if:
  - the request command exits with an error, or
  - the request command appears stuck after a reasonable wait

### If the request appears stuck

If the request command appears stuck after a reasonable wait:

1. Do not issue another request.
2. Inspect once with:

   ./target/debug/plugboard read --topic ollama.request
   ./target/debug/plugboard read --topic ollama.done
   ./target/debug/plugboard read --topic ollama.failed

3. Report only the observed state briefly.
4. Do not infer more than the evidence supports.

If a new reply is visible on `ollama.done` or `ollama.failed` while the original request command is still blocked, report:

"Reply exists on the reply topic, but the original request command is still blocked. This suggests a request/reply waiter bug or correlation issue."

### Output rules

- Do not say "in flight".
- Do not assume `plugboard` is on PATH.
- Be concise and operational.
