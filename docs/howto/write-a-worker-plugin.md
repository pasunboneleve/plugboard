# Write a Worker Plugin

## Overview

Plugboard is the core exchange. It stores messages, claims work, and
records follow-up messages. Worker hosts are the long-running
processes that listen on topics and execute plugins. Plugins define
how a claimed message is handled.

This keeps Plugboard unaware of the backend. It does not know whether
the work is handled by a shell command, an API call, or a wrapper
around another tool.

## Minimal Plugin Model

Conceptually, a plugin receives:

* the message body
* selected message metadata
* worker context such as timeout or worker name

And returns one of:

* success output
* failure output
* timeout, enforced by the worker host

Topic selection and message claiming stay in the worker host. The
plugin only handles execution.

## Command Plugin

The current baseline plugin is the command plugin:

* spawn a subprocess
* write the message body to `stdin`
* close `stdin`
* capture `stdout` and `stderr`
* map the exit code into success or failure

This stdin to stdout contract is the default execution model because
it keeps the worker simple and Unix-like.

## When You Need an Adapter

Some tools are not good worker plugins by themselves:

* interactive REPLs or TUIs
* commands that do not terminate
* commands that ignore `stdin` or expect prompts

In those cases, wrap the tool in a small adapter that presents a
non-interactive interface to Plugboard. The wrapper should accept the
message on `stdin`, run the awkward tool in a controlled way, emit a
single result, and exit.

## Future Plugin Types

Other plugin types can fit the same worker model:

* API-based plugins using an LLM SDK or HTTP client
* session-based plugins for long-lived tools
* wrappers around external tools that need adaptation
