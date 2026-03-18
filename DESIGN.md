# Plugboard Design

## Overview

Plugboard is a local textual message exchange for cooperating
programs.

It provides a small local coordination surface where independent
programs publish and consume text. It does not define an agent
framework, workflow engine, or shared object model. Its purpose is to
let separately built tools cooperate through loose textual interfaces.

Programs interact with Plugboard by appending messages to the exchange
and by reading or claiming messages that match their interests.
Optional worker hosts watch selected topics, invoke plugins, and
publish results back into the exchange.

The system acts as a software switchboard for cooperating processes.

## Motivation

Many systems for automation and AI coordination rely on tightly
coupled abstractions:

* shared SDKs
* rigid schemas
* centralized orchestration
* strongly typed RPC
* agent object models

These approaches can make systems harder to evolve, because every
participant must conform to the same framework and protocol
vocabulary.

Plugboard takes a different approach. It treats cooperating programs
as independent processes connected through exchanged text. The
exchange manages message lifecycle, not program behaviour.

This follows an older Unix tradition: small tools coordinated through
simple interfaces, often textual, with structure emerging through
convention rather than enforcement.

Plugboard also draws on the spool-directory tradition, where
independent programs coordinate asynchronously through a shared local
medium. The initial implementation uses SQLite rather than filesystem
directories, to simplify atomic claims, retention, and crash recovery
while preserving the same local and inspectable operating model.

## Design goals

Plugboard should be:

### Local-first

The exchange runs on a single machine and does not require a network
service.

### Text-first

Messages are textual. Programs interpret them independently.

### Minimal

The core manages message publication, claims, and inspection. It does
not try to understand workflows or agent behaviour.

### Decoupled

Programs should be able to cooperate without sharing a framework,
object model, or vendor runtime.

### Inspectable

The state of the exchange should be easy to observe and reason about.

### Practical for ordinary tools

The system should support both long-running workers and passive
command-line tools through small adapters.

## Non-goals

Plugboard is intentionally not:

* an agent framework
* a workflow orchestration engine
* a typed RPC system
* a distributed scheduler
* a vendor-specific AI runtime
* a general-purpose networked message broker
* a full task queue product competing on delivery guarantees

Plugboard should not encode business workflows, agent abstractions, or
rich control logic into the exchange itself.

## Core model

Plugboard has three cooperating layers:

### 1. Plugboard core exchange

The exchange stores messages and exposes operations over them.

It is responsible for:

* publishing messages
* listing or reading messages
* claiming messages for processing
* recording completion, failure, or timeout outcomes
* appending follow-up messages
* making the local coordination history inspectable

The core is agnostic to who reads messages. Delivery is topic-based,
not identity-based. Plugboard does not know whether a topic is read by
a human, a script, a worker host, or an agent wrapper.

### 2. Worker host

A worker host is an optional adapter runtime that turns the exchange
into an execution loop.

It is responsible for:

* waiting for matching messages
* claiming one message at a time
* invoking a plugin
* enforcing timeout and lifecycle rules
* publishing success, failure, or timeout follow-up messages

The worker host is not the exchange. It is a client of the exchange.

### 3. Plugins

Plugins implement actual behaviour behind a worker host.

A plugin may:

* wrap a command-line tool
* call an API or SDK
* adapt an awkward local CLI into a non-interactive contract

This keeps agent and tool behaviour outside the core while still
allowing asynchronous execution over Plugboard topics.

## Backend alternatives

Plugboard should stay open to multiple backend styles without making
the core agent-aware.

### Simple stateless transforms

This is the baseline worker shape:

* one message triggers one process
* message text goes in
* textual result comes out
* the process exits

This is ideal for shell filters, deterministic transforms, and simple
command adapters.

### Local model plugins

Plugboard also needs a strong local-model path. A local model plugin
can talk to a local inference runtime or service and make Plugboard
feel responsive during development.

This matters because a good product demo should not depend on the cold
start behaviour of a hosted agent CLI.

### Already-running agent or session-backed plugins

Some useful backends are warm, already-running processes. A plugin may
talk to one of those backends when cold start would otherwise dominate
latency.

That does not mean Plugboard should manage sessions. Session or
persistence concerns belong in the plugin layer, not in the core
exchange.

### API plugins

Some integrations are cleanest as direct API calls. A plugin can turn
a claimed message into an HTTP or SDK request and then return the
final textual result through the normal worker lifecycle.

From Plugboard's point of view, this is still the same pattern:

* publish
* claim
* execute plugin
* publish follow-up

The exchange remains topic-based and agent-agnostic.

## Message semantics

### Messages are textual

The body of a message is text. Plugboard does not require a typed
schema for message contents.

A message may also carry small metadata such as:

* message id
* topic
* creation time
* parent or causal reference
* producer identity
* correlation id or conversation id

The body remains the primary payload.

### Messages are immutable

Messages are append-only records of communication. Programs
communicate by appending new messages rather than modifying existing
ones.

This is not a commitment to permanent event-sourcing ideology or
infinite retention. It is a simpler semantic rule:

when a participant has something new to say, it writes a new message.

### Claims are separate from message content

Claiming a message is operational state, not message content.

This preserves the distinction between:

* what was communicated
* what operational step was taken to process it

For worker pools, Plugboard distinguishes between:

* a stable `worker_group`, which identifies the logical worker class or configuration
* an ephemeral `worker_instance_id`, which identifies one concrete running process

Claim ownership is recorded against the instance id, while the worker
group makes inspection and pool-level reasoning easier.

