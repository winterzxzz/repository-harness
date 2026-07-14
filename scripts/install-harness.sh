#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-harness.sh [options] [path]

Apply the Harness v0 files and folders to a target project directory.

Options:
  -d, --directory <path>  Target directory. Defaults to the current directory.
  -y, --yes              Accept defaults and skip prompts.
      --merge            On protected-path conflict, keep existing files in
                         place and install only missing Harness files.
      --refresh-agent-shim
                         Refresh an existing AGENTS.md into the small Harness
                         shim after backing it up. Old Harness-generated files
                         are replaced; custom files receive a marked block.
      --claude           Refresh or append the marked CLAUDE.md Harness block
                         for an existing install. Fresh installs include
                         CLAUDE.md by default; this flag remains for backward
                         compatibility and existing custom files.
      --override         On protected-path conflict, back up and replace
                         AGENTS.md, docs/, and scripts/.
      --force            Overwrite existing files after backing them up.
      --update           Update tracked Harness files without replacing local edits.
      --adopt            Record existing Harness files before a future --update.
      --dry-run          Show what would change without writing files.
  -h, --help             Show this help.

Safety:
  If AGENTS.md, docs/, or scripts/ already exist, interactive installs ask
  whether to merge missing files, override after backup, or stop. Merge is the
  safe update path for repositories that already have Harness: existing files
  stay in place and new Harness files are appended by path. Non-
  interactive installs stop unless --merge or --override is provided. If a
  target .gitignore already exists, Harness appends its local database rules
  unless --force is used.

Examples:
  scripts/install-harness.sh
  scripts/install-harness.sh --directory /path/to/project --yes
  scripts/install-harness.sh ./my-project --force
  scripts/install-harness.sh --update --yes
  scripts/install-harness.sh --update --adopt --yes
  curl -fsSL https://raw.githubusercontent.com/winterzxzz/repository-harness/main/scripts/install-harness.sh | bash -s -- --yes
  curl -fsSL https://raw.githubusercontent.com/winterzxzz/repository-harness/main/scripts/install-harness.sh | bash -s -- --merge --yes
  curl -fsSL https://raw.githubusercontent.com/winterzxzz/repository-harness/main/scripts/install-harness.sh | bash -s -- --merge --refresh-agent-shim --yes
  curl -fsSL https://raw.githubusercontent.com/winterzxzz/repository-harness/main/scripts/install-harness.sh | bash -s -- --claude --yes
EOF
}

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

warn_stop() {
  printf 'Warning: %s\n' "$*" >&2
  exit 1
}

can_prompt() {
  [ -r /dev/tty ] && [ -w /dev/tty ]
}

prompt_tty() {
  printf '%s' "$1" > /dev/tty
}

read_tty() {
  local value
  IFS= read -r value < /dev/tty
  printf '%s\n' "$value"
}

