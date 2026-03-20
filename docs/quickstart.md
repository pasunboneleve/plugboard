# Quickstart

This is the shortest path to a working Plugboard setup.

Build the binaries:

```bash
cargo build
export PATH="$PWD/target/debug:$PATH"
```

Publish one message:

```bash
plugboard publish review.request "Review this code"
```

Handle it with a worker:

```bash
timeout 2 plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- sh -c 'tr a-z A-Z'
```

Read the follow-up:

```bash
plugboard read --topic review.done
```

What happened:

* `publish` appended a message to `review.request`
* `run` claimed that message and passed the body to the command on `stdin`
* the command wrote a result to `stdout`
* Plugboard published that result to `review.done`

For the broader model, see [Design](design.md) and
[Architecture](architecture.md). For plugin-specific guidance, see
[Write a worker plugin](howto/write-a-worker-plugin.md).