A claim is considered live only when:

* `status = active`
* `lease_until > now`

Lease expiry is the v1 stale-claim recovery mechanism. Plugboard does
not need a startup sweep, background sweeper, or heartbeat protocol to
recover from a crashed worker in this first version. Recovery happens
in the transactional claim path itself.

Terminal claims still remain as processing history in v1, so lease
recovery applies only to stale active ownership, not to messages that
already completed, failed, or timed out cleanly.

### Topics route interest

Each message has a topic. Topics are the primary mechanism for routing
interest.

Examples:

* code.generate
* code.generated
* review.request
* review.completed
* shell.exec
* shell.failed

The topic expresses coarse intent. The body carries the actual work
text.

Plugboard does not parse message bodies to decide routing.

## Activation model

Plugboard separates asynchronous exchange from participant activation.

The exchange stores and exposes messages. It does not control when or
how participants run.

### The exchange is asynchronous

Messages may be published independently of when they are consumed.

### Asynchronous vs blocking

Plugboard is asynchronous at the system level.

This means:

* a message may be published without an immediate consumer
* processing may happen later
* replies appear as new messages, not direct return values

This is independent of how any single participant waits.

A participant may choose to:

* block waiting for a message
* poll for messages
* publish and continue without waiting
* publish and later read results

Blocking is a local implementation choice. It does not make the
system synchronous.

A synchronous system would require direct invocation and immediate
return between participants.

Plugboard instead uses a shared exchange:

* publication, processing, and reply are decoupled in time
* participants do not share a call stack
* coordination happens through messages, not function calls

This allows both:

* blocking reactive workers that avoid polling
* non-blocking producers that continue immediately

The exchange remains asynchronous in both cases.

### Participants may be persistent or reactive

A participant may be:

* a long-running worker
* a reactive one-shot process
* a wrapper around a passive CLI tool
* a plugin adapter around an external system

### Persistent workers

A persistent worker is a long-running process.

It:

* continuously waits for matching messages
* claims and processes messages over time
* may use polling or blocking internally

This mode favours throughput and simplicity.

### Reactive (one-shot) workers

A reactive worker is a short-lived process.

It:

* waits for a matching message without polling
* claims exactly one message
* processes it
* publishes a follow-up
* exits immediately

Reactive mode is the preferred model for passive tools and local
integrations.

### Wakeup semantics

The exchange (SQLite) is the source of truth.

To avoid polling:

* publishing a message MAY coincide with a local wakeup hint
* workers MAY block waiting for such signals
* wakeups are advisory, not authoritative
* workers MUST always claim messages transactionally

The system must not rely on filesystem notification delivery for
correctness. A bounded re-check interval around blocking waits is
acceptable because the database remains authoritative.

In the current product, that bounded re-check defaults to 250 ms. So
when notifier delivery fails, correctness falls back to effectively
polling the database every 250 ms plus normal processing overhead.

### Design principle

The exchange stores state.
Activation is external.
Wakeups are advisory.
Claims are authoritative.

## Worker model

A minimal worker host is in scope because it demonstrates how passive
tools participate.

### Responsibilities

A worker host should:

* wait for matching messages (blocking or polling)
* claim one message at a time
* invoke a configured plugin
* pass message body and metadata
* collect stdout, stderr, exit status, and timeout
* publish success or failure follow-ups

### Execution contract

For simple command plugins:

* write message body to stdin
* read stdout as success output
* treat non-zero exit as failure
* capture stderr
* enforce timeout

### Worker mappings are external

Topic → plugin mappings are configuration, not protocol.

### Avoid orchestration

The worker host must not become a workflow engine.

## Storage model

### Initial backend: SQLite

Reasons:

* local single-file storage
* transactional semantics
* atomic claims
* crash recovery
* easy inspection

SQLite remains the source of truth even when wakeup mechanisms are
introduced.

## Retention and persistence

Persistence and retention are separate concerns.

Initial stance:

* persist locally
* allow cleanup
* do not require infinite retention

## Difference from other systems

### Not RabbitMQ

Not a general broker.

### Not Celery or Sidekiq

Not a structured job system.

### Not workflow engines

No central orchestration.

### Not an agent framework

No definition of “agent”.

## Plugin Model

Plugins receive:

* message body
* metadata
* execution context

And return:

* success or failure output

They should be:

* non-interactive
* terminating
* replaceable

## Operational principles

### Keep the core dumb

### Keep programs independent

### Prefer convention over schema

### Keep state local

### Make inspection easy

### Keep adapters replaceable

### Prefer event-driven activation

Polling is acceptable for persistent workers, but reactive paths
should avoid polling in favour of blocking and wakeup signals.

## Possible command surface

* plugboard publish
* plugboard read
* plugboard claim
* plugboard complete
* plugboard fail
* plugboard run
* plugboard inspect

Future additions may include request/reply helpers.

## First implementation milestone

A minimal prototype should support:

* publish
* read
* claim
* complete/fail
* follow-ups
* worker execution

## End-to-end example

1. publish `review.request`
2. worker claims
3. plugin runs
4. publish `review.done`

## Open questions

* schema details
* claim leases
* notifier mechanism
* retention policy
* worker parallelism

## Summary

Plugboard is a local textual exchange for coordinating independent
programs.

It is:

* text-first
* topic-based
* local
* inspectable
* framework-agnostic

It separates:

* communication (exchange)
* execution (workers/plugins)
* activation (external)

This gives Plugboard its identity:

a software switchboard for cooperating programs, built around text
rather than shared structure.
