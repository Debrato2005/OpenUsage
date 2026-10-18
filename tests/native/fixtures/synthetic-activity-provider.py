#!/usr/bin/python3
"""Owned activity transport fixture; no network, credentials, login, or inference."""
import datetime
import json
import os
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent
LOG = ROOT / "synthetic-activity-observations.jsonl"
CONFIG = ROOT / "synthetic-activity-provider-config.json"
ALLOWED_ENVIRONMENT = {"HOME", "CODEX_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR", "TMPDIR", "PATH", "LANG", "LC_CTYPE"}
MODES = {"success", "sparse", "null_summary", "null_buckets", "empty_buckets", "bound", "too_many", "unknown", "unsupported", "signed_out", "post_unknown", "changed_email", "changed_plan", "changed_auth", "early_auth", "auth_notice", "late_auth", "negative", "fractional", "string_count", "overflow", "missing_summary", "invalid_date", "duplicate_date", "thread_scope", "wrong_id", "rpc_error", "unsupported_method", "malformed", "oversized", "partial", "pending", "post_hold", "cleanup_fault"}


def observe(kind, **fields):
    with LOG.open("a", encoding="utf8") as stream:
        stream.write(json.dumps({"kind": kind, "pid": os.getpid(), "processGroup": os.getpgrp(), **fields}) + "\n")
    LOG.chmod(0o600)


observe("start", arguments=sys.argv[1:], environment={name: os.environ[name] for name in ALLOWED_ENVIRONMENT if name in os.environ}, unexpectedEnvironmentKeys=sorted(set(os.environ) - ALLOWED_ENVIRONMENT))
if sys.argv[1:] == ["--version"]:
    print("codex-cli 0.163.0", flush=True)
    sys.exit(0)
assert sys.argv[1:] == ["app-server", "--listen", "stdio://", "-c", 'cli_auth_credentials_store="file"', "-c", "analytics.enabled=false", "-c", "feedback.enabled=false", "-c", 'otel.exporter="none"', "-c", 'otel.metrics_exporter="none"', "-c", "check_for_update_on_startup=false"]
home = pathlib.Path(os.environ["CODEX_HOME"])


def mode():
    try:
        value = json.loads(CONFIG.read_text(encoding="utf8"))
    except FileNotFoundError:
        value = {}
    assert set(value) <= {"defaultMode", "modesByHome"}
    selected = value.get("modesByHome", {}).get(str(home), value.get("defaultMode", "success"))
    assert selected in MODES
    return selected


MODE = mode()


def read_request():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    value = json.loads(line)
    observe("request", request=value)
    return value


def send(value):
    print(json.dumps(value), flush=True)


def account(request_id, final=False):
    result = {"account": {"type": "chatgpt", "email": "synthetic-activity@example.invalid", "planType": "plus"}, "requiresOpenaiAuth": True}
    if MODE == "unknown":
        result["account"]["email"] = None
    if MODE == "unsupported":
        result["account"] = {"type": "apiKey"}
    if MODE == "signed_out" or MODE == "post_unknown" and final:
        result["account"] = None
    if MODE == "changed_email" and final:
        result["account"]["email"] = "changed@example.invalid"
    if MODE == "changed_plan" and final:
        result["account"]["planType"] = "pro"
    if MODE == "changed_auth" and final:
        result["requiresOpenaiAuth"] = False
    send({"id": request_id, "result": result})


assert read_request()["method"] == "initialize"
send({"id": 1, "result": {"userAgent": "synthetic private activity provider", "codexHome": str(home), "platformFamily": "unix", "platformOs": "linux"}})
assert read_request() == {"method": "initialized", "params": {}}
assert read_request() == {"id": 2, "method": "account/read", "params": {"refreshToken": False}}
if MODE == "early_auth":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
account(2)
if MODE in ["unknown", "unsupported", "signed_out", "early_auth"]:
    time.sleep(60)
    sys.exit(0)
assert read_request() == {"id": 3, "method": "account/usage/read", "params": {}}
observe("activity-read-pending", mode=MODE)
while mode() == "pending":
    time.sleep(0.05)
if MODE == "partial":
    sys.stdout.write('{"id":3')
    sys.stdout.flush()
    time.sleep(60)
    sys.exit(0)
if MODE == "oversized":
    print("x" * 1048577, flush=True)
    time.sleep(60)
    sys.exit(0)
if MODE == "malformed":
    print("{", flush=True)
    time.sleep(60)
    sys.exit(0)
if MODE in {"rpc_error", "unsupported_method"}:
    send({"id": 3, "error": {"code": -32601 if MODE == "unsupported_method" else -32603, "message": "synthetic-private-activity-error"}})
    time.sleep(60)
    sys.exit(0)
if MODE == "auth_notice":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
if MODE == "sparse":
    send({"method": "account/rateLimits/updated", "params": {"rateLimits": {"primary": {"usedPercent": 99}}}})
summary = {"lifetimeTokens": 9007199254740993, "peakDailyTokens": 9223372036854775807,
           "longestRunningTurnSec": None, "currentStreakDays": 0, "longestStreakDays": None}
buckets = [{"startDate": "2024-02-29", "tokens": 9007199254740993}, {"startDate": "2024-03-01", "tokens": 0}]
result = {"summary": summary, "dailyUsageBuckets": buckets, "threadUsage": None,
          "accountId": "synthetic-private-activity-id", "workTokens": 123, "modelCounts": {"synthetic-private-model": 7}}
if MODE == "null_summary":
    result["summary"] = {key: None for key in summary}
if MODE == "null_buckets":
    result["dailyUsageBuckets"] = None
if MODE == "empty_buckets":
    result["dailyUsageBuckets"] = []
if MODE in ["bound", "too_many"]:
    result["dailyUsageBuckets"] = [{"startDate": (datetime.date(2020, 1, 1) + datetime.timedelta(days=index)).isoformat(), "tokens": index}
                                  for index in range(4096 if MODE == "bound" else 4097)]
if MODE == "negative":
    summary["lifetimeTokens"] = -1
if MODE == "fractional":
    summary["lifetimeTokens"] = 1.5
if MODE == "string_count":
    summary["lifetimeTokens"] = "9007199254740993"
if MODE == "overflow":
    summary["lifetimeTokens"] = 9223372036854775808
if MODE == "missing_summary":
    del result["summary"]
if MODE == "invalid_date":
    buckets[0]["startDate"] = "2023-02-29"
if MODE == "duplicate_date":
    buckets[1]["startDate"] = buckets[0]["startDate"]
if MODE == "thread_scope":
    result["threadUsage"] = {"threadId": "synthetic-private-thread", "estimatedUsageCreditsMicros": 1, "groups": []}
if MODE == "cleanup_fault":
    runtime = pathlib.Path(os.environ["HOME"]).parent
    runtime.rename(runtime.with_name(runtime.name + "-moved"))
send({"id": 77 if MODE == "wrong_id" else 3, "result": result})
assert read_request() == {"id": 4, "method": "account/read", "params": {"refreshToken": False}}
if MODE == "post_hold":
    observe("post-read-pending")
    while mode() == "post_hold":
        time.sleep(0.005)
if MODE == "late_auth":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
account(4, final=True)
time.sleep(60)
