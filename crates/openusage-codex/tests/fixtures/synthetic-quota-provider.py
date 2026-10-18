#!/usr/bin/python3
"""Owned transport fixture; no network, credential reads, login, or inference."""
import json
import os
import pathlib
import sys
import time

MODE = "__MODE__"
if sys.argv[1:] == ["--version"]:
    print("codex-cli 0.163.0", flush=True)
    sys.exit(0)
assert not any(name in os.environ for name in ["OPENAI_API_KEY", "OPENAI_ACCESS_TOKEN", "DBUS_SESSION_BUS_ADDRESS"])
home = pathlib.Path(os.environ["CODEX_HOME"])
(home / "synthetic-child-pid").write_text(str(os.getpid()))
(home / "synthetic-runtime-home").write_text(os.environ["HOME"])


def read_request():
    value = json.loads(sys.stdin.readline())
    with (home / "synthetic-quota-requests.jsonl").open("a") as stream:
        stream.write(json.dumps(value) + "\n")
    return value


def send(value):
    print(json.dumps(value), flush=True)


def account(request_id, final=False):
    result = {"account": {"type": "chatgpt", "email": "synthetic@example.invalid", "planType": "plus"}, "requiresOpenaiAuth": True}
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
send({"id": 1, "result": {"userAgent": "synthetic quota provider", "codexHome": str(home), "platformFamily": "unix", "platformOs": "linux"}})
assert read_request()["method"] == "initialized"
assert read_request() == {"id": 2, "method": "account/read", "params": {"refreshToken": False}}
if MODE == "early_auth":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
account(2)
if MODE in ["unknown", "unsupported", "signed_out", "early_auth"]:
    time.sleep(60)
    sys.exit(0)
assert read_request() == {"id": 3, "method": "account/rateLimits/read", "params": {}}
(home / "synthetic-quota-pending").write_text("ready")
if MODE == "hang":
    time.sleep(60)
    sys.exit(0)
if MODE == "auth_notice":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
if MODE == "sparse":
    send({"method": "account/rateLimits/updated", "params": {"rateLimits": {"limitId": "codex", "primary": {"usedPercent": 90}}}})
limit = {"limitId": "different" if MODE == "mismatch" else "codex", "primary": {"usedPercent": 101 if MODE == "invalid" else 25, "windowDurationMins": 300, "resetsAt": 1893456000}, "secondary": None}
send({"id": 3, "result": {"accountId": "synthetic-private-account-id", "rateLimitsByLimitId": {"codex": limit}, "rateLimits": {"limitId": "must-not-double-count"}}})
if MODE in ["auth_notice", "invalid", "mismatch"]:
    time.sleep(60)
    sys.exit(0)
assert read_request() == {"id": 4, "method": "account/read", "params": {"refreshToken": False}}
if MODE == "late_auth":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
account(4, final=True)
time.sleep(60)
