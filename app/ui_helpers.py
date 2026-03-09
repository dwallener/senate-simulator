from __future__ import annotations


def yes_no_label(value: bool) -> str:
    return "Yes" if value else "No"


def compact_percent(value: float) -> str:
    return f"{value:.0%}"


def stage_badge(stage: str) -> str:
    return stage.replace("_", " ")