expand_path() {
  case "$1" in
    "~")
      printf '%s\n' "$HOME"
      ;;
    \~/*)
      printf '%s/%s\n' "$HOME" "${1#~/}"
      ;;
    /*)
      printf '%s\n' "$1"
      ;;
    *)
      printf '%s/%s\n' "$PWD" "$1"
      ;;
  esac
}

make_absolute_parent() {
  local path="$1"
  local parent
  parent="$(dirname "$path")"
  [ -d "$parent" ] || fail "Parent directory does not exist: $parent"
  (cd "$parent" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$path")")
}

copy_file() {
  local relative="$1"
  local target="$TARGET_DIR/$relative"

  if [ "$relative" = ".gitignore" ] && [ -e "$target" ]; then
    merge_gitignore "$target"
    return
  fi

  if [ -e "$target" ]; then
    if [ "$SOURCE_MODE" = "local" ] && [ "$SOURCE_ROOT/$relative" -ef "$target" ]; then
      log "skip     $relative (source file)"
      SKIPPED=$((SKIPPED + 1))
      return
    fi

    if [ "$CONFLICT_ACTION" = "merge" ]; then
      log "skip     $relative (merge keeps existing file)"
      SKIPPED=$((SKIPPED + 1))
    elif [ "$FORCE" -eq 1 ]; then
      if [ "$DRY_RUN" -eq 1 ]; then
        log "overwrite $relative (backup first)"
      else
        local backup="$BACKUP_DIR/$relative"
        mkdir -p "$(dirname "$backup")"
        cp -p "$target" "$backup"
        write_source_file "$relative" "$target"
        record_managed_file "$relative"
        log "updated $relative (backup: ${backup#"$TARGET_DIR"/})"
      fi
      UPDATED=$((UPDATED + 1))
    else
      log "skip     $relative (already exists)"
      SKIPPED=$((SKIPPED + 1))
    fi
    return
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    log "create   $relative"
  else
    mkdir -p "$(dirname "$target")"
    write_source_file "$relative" "$target"
    record_managed_file "$relative"
    log "created  $relative"
  fi
  CREATED=$((CREATED + 1))
}

merge_gitignore() {
  local target="$1"
  local rules="# Harness durable layer
harness.db
harness.db-wal
harness.db-shm
scripts/bin/harness-cli
scripts/bin/harness-cli.exe
.symphony/
.worktrees/
!.harness/
.harness/*
!.harness/changesets/
!.harness/changesets/*.changeset.jsonl"
  local missing=""
  while IFS= read -r rule || [ -n "$rule" ]; do
    grep -Fxq "$rule" "$target" || missing="${missing}${rule}
"
  done <<EOF
$rules
EOF

  if [ -z "$missing" ]; then
    log "skip     .gitignore (harness rules already present)"
    SKIPPED=$((SKIPPED + 1))
    return
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    log "update   .gitignore (append harness rules)"
  else
    if [ -s "$target" ]; then
      printf '\n' >> "$target"
    fi
    printf '%s' "$missing" >> "$target"
    log "updated  .gitignore (appended harness rules)"
  fi
  UPDATED=$((UPDATED + 1))
}

write_source_file() {
  local relative="$1"
  local target="$2"

  if [ "$SOURCE_MODE" = "local" ]; then
    local source="$SOURCE_ROOT/$relative"
    [ -f "$source" ] || fail "Source file missing: $source"
    cp -p "$source" "$target"
    return
  fi

  local url="$SOURCE_BASE_URL/$relative"
  curl -fsSL "$url" -o "$target" || fail "Could not download $url"
}

read_payload_manifest() {
  if [ "$SOURCE_MODE" = "local" ]; then
    local manifest="$SOURCE_ROOT/$PAYLOAD_MANIFEST"
    [ -f "$manifest" ] || fail "Payload manifest missing: $manifest"
    cat "$manifest"
    return
  fi

  local url="$SOURCE_BASE_URL/$PAYLOAD_MANIFEST"
  curl -fsSL "$url" || fail "Could not download $url"
}

discover_schema_files() {
  if [ "$SOURCE_MODE" = "local" ]; then
    local schema_root="$SOURCE_ROOT/$SCHEMA_DIR"
    [ -d "$schema_root" ] || fail "Schema directory missing: $schema_root"
    find "$schema_root" -maxdepth 1 -type f -name '*.sql' -print |
      while IFS= read -r path; do
        printf '%s/%s\n' "$SCHEMA_DIR" "$(basename "$path")"
      done |
      sort
    return
  fi

  case "$SOURCE_BASE_URL" in
    file://*)
      local source_root="${SOURCE_BASE_URL#file://}"
      local schema_root="$source_root/$SCHEMA_DIR"
      [ -d "$schema_root" ] || fail "Schema directory missing: $schema_root"
      find "$schema_root" -maxdepth 1 -type f -name '*.sql' -print |
        while IFS= read -r path; do
          printf '%s/%s\n' "$SCHEMA_DIR" "$(basename "$path")"
        done |
        sort
      ;;
    https://raw.githubusercontent.com/*)
      local raw_path="${SOURCE_BASE_URL#https://raw.githubusercontent.com/}"
      local owner repo ref api_url
      IFS=/ read -r owner repo ref _rest <<EOF
$raw_path
EOF
      [ -n "${owner:-}" ] && [ -n "${repo:-}" ] && [ -n "${ref:-}" ] ||
        fail "Cannot infer GitHub repository from $SOURCE_BASE_URL"
      api_url="https://api.github.com/repos/$owner/$repo/git/trees/$ref?recursive=1"
      curl -fsSL "$api_url" |
        sed -n "s#.*\"path\": \"\\($SCHEMA_DIR/[^\"]*\\.sql\\)\".*#\\1#p" |
        sort
      ;;
    *)
      fail "Cannot discover remote schema files from $SOURCE_BASE_URL. Use a local source, file:// source, or raw.githubusercontent.com source."
      ;;
  esac
}

copy_payload_files() {
  local manifest
  local relative
  local copied_schema=0

  manifest="$(read_payload_manifest)"
  while IFS= read -r relative || [ -n "$relative" ]; do
    relative="${relative%$'\r'}"
    case "$relative" in
      ""|\#*)
        continue
        ;;
    esac
    copy_file "$relative"
  done <<EOF
$manifest
EOF

  while IFS= read -r relative || [ -n "$relative" ]; do
    [ -n "$relative" ] || continue
    copy_file "$relative"
    copied_schema=$((copied_schema + 1))
  done <<EOF
$(discover_schema_files)
EOF

  [ "$copied_schema" -gt 0 ] || fail "No schema migrations found in $SCHEMA_DIR"
}

list_payload_files() {
  local manifest relative

  manifest="$(read_payload_manifest)"
  while IFS= read -r relative || [ -n "$relative" ]; do
    relative="${relative%$'\r'}"
    case "$relative" in
      ""|\#*) continue ;;
    esac
    printf '%s\n' "$relative"
  done <<EOF
$manifest
EOF

  discover_schema_files
}

backup_target_file() {
  local relative="$1"
  local target="$TARGET_DIR/$relative"
  local backup="$BACKUP_DIR/$relative"

  mkdir -p "$(dirname "$backup")"
  cp -p "$target" "$backup"
}

assert_update_path_is_safe() {
  local relative="$1"
  local target parent project_root resolved_parent

  case "$relative" in
    ""|/*|..|../*|*/../*|*/..)
      fail "Invalid managed file path: $relative"
      ;;
  esac

  project_root="$(cd -P "$TARGET_DIR" && pwd -P)"
  target="$TARGET_DIR/$relative"
  [ ! -L "$target" ] || fail "Refusing to update symlinked Harness path: $relative"

  parent="$(dirname "$target")"
  while :; do
    [ ! -L "$parent" ] || fail "Refusing to update through symlinked Harness path: $relative"
    [ -d "$parent" ] && break
    [ "$parent" != "$TARGET_DIR" ] || fail "Managed file parent is missing: $relative"
    parent="$(dirname "$parent")"
  done

  resolved_parent="$(cd -P "$parent" && pwd -P)"
  case "$resolved_parent" in
    "$project_root"|"$project_root"/*) ;;
    *) fail "Managed file path escapes the target project: $relative" ;;
  esac
}

preflight_managed_paths() {
  local relative

  while IFS= read -r relative || [ -n "$relative" ]; do
    [ -n "$relative" ] || continue
    assert_update_path_is_safe "$relative"
  done < <(list_payload_files)
  assert_update_path_is_safe "scripts/bin/harness-cli"
}

update_managed_file() {
  local relative="$1"
  local target="$TARGET_DIR/$relative"
  local recorded_hash current_hash source_hash source_tmp

  if [ ! -e "$target" ]; then
    if [ "$DRY_RUN" -eq 1 ]; then
      log "create   $relative"
    else
      mkdir -p "$(dirname "$target")"
      write_source_file "$relative" "$target"
      record_managed_file "$relative"
      log "created  $relative"
    fi
    CREATED=$((CREATED + 1))
    return 0
  fi

  recorded_hash="$(recorded_hash_for "$relative")"
  if [ -z "$recorded_hash" ]; then
    log "skip     $relative (untracked existing file)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  current_hash="$(sha256_file "$target")"
  if [ "$current_hash" != "$recorded_hash" ] && [ "$FORCE" -eq 0 ]; then
    log "skip     $relative (modified locally)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    if [ "$current_hash" != "$recorded_hash" ]; then
      log "overwrite $relative (backup first)"
    else
      log "update   $relative"
    fi
    UPDATED=$((UPDATED + 1))
    return 0
  fi

  source_tmp="$(mktemp)"
  write_source_file "$relative" "$source_tmp"
  source_hash="$(sha256_file "$source_tmp")"
  if [ "$source_hash" = "$current_hash" ]; then
    rm -f "$source_tmp"
    log "skip     $relative (already current)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  if [ "$current_hash" != "$recorded_hash" ]; then
    backup_target_file "$relative"
  fi
  cp "$source_tmp" "$target"
  rm -f "$source_tmp"
  record_managed_file "$relative"
  UPDATED=$((UPDATED + 1))
  if [ "$current_hash" != "$recorded_hash" ]; then
    log "updated  $relative (backup: ${BACKUP_DIR#"$TARGET_DIR"/}/$relative)"
  else
    log "updated  $relative"
  fi
}

adopt_existing_files() {
  local relative target adopted=0

  while IFS= read -r relative || [ -n "$relative" ]; do
    [ -n "$relative" ] || continue
    target="$TARGET_DIR/$relative"
    if [ -f "$target" ]; then
      record_managed_file "$relative"
      adopted=$((adopted + 1))
      log "adopted  $relative"
    fi
  done < <(list_payload_files)

  if [ -f "$TARGET_DIR/scripts/bin/harness-cli" ]; then
    record_managed_file "scripts/bin/harness-cli"
    adopted=$((adopted + 1))
    log "adopted  scripts/bin/harness-cli"
  fi

  [ "$adopted" -gt 0 ] || fail "No Harness files are available to adopt in $TARGET_DIR"
}

update_payload_files() {
  local relative copied_schema=0

  while IFS= read -r relative || [ -n "$relative" ]; do
    [ -n "$relative" ] || continue
    update_managed_file "$relative"
    case "$relative" in
      "$SCHEMA_DIR"/*.sql) copied_schema=$((copied_schema + 1)) ;;
    esac
  done < <(list_payload_files)

  [ "$copied_schema" -gt 0 ] || fail "No schema migrations found in $SCHEMA_DIR"
}

agent_shim_block() {
  cat <<'EOF'
<!-- HARNESS:BEGIN -->
## Harness

This repo uses Harness. Before work, read:

- `README.md`
- `docs/HARNESS.md`
- `docs/FEATURE_INTAKE.md`
- `docs/ARCHITECTURE.md`
- `docs/CONTEXT_RULES.md`
- `docs/TOOL_REGISTRY.md`
- `scripts/bin/harness-cli query stats` (full `query matrix` during intake)

Use the Rust Harness CLI at `scripts/bin/harness-cli` as the main operational
tool.

For an explicitly approved, execution-ready story, hand implementation to
Symphony with `harness-symphony run <story-id>` so the user can monitor the run
in the local Web UI. Do not pass `--no-web`; report the controller URL printed
by Symphony to the user. Keep intake, investigation, planning, and direct tiny
edits in the current agent session. If `HARNESS_RUN_ID` is already set, continue
inside the current Symphony run instead of starting a nested run.

Before a step that could use an external tool, run
`scripts/bin/harness-cli query tools --capability <name> --status present` to
see what is equipped; an absent capability is a clean skip.
<!-- HARNESS:END -->
EOF
}

claude_shim_block() {
  cat <<'EOF'
<!-- HARNESS:BEGIN -->
## Harness

Claude Code loads this file into every session, but it does not auto-load
`AGENTS.md`. The bare `@` lines below import the always-required harness
context (the "Must in all lanes" set from `docs/CONTEXT_RULES.md`) at
context-load time. Never wrap them in backticks; that disables the import.

@AGENTS.md

@docs/FEATURE_INTAKE.md

Also run `scripts/bin/harness-cli query stats` before starting work; pull the
full `query matrix` only during intake or when a story's proof status matters,
because the full matrix output is large.

Lane-dependent context (`README.md`, `docs/HARNESS.md`, `docs/ARCHITECTURE.md`,
`docs/CONTEXT_RULES.md`, product docs, stories, decisions) is intentionally not
imported — read it per lane, as `docs/CONTEXT_RULES.md` prescribes.
<!-- HARNESS:END -->
EOF
}

is_old_harness_agent_file() {
  local target="$1"

  grep -Fxq "# Agent Operating Guide" "$target" &&
    grep -Fxq "This repository is in Harness v0. There is no product implementation yet." "$target" &&
    grep -Fxq "## Source Of Truth" "$target" &&
    grep -Fxq "## Task Loop" "$target" &&
    grep -Fxq "## Done Definition" "$target"
}

backup_agent_file() {
  local target="$TARGET_DIR/AGENTS.md"

  [ -e "$target" ] || return 0
  mkdir -p "$BACKUP_DIR"
  [ -e "$BACKUP_DIR/AGENTS.md" ] && return 0
  cp -p "$target" "$BACKUP_DIR/AGENTS.md"
}

extract_obvious_agent_custom_section() {
  local target="$1"
  local output="$2"

  awk '
    /^## (Project-specific|Project Specific|Local|Custom).*Instructions/ {
      capture = 1
      print
      next
    }
    /^## / && capture {
      capture = 0
    }
    capture {
      print
    }
  ' "$target" > "$output"
}

insert_agent_custom_section() {
  local target="$1"
  local custom="$2"
  local tmp

  [ -s "$custom" ] || return 0
  tmp="$(mktemp)"
  awk '
    $0 == "Add project-specific agent instructions here." {
      while ((getline line < custom_file) > 0) {
        print line
      }
      next
    }
    { print }
  ' custom_file="$custom" "$target" > "$tmp"
  mv "$tmp" "$target"
}

append_or_replace_agent_harness_block() {
  local target="$TARGET_DIR/AGENTS.md"
  local tmp

  tmp="$(mktemp)"
  if grep -Fq "<!-- HARNESS:BEGIN -->" "$target" &&
     grep -Fq "<!-- HARNESS:END -->" "$target"; then
    awk '
      /<!-- HARNESS:BEGIN -->/ {
        while ((getline line < block_file) > 0) {
          print line
        }
        in_block = 1
        next
      }
      /<!-- HARNESS:END -->/ && in_block {
        in_block = 0
        next
      }
      !in_block { print }
    ' block_file=<(agent_shim_block) "$target" > "$tmp"
  else
    {
      cat "$target"
      printf '\n'
      agent_shim_block
    } > "$tmp"
  fi
  mv "$tmp" "$target"
}

refresh_agent_shim() {
  [ "$REFRESH_AGENT_SHIM" -eq 1 ] || return 0

  local target="$TARGET_DIR/AGENTS.md"
  [ -e "$target" ] || return 0

  if [ "$SOURCE_MODE" = "local" ] && [ "$SOURCE_ROOT/AGENTS.md" -ef "$target" ]; then
    log "skip     AGENTS.md (source file)"
    return 0
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    if is_old_harness_agent_file "$target"; then
      log "refresh  AGENTS.md (old Harness guide -> shim, backup first)"
    else
      log "refresh  AGENTS.md (append or replace marked Harness block, backup first)"
    fi
    UPDATED=$((UPDATED + 1))
    return 0
  fi

  backup_agent_file
  if is_old_harness_agent_file "$target"; then
    local custom_tmp
    custom_tmp="$(mktemp)"
    extract_obvious_agent_custom_section "$target" "$custom_tmp"
    write_source_file "AGENTS.md" "$target"
    insert_agent_custom_section "$target" "$custom_tmp"
    rm -f "$custom_tmp"
    log "updated  AGENTS.md (old Harness guide -> shim; backup: ${BACKUP_DIR#"$TARGET_DIR"/}/AGENTS.md)"
  else
    append_or_replace_agent_harness_block
    log "updated  AGENTS.md (refreshed Harness block; backup: ${BACKUP_DIR#"$TARGET_DIR"/}/AGENTS.md)"
  fi
  UPDATED=$((UPDATED + 1))
}

backup_claude_file() {
  local target="$TARGET_DIR/CLAUDE.md"

  [ -e "$target" ] || return 0
  mkdir -p "$BACKUP_DIR"
  [ -e "$BACKUP_DIR/CLAUDE.md" ] && return 0
  cp -p "$target" "$BACKUP_DIR/CLAUDE.md"
}

write_claude_shim() {
  [ "$INSTALL_CLAUDE_SHIM" -eq 1 ] || return 0

  local target="$TARGET_DIR/CLAUDE.md"
  local block_tmp tmp

  if [ "$SOURCE_MODE" = "local" ] && [ -e "$target" ] &&
     [ "$SOURCE_ROOT/CLAUDE.md" -ef "$target" ]; then
    log "skip     CLAUDE.md (source file)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  block_tmp="$(mktemp)"
  claude_shim_block > "$block_tmp"

  if [ -e "$target" ] &&
     grep -Fq "<!-- HARNESS:BEGIN -->" "$target" &&
     grep -Fq "<!-- HARNESS:END -->" "$target"; then
    local current_tmp
    current_tmp="$(mktemp)"
    awk '
      /<!-- HARNESS:BEGIN -->/ { in_block = 1 }
      in_block { print }
      /<!-- HARNESS:END -->/ { in_block = 0 }
    ' "$target" > "$current_tmp"
    if cmp -s "$current_tmp" "$block_tmp"; then
      log "skip     CLAUDE.md (Harness block current)"
      SKIPPED=$((SKIPPED + 1))
      rm -f "$current_tmp" "$block_tmp"
      return 0
    fi
    rm -f "$current_tmp"

    if [ "$DRY_RUN" -eq 1 ]; then
      log "update   CLAUDE.md (refresh marked Harness block, backup first)"
    else
      backup_claude_file
      tmp="$(mktemp)"
      awk '
        /<!-- HARNESS:BEGIN -->/ {
          while ((getline line < block_file) > 0) {
            print line
          }
          in_block = 1
          next
        }
        /<!-- HARNESS:END -->/ && in_block {
          in_block = 0
          next
        }
        !in_block { print }
      ' block_file="$block_tmp" "$target" > "$tmp"
      mv "$tmp" "$target"
      log "updated  CLAUDE.md (refreshed Harness block; backup: ${BACKUP_DIR#"$TARGET_DIR"/}/CLAUDE.md)"
    fi
    UPDATED=$((UPDATED + 1))
  elif [ -e "$target" ]; then
    if [ "$DRY_RUN" -eq 1 ]; then
      log "update   CLAUDE.md (append Harness block, backup first)"
    else
      backup_claude_file
      {
        printf '\n'
        cat "$block_tmp"
      } >> "$target"
      log "updated  CLAUDE.md (appended Harness block; backup: ${BACKUP_DIR#"$TARGET_DIR"/}/CLAUDE.md)"
    fi
    UPDATED=$((UPDATED + 1))
  else
    if [ "$DRY_RUN" -eq 1 ]; then
      log "create   CLAUDE.md"
    else
      {
        printf '# Project Rules\n\n'
        cat "$block_tmp"
      } > "$target"
      log "created  CLAUDE.md"
    fi
    CREATED=$((CREATED + 1))
  fi
  rm -f "$block_tmp"
}

detect_cli_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os:$arch" in
    Darwin:arm64)  printf 'macos-arm64' ;;
    Darwin:x86_64) printf 'macos-x64' ;;
    Linux:x86_64)  printf 'linux-x64' ;;
    Linux:aarch64|Linux:arm64) printf 'linux-arm64' ;;
    *)
      fail "Unsupported Harness CLI platform: $os/$arch."
      ;;
  esac
}

sha256_file() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{ print $1 }'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{ print $1 }'
  else
    fail "shasum or sha256sum is required to verify the Harness CLI download"
  fi
}

state_file() {
  printf '%s\n' "$TARGET_DIR/.harness/install-state.tsv"
}

assert_state_paths_are_safe() {
  local relative path

  [ ! -L "$TARGET_DIR" ] || fail "Refusing to use symlinked target project: $TARGET_DIR"

  for relative in .harness .harness-backup; do
    path="$TARGET_DIR/$relative"
    [ ! -L "$path" ] || fail "Refusing to use symlinked Harness state path: $relative"
    if [ -e "$path" ] && [ ! -d "$path" ]; then
      fail "Harness state path is not a directory: $relative"
    fi
  done
}

read_kit_version() {
  local version_file="scripts/harness-kit-version"
  local version=""

  if [ "$SOURCE_MODE" = "local" ]; then
    [ -f "$SOURCE_ROOT/$version_file" ] || fail "Harness kit version is missing: $SOURCE_ROOT/$version_file"
    version="$(awk 'NF && $1 !~ /^#/ { print $1; exit }' "$SOURCE_ROOT/$version_file")"
  else
    version="$(curl -fsSL "$SOURCE_BASE_URL/$version_file" 2>/dev/null || true)"
    version="$(printf '%s\n' "$version" | awk 'NF && $1 !~ /^#/ { print $1; exit }')"
  fi

  case "$version" in
    [0-9]*.[0-9]*.[0-9]*)
      printf '%s\n' "$version"
      ;;
    *)
      fail "Invalid Harness kit version: ${version:-missing}"
      ;;
  esac
}

STATE_ENTRIES=()

load_install_state() {
  local file kind relative hash
  file="$(state_file)"
  STATE_ENTRIES=()
  [ -f "$file" ] || return 0

  while IFS=$'\t' read -r kind relative hash || [ -n "$kind" ]; do
    [ "$kind" = "file" ] || continue
    [ -n "$relative" ] && [ -n "$hash" ] || continue
    STATE_ENTRIES+=("$relative"$'\t'"$hash")
  done < "$file"
}

record_managed_file() {
  local relative="$1"
  local target="$TARGET_DIR/$relative"
  local hash entry entry_relative entry_hash found=0
  local updated_entries=()

  [ -f "$target" ] || return 0
  hash="$(sha256_file "$target")"

  if [ "${#STATE_ENTRIES[@]}" -gt 0 ]; then
    for entry in "${STATE_ENTRIES[@]}"; do
      entry_relative="${entry%%$'\t'*}"
      entry_hash="${entry#*$'\t'}"
      if [ "$entry_relative" = "$relative" ]; then
        updated_entries+=("$relative"$'\t'"$hash")
        found=1
      else
        updated_entries+=("$entry_relative"$'\t'"$entry_hash")
      fi
    done
  fi

  if [ "$found" -eq 0 ]; then
    updated_entries+=("$relative"$'\t'"$hash")
  fi
  STATE_ENTRIES=("${updated_entries[@]}")
}

recorded_hash_for() {
  local relative="$1"
  local entry entry_relative

  if [ "${#STATE_ENTRIES[@]}" -gt 0 ]; then
    for entry in "${STATE_ENTRIES[@]}"; do
      entry_relative="${entry%%$'\t'*}"
      if [ "$entry_relative" = "$relative" ]; then
        printf '%s\n' "${entry#*$'\t'}"
        return 0
      fi
    done
  fi
}

write_install_state() {
  local file tmp entry

  [ "$DRY_RUN" -eq 0 ] || return 0
  file="$(state_file)"
  mkdir -p "$(dirname "$file")"
  tmp="$(mktemp "${file}.tmp.XXXXXX")"
  {
    printf 'version\t%s\n' "$KIT_VERSION"
    if [ "${#STATE_ENTRIES[@]}" -gt 0 ]; then
      for entry in "${STATE_ENTRIES[@]}"; do
        printf 'file\t%s\n' "$entry"
      done
    fi
  } > "$tmp"
  mv "$tmp" "$file"
}

download_file() {
  local url="$1"
  local target="$2"
  curl -fsSL "$url" -o "$target" || fail "Could not download $url"
}

read_cli_release_tag() {
  local tag_file="scripts/harness-cli-release-tag"
  local tag=""

  if [ "$SOURCE_MODE" = "local" ]; then
    if [ -f "$SOURCE_ROOT/$tag_file" ]; then
      tag="$(awk 'NF && $1 !~ /^#/ { print $1; exit }' "$SOURCE_ROOT/$tag_file")"
    fi
  else
    local tmp_file
    tmp_file="$(mktemp)"
    if curl -fsSL "$SOURCE_BASE_URL/$tag_file" -o "$tmp_file" 2>/dev/null; then
      tag="$(awk 'NF && $1 !~ /^#/ { print $1; exit }' "$tmp_file")"
    fi
    rm -f "$tmp_file"
  fi

  printf '%s\n' "$tag"
}

read_upstream_repository() {
  local config="scripts/harness-upstream-repository"
  local repository=""

  if [ "$SOURCE_MODE" = "local" ] && [ -f "$SOURCE_ROOT/$config" ]; then
    repository="$(awk 'NF && $1 !~ /^#/ { print $1; exit }' "$SOURCE_ROOT/$config")"
  fi

  if [ -z "$repository" ]; then
    repository="winterzxzz/repository-harness"
  fi

  case "$repository" in
    */*)
      printf '%s\n' "$repository"
      ;;
    *)
      fail "Invalid Harness upstream repository: $repository"
      ;;
  esac
}

