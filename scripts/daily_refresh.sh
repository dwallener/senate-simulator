#!/usr/bin/env bash
set -euo pipefail

DATE="${1:-$(date +%F)}"
STANCE_MODE="${STANCE_MODE:-feature}"
STEPS="${STEPS:-3}"

cargo run -q -- ingest --date "$DATE" --source live
cargo run -q -- features-build --date "$DATE"
cargo run -q -- predict-export --date "$DATE" --tracked-bills-file tracked_bills.json --out data/public --stance-mode "$STANCE_MODE" --steps "$STEPS"

git add data/public
if git diff --cached --quiet; then
  echo "No public artifact changes to commit for $DATE"
  exit 0
fi

git commit -m "daily refresh $DATE"
git push
