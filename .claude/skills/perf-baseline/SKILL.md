---
name: perf-baseline
description: Capture Meteor's resource baseline (idle CPU, wakeups, memory, disk writes, cache sizes) with docs/perf/capture.ps1 and diff it against a previous run. Use when the user says "baseline", "medir", "¿cuánto consume?", or before/after any optimization phase.
---

# Performance baseline

Every optimization in the roadmap has a numeric exit criterion. This produces the
numbers. **Never report an improvement without a captured before and after.**

## Capture

The app must be **running and hidden to the tray**, with no game open:

```powershell
powershell -File docs\perf\capture.ps1 -Label <label> -Minutes 10
```

Writes `docs/perf/<label>.json`. To compare:

```powershell
powershell -File docs\perf\capture.ps1 -Label <label> -Minutes 10 -Compare docs\perf\baseline.json
```

## Report

Diff these keys and give each an absolute value and a percentage:

| Key | What good looks like |
|---|---|
| `idle.cpu_percent_mean` | ≈ 0 with the window in the tray |
| `idle.context_switches_s` | near 0 (watcher, sampler and cputemp all park) |
| `idle.files_written_count` | **0** over 10 minutes |
| `idle.meteor_private_mb` | drops after 60 s without a game (backends released) |
| `idle.webview_workingset_mb` | drops ~10 s after hiding the window |
| `idle.nvml_loaded` / `adlx_loaded` | `false` until a game runs |
| `caches.covers_mb` | under the 200 MB cap |

Rules:
- A metric that was not captured is reported as unknown — never as unchanged.
- Flag anything more than 5 % worse than the comparison run as a regression.
- Machine state matters: note anything unusual (a game updating, a scan running)
  in the report, since it invalidates the numbers.

## Beyond the script

- In-game HUD: `METEOR_OVERLAY_DEBUG=1` and confirm the composition mode stays
  `OVERLAY`. A drop to `COMPOSED` means the overlay is costing the game FPS and
  the change that caused it must be reverted.
- `get_library` timings: `METEOR_PERF=1` prints `perf <name> <ms>` on stderr.
- Grid rendering: `NEXT_PUBLIC_MOCK_LIBRARY=500 npm run dev` + the React
  DevTools profiler (commits per cover pass, longest commit).