default_cli_base_url() {
  local release_tag="${HARNESS_CLI_RELEASE_TAG:-}"

  if [ -z "$release_tag" ]; then
    release_tag="$(read_cli_release_tag)"
  fi

  if [ -n "$release_tag" ] && [ "$release_tag" != "latest" ]; then
    printf 'https://github.com/%s/releases/download/%s\n' \
      "$HARNESS_UPSTREAM_REPOSITORY" "$release_tag"
  else
    printf 'https://github.com/%s/releases/latest/download\n' \
      "$HARNESS_UPSTREAM_REPOSITORY"
  fi
}

PREPARED_CLI_DIR=""
PREPARED_CLI_BINARY=""

prepare_harness_cli_binary() {
  local platform binary_name binary_url checksum_url checksum_tmp expected actual

  platform="${HARNESS_CLI_PLATFORM:-$(detect_cli_platform)}"
  binary_name="harness-cli-$platform"
  binary_url="$CLI_BASE_URL/$binary_name"
  checksum_url="$binary_url.sha256"
  PREPARED_CLI_DIR="$(mktemp -d)"
  PREPARED_CLI_BINARY="$PREPARED_CLI_DIR/$binary_name"
  checksum_tmp="$PREPARED_CLI_DIR/$binary_name.sha256"

  if [ -n "$LOCAL_CLI_BINARY_PATH" ] || [ -n "$LOCAL_CLI_CHECKSUM_PATH" ]; then
    [ -n "$LOCAL_CLI_BINARY_PATH" ] && [ -n "$LOCAL_CLI_CHECKSUM_PATH" ] || fail "HARNESS_CLI_BINARY_PATH and HARNESS_CLI_CHECKSUM_PATH must be set together"
    [ -f "$LOCAL_CLI_BINARY_PATH" ] || fail "Local Harness CLI binary is missing: $LOCAL_CLI_BINARY_PATH"
    [ -f "$LOCAL_CLI_CHECKSUM_PATH" ] || fail "Local Harness CLI checksum file is missing: $LOCAL_CLI_CHECKSUM_PATH"
    cp "$LOCAL_CLI_BINARY_PATH" "$PREPARED_CLI_BINARY"
    cp "$LOCAL_CLI_CHECKSUM_PATH" "$checksum_tmp"
  else
    command -v curl >/dev/null 2>&1 || fail "curl is required to download the Harness CLI"
    download_file "$binary_url" "$PREPARED_CLI_BINARY"
    download_file "$checksum_url" "$checksum_tmp"
  fi

  expected="$(awk '{ print $1; exit }' "$checksum_tmp")"
  [ -n "$expected" ] || fail "Checksum file is empty: $checksum_url"
  actual="$(sha256_file "$PREPARED_CLI_BINARY")"
  [ "$actual" = "$expected" ] || fail "Checksum mismatch for $binary_name: expected $expected, got $actual"
}

