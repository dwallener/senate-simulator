from __future__ import annotations

import streamlit as st

from app.load import DEFAULT_PUBLIC_DIR, safe_load_bill_detail, safe_load_summary_bundle
from app.views import render_bill_detail, render_overview


st.set_page_config(page_title="Senate Simulator", layout="wide")


def main() -> None:
    bundle, error = safe_load_summary_bundle(DEFAULT_PUBLIC_DIR)
    if error:
        st.error(error)
        st.stop()

    selected = render_overview(bundle)
    if not selected:
        return

    detail, error = safe_load_bill_detail(selected, DEFAULT_PUBLIC_DIR)
    if error:
        st.warning(error)
        return

    render_bill_detail(detail)


if __name__ == "__main__":
    main()
