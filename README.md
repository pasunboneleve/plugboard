Plugboard
=========

**Plugboard** is a local textual message exchange for cooperating
programs.

Plugboard provides a small local exchange where independent programs
coordinate by publishing and consuming text. It does not define an
agent framework, workflow DSL, or shared object model. Its purpose is
to let separately-built tools cooperate through loose textual
interfaces.

Programs interact with Plugboard by appending messages and consuming
messages that match their interests. Each participant remains
independent. The system acts as a **software switchboard** for
cooperating processes.

---

Why
---

Many modern automation and AI systems rely on tightly coupled frameworks:

- shared SDKs
- rigid schemas
- centralized orchestration
- strongly typed RPC between services

These approaches couple tools together and make systems harder to evolve.

Unix took a different path: small programs communicating through
**text streams**. Tools could be composed without sharing internal
structure.

Plugboard applies a similar idea to cooperating programs:

- programs remain independent
- coordination happens through exchanged text
- structure emerges through conventions rather than enforced APIs

The exchange only manages message lifecycle. It does not attempt to understand program behaviour.

---

Design goals
------------

- **Local-first**
  Designed to run on a single machine.

- **Text-first**
  Messages are textual. Programs interpret them independently.

- **Minimal core**
  The exchange manages message storage and delivery, not behaviour.

- **Decoupled participants**
  Programs do not need to share a framework or object model.

- **Inspectable system**
  The message exchange should be easy to observe and reason about.

---

Non-goals
---------

Plugboard intentionally avoids several common platform features.

Plugboard is not:

- an agent framework
- a workflow orchestration engine
- a typed RPC system
- a vendor specific AI runtime
- a distributed task scheduler

Programs remain ordinary processes outside the exchange.

---

Conceptual model
----------------

Programs publish textual messages into the exchange. Other programs
consume messages that match their interests.


             ┌─────────────────────────┐
             │        Plugboard        │
             │   textual message bus   │
             └───────────┬─────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
   publishes         publishes        publishes
        │                │                │
        ▼                ▼                ▼
  ┌──────────┐     ┌──────────┐     ┌──────────┐
  │ program  │     │ program  │     │ program  │
  │ planner  │     │ coder    │     │ reviewer │
  └────┬─────┘     └────┬─────┘     └────┬─────┘
       │                │                │
       └──── consumes matching textual messages ────┘

The exchange manages:

- message storage
- message visibility
- claiming and acknowledging work

It does not define what the messages mean.

---

Example message flow
--------------------

Program A publishes a request:

```
topic: code.generate
body:
Write a Python function that merges two sorted lists.
```

Program B consumes messages from `code.generate` and produces a
result:

```
topic: code.generated
body:
def merge(a, b):
    ...
```

Plugboard routes messages. Programs decide how to interpret them.

---

First milestone
---------------

The first implementation should remain deliberately small.

A minimal exchange should support:

- publishing a textual message
- listing or polling messages by topic
- claiming a message for processing
- acknowledging completion
- publishing follow-up messages

The system should remain easy to inspect locally.

---

Philosophy
----------

Plugboard follows the Unix tradition of loose composition:

- programs communicate through text
- structure emerges from usage
- the coordination mechanism stays simple

The exchange connects programs without forcing them into a shared framework.

---

Status
------

Early design stage.

---

Licence
-------

MIT
