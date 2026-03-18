# Plugin Backend Options

Plugboard is the textual exchange. Backend choice lives in the worker
host and plugin layer.

That is the key design boundary:

* Plugboard remains topic-based and agent-agnostic
* Plugboard does not manage agent identity, presence, routing, or sessions
* different backend styles are plugin alternatives, not protocol changes

## Why this matters

Different backends make Plugboard feel very different in practice.
Some are ideal for tiny local demos. Others are better for warm agent
setups or hosted integrations.

The exchange does not need to change for any of them:

```text
publish -> topic -> worker host -> plugin/backend -> follow-up topic
```

For the common passive request/reply case, Plugboard can also sit at the
edge as:

```text
request -> topic -> worker host -> plugin/backend -> follow-up topic -> request waiter
```

That helper still uses the same message log, conversation correlation,
and advisory wakeup rules.

## 1. Simple stateless transforms

This is the baseline worker contract:

* one message triggers one process
* the worker writes the message body to plugin `stdin`
* the plugin writes the result to `stdout`
* the plugin exits

Best fit:

* shell filters
* deterministic transforms
* small wrappers around passive CLIs
* fast end-to-end demos

This is the easiest path to understand and debug. It keeps the worker
model boring and Unix-like.

## 2. Local model plugins

Local model plugins are for fast demos and development when a hosted
agent CLI is too slow to start or too expensive to use for every test.

A local model plugin may talk to:

* a local inference server
* a local runtime such as Ollama or llama.cpp
* a model process already available on the machine

Best fit:

* responsive local demos
* developer setup where low latency matters
* proving Plugboard's usefulness without network dependencies

From Plugboard's point of view, this is still just a plugin. The core
does not care whether the backend is local or remote.

## 3. Already-running agent or session-backed plugins

Sometimes cold start is the real problem, not the model itself. In
that case, a plugin can talk to an already-running agent or warm
backend.

This is appropriate when:

* startup time dominates execution time
* the backend keeps useful warm state
* a long-lived local service or agent process already exists

Important boundary:

* any persistence or session logic belongs in the plugin or adapter
* Plugboard still only sees topics, claims, follow-ups, and outcomes

Plugboard should not grow built-in session management to support this.

## 4. API plugins

An API plugin calls an external service directly.

Best fit:

* hosted models
* SaaS integrations
* service APIs that are already natural request/response systems

An API plugin still fits the same exchange pattern:

* worker claims one message
* plugin turns the message into an API request
* plugin writes the final textual result back to `stdout`
* worker publishes the follow-up message

This keeps the core agent-agnostic while still allowing practical
hosted integrations.

## Choosing the right backend

Use a simple stateless transform when you want the smallest possible
workflow and the backend already fits stdin/stdout.

Use a local model plugin when you want Plugboard to feel fast and
useful on a developer machine.

The current recommended local path in this repository is an Ollama
adapter that talks to a local `ollama serve` instance and a small model
such as `gemma3:1b`.

Use a session-backed plugin when warm state matters but you still want
Plugboard to stay minimal.

Use an API plugin when the cleanest integration point is a hosted
service rather than a local CLI.

## Current direction

Plugboard already demonstrates the stateless command path and a real
Gemini adapter. It now also includes a local Ollama-backed plugin for
fast local demos. The next product direction is to strengthen:

* the local-model path for low-latency demos
* a clean API-plugin path for hosted backends

Those additions belong in plugins, docs, and worker integrations, not
in Plugboard core.
