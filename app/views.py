from __future__ import annotations

from typing import Any

import streamlit as st

from app.ui_helpers import compact_percent, stage_badge, yes_no_label


def render_overview(bundle: dict[str, Any]) -> str | None:
    last_updated = bundle["last_updated"]
    summary = bundle["summary"]
    tracked = bundle["tracked_bills"]

    st.title("Senate Simulator")
    st.caption("Read-only public dashboard over precomputed daily artifacts.")

    col1, col2, col3 = st.columns(3)
    col1.metric("Snapshot Date", str(last_updated.get("snapshot_date", "unknown")))
    col2.metric("Tracked Bills", int(last_updated.get("tracked_bill_count", 0)))
    col3.metric("Exported Bills", int(last_updated.get("exported_bill_count", 0)))
    st.caption(f"Generated at {last_updated.get('generated_at', 'unknown')}")

    if last_updated.get("notes"):
        with st.expander("Refresh Notes"):
            for note in last_updated["notes"]:
                st.write(f"- {note}")

    rows = summary.get("rows", [])
    if not rows:
        st.warning("No tracked bill exports were found in the public artifact bundle.")
        return None

    selected_object_id = st.selectbox(
        "Tracked bill",
        options=[row["object_id"] for row in rows],
        format_func=lambda object_id: _bill_label(object_id, tracked, rows),
    )

    table_rows = [
        {
            "Object ID": row["object_id"],
            "Title": row["title"],
            "Stage": stage_badge(row["stage"]),
            "Support": row["support_count"],
            "Lean Support": row["lean_support_count"],
            "Undecided": row["undecided_count"],
            "Oppose": row["oppose_count"],
            "Majority": yes_no_label(row["majority_viable"]),
            "Cloture": yes_no_label(row["cloture_viable"]),
            "Next Event": row["predicted_next_event"],
            "Confidence": compact_percent(row["next_event_confidence"]),
        }
        for row in rows
    ]
    st.dataframe(table_rows, use_container_width=True, hide_index=True)
    return selected_object_id


def render_bill_detail(detail: dict[str, Any]) -> None:
    st.header(f"{detail['object_id']} — {detail['title']}")
    st.write(detail["summary"])

    col1, col2, col3, col4 = st.columns(4)
    col1.metric("Stage", stage_badge(detail["stage"]))
    col2.metric("Majority Viable", yes_no_label(detail["majority_viable"]))
    col3.metric("Cloture Viable", yes_no_label(detail["cloture_viable"]))
    col4.metric("Filibuster Risk", compact_percent(detail["filibuster_risk"]))

    st.subheader("Coalition")
    coalition_cols = st.columns(4)
    coalition_cols[0].metric("Support", int(detail["support_count"]))
    coalition_cols[1].metric("Lean Support", int(detail["lean_support_count"]))
    coalition_cols[2].metric("Undecided", int(detail["undecided_count"]))
    coalition_cols[3].metric("Oppose", int(detail["oppose_count"]))

    st.subheader("Prediction")
    pred_cols = st.columns(4)
    pred_cols[0].metric("Floor Action", detail["predicted_floor_action"])
    pred_cols[1].metric("Next Event", detail["predicted_next_event"])
    pred_cols[2].metric("Event Score", f"{detail['next_event_score']:.2f}")
    pred_cols[3].metric("Confidence", f"{detail['next_event_confidence']:.2f}")

    if detail.get("alternatives"):
        st.subheader("Alternative Events")
        for alternative in detail["alternatives"]:
            st.write(
                f"- {alternative['event']} ({alternative['score']:.2f}): {alternative['reason']}"
            )

    if detail.get("top_reasons"):
        st.subheader("Why")
        for reason in detail["top_reasons"]:
            st.write(f"- {reason}")

    if detail.get("pivots"):
        st.subheader("Pivots")
        for pivot in detail["pivots"][:10]:
            st.write(f"- {pivot['senator_id']}: {pivot['reason']}")

    if detail.get("blockers"):
        st.subheader("Blockers")
        for blocker in detail["blockers"][:10]:
            st.write(f"- {blocker['senator_id']}: {blocker['reason']}")

    if detail.get("rollout_steps"):
        st.subheader("Rollout")
        for step in detail["rollout_steps"]:
            st.write(
                f"- Step {step['step_index'] + 1}: {stage_badge(step['starting_stage'])} -> "
                f"{step['predicted_event']} ({step['confidence']:.2f})"
            )
        st.caption(f"Terminated: {detail.get('termination_reason', 'unknown')}")


def _bill_label(object_id: str, tracked: list[dict[str, Any]], rows: list[dict[str, Any]]) -> str:
    tracked_label = next(
        (item.get("label") for item in tracked if item.get("object_id") == object_id),
        None,
    )
    row_title = next((row.get("title") for row in rows if row.get("object_id") == object_id), object_id)
    return f"{object_id} — {tracked_label or row_title}"
