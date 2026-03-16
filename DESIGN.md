# Plugboard Design

## Overview

Plugboard is a local textual message exchange for cooperating
programs.

It provides a small local coordination surface where independent
programs publish and consume text. It does not define an agent
framework, workflow engine, or shared object model. Its purpose is to
let separately built tools cooperate through loose textual interfaces.

Programs interact with Plugboard by appending messages to the exchange
and by reading or claiming messages that match their
interests. Optional runners watch selected topics, invoke ordinary
commands, and publish results back into the exchange.

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

Plugboard has two main concerns:

### 1. The exchange

The exchange stores messages and exposes operations over them.

It is responsible for:

* publishing messages
* listing or reading messages
* claiming messages for processing
* recording completion, failure, or timeout outcomes
* appending follow-up messages
* making the local coordination history inspectable

### 2. The runner

A runner is an optional edge adapter that turns an ordinary command
into an asynchronous participant.

It is responsible for:

* watching for messages of interest
* claiming a message
* invoking a command
* passing message text to that command
* capturing stdout, stderr, exit status, and timeout
* publishing result or failure messages back into the exchange

The runner is not the exchange. It is a client of the exchange.

This separation is important. Plugboard stores and exposes
messages. Runners implement local activation policy.

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

This keeps the exchange focused on communication records rather than
mutable task objects.

### Claims are separate from message content

Claiming a message is operational state, not message content. The
initial implementation may store claim state in separate tables or
related records.

This keeps the message log itself clean and preserves the distinction
between:

* what was communicated
* what operational step was taken to process it

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

Plugboard should not rely on parsing message bodies to decide which
runner should react.

## Activation model

One of the central questions for Plugboard is how participants are
activated.

Not every cooperating tool is a long-running service. Many AI tools
and command-line programs are passive and only perform work when
invoked.

Plugboard therefore separates asynchronous exchange from participant
activation.

### The exchange is asynchronous

Messages may be published independently of when they are consumed.

### Participants may be active or passive

A participant may be:

* a long-running worker process
* a polling command
* a wrapper around a passive CLI tool
* an interactive or user-mediated tool invoked by a runner

### Runners bridge passive tools

A runner allows an ordinary command-line program to participate in
asynchronous coordination without becoming a service.

The runner provides the missing loop:

* wait or poll for matching messages
* claim one
* invoke a command
* publish the result

This is a key property of Plugboard. It should support asynchronous
coordination without requiring all participants to be long-lived
daemons.

### Push is not required in the core

The initial design does not require true broker-style push delivery
from the exchange into subscribers.

The system may support:

* polling
* blocking reads
* lightweight local notifications

But Plugboard should avoid making callback registration, service
liveness, or network delivery semantics part of the core model.

## Runner model

A minimal runner is in scope because it demonstrates the usefulness of
the exchange for passive tools.

Without a runner, Plugboard risks collapsing into a small message
store with no clear distinction from ordinary queueing tools.

### Minimal responsibilities

A runner should do only a few things:

* select messages by topic, optionally with simple metadata filters
* claim one message at a time
* invoke a configured command
* pass message body to stdin, file, or arguments
* collect stdout, stderr, exit status, and timeout
* publish success or failure follow-up messages

### Runner mappings are external configuration

Plugboard itself should not know that a given topic means a specific
command must be invoked.

Mappings from topic to command are local policy and belong in runner
configuration, not in the exchange protocol.

For example, a runner configuration may say:

* match topic code.generate
* invoke codex exec
* publish success to code.generated
* publish failure to code.generate.failed
* enforce timeout 120 seconds

This is configuration at the edge, not logic embedded in the exchange.

### Avoid rich orchestration in the runner

The runner should remain small. It should not become:

* a workflow engine
* a DAG executor
* a scheduler
* a retry platform
* a policy language

If more complex coordination is needed, that coordination should
emerge through programs publishing further messages, not through a
richer central controller.

## Storage model

Storage is an implementation detail, but it shapes operational
behaviour enough to state clearly.

### Initial backend: SQLite

The initial implementation should use SQLite.

Reasons:

* local single-file storage
* transactional semantics
* atomic claims
* easier crash recovery
* straightforward retention and cleanup
* easy inspection with ordinary tools
* no separate service required

