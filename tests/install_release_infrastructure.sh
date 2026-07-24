#!/usr/bin/env sh

set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
TMP_ROOT=$(mktemp -d)
trap 'rm -rf "$TMP_ROOT"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

make_common_fakes() {
  fake_bin="$1"
  mkdir -p "$fake_bin"

  cat >"$fake_bin/uname" <<'EOF'
#!/usr/bin/env sh
case "${1:-}" in
  -s) echo Linux ;;
  -m) echo x86_64 ;;
  *) echo Linux ;;
esac
EOF

  cat >"$fake_bin/sudo" <<'EOF'
#!/usr/bin/env sh
exit 0
EOF

  cat >"$fake_bin/apt-get" <<'EOF'
#!/usr/bin/env sh
exit 0
EOF

  chmod +x "$fake_bin/uname" "$fake_bin/sudo" "$fake_bin/apt-get"
}

test_source_install_checks_out_requested_version_and_replaces_atomically() {
  case_dir="$TMP_ROOT/source"
  fake_bin="$case_dir/bin"
  home_dir="$case_dir/home"
  log_dir="$case_dir/log"
  mkdir -p "$home_dir" "$log_dir"
  make_common_fakes "$fake_bin"

  cat >"$fake_bin/git" <<'EOF'
#!/usr/bin/env sh
printf '%s\n' "$*" >"$TEST_LOG_DIR/git.args"
for last_arg do :; done
mkdir -p "$last_arg/target/release"
printf '#!/usr/bin/env sh\nexit 0\n' >"$last_arg/target/release/blockcell"
chmod +x "$last_arg/target/release/blockcell"
EOF

  cat >"$fake_bin/cargo" <<'EOF'
#!/usr/bin/env sh
exit 0
EOF

  cat >"$fake_bin/cp" <<'EOF'
#!/usr/bin/env sh
printf '%s\n' "$*" >>"$TEST_LOG_DIR/cp.args"
/bin/cp "$@"
EOF

  cat >"$fake_bin/mv" <<'EOF'
#!/usr/bin/env sh
printf '%s\n' "$*" >>"$TEST_LOG_DIR/mv.args"
/bin/mv "$@"
EOF

  chmod +x "$fake_bin/git" "$fake_bin/cargo" "$fake_bin/cp" "$fake_bin/mv"

  HOME="$home_dir" \
  PATH="$fake_bin:/usr/bin:/bin" \
  TEST_LOG_DIR="$log_dir" \
  BLOCKCELL_INSTALL_METHOD=source \
  BLOCKCELL_VERSION=v9.8.7 \
  BLOCKCELL_INSTALL_DIR="$case_dir/install" \
    sh "$ROOT/install.sh" >/dev/null

  grep -F -- '--branch v9.8.7' "$log_dir/git.args" >/dev/null \
    || fail "source install did not clone the requested version"
  test -f "$case_dir/install/blockcell" \
    || fail "source install did not produce the installed binary"
  grep -F -- "$case_dir/install/.blockcell.tmp." "$log_dir/cp.args" >/dev/null \
    || fail "source install copied directly over the live executable"
  grep -F -- "$case_dir/install/blockcell" "$log_dir/mv.args" >/dev/null \
    || fail "source install did not atomically rename the verified executable"
}

make_release_fakes() {
  fake_bin="$1"
  make_common_fakes "$fake_bin"

  cat >"$fake_bin/curl" <<'EOF'
#!/usr/bin/env sh
url=""
output=""
previous=""
for arg do
  if [ "$previous" = "-o" ]; then
    output="$arg"
    previous=""
    continue
  fi
  case "$arg" in
    -o) previous="-o" ;;
    http*) url="$arg" ;;
  esac
done

case "$url" in
  */releases/latest)
    printf '%s\n' '{"browser_download_url":"https://github.com/blockcell-labs/blockcell/releases/download/v9.8.7/blockcell-v9.8.7-linux-amd64.tar.gz"}'
    ;;
  */blockcell-v9.8.7-linux-amd64.tar.gz)
    printf '%s' 'archive bytes' >"$output"
    ;;
  */checksums.txt)
    printf '%s  %s\n' "$TEST_RELEASE_SHA" 'blockcell-v9.8.7-linux-amd64.tar.gz'
    ;;
  *)
    echo "unexpected curl URL: $url" >&2
    exit 1
    ;;
esac
EOF

  cat >"$fake_bin/tar" <<'EOF'
#!/usr/bin/env sh
case " $* " in
  *' -tzf '*) printf '%s\n' "$TEST_TAR_ENTRY" ;;
  *' -tvzf '*) printf '%s\n' "${TEST_TAR_VERBOSE:--rwxr-xr-x user/group 1 2026-01-01 00:00 blockcell}" ;;
  *)
    destination=""
    previous=""
    for arg do
      if [ "$previous" = "-C" ]; then destination="$arg"; previous=""; continue; fi
      [ "$arg" = "-C" ] && previous="-C"
    done
    mkdir -p "$destination"
    printf '#!/usr/bin/env sh\nexit 0\n' >"$destination/blockcell"
    chmod +x "$destination/blockcell"
    ;;
esac
EOF

  chmod +x "$fake_bin/curl" "$fake_bin/tar"
}

run_release_case() {
  name="$1"
  release_sha="$2"
  tar_entry="$3"
  case_dir="$TMP_ROOT/$name"
  fake_bin="$case_dir/bin"
  home_dir="$case_dir/home"
  mkdir -p "$home_dir"
  make_release_fakes "$fake_bin"

  HOME="$home_dir" \
  PATH="$fake_bin:/usr/bin:/bin" \
  TEST_RELEASE_SHA="$release_sha" \
  TEST_TAR_ENTRY="$tar_entry" \
  BLOCKCELL_INSTALL_METHOD=release \
  BLOCKCELL_INSTALL_DIR="$case_dir/install" \
    sh "$ROOT/install.sh" >/dev/null 2>&1
}

test_release_rejects_checksum_mismatch() {
  bad_sha=0000000000000000000000000000000000000000000000000000000000000000
  if run_release_case checksum_mismatch "$bad_sha" blockcell; then
    fail "release install accepted an asset whose checksum did not match"
  fi
}

test_release_rejects_unsafe_archive_paths() {
  if command -v sha256sum >/dev/null 2>&1; then
    archive_sha=$(printf '%s' 'archive bytes' | sha256sum | awk '{print $1}')
  else
    archive_sha=$(printf '%s' 'archive bytes' | shasum -a 256 | awk '{print $1}')
  fi
  if run_release_case unsafe_archive "$archive_sha" '../escaped'; then
    fail "release install accepted a traversing archive entry"
  fi
}

test_source_install_checks_out_requested_version_and_replaces_atomically
test_release_rejects_checksum_mismatch
test_release_rejects_unsafe_archive_paths

echo "install release infrastructure tests: ok"
