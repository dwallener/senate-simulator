from __future__ import annotations

import json
from pathlib import Path
from typing import Any


DEFAULT_PUBLIC_DIR = Path("data/public")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def load_last_updated(public_dir: Path = DEFAULT_PUBLIC_DIR) -> dict[str, Any]:
    return load_json(public_dir / "last_updated.json")


def load_summary(public_dir: Path = DEFAULT_PUBLIC_DIR) -> dict[str, Any]:
    return load_json(public_dir / "summary.json")


def list_tracked_bills(public_dir: Path = DEFAULT_PUBLIC_DIR) -> list[dict[str, Any]]:
    payload = load_json(public_dir / "tracked_bills.json")
    return payload.get("tracked", [])


def load_bill_detail(object_id: str, public_dir: Path = DEFAULT_PUBLIC_DIR) -> dict[str, Any]:
    return load_json(public_dir / "bills" / f"{object_id}.json")


def safe_load_summary_bundle(public_dir: Path = DEFAULT_PUBLIC_DIR) -> tuple[dict[str, Any] | None, str | None]:
    try:
        return {
            "last_updated": load_last_updated(public_dir),
            "summary": load_summary(public_dir),
            "tracked_bills": list_tracked_bills(public_dir),
        }, None
    except FileNotFoundError as exc:
        return None, f"Missing public artifact: {exc.filename}"
    except json.JSONDecodeError as exc:
        return None, f"Malformed JSON in public artifacts: {exc}"


def safe_load_bill_detail(object_id: str, public_dir: Path = DEFAULT_PUBLIC_DIR) -> tuple[dict[str, Any] | None, str | None]:
    try:
        return load_bill_detail(object_id, public_dir), None
    except FileNotFoundError as exc:
        return None, f"Missing bill detail artifact: {exc.filename}"
    except json.JSONDecodeError as exc:
        return None, f"Malformed bill detail JSON: {exc}"
