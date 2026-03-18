# Install a Local Model Backend

This guide sets up the local-model path used by Plugboard today:

* Ollama as the local runtime
* `gemma3:1b` as the default demo model
* `ollama-plugin` as the Plugboard adapter

This is the fastest practical way to make Plugboard feel responsive on a
developer machine without depending on a hosted agent CLI cold start.

## Why Ollama

Ollama is a pragmatic fit for Plugboard's local path:

* realistic for developers to install locally
* runs a local inference service on one machine
* supports small models that start quickly enough for demos
* exposes a simple local API that a plugin can call directly

The current adapter defaults to the official Ollama model tag
`gemma3:1b`, which is small enough to be practical for local demos
while still being more useful than a toy shell transform.

Official model references:

* `gemma3:1b` — https://ollama.com/library/gemma3:1b
* `qwen2:1.5b` — https://ollama.com/library/qwen2:1.5b

## Prerequisites

You need:

* a machine that can run Ollama locally
* the Ollama CLI installed
* disk space for the selected model

Install Ollama from the official download page:

* https://ollama.com/download

## Start the local runtime

Ollama uses a local service. Start it before using Plugboard:

```bash
ollama serve
```

Keep that running in a separate terminal.

## Pull the model

In another terminal, pull the default demo model:

```bash
ollama pull gemma3:1b
```

`gemma3:1b` currently requires Ollama 0.6 or later.

If you want a different local model later, set `OLLAMA_PLUGIN_MODEL`
when you run the plugin.

## Verify Ollama independently

Before involving Plugboard, verify the local backend directly:

```bash
ollama run gemma3:1b "Reply with exactly: local model ok"
```

You should get a short response from the local model.

You can also verify the same API shape used by `ollama-plugin`:

```bash
curl http://127.0.0.1:11434/api/generate \
  -d '{
    "model": "gemma3:1b",
    "prompt": "Reply with exactly: local model ok",
    "stream": false
  }'
```

The JSON response should include a `response` field.

## Verify the Plugboard adapter

Build the local model plugin:

```bash
cargo build --bin ollama-plugin
```

Then verify the adapter directly:

```bash
printf 'Reply with exactly: local model ok' | ./target/debug/ollama-plugin
```

That confirms the full local adapter boundary:

* Plugboard-style stdin into the plugin
* plugin call to the local Ollama backend
* one bounded stdout result back from the plugin

## Next step

Once this works, continue to
[Plugboard with a Local Model](plugboard-with-local-model.md) for the
full request/reply worker flow.
