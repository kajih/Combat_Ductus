## Parent

None - dev tooling, follow-up from docs/issues/tooling/makefile-build-targets.md.

## What to build

`trunk build --release` currently fails on this machine partway through its `wasm-opt` post-processing step:

```
error: error from build pipeline

Caused by:
    0: HTML build pipeline failed (1 errors), showing first
    1: error from asset pipeline
    2: running wasm-opt
    3: error copying (optimized) wasm file to dist dir
    4: The system cannot find the path specified. (os error 3)
```

Discovered while verifying `make web-release` (makefile-build-targets.md) - it reproduces with the bare `trunk build --release` command too, so it's unrelated to the Makefile itself. Per `docs/adr/0001-web-wasm-as-primary-target.md`, web/wasm is the primary target - this currently blocks producing any real production web build on this machine at all. (`trunk serve`, the dev-mode path, is unaffected - it skips `wasm-opt` entirely and was confirmed working.)

Root cause not yet diagnosed - could be a `wasm-opt`/trunk version issue specific to Windows, a path-length or path-format issue, an antivirus/file-lock interaction, or something else. Diagnosing that is part of this issue's own scope, not decided upfront.

## Acceptance criteria

- [ ] `trunk build --release` completes successfully on this machine (or the diagnosed root cause is documented here if it turns out to be genuinely unfixable on this setup)
- [ ] `dist/` contains a working production build (`index.html`, the optimized `.wasm`, the generated `.js`) after a `trunk build --release`
- [ ] `make web-release`/`make all` (which depend on this) work end-to-end
- [ ] If the fix is version-specific (e.g. pinning `trunk` or a bundled `wasm-opt` to a different version), that's documented in `CLAUDE.md`/`README.md` alongside the project's other prerequisite notes

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/42
