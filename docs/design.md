# Plugboard Design

This document covers intent and boundaries. It answers what Plugboard is
for and why it is shaped this way. For implementation mechanics, see
[Architecture](architecture.md).

## Overview

Plugboard is a local textual exchange for cooperating programs.

It gives separate tools a shared, inspectable place to publish text,
read text, and coordinate work without requiring a shared SDK, object
model, or orchestration framework.

The exchange is the product. Workers and plugins are adapters around it.

## Why it exists

Many automation systems start from tight coupling:

* shared SDKs
* typed RPC
* centralized workflow engines
* agent frameworks with built-in identities and roles

That can work, but it also makes participants harder to replace and
harder to compose across tool boundaries.

Plugboard takes the opposite approach:

* keep the core local
* keep messages textual
* route by topic rather than identity
* leave behavior to processes outside the core

It is closer to a local spool or switchboard than to a distributed
workflow platform.

## Design goals

### Local-first

Plugboard should run on one machine with no required network service.

### Text-first

The primary payload is text. Rich behavior belongs in conventions and
plugins, not in a core schema.

### Minimal

The core should manage publication, reading, claims, and inspection. It
should not grow orchestration logic.

### Decoupled

Participants should be able to cooperate without sharing a framework,
runtime, or vendor-specific abstractions.

### Inspectable

Operators should be able to read the exchange directly and understand
what happened.

### Practical

The system should work with ordinary command-line tools, wrappers, and
small adapters, not only with custom frameworks.

## Non-goals

Plugboard is not:

* an agent framework
* a workflow engine
* a typed RPC system
* a distributed broker
* a scheduler with retries, priorities, and dead-letter queues
* a vendor-specific AI runtime

Those concerns belong outside the core.

## Core concepts

### Messages are textual and append-only

Participants communicate by appending messages. New information becomes
a new message rather than an in-place update.

### Topics route interest

Plugboard routes by topic. It does not do identity-based delivery.
Names such as `gemini.review.request` are conventions between
participants, not built-in addressing rules.

### Claims are operational state

Processing state is separate from communication. Claims say that a
worker is handling a message; they are not part of the message body.

### The exchange is asynchronous

The characteristic Plugboard workflow is:

1. publish work
2. do something else
3. read replies later

Blocking request/reply is allowed, but it is an edge convenience rather
than the primary model.

## System boundary

Plugboard has three layers:

### 1. Core exchange

Stores messages, records claims, and exposes an inspectable history.

### 2. Worker host

Claims matching messages, runs a backend, and publishes follow-up
messages.

### 3. Plugin or backend

Implements the actual behavior: shell command, local model adapter, API
call, or wrapper around another tool.

The boundary matters:

* design principles live in this document
* execution mechanics live in [Architecture](architecture.md)

## Operational principles

* Keep the core dumb.
* Prefer conventions over schemas.
* Keep state local and easy to inspect.
* Keep adapters replaceable.
* Do not encode workflow logic into the exchange.

## Backend philosophy

Plugboard should stay open to different backend styles:

* simple stateless transforms
* local model adapters
* warm or session-backed adapters
* direct API plugins

Those are plugin choices. They should not require changing the exchange
model.
