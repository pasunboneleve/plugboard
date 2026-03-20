# Plugin Backend Options

This page is about backend choice at the plugin layer. For the core
boundary, see [Design](design.md). For the worker and message model, see
[Architecture](architecture.md).

Plugboard does not need to change for these variants:

```text
publish -> topic -> worker host -> plugin/backend -> follow-up topic
```

## Command-style transforms

Best when the backend already fits the default contract:

* read one input from `stdin`
* write one result to `stdout`
* exit

This is the simplest path for shell filters and deterministic tools.

## Local model adapters

Best when you want a fast local demo or a low-latency development loop.

The current repository example is `ollama-plugin`, which talks to a
local Ollama service.

## API adapters

Best when the natural integration point is a hosted service.

The current repository example is `gemini-plugin`, which shells out to
the Gemini CLI and returns a textual result.

## Warm or session-backed adapters

Best when startup cost dominates execution time and a long-lived backend
already exists.

Any session logic still belongs in the plugin layer. Plugboard core
continues to see only topics, claims, and follow-up messages.

## Choosing among them

Use the simplest backend that fits the job:

* command transform for small deterministic tasks
* local model adapter for responsive local work
* API adapter for hosted integrations
* warm adapter when persistent backend state matters
