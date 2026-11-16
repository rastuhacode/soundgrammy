#!/usr/bin/env python3
"""Measure macOS audit bundles, never installed/production app binaries.

Requires two prepared bundles under a temporary root. The Tauri audit build must
use an audit identifier, isolated paths and the fixture/ready adapter described
in docs/gpui-audit.md. Run outside the shell sandbox to read ps counters.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import tempfile
import time


def processes():
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,time=,comm='], text=True)
    result = {}
    for line in output.splitlines():
        fields = line.strip().split(None, 4)
        if len(fields) != 5:
            continue
        pid, parent, rss, cpu, command = fields
        parts = cpu.replace('-', ':').split(':')
        seconds = 0.
        for part in parts:
            seconds = seconds * 60 + float(part)
        result[int(pid)] = dict(parent=int(parent), rss_kib=int(rss), cpu_seconds=seconds, command=command)
    return result


def related(table, root, previous, include_webkit):
    ids = {root}
    while True:
        children = {pid for pid, p in table.items() if p['parent'] in ids}
        if children <= ids:
            break
        ids |= children
    if include_webkit:
        ids |= {pid for pid, p in table.items() if pid not in previous and '/com.apple.WebKit.' in p['command']}
    return {pid: table[pid] for pid in ids if pid in table}


def measure(name, binary, fixture, warmup, duration, index):
    previous = processes()
    with tempfile.TemporaryDirectory(prefix='soundgrammy-benchmark-', dir='/private/tmp') as directory:
        env = os.environ.copy()
        env['SOUNDGRAMMY_DATA_DIR'] = directory
        env['SOUNDGRAMMY_BENCH_READY_FILE'] = str(Path(directory) / 'ready.json')
        env['SOUNDGRAMMY_BENCH_FIXTURE'] = '1'
        args = [str(binary)]
        if name == 'GPUI':
            args += ['--preview-fixture', str(fixture)]
        log_path = Path(directory) / 'app.log'
        with log_path.open('w') as log:
            child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
            tracked = {}
            samples = []
            try:
                start = time.monotonic()
                while time.monotonic() - start < warmup:
                    if child.poll() is not None:
                        raise RuntimeError(f'{name} exited: {log_path.read_text()}')
                    time.sleep(.2)
                initial = related(processes(), child.pid, previous, name == 'Tauri')
                tracked.update(initial)
                for _ in range(duration):
                    time.sleep(1)
                    if child.poll() is not None:
                        raise RuntimeError(f'{name} exited: {log_path.read_text()}')
                    table = related(processes(), child.pid, previous, name == 'Tauri')
                    tracked.update(table)
                    samples.append({'at': time.monotonic(), 'rss_mib': sum(p['rss_kib'] for p in table.values()) / 1024,
                                    'cpu': {str(pid): p['cpu_seconds'] for pid, p in table.items()}, 'process_count': len(table)})
                first, last = samples[0], samples[-1]
                delta = sum(last['cpu'].get(pid, value) - value for pid, value in first['cpu'].items())
                elapsed = last['at'] - first['at']
                ready = Path(env['SOUNDGRAMMY_BENCH_READY_FILE'])
                return {'app': name, 'run': index, 'rss_mib_median': statistics.median(s['rss_mib'] for s in samples),
                        'rss_mib_max': max(s['rss_mib'] for s in samples), 'cpu_percent_one_core': delta / elapsed * 100,
                        'elapsed_seconds': elapsed, 'processes': [dict(pid=pid, **p) for pid, p in tracked.items()],
                        'ready': json.loads(ready.read_text()) if ready.exists() else None, 'samples': samples}
            finally:
                # Only this launched process group and newly observed helpers.
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
    parser.add_argument('--warmup', type=int, default=10)
    parser.add_argument('--seconds', type=int, default=20)
    args = parser.parse_args()
    for binary in (args.tauri, args.gpui):
        if not binary.is_absolute() or not str(binary).startswith(('/private/tmp/', '/tmp/')) or '.app/Contents/MacOS/' not in str(binary):
            parser.error('Only temporary audit bundle executables are accepted')
    result = {'date_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              'system': subprocess.check_output(['sw_vers'], text=True),
              'hardware': subprocess.check_output(['sysctl', '-n', 'hw.model', 'hw.memsize', 'hw.logicalcpu'], text=True),
              'binary_bytes': {'Tauri': args.tauri.stat().st_size, 'GPUI': args.gpui.stat().st_size},
              'fixture_tracks': len(json.loads(args.fixture.read_text())['tracks']), 'warmup_seconds': args.warmup, 'runs': []}
    for index in range(args.runs):
        order = [('Tauri', args.tauri), ('GPUI', args.gpui)]
        if index % 2:
            order.reverse()
        for name, binary in order:
            item = measure(name, binary, args.fixture, args.warmup, args.seconds, index + 1)
            result['runs'].append(item)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(result, indent=2) + '\n')
            print(f"{name} run {index+1}: RSS {item['rss_mib_median']:.1f} MiB; CPU {item['cpu_percent_one_core']:.2f}%", flush=True)


if __name__ == '__main__':
    main()