discard_prepared_harness_cli() {
  [ -n "$PREPARED_CLI_DIR" ] && rm -rf "$PREPARED_CLI_DIR"
  PREPARED_CLI_DIR=""
  PREPARED_CLI_BINARY=""
}

update_harness_cli_binary() {
  local relative="scripts/bin/harness-cli"
  local target="$TARGET_DIR/$relative"
  local recorded_hash current_hash source_hash

  if [ ! -e "$target" ]; then
    if [ "$DRY_RUN" -eq 1 ]; then
      log "create   $relative"
    else
      prepare_harness_cli_binary
      mkdir -p "$(dirname "$target")"
      cp "$PREPARED_CLI_BINARY" "$target"
      chmod 755 "$target"
      discard_prepared_harness_cli
      record_managed_file "$relative"
      log "created  $relative"
    fi
    CREATED=$((CREATED + 1))
    return 0
  fi

  recorded_hash="$(recorded_hash_for "$relative")"
  if [ -z "$recorded_hash" ]; then
    log "skip     $relative (untracked existing file)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  current_hash="$(sha256_file "$target")"
  if [ "$current_hash" != "$recorded_hash" ] && [ "$FORCE" -eq 0 ]; then
    log "skip     $relative (modified locally)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    if [ "$current_hash" != "$recorded_hash" ]; then
      log "overwrite $relative (backup first)"
    else
      log "update   $relative"
    fi
    UPDATED=$((UPDATED + 1))
    return 0
  fi

  prepare_harness_cli_binary
  source_hash="$(sha256_file "$PREPARED_CLI_BINARY")"
  if [ "$source_hash" = "$current_hash" ]; then
    discard_prepared_harness_cli
    log "skip     $relative (already current)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  if [ "$current_hash" != "$recorded_hash" ]; then
    backup_target_file "$relative"
  fi
  cp "$PREPARED_CLI_BINARY" "$target"
  chmod 755 "$target"
  discard_prepared_harness_cli
  record_managed_file "$relative"
  UPDATED=$((UPDATED + 1))
  if [ "$current_hash" != "$recorded_hash" ]; then
    log "updated  $relative (backup: ${BACKUP_DIR#"$TARGET_DIR"/}/$relative)"
  else
    log "updated  $relative"
  fi
}

