from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from app.load import (
    list_tracked_bills,
    load_bill_detail,
    load_last_updated,
    load_summary,
    safe_load_bill_detail,
    safe_load_summary_bundle,
)


class AppLoadTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = Path(tempfile.mkdtemp(prefix="senate_sim_app_load_"))
        public_dir = self.temp_dir / "data" / "public" / "bills"
        public_dir.mkdir(parents=True, exist_ok=True)
        self.public_root = self.temp_dir / "data" / "public"

        (self.public_root / "last_updated.json").write_text(
            json.dumps({"schema_version": 1, "snapshot_date": "2026-03-09"}),
            encoding="utf-8",
        )
        (self.public_root / "summary.json").write_text(
            json.dumps({"schema_version": 1, "rows": [{"object_id": "hr144"}]}),
            encoding="utf-8",
        )
        (self.public_root / "tracked_bills.json").write_text(
            json.dumps({"schema_version": 1, "tracked": [{"object_id": "hr144", "label": "TVA Act"}]}),
            encoding="utf-8",
        )
        (self.public_root / "bills" / "hr144.json").write_text(
            json.dumps({"schema_version": 1, "object_id": "hr144", "title": "TVA Act"}),
            encoding="utf-8",
        )

    def test_load_helpers_read_public_artifacts(self) -> None:
        self.assertEqual(load_last_updated(self.public_root)["schema_version"], 1)
        self.assertEqual(load_summary(self.public_root)["rows"][0]["object_id"], "hr144")
        self.assertEqual(list_tracked_bills(self.public_root)[0]["label"], "TVA Act")
        self.assertEqual(load_bill_detail("hr144", self.public_root)["title"], "TVA Act")

    def test_safe_bundle_handles_missing_files(self) -> None:
        missing_root = self.temp_dir / "missing"
        payload, error = safe_load_summary_bundle(missing_root)
        self.assertIsNone(payload)
        self.assertIn("Missing public artifact", error)

    def test_safe_bill_detail_handles_missing_file(self) -> None:
        payload, error = safe_load_bill_detail("missing", self.public_root)
        self.assertIsNone(payload)
        self.assertIn("Missing bill detail artifact", error)


if __name__ == "__main__":
    unittest.main()
