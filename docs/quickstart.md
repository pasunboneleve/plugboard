# Quickstart

This is the shortest path to a working Plugboard setup with a passive
worker.

Build the binary first if it is not already available on your `PATH`:

```bash
cargo build
export PATH="$PWD/target/debug:$PATH"
```

Then run:

```bash
plugboard publish review.request "Review this code"

timeout 2 plugboard run \
  --topic review.request \
  --success-topic review.done \
  --failure-topic review.failed \
  -- sh -c 'tr a-z A-Z'

plugboard read --topic review.done
```

What happens:

* A message is published to the `review.request` topic.
* A worker host listens on that topic.
* The worker claims the message from the exchange.
* The message body is passed to the command on `stdin`.
* The command writes its result to `stdout`.
* Plugboard publishes that output to `review.done`.

## Worker Contract

For the baseline command plugin, the child command must:

* read input from `stdin`
* write its result to `stdout`
* exit when done
* use its exit code to signal success or failure

An exit code of `0` is treated as success. A non-zero exit code is
treated as failure and published to the configured failure topic.

Some tools, especially interactive CLIs, do not satisfy this contract.
Those tools need a wrapper or dedicated plugin that turns them into a
non-interactive command.

For a concrete request/reply pattern using topic conventions, see
[Codex to Gemini Workflow](howto/codex-to-gemini.md).

## Example Plugin Demo

The repository also includes a deterministic example plugin binary
named `example-review-plugin`. It reads the claimed message from
`stdin`, writes a review-style response to `stdout`, and exits.

For the exact command sequence, see
[Write a Worker Plugin](howto/write-a-worker-plugin.md).

You should see a `review.done` message whose body starts like this:

```text
Review status: ok
Reviewer: example-review-plugin
```
