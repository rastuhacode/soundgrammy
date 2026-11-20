#!/usr/bin/env python3
"""Compare synthetic scrolling cadence in temporary, isolated macOS audit apps.

Tauri requires the opt-in frame adapter in docs/performance/tauri-fixture.patch.
GPUI's probe is enabled only alongside --preview-fixture and temporary output.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

from benchmark import processes, related


def measure(name, mode, binary, fixture, index):
    previous = processes()
    with tempfile.TemporaryDirectory(prefix='soundgrammy-frames-', dir='/private/tmp') as directory:
        output = Path(directory) / 'frames.json'
        env = dict(os.environ, SOUNDGRAMMY_DATA_DIR=directory,
                   SOUNDGRAMMY_BENCH_FIXTURE='1', SOUNDGRAMMY_PREVIEW_PERF_MODE=mode,
                   SOUNDGRAMMY_PREVIEW_PERF=str(output), SOUNDGRAMMY_BENCH_READY_FILE=str(output))
        args = [str(binary)] + (['--preview-fixture', str(fixture)] if name == 'GPUI' else [])
        with (Path(directory) / 'app.log').open('w') as log:
            child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
            tracked = {}
            try:
                deadline = time.monotonic() + 40
                while time.monotonic() < deadline:
                    tracked.update(related(processes(), child.pid, previous, name == 'Tauri'))
                    if output.exists():
                        try:
                            result = json.loads(output.read_text())
                        except json.JSONDecodeError:
                            result = {}
                        if 'intervals_ms' in result:
                            result.update(app=name, run=index)
                            return result
                    if child.poll() is not None:
                        raise RuntimeError((Path(directory) / 'app.log').read_text())
                    time.sleep(.2)
                raise RuntimeError(f'{name} {mode} frame probe timed out')
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait()
                # WebKit XPC processes are owned by launchd. Let macOS reap
                # them when this audit app exits; never kill globally discovered
                # helpers, which could belong to another running application.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tauri', type=Path, required=True)
    parser.add_argument('--gpui', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=3)
    args = parser.parse_args()
    for binary in (args.tauri, args.gpui):
        if not binary.is_absolute() or not str(binary).startswith(('/private/tmp/', '/tmp/')) or '.app/Contents/MacOS/' not in str(binary):
            parser.error('Only temporary audit bundle executables are accepted')
    results = []
    for index in range(1, args.runs + 1):
        for app, mode, binary in [('Tauri', 'scroll', args.tauri), ('GPUI', 'scroll', args.gpui),
                                  ('GPUI', 'search', args.gpui), ('GPUI', 'drawer', args.gpui)]:
            item = measure(app, mode, binary, args.fixture, index)
            results.append(item)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(results, indent=2) + '\n')
            print(f'{app} {mode} run {index}: {item["fps"]:.1f} FPS; p95 {item["p95_frame_ms"]:.2f} ms; opening {item.get("opening_next_frame_ms")}', flush=True)


if __name__ == '__main__':
    main()