install_harness_cli_binary() {
  [ "$INSTALL_RUST_CLI" -eq 1 ] || return 0

  if [ "$UPDATE_MODE" -eq 1 ]; then
    update_harness_cli_binary
    return 0
  fi

  local platform target
  platform="${HARNESS_CLI_PLATFORM:-$(detect_cli_platform)}"
  target="$TARGET_DIR/scripts/bin/harness-cli"

  if [ -e "$target" ] && [ "$CONFLICT_ACTION" = "merge" ] && [ "$FORCE" -eq 0 ]; then
    log "skip     scripts/bin/harness-cli (merge keeps existing file)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  if [ "$DRY_RUN" -eq 1 ]; then
    if [ -n "$LOCAL_CLI_BINARY_PATH" ]; then
      log "copy     local Harness CLI -> scripts/bin/harness-cli"
      log "verify   local Harness CLI checksum"
    else
      log "download harness-cli-$platform -> scripts/bin/harness-cli"
      log "verify   harness-cli-$platform.sha256"
    fi
    CREATED=$((CREATED + 1))
    return 0
  fi

  prepare_harness_cli_binary
  mkdir -p "$(dirname "$target")"
  if [ -e "$target" ]; then
    if [ "$FORCE" -eq 1 ]; then
      backup_target_file "scripts/bin/harness-cli"
    fi
    UPDATED=$((UPDATED + 1))
    log "updated  scripts/bin/harness-cli"
  else
    CREATED=$((CREATED + 1))
    log "created  scripts/bin/harness-cli"
  fi

  cp "$PREPARED_CLI_BINARY" "$target"
  chmod 755 "$target"
  discard_prepared_harness_cli
  record_managed_file "scripts/bin/harness-cli"
  log "verified scripts/bin/harness-cli ($platform)"
}