SQLite is a better fit than spool directories for reliability and a
better fit than DuckDB for transactional coordination state.

DuckDB is excellent for analytical workloads. Plugboard v1 is not an
analytical system. It is a small local transactional exchange.

### Spool-directory lineage, not spool-directory implementation

Plugboard follows the spool-directory coordination style conceptually,
but does not need to implement it literally using filesystem
directories.

The core model remains the same:

* publish a work item
* claim a work item
* process it
* append follow-up records

SQLite provides a more robust implementation of this model.

## Retention and persistence

Persistence and retention should not be treated as ideological
commitments.

These are separate choices:

* message semantics: immutable records
* storage backend: SQLite
* retention policy: bounded, configurable, or persistent

The first version should keep retention simple and practical.

Recommended initial stance:

* persist messages locally in SQLite
* provide a straightforward cleanup or archive mechanism
* do not require infinite retention
* do not require replay guarantees
* do not force durability policies beyond local persistence

A persistent local history is useful for inspection and debugging, but
Plugboard should not drift into becoming an event-sourcing platform.

## Difference from other systems

Plugboard may resemble several existing tool categories, but its
intended identity is narrower and more opinionated.

### Not RabbitMQ

RabbitMQ is a general message broker focused on routing and delivery
semantics.

Plugboard is a local textual coordination surface for cooperating
programs.

It is not trying to compete on broker features, network distribution,
exchange types, or enterprise delivery guarantees.

### Not Celery or Sidekiq

Those systems are job queues built around structured job execution and
framework-specific worker models.

Plugboard is intentionally text-first and should support independently
built programs that need not share a framework or job object model.

### Not Airflow, Temporal, or workflow engines

Those tools encode orchestration logic, retries, scheduling, and
workflow graphs.

Plugboard should keep orchestration at the edges, expressed through
further messages rather than central workflow state.

### Not an agent framework

Plugboard does not define what an agent is. It only provides a small
local exchange where ordinary programs can coordinate through text.

This is a core identity constraint.

## Operational principles

The following principles should guide implementation decisions:

### Keep the core dumb

The exchange should manage message lifecycle and little else.

### Keep programs independent

Participants remain ordinary processes.

### Prefer convention over schema

Topics and textual conventions are enough for initial coordination.

### Keep state local

The system should run without requiring external infrastructure.

### Make inspection easy

A user should be able to understand what happened by reading the
exchange state.

### Keep adapters replaceable

The built-in runner should be small enough that someone could replace
it with their own runner without changing the exchange model.

## Possible command surface

The CLI should stay small and direct.

A plausible initial set is:

* plugboard publish
* plugboard read
* plugboard claim
* plugboard complete
* plugboard fail
* plugboard run
* plugboard inspect

These commands should expose the model plainly rather than hiding it
behind heavy abstractions.

The most important command after publish is likely run, because it
demonstrates how passive tools can participate asynchronously through
a thin wrapper.

## First implementation milestone

The first milestone should prove the model, not maximize features.

A minimal useful prototype should support:

* publishing a textual message with a topic
* listing or reading messages by topic
* claiming a message atomically
* recording completion or failure
* publishing follow-up messages that reference earlier ones
* running a configured command against claimed messages
* publishing the command output as a new message

This is enough to show three independent programs cooperating through
a local textual exchange without a shared framework.

## Open questions

The first design intentionally leaves several questions open:

* exact schema for messages and claims
* how claim leases and timeouts are represented
* whether blocked reads are supported initially or only polling
* how much metadata is first-class versus conventional
* what retention and cleanup commands look like
* whether the runner supports parallelism in v1 or stays
  single-message-at-a-time

These questions should be resolved in implementation only as
needed. The project should not overdesign them up front.

## Summary

Plugboard is a small local exchange for textual coordination between
independent programs.

Its core commitments are:

* text-first messages
* immutable communication records
* topic-based routing of interest
* activation handled by optional runners
* local inspectable state
* no agent framework
* no workflow engine
* no typed inter-program contract requirement

The initial implementation uses SQLite to provide a practical local
transactional backend while preserving the spool-directory style
coordination model that inspires the design.

This gives Plugboard a clear identity:

a software switchboard for cooperating programs, built around
exchanged text rather than shared structure.
