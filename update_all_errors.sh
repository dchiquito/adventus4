#!/usr/bin/env bash

# This script will update all error snapshots in test_cases with the current behavior.
# This is useful for when error formatting is updated and all tests need to be updated simultaneously.
# This script will replace paths in the error snapshot with their templated equivalents.

CARGO_BUILD_WARNINGS=allow cargo build --quiet
BINARY="target/debug/adventus4"

function update_test_case {
  for error in ${1}/*.err; do
    if [ -f $error ]; then
      echo "Updating $error..."
      local case_id="${error%.*}"
      if [ -f "${case_id}.in" ]; then
        cat "${case_id}.in" \
          | $BINARY "$1/test.adv" 2>&1 >/dev/null \
          | sed "s+$1/test.adv+{{TEST_ADV}}+g" > "$error"
      else
        $BINARY "$1/test.adv" 2>&1 >/dev/null \
          | sed "s+$1/test.adv+{{TEST_ADV}}+g" > "$error"
      fi
    fi
  done
}

for test_dir in test_cases/*; do
  if [ -d $test_dir ]; then
    update_test_case $test_dir
  fi
done
