#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPOSITORY_URL="${REPOSITORY_URL:-https://github.com/DevComputaria/DevComputaria.Prompting.git}"
readonly EXPECTED_REPOSITORY_NAME="${EXPECTED_REPOSITORY_NAME:-DevComputaria.Prompting}"
readonly REPOSITORY_ROOT="${REPOSITORY_ROOT:-/tmp/repositories}"
readonly REPOSITORY_PATH="${REPOSITORY_ROOT}/${EXPECTED_REPOSITORY_NAME}"
readonly REPORT_FILE="${REPORT_FILE:-/reports/container-integration-test.md}"
readonly DATABASE_URL="${DATABASE_URL:?DATABASE_URL is required}"

CURRENT_STAGE="initialization"
ASSERTIONS_PASSED=0
REPORT_FINALIZED=0

mkdir -p "$(dirname "${REPORT_FILE}")" "${REPOSITORY_ROOT}"

cat >"${REPORT_FILE}" <<EOF
# GitDatabase container integration test

- Repository: \`${REPOSITORY_URL}\`
- Expected repository name: \`${EXPECTED_REPOSITORY_NAME}\`
- Started at: \`$(date -u +'%Y-%m-%dT%H:%M:%SZ')\`

| Test | Result | Observed value |
| --- | --- | ---: |
EOF

report_row() {
  local name="$1"
  local result="$2"
  local value="$3"
  value="${value//$'\n'/ }"
  value="${value//|/\\|}"
  printf '| %s | %s | `%s` |\n' "${name}" "${result}" "${value}" >>"${REPORT_FILE}"
  printf '[%s] %s (observed: %s)\n' "${result}" "${name}" "${value}"
}

finish() {
  local exit_code=$?
  trap - EXIT

  if (( REPORT_FINALIZED == 0 )); then
    if (( exit_code == 0 )); then
      printf '\n**Result: PASS — %d SQL assertions passed.**\n' "${ASSERTIONS_PASSED}" >>"${REPORT_FILE}"
    else
      report_row "Integration pipeline" "FAIL" "stage=${CURRENT_STAGE}, exit=${exit_code}"
      printf '\n**Result: FAIL — stopped during `%s`.**\n' "${CURRENT_STAGE}" >>"${REPORT_FILE}"
    fi
    printf '\n- Finished at: `%s`\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >>"${REPORT_FILE}"
    REPORT_FINALIZED=1
  fi

  printf '\n========== Integration test report ==========\n'
  cat "${REPORT_FILE}"
  exit "${exit_code}"
}
trap finish EXIT

run_stage() {
  CURRENT_STAGE="$1"
  shift
  printf '\n========== %s ==========\n' "${CURRENT_STAGE}"
  "$@"
}

sql_scalar() {
  local query="$1"
  printf '%s\n' "${query}" | psql "${DATABASE_URL}" \
    --no-psqlrc \
    --set ON_ERROR_STOP=1 \
    --set "repo_name=${EXPECTED_REPOSITORY_NAME}" \
    --tuples-only \
    --no-align | tr -d '[:space:]'
}

assert_greater_than_zero() {
  local name="$1"
  local query="$2"
  local value
  value="$(sql_scalar "${query}")"

  if [[ "${value}" =~ ^[0-9]+$ ]] && (( value > 0 )); then
    report_row "${name}" "PASS" "${value}"
    ASSERTIONS_PASSED=$((ASSERTIONS_PASSED + 1))
  else
    report_row "${name}" "FAIL" "${value:-empty}"
    return 1
  fi
}

assert_equals() {
  local name="$1"
  local expected="$2"
  local query="$3"
  local value
  value="$(sql_scalar "${query}")"

  if [[ "${value}" == "${expected}" ]]; then
    report_row "${name}" "PASS" "${value}"
    ASSERTIONS_PASSED=$((ASSERTIONS_PASSED + 1))
  else
    report_row "${name}" "FAIL" "${value:-empty} (expected ${expected})"
    return 1
  fi
}

run_stage "clone repository" \
  git clone --no-tags "${REPOSITORY_URL}" "${REPOSITORY_PATH}"

CLONED_COMMIT="$(git -C "${REPOSITORY_PATH}" rev-parse HEAD)"
report_row "Repository cloned" "PASS" "${CLONED_COMMIT}"

run_stage "database health and migrations" \
  gitbase health --database-url "${DATABASE_URL}"

run_stage "repository metadata indexing" \
  gitbase sync --database-url "${DATABASE_URL}" --repo-roots "${REPOSITORY_ROOT}"

run_stage "blob hydration" \
  gitbase hydrate-blobs --database-url "${DATABASE_URL}" --repo-roots "${REPOSITORY_ROOT}"

run_stage "code search indexing" \
  gitbase search-index --database-url "${DATABASE_URL}"

CURRENT_STAGE="SQL assertions"
printf '\n========== %s ==========\n' "${CURRENT_STAGE}"

assert_equals "Target repository indexed exactly once" "1" \
  "SELECT count(*) FROM gitbase.repositories WHERE name = :'repo_name';"

assert_greater_than_zero "Git refs indexed" \
  "SELECT count(*) FROM gitbase.refs r JOIN gitbase.repositories repo ON repo.id = r.repository_id WHERE repo.name = :'repo_name';"

assert_greater_than_zero "Git commits indexed" \
  "SELECT count(*) FROM gitbase.commits c JOIN gitbase.repositories repo ON repo.id = c.repository_id WHERE repo.name = :'repo_name';"

assert_greater_than_zero "Repository files indexed" \
  "SELECT count(*) FROM gitbase.files f JOIN gitbase.repositories repo ON repo.id = f.repository_id WHERE repo.name = :'repo_name';"

assert_greater_than_zero "Text blobs hydrated" \
  "SELECT count(DISTINCT b.hash) FROM gitbase.blobs b JOIN gitbase.files f ON f.blob_hash = b.hash JOIN gitbase.repositories repo ON repo.id = f.repository_id WHERE repo.name = :'repo_name' AND b.content IS NOT NULL;"

assert_greater_than_zero "Code-search documents indexed" \
  "SELECT count(DISTINCT ci.blob_hash) FROM gitbase.code_index ci JOIN gitbase.files f ON f.blob_hash = ci.blob_hash JOIN gitbase.repositories repo ON repo.id = f.repository_id WHERE repo.name = :'repo_name';"

assert_greater_than_zero "README.md query returns data" \
  "SELECT count(*) FROM gitbase.files f JOIN gitbase.repositories repo ON repo.id = f.repository_id WHERE repo.name = :'repo_name' AND f.path = 'README.md';"

assert_greater_than_zero "search_code('prompt') returns matches" \
  "SELECT count(*) FROM gitbase.search_code('prompt') result JOIN gitbase.repositories repo ON repo.id = result.repository_id WHERE repo.name = :'repo_name';"

assert_equals "No indexed file references a missing repository" "0" \
  "SELECT count(*) FROM gitbase.files f LEFT JOIN gitbase.repositories repo ON repo.id = f.repository_id WHERE repo.id IS NULL;"

REPORT_FINALIZED=1
printf '\n**Result: PASS — %d SQL assertions passed.**\n' "${ASSERTIONS_PASSED}" >>"${REPORT_FILE}"
printf '\n- Finished at: `%s`\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >>"${REPORT_FILE}"