check_protected_target_paths() {
  local conflicts=()

  [ -e "$TARGET_DIR/AGENTS.md" ] && conflicts+=("AGENTS.md")
  [ -e "$TARGET_DIR/docs" ] && conflicts+=("docs/")
  [ -e "$TARGET_DIR/scripts" ] && conflicts+=("scripts/")

  [ "${#conflicts[@]}" -gt 0 ] || return 0

  local joined=""
  local item
  for item in "${conflicts[@]}"; do
    if [ -n "$joined" ]; then
      joined="$joined, $item"
    else
      joined="$item"
    fi
  done

  case "$REQUESTED_CONFLICT_ACTION" in
    merge)
      CONFLICT_ACTION="merge"
      log "Continuing with merge. Existing files will be skipped."
      return 0
      ;;
    override)
      CONFLICT_ACTION="override"
      override_protected_target_paths
      return 0
      ;;
    stop)
      warn_stop "target already contains protected Harness paths: $joined. Refusing to install so existing project instructions or docs are not mixed or overwritten."
      ;;
  esac

  if [ "$YES" -eq 1 ] || ! can_prompt; then
    warn_stop "target already contains protected Harness paths: $joined. Refusing to install so existing project instructions or docs are not mixed or overwritten. Use an empty target directory, or move those paths before running the installer."
  fi

  {
    printf 'Warning: target already contains protected Harness paths: %s\n' "$joined"
    printf 'Choose how to continue:\n'
    printf '  1. Merge    Copy missing Harness files and skip existing files\n'
    printf '  2. Override Back up and replace AGENTS.md, docs/, and scripts/\n'
    printf '  3. Stop     Exit without writing files (recommended)\n'
  } > /dev/tty
  prompt_tty 'Choice [1/2/3, default 3]: '

  local choice
  choice="$(read_tty)"
  case "$choice" in
    1|m|M|merge|Merge)
      CONFLICT_ACTION="merge"
      log "Continuing with merge. Existing files will be skipped."
      ;;
    2|o|O|override|Override)
      CONFLICT_ACTION="override"
      override_protected_target_paths
      ;;
    ""|3|s|S|stop|Stop)
      warn_stop "installation stopped by user."
      ;;
    *)
      warn_stop "unknown choice: $choice"
      ;;
  esac
}

