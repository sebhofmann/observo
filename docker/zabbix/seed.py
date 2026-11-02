#!/usr/bin/env python3
"""Seed the local Zabbix instance with deterministic demo problems.

Only the Python standard library is required.

Examples:
    python3 docker/zabbix/seed.py setup     # create hosts, items and triggers
    python3 docker/zabbix/seed.py trigger   # raise the demo problems
    python3 docker/zabbix/seed.py recover   # resolve the demo problems
    python3 docker/zabbix/seed.py status    # print current problems
    python3 docker/zabbix/seed.py ack       # acknowledge the newest problem
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request

DEFAULT_BASE_URL = "http://localhost:8080"
USERNAME = "Admin"
PASSWORD = "zabbix"
GROUP_NAME = "Observo Demo"
TRAPPER_HOSTS = "127.0.0.1,172.16.0.0/12,192.168.0.0/16,10.0.0.0/8"

# host, item name, item key, trigger description, severity, comparison,
# threshold, raised value, healthy value
PROBLEMS = [
    (
        "web-01",
        "CPU load (1m)",
        "demo.cpu.load",
        "High CPU load on web-01",
        4,  # High
        ">",
        50,
        93,
        8,
    ),
    (
        "db-01",
        "Disk usage /var (%)",
        "demo.disk.usage",
        "Disk space critically low on db-01",
        5,  # Disaster
        ">",
        80,
        96,
        41,
    ),
    (
        "cache-01",
        "Memory usage (%)",
        "demo.memory.usage",
        "Memory usage high on cache-01",
        2,  # Warning
        ">",
        75,
        87,
        44,
    ),
    (
        "web-02",
        "nginx workers",
        "demo.nginx.workers",
        "nginx is not running on web-02",
        4,  # High
        "<",
        1,
        0,
        5,
    ),
    (
        "batch-01",
        "Queue backlog",
        "demo.queue.backlog",
        "Nightly batch job is falling behind",
        3,  # Average
        ">",
        100,
        1240,
        3,
    ),
]


class ZabbixError(RuntimeError):
    pass


class Zabbix:
    def __init__(self, base_url: str, username: str, password: str) -> None:
        self.endpoint = base_url.rstrip("/") + "/api_jsonrpc.php"
        self.username = username
        self.password = password
        self.auth = ""
        self.request_id = 0

    def call(self, method: str, params=None):
        self.request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params if params is not None else {},
            "id": self.request_id,
        }
        headers = {"Content-Type": "application/json-rpc"}
        if self.auth:
            headers["Authorization"] = f"Bearer {self.auth}"
        request = urllib.request.Request(
            self.endpoint,
            data=json.dumps(payload).encode(),
            headers=headers,
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = json.loads(response.read())
        except urllib.error.HTTPError as error:
            detail = error.read().decode(errors="replace")
            raise ZabbixError(f"HTTP {error.code}: {detail}") from error
        except urllib.error.URLError as error:
            raise ZabbixError(f"cannot reach {self.endpoint}: {error.reason}") from error
        if "error" in body:
            err = body["error"]
            raise ZabbixError(err.get("data") or err.get("message") or str(err))
        return body.get("result")

    def login(self) -> None:
        self.auth = self.call(
            "user.login", {"username": self.username, "password": self.password}
        )


def first(result):
    return result[0] if result else None


def ensure_group(zbx: Zabbix) -> str:
    group = first(zbx.call("hostgroup.get", {"filter": {"name": [GROUP_NAME]}}))
    if group:
        return group["groupid"]
    return zbx.call("hostgroup.create", {"name": GROUP_NAME})["groupids"][0]


def ensure_host(zbx: Zabbix, group_id: str, host: str) -> str:
    existing = first(zbx.call("host.get", {"filter": {"host": [host]}}))
    if existing:
        return existing["hostid"]
    return zbx.call(
        "host.create",
        {
            "host": host,
            "name": host,
            "groups": [{"groupid": group_id}],
            "interfaces": [
                {
                    "type": 1,
                    "main": 1,
                    "useip": 1,
                    "ip": "127.0.0.1",
                    "dns": "",
                    "port": "10050",
                }
            ],
            "tags": [{"tag": "observo-demo", "value": "1"}],
        },
    )["hostids"][0]


def ensure_item(zbx: Zabbix, host_id: str, name: str, key: str) -> str:
    existing = first(
        zbx.call(
            "item.get",
            {"hostids": host_id, "filter": {"key_": [key]}, "output": ["itemid"]},
        )
    )
    if existing:
        return existing["itemid"]
    return zbx.call(
        "item.create",
        {
            "hostid": host_id,
            "name": name,
            "key_": key,
            "type": 2,  # Zabbix trapper
            "value_type": 3,  # numeric unsigned
            "delay": "0",
            "trapper_hosts": TRAPPER_HOSTS,
            "tags": [{"tag": "observo-demo", "value": "1"}],
        },
    )["itemids"][0]


def ensure_trigger(
    zbx: Zabbix,
    host: str,
    description: str,
    key: str,
    severity: int,
    comparison: str,
    threshold: int,
) -> str:
    existing = first(
        zbx.call(
            "trigger.get",
            {
                "filter": {"description": [description]},
                "output": ["triggerid", "expression"],
            },
        )
    )
    expression = f"last(/{host}/{key}){comparison}{threshold}"
    if existing:
        # Repair an expression left over from an older seed run.
        if existing.get("expression") and existing["expression"] != expression:
            zbx.call(
                "trigger.update",
                {"triggerid": existing["triggerid"], "expression": expression},
            )
        return existing["triggerid"]
    return zbx.call(
        "trigger.create",
        {
            "description": description,
            "expression": expression,
            "priority": severity,
            "opdata": "Current value: {ITEM.LASTVALUE1}",
            "manual_close": 1,
            "tags": [{"tag": "observo-demo", "value": "1"}],
        },
    )["triggerids"][0]


def setup(zbx: Zabbix) -> None:
    group_id = ensure_group(zbx)
    for (
        host,
        item_name,
        key,
        description,
        severity,
        comparison,
        threshold,
        _raised,
        _healthy,
    ) in PROBLEMS:
        host_id = ensure_host(zbx, group_id, host)
        ensure_item(zbx, host_id, item_name, key)
        ensure_trigger(
            zbx, host, description, key, severity, comparison, threshold
        )
        print(f"  {host:<10} {description}")


def push(zbx: Zabbix, host: str, key: str, value: int) -> None:
    zbx.call("history.push", [{"host": host, "key": key, "value": value}])


def trigger(zbx: Zabbix) -> None:
    for (
        host,
        _item_name,
        key,
        _description,
        _severity,
        _comparison,
        _threshold,
        raised,
        _healthy,
    ) in PROBLEMS:
        push(zbx, host, key, raised)
    print("Raised demo problems. Zabbix needs a few seconds to evaluate triggers.")


def recover(zbx: Zabbix) -> None:
    for (
        host,
        _item_name,
        key,
        _description,
        _severity,
        _comparison,
        _threshold,
        _raised,
        healthy,
    ) in PROBLEMS:
        push(zbx, host, key, healthy)
    print("Recovered demo problems.")


def status(zbx: Zabbix) -> None:
    triggers = zbx.call(
        "trigger.get",
        {
            "only_true": True,
            "monitored": True,
            "active": True,
            "output": ["triggerid", "description", "priority", "lastchange"],
            "selectHosts": ["host", "name"],
            "selectLastEvent": [
                "eventid",
                "name",
                "clock",
                "acknowledged",
                "value",
                "severity",
                "opdata",
            ],
            "sortfield": "lastchange",
            "sortorder": "DESC",
        },
    )
    if not triggers:
        print("No active problems.")
        return
    for trigger in triggers:
        event = trigger.get("lastEvent") or {}
        hosts = ", ".join(h.get("name", h.get("host", "?")) for h in trigger.get("hosts", []))
        ack = "ACK" if event.get("acknowledged") == "1" else "open"
        print(
            f"[{trigger.get('priority')}] {trigger.get('description')} "
            f"({hosts}) event={event.get('eventid', '-')} {ack}"
        )


def acknowledge(zbx: Zabbix, message: str) -> None:
    event = first(
        zbx.call(
            "event.get",
            {
                "value": 1,
                "source": 0,
                "object": 0,
                "acknowledged": False,
                "sortfield": ["clock", "eventid"],
                "sortorder": "DESC",
                "limit": 1,
                "output": ["eventid", "name"],
            },
        )
    )
    if not event:
        print("Nothing to acknowledge.")
        return
    zbx.call(
        "event.acknowledge",
        {"eventids": event["eventid"], "action": 2 | 4, "message": message},
    )
    print(f"Acknowledged event {event['eventid']}: {event.get('name', '')}")


def connect(base_url: str, user: str, password: str, timeout: float = 240.0) -> "Zabbix":
    """Waits for the web frontend and a working API login."""
    deadline = time.monotonic() + timeout
    last_error = "web frontend not reachable"
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(base_url, timeout=5) as response:
                if response.status >= 500:
                    time.sleep(3)
                    continue
        except Exception as error:
            last_error = str(error)
            time.sleep(3)
            continue
        try:
            zbx = Zabbix(base_url, user, password)
            zbx.login()
            return zbx
        except ZabbixError as error:
            last_error = str(error)
            time.sleep(3)
    raise SystemExit(f"Zabbix did not become ready at {base_url}: {last_error}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=[
            "setup",
            "trigger",
            "recover",
            "status",
            "ack",
            "wait",
            "demo",
            "token",
        ],
        help="what to do",
    )
    parser.add_argument("--url", default=DEFAULT_BASE_URL, help="Zabbix base URL")
    parser.add_argument("--user", default=USERNAME)
    parser.add_argument("--password", default=PASSWORD)
    parser.add_argument("--message", default="Acknowledged from observo demo fixture")
    args = parser.parse_args()

    zbx = connect(args.url, args.user, args.password)
    try:
        if args.command == "wait":
            print("Zabbix is ready.")
        elif args.command == "setup":
            setup(zbx)
        elif args.command == "trigger":
            trigger(zbx)
        elif args.command == "recover":
            recover(zbx)
        elif args.command == "status":
            status(zbx)
        elif args.command == "ack":
            acknowledge(zbx, args.message)
        elif args.command == "token":
            user = first(
                zbx.call(
                    "user.get",
                    {"filter": {"username": [args.user]}, "output": ["userid"]},
                )
            )
            if not user:
                raise ZabbixError(f"user {args.user} not found")
            existing = zbx.call(
                "token.get",
                {
                    "filter": {"name": ["observo-live-test"]},
                    "output": ["tokenid"],
                },
            )
            if existing:
                zbx.call(
                    "token.delete",
                    [item["tokenid"] for item in existing],
                )
            created = zbx.call(
                "token.create",
                {
                    "name": "observo-live-test",
                    "userid": user["userid"],
                    "status": 0,
                    "expires_at": 0,
                },
            )
            generated = zbx.call("token.generate", created["tokenids"])
            print(generated[0]["token"])
        elif args.command == "demo":
            setup(zbx)
            trigger(zbx)
            time.sleep(10)
            status(zbx)
    except ZabbixError as error:
        print(f"Zabbix error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
