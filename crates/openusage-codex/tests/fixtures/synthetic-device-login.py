#!/usr/bin/python3
"""Owned test process. No network, real authentication, or credential-file access."""
import json
import os
import pathlib
import sys
import time

MODE = "__MODE__"
if sys.argv[1:] == ["--version"]:
    print("codex-cli 0.163.0", flush=True)
    sys.exit(0)

home = pathlib.Path(os.environ["CODEX_HOME"])
(home / "synthetic-child-pid").write_text(str(os.getpid()))
(home / "synthetic-runtime-home").write_text(os.environ["HOME"])


def read_request():
    value = json.loads(sys.stdin.readline())
    with (home / "synthetic-login-requests.jsonl").open("a") as stream:
        stream.write(json.dumps(value) + "\n")
    return value


def send(value):
    print(json.dumps(value), flush=True)


assert read_request()["method"] == "initialize"
send({"id": 1, "result": {"userAgent": "synthetic device login", "codexHome": str(home),
                         "platformFamily": "unix", "platformOs": "linux"}})
assert read_request()["method"] == "initialized"
request = read_request()
if request["method"] == "account/read":
    time.sleep(60)
    sys.exit(0)
assert request == {"id": 3, "method": "account/login/start", "params": {"type": "chatgptDeviceCode"}}
if MODE == "start_hang":
    (home / "synthetic-login-start-pending").write_text("ready")
    time.sleep(60)
completion = {"emittedAtMs": 1792573000123, "method": "account/login/completed", "params": {
    "loginId": "synthetic-device-attempt", "success": True, "error": None}}
if MODE == "early":
    send(completion)
if MODE == "flood":
    for _ in range(40):
        send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
send({"id": 3, "result": {"type": "chatgptDeviceCode", "loginId": "synthetic-device-attempt",
                         "verificationUrl": "https://chatgpt.com/codex/device" if MODE == "bad_url"
                         else "https://auth.openai.com/codex/device",
                         "userCode": "synthetic\u202e-code" if MODE == "bad_code" else "SYNTH-1234"}})
if MODE == "wrong":
    completion["params"]["loginId"] = "another-attempt"
elif MODE == "null":
    completion["params"]["loginId"] = None
elif MODE == "failed":
    completion["params"]["success"] = False
    completion["params"]["error"] = "synthetic private error; never expose"
if MODE == "updated_only":
    send({"method": "account/updated", "params": {"authMode": "chatgpt"}})
elif MODE not in ("pending", "cancel_unresponsive", "early", "bad_url", "bad_code", "flood"):
    send(completion)
request = read_request()
if request["method"] == "account/login/cancel":
    assert request == {"id": 5, "method": "account/login/cancel", "params": {"loginId": "synthetic-device-attempt"}}
    if MODE != "cancel_unresponsive":
        send({"id": 5, "result": {"status": "canceled"}})
elif request["method"] == "account/read":
    assert request == {"id": 4, "method": "account/read", "params": {"refreshToken": False}}
    send({"id": 4, "result": {"account": None if MODE == "account_missing" else {
        "type": "chatgpt", "email": "synthetic@example.invalid", "planType": "plus"},
        "requiresOpenaiAuth": True}})
else:
    raise AssertionError("Unexpected synthetic request")
time.sleep(60)
