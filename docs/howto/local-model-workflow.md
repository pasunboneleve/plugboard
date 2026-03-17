# Local Model Workflow

This is the shortest practical local-model demo for Plugboard.

Assumptions:

* `ollama serve` is already running
* `gemma3:1b` is already pulled
* `cargo build` has been run
* `target/debug` is on your `PATH`

## Commands

```bash
plugboard publish local.review.request "Summarize this code review in one sentence."

timeout 30 plugboard run \
  --topic local.review.request \
  --success-topic local.review.done \
  --failure-topic local.review.failed \
  --timeout-seconds 20 \
  -- ollama-plugin

plugboard read --topic local.review.done
```

## What it proves

* a message is published to a topic
* a long-running worker can listen on that topic
* the worker can invoke a real local model adapter
* the reply is published back into Plugboard on a follow-up topic

For installation and setup from scratch, see
[Install a Local Model Backend](install-local-model-backend.md).

For the fuller Plugboard tutorial, see
[Plugboard with a Local Model](plugboard-with-local-model.md).
