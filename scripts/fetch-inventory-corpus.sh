#!/bin/bash
# Clone the real-world corpus used for manual inventory-mode validation.
# Output goes into the existing gitignored repo cache (tests/integration_test/repos_cache/),
# the same convention used by tests/uncomment_integration.rs for removal-mode AST checks.
set -e

REPO_URL="https://github.com/MadHuslista/madhus.project.handgrip"
BRANCH="tmain"
DEST="tests/integration_test/repos_cache/handgrip-calibration"

if [ -d "$DEST" ]; then
  echo "Corpus already cloned at $DEST"
  exit 0
fi

git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$DEST"
echo "Cloned $REPO_URL@$BRANCH into $DEST"