override_protected_target_paths() {
  local protected

  for protected in AGENTS.md docs scripts; do
    [ -e "$TARGET_DIR/$protected" ] || continue

    if [ "$DRY_RUN" -eq 1 ]; then
      log "override $protected (backup first)"
      continue
    fi

    mkdir -p "$BACKUP_DIR"
    mv "$TARGET_DIR/$protected" "$BACKUP_DIR/$protected"
    log "removed  $protected (backup: ${BACKUP_DIR#"$TARGET_DIR"/}/$protected)"
  done
}

TARGET_INPUT="${HARNESS_TARGET_DIR:-$PWD}"
YES=0
FORCE=0
DRY_RUN=0
INSTALL_RUST_CLI=1
REFRESH_AGENT_SHIM=0
INSTALL_CLAUDE_SHIM=0
REQUESTED_CONFLICT_ACTION=""
POSITIONAL_TARGET=""
UPDATE_MODE=0
ADOPT_MODE=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    -d|--directory)
      [ "$#" -ge 2 ] || fail "$1 requires a path"
      TARGET_INPUT="$2"
      shift 2
      ;;
    -y|--yes)
      YES=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --update)
      UPDATE_MODE=1
      shift
      ;;
    --adopt)
      UPDATE_MODE=1
      ADOPT_MODE=1
      shift
      ;;
    --merge)
      REQUESTED_CONFLICT_ACTION="merge"
      shift
      ;;
    --refresh-agent-shim)
      REFRESH_AGENT_SHIM=1
      shift
      ;;
    --claude)
      INSTALL_CLAUDE_SHIM=1
      shift
      ;;
    --override)
      REQUESTED_CONFLICT_ACTION="override"
      shift
      ;;
    --stop)
      REQUESTED_CONFLICT_ACTION="stop"
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      fail "Unknown option: $1"
      ;;
    *)
      [ -z "$POSITIONAL_TARGET" ] || fail "Only one target path is supported"
      POSITIONAL_TARGET="$1"
      shift
      ;;
  esac
