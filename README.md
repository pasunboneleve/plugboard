# Plugboard

[![cargo-test](https://github.com/pasunboneleve/plugboard/actions/workflows/cargo-test.yml/badge.svg)](https://github.com/pasunboneleve/plugboard/actions/workflows/cargo-test.yml)

Plugboard is a local textual exchange where independent programs
cooperate.

It is designed for asynchronous workflows — from simple tools like
`grep` to AI agents.  Publish work, do something else, read the
results later — all through plain text.

---

<p align="center">
  <a href="https://blog.sciencemuseum.org.uk/life-on-the-exchange-stories-from-the-hello-girls/"
  target="_blank"
  rel="noopener noreferrer">
    <img
        src="docs/images/plugboard-switchboard.jpg"
        alt="Manual switchboard operators routing connections"
        style="width:45%;"
        />
  </a>
</p>

<p align="center">
    <sub>Operators routing connections</sub>
    <sub>— an early “plugboard” for human communication.</sub>
</p>

It gives independent tools one small shared surface:

* publish text to a topic
* read text from the exchange
* run workers that claim messages and append follow-ups

The core mental model is:

* local textual exchange, backed by **SQLite**
* async-first workflow: enqueue work now, read replies later
* workers and plugins at the edge, not inside the core

`plugboard request` exists for quick publish-and-wait experiments, but
the main model is still durable asynchronous exchange.

## Getting started

Build the binaries:

```bash
cargo build
export PATH="$PWD/target/debug:$PATH"
```

In one terminal, start a worker:

```bash
plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- sh -c 'tr a-z A-Z'
```

In another terminal, publish work and read the reply:

```bash
plugboard publish review.request "Review this code"
plugboard read --topic review.done
```

That is the whole shape: publish to a topic, let a worker claim the
message, and read the follow-up later.

## Documentation

Start here:

* [Docs index](docs/README.md)
* [Design](docs/design.md)
* [Architecture](docs/architecture.md)
* [Quickstart](docs/quickstart.md)

Task-oriented guides:

* [Async inbox workflow](docs/howto/async-inbox.md)
* [Write a worker plugin](docs/howto/write-a-worker-plugin.md)
* [Install a local model
  backend](docs/howto/install-local-model-backend.md)
* [Plugboard with a local
  model](docs/howto/plugboard-with-local-model.md)
* [Codex to Gemini workflow](docs/howto/codex-to-gemini.md)

Additional reference:

* [Plugin backend options](docs/plugin-backends.md)
* [Completion notifications](docs/howto/completion-notifications.md)
* [Measure local latency](docs/howto/measure-latency.md)

## License

[LICENSE](LICENSE)
