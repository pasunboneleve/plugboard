#!/usr/bin/env python3
"""Measure direct, reactive, and persistent local latency paths."""

from __future__ import annotations

import sqlite3
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
PLUGBOARD = REPO / "target" / "debug" / "plugboard"
EXAMPLE_PLUGIN = REPO / "target" / "debug" / "example-review-plugin"
BODY = "Explain Rust ownership in one short paragraph."


def ensure_binary(path: Path) -> None:
    if not path.exists():
        raise SystemExit(
            f"missing {path}; run `cargo build --bin plugboard --bin example-review-plugin` first"
        )


def direct_once() -> float:
    start = time.time()
    subprocess.run(
        [str(EXAMPLE_PLUGIN)],
        input=BODY + "\n",
        text=True,
        capture_output=True,
        check=True,
    )
    return round((time.time() - start) * 1000, 3)


def exchange_once(persistent: bool) -> dict[str, str | float]:
    db = tempfile.NamedTemporaryFile(prefix="plugboard-latency-", suffix=".db", delete=False)
    db.close()
    topic = f"review_latency_{'persistent' if persistent else 'reactive'}_{time.time_ns()}"
    success = f"{topic}_done"
    failure = f"{topic}_failed"

    worker_args = [
        str(PLUGBOARD),
        "--database",
        db.name,
        "run",
    ]
    if not persistent:
        worker_args.append("--once")
    worker_args += [
        "--topic",
        topic,
        "--success-topic",
        success,
        "--failure-topic",
        failure,
        "--",
        str(EXAMPLE_PLUGIN),
    ]
    worker = subprocess.Popen(
        worker_args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    time.sleep(0.2)

    start = time.time()
    request = subprocess.run(
        [
            str(PLUGBOARD),
            "--database",
            db.name,
            "request",
            topic,
            "--success-topic",
            success,
            "--failure-topic",
            failure,
            "--body",
            BODY,
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    end = time.time()

    if persistent:
        worker.terminate()
        try:
            worker.wait(timeout=5)
        except subprocess.TimeoutExpired:
            worker.kill()
            worker.wait(timeout=5)
    else:
        worker.wait(timeout=5)

    connection = sqlite3.connect(db.name)
    cursor = connection.cursor()
    request_id, request_created = cursor.execute(
        "select id, created_at from messages where topic = ? order by created_at desc limit 1",
        (topic,),
    ).fetchone()
    claim_created, claim_completed = cursor.execute(
        "select claimed_at, completed_at from claims where message_id = ? order by claimed_at desc limit 1",
        (request_id,),
    ).fetchone()
    follow_created = cursor.execute(
        "select created_at from messages where topic = ? order by created_at desc limit 1",
        (success,),
    ).fetchone()[0]
    Path(db.name).unlink(missing_ok=True)

    return {
        "request_created": request_created,
        "claim_created": claim_created,
        "claim_completed": claim_completed,
        "follow_created": follow_created,
        "request_reply_ms": round((end - start) * 1000, 3),
        "reply": request.stdout.strip(),
    }


def main() -> None:
    ensure_binary(PLUGBOARD)
    ensure_binary(EXAMPLE_PLUGIN)

    direct = [direct_once() for _ in range(5)]
    reactive = [exchange_once(persistent=False) for _ in range(5)]
    persistent = [exchange_once(persistent=True) for _ in range(5)]

    print("Direct plugin invocation (ms):", direct)
    print("Direct median (ms):", statistics.median(direct))
    print()
    print("Reactive request/reply (ms):", [sample["request_reply_ms"] for sample in reactive])
    print(
        "Reactive median (ms):",
        statistics.median(sample["request_reply_ms"] for sample in reactive),
    )
    print("Reactive sample:", reactive[0])
    print()
    print(
        "Persistent request/reply (ms):",
        [sample["request_reply_ms"] for sample in persistent],
    )
    print(
        "Persistent median (ms):",
        statistics.median(sample["request_reply_ms"] for sample in persistent),
    )
    print("Persistent sample:", persistent[0])


if __name__ == "__main__":
    main()
