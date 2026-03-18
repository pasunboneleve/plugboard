# Measuring Local Latency

This note gives a small, reproducible latency breakdown for the three
main local execution modes that Plugboard now supports:

1. direct plugin invocation
2. reactive one-shot worker with `plugboard run --once` and `plugboard request`
3. persistent worker with long-running `plugboard run` and `plugboard request`

It is not a benchmarking framework. It is a practical way to
understand what part of the path costs time.

## What each timestamp means

For the exchange-backed modes, the useful timestamps are already in the
database:

* `request created_at`
  The request message was appended to the exchange.
* `claim claimed_at`
  A worker successfully claimed the request.
* `claim completed_at`
  The backend finished and the claim moved to a terminal state.
* `follow-up created_at`
  The success or failure message was appended.

Read them like this:

* `publish -> claim`
  activation, wake, and claim latency
* `claim -> completed`
  plugin or backend execution time
* `completed -> follow-up`
  local follow-up overhead
* `request -> reply`
  total end-to-end latency as observed by the caller

For direct plugin invocation there is no exchange state, so the lower
bound is just the caller-observed wall clock.

## Reproduce locally

Build the binaries first:

```bash
cargo build --bin plugboard --bin example-review-plugin
```

Then run:

```bash
python3 scripts/measure_latency.py
```

That script uses the deterministic `example-review-plugin`, so the
numbers mostly reflect Plugboard and process overhead rather than model
latency.

## Example measurements

The following numbers were captured locally from five runs per mode
using `example-review-plugin` and `python3 scripts/measure_latency.py`.
They are useful for understanding the path, not for pretending local
latency is perfectly stable to the millisecond.

### Direct plugin invocation

Observed wall-clock times:

```text
0.913 ms, 0.856 ms, 0.833 ms, 0.720 ms, 0.752 ms
```

Median:

```text
0.833 ms
```

This is the lower bound. It includes neither exchange I/O nor worker
activation.

### Reactive one-shot worker

Observed end-to-end request/reply times:

```text
25.220 ms, 22.314 ms, 24.959 ms, 7.873 ms, 7.085 ms
```

Median:

```text
22.314 ms
```

One sample breakdown:

```text
request created_at   2026-03-18T22:51:28.353738263Z
claim claimed_at     2026-03-18T22:51:28.356096237Z
claim completed_at   2026-03-18T22:51:28.361532260Z
follow-up created_at 2026-03-18T22:51:28.362856214Z
request -> reply     25.220 ms
```

Interpretation:

* publish -> claim: about 2.4 ms
* claim -> completed: about 5.4 ms
* completed -> follow-up: about 1.3 ms
* the remaining time is mostly caller and process overhead around `request` and the short-lived worker

Reactive one-shot mode avoids worker loitering and now avoids relying
on pure polling for correctness. It is a good fit for passive tools
when a short-lived process is acceptable.

### Persistent worker

Observed end-to-end request/reply times:

```text
11.882 ms, 8.445 ms, 20.570 ms, 22.699 ms, 22.838 ms
```

Median:

```text
20.570 ms
```

One sample breakdown:

```text
request created_at   2026-03-18T22:51:29.448183971Z
claim claimed_at     2026-03-18T22:51:29.449090024Z
claim completed_at   2026-03-18T22:51:29.451405195Z
follow-up created_at 2026-03-18T22:51:29.452122107Z
request -> reply     11.882 ms
```

Interpretation:

* publish -> claim: about 0.9 ms
* claim -> completed: about 2.3 ms
* completed -> follow-up: about 0.7 ms
* the worker is already resident, so activation overhead can be lower or more stable when a long-lived process is acceptable

## What these numbers show

* direct plugin invocation is the lower bound
* for a deterministic local backend, both reactive and persistent
  Plugboard paths add only a small local overhead
* plugin execution time is often not the dominant cost for a fast local backend
* persistent workers can reduce activation overhead further when
  keeping a resident process is acceptable

These measurements do **not** change the current wakeup caveat:

* notifier wakeups are advisory only
* correctness currently falls back to bounded re-checks
* the default bounded interval is 250 ms
* under notifier failure, worst-case detection latency is therefore about
  250 ms plus normal process and SQLite overhead
