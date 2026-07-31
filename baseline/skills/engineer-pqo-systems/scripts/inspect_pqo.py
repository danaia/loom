#!/usr/bin/env python3
"""Validate a Pqo source and emit a compact graph/ABI inspection report."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


def run_pqo(pqo: str, command: str, source: Path) -> tuple[int, dict[str, Any] | None, str]:
    result = subprocess.run(
        [pqo, command, str(source)],
        check=False,
        capture_output=True,
        text=True,
    )
    output = result.stdout.strip()
    try:
        payload = json.loads(output) if output else None
    except json.JSONDecodeError:
        payload = None
    detail = result.stderr.strip()
    if payload is None and output:
        detail = f"{output}\n{detail}".strip()
    return result.returncode, payload, detail


def names_by_id(nodes: list[dict[str, Any]]) -> dict[int, str]:
    return {int(node["id"]): str(node["name"]) for node in nodes}


def implementation_kind(implementation: dict[str, Any]) -> str:
    source = str(implementation.get("source", ""))
    return "native" if source.startswith("pqo://generated/") else "external"


def explain_summary(payload: dict[str, Any]) -> dict[str, Any]:
    graph = payload["graph"]
    kernel_nodes = graph["kernels"]["nodes"]
    pass_nodes = graph["passes"]["nodes"]
    view_nodes = graph["views"]
    schedule_nodes = graph["schedules"]["nodes"]

    pass_names = names_by_id(pass_nodes)
    view_names = names_by_id(view_nodes)
    schedule_names = names_by_id(schedule_nodes)

    kernels = []
    for kernel in kernel_nodes:
        slots = {int(slot["id"]): slot for slot in kernel["slots"]}
        binding_order = [
            slots[int(slot_id)]["name"] for slot_id in kernel["abi"]["binding_order"]
        ]
        kernels.append(
            {
                "name": kernel["name"],
                "binding_order": binding_order,
                "implementations": [
                    {
                        "kind": implementation_kind(implementation),
                        "source": implementation["source"],
                        "entry": implementation["entry"],
                    }
                    for implementation in kernel["implementations"]
                ],
            }
        )

    schedules = []
    plans = payload["execution_plan"]["schedules"]
    for plan in plans:
        schedule_id = int(plan["schedule"])
        order = []
        for item in plan["order"]:
            if "Pass" in item:
                order.append(f"pass:{pass_names[int(item['Pass'])]}")
            elif "View" in item:
                order.append(f"view:{view_names[int(item['View'])]}")
            else:
                order.append(str(item))
        schedules.append(
            {
                "name": schedule_names[schedule_id],
                "order": order,
                "effective_ticks": plan["effective_ticks"],
                "effective_render_frames": plan["effective_render_frames"],
                "completion_requirements": len(plan["completion_requirements"]),
            }
        )

    return {
        "status": payload["status"],
        "source_graph_hash": payload["source_graph_hash"],
        "artifact_fingerprint": payload["artifact_fingerprint"],
        "kernels": kernels,
        "schedules": schedules,
        "intervention_passes": len(payload["execution_plan"]["intervention_passes"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run pqo check/explain and summarize kernel ABIs and schedule order."
    )
    parser.add_argument("source", type=Path, help="Primary .pqo source or .lmp package")
    parser.add_argument(
        "--pqo",
        default=shutil.which("pqo"),
        help="Pqo executable (defaults to the pqo found on PATH)",
    )
    args = parser.parse_args()

    if not args.pqo:
        parser.error("pqo was not found on PATH; pass --pqo /path/to/pqo")
    if not args.source.exists():
        parser.error(f"source does not exist: {args.source}")

    check_code, check, check_detail = run_pqo(args.pqo, "check", args.source)
    if check is None:
        print(json.dumps({"status": "tool_error", "detail": check_detail}, indent=2))
        return check_code or 2
    if check.get("status") != "valid":
        print(json.dumps({"check": check}, indent=2))
        return check_code or 1

    explain_code, explain, explain_detail = run_pqo(args.pqo, "explain", args.source)
    if explain is None:
        print(
            json.dumps(
                {"check": check, "explain_status": "tool_error", "detail": explain_detail},
                indent=2,
            )
        )
        return explain_code or 2

    print(json.dumps({"check": check, "explain": explain_summary(explain)}, indent=2))
    return 0 if explain.get("status") == "valid" else explain_code or 1


if __name__ == "__main__":
    sys.exit(main())