done

if [ "$#" -gt 0 ]; then
  [ -z "$POSITIONAL_TARGET" ] || fail "Only one target path is supported"
  POSITIONAL_TARGET="$1"
  shift
fi

[ "$#" -eq 0 ] || fail "Unexpected extra arguments"

if [ "$UPDATE_MODE" -eq 1 ] && [ -n "$REQUESTED_CONFLICT_ACTION" ]; then
  fail "--update cannot be combined with --merge, --override, or --stop"
fi

if [ -n "$POSITIONAL_TARGET" ]; then
  TARGET_INPUT="$POSITIONAL_TARGET"
fi

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" 2>/dev/null && pwd -P || printf '')"
SOURCE_ROOT=""
SOURCE_MODE="remote"
PAYLOAD_MANIFEST="scripts/harness-install-files.txt"
SCHEMA_DIR="scripts/schema"

if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/../AGENTS.md" ] && [ -f "$SCRIPT_DIR/../docs/HARNESS.md" ]; then
  SOURCE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
  SOURCE_MODE="local"
fi

HARNESS_UPSTREAM_REPOSITORY="${HARNESS_UPSTREAM_REPOSITORY:-$(read_upstream_repository)}"
SOURCE_BASE_URL="${HARNESS_SOURCE_BASE_URL:-https://raw.githubusercontent.com/$HARNESS_UPSTREAM_REPOSITORY/main}"
SOURCE_BASE_URL="${SOURCE_BASE_URL%/}"
CLI_BASE_URL="${HARNESS_CLI_BASE_URL:-}"
CLI_BASE_URL="${CLI_BASE_URL%/}"
LOCAL_CLI_BINARY_PATH="${HARNESS_CLI_BINARY_PATH:-}"
LOCAL_CLI_CHECKSUM_PATH="${HARNESS_CLI_CHECKSUM_PATH:-}"

if [ -z "$CLI_BASE_URL" ]; then
  CLI_BASE_URL="$(default_cli_base_url)"
fi

if [ "$UPDATE_MODE" -eq 0 ] && [ "$YES" -eq 0 ] && can_prompt; then
  prompt_tty "Install Harness v0 into [$TARGET_INPUT]: "
  REPLY_TARGET="$(read_tty)"
  if [ -n "$REPLY_TARGET" ]; then
    TARGET_INPUT="$REPLY_TARGET"
  fi
fi

TARGET_DIR="$(make_absolute_parent "$(expand_path "$TARGET_INPUT")")"
assert_state_paths_are_safe
BACKUP_DIR="$TARGET_DIR/.harness-backup/$(date +%Y%m%d%H%M%S)"
CREATED=0
UPDATED=0
SKIPPED=0
CONFLICT_ACTION="install"
KIT_VERSION="$(read_kit_version)"
load_install_state

if [ "$UPDATE_MODE" -eq 1 ] && [ ! -d "$TARGET_DIR" ]; then
  fail "Cannot update a missing target directory: $TARGET_DIR"
elif [ "$DRY_RUN" -eq 1 ]; then
  log "Dry run: no files will be written."
elif [ ! -d "$TARGET_DIR" ]; then
  mkdir -p "$TARGET_DIR"
fi

if [ ! -d "$TARGET_DIR" ]; then
  [ "$DRY_RUN" -eq 1 ] || fail "Target directory could not be created: $TARGET_DIR"
  log "Target directory would be created: $TARGET_DIR"
fi

if [ -d "$TARGET_DIR" ]; then
  [ -w "$TARGET_DIR" ] || fail "Target directory is not writable: $TARGET_DIR"
else
  [ -w "$(dirname "$TARGET_DIR")" ] || fail "Target parent directory is not writable: $(dirname "$TARGET_DIR")"
fi

if [ "$UPDATE_MODE" -eq 0 ] && [ -d "$TARGET_DIR" ]; then
  check_protected_target_paths
fi

if [ "$SOURCE_MODE" = "local" ]; then
  log "Harness source: $SOURCE_ROOT"
else
  command -v curl >/dev/null 2>&1 || fail "curl is required for remote installation"
  log "Harness source: $SOURCE_BASE_URL"
fi
if [ "$INSTALL_RUST_CLI" -eq 1 ]; then
  log "Harness CLI source: $CLI_BASE_URL"
else
  log "Harness CLI source: skipped"
fi
log "Target project: $TARGET_DIR"

if [ "$UPDATE_MODE" -eq 1 ]; then
  if [ ! -f "$(state_file)" ] && [ "$ADOPT_MODE" -eq 0 ]; then
    fail "Run 'harness update --adopt' to begin tracking this legacy installation."
  fi
  preflight_managed_paths

  if [ "$ADOPT_MODE" -eq 1 ]; then
    adopt_existing_files
  else
    update_payload_files
    install_harness_cli_binary
  fi
else
  if [ -d "$TARGET_DIR" ]; then
    preflight_managed_paths
  fi
  copy_payload_files
  refresh_agent_shim
  write_claude_shim
  install_harness_cli_binary
fi
write_install_state

log ""
log "Done. Created: $CREATED, updated: $UPDATED, skipped: $SKIPPED."

if [ "$SKIPPED" -gt 0 ] && [ "$FORCE" -eq 0 ]; then
  log "Existing files were left untouched. Re-run with --force to overwrite with backups."
fi

if [ "$FORCE" -eq 1 ] && [ "$UPDATED" -gt 0 ] && [ "$DRY_RUN" -eq 0 ]; then
  log "Backups were written to: $BACKUP_DIR"
fi
