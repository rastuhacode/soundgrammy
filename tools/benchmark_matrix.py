#!/usr/bin/env python3
"""Repeated, isolated macOS release workload matrix. Never launch installed apps."""
import argparse, base64, hashlib, json, os, platform, signal, statistics, subprocess, tempfile, time
from pathlib import Path
from benchmark import processes, related
CASES=[('idle-500',500,False),('idle-5000',5000,False),('idle-20000',20000,False),('artwork-idle',5000,True),('scroll',5000,False),('artwork-scroll',5000,True),('search',5000,False),('search-filter',5000,False),('drawer',5000,True),('settings',5000,True),('fullscreen',5000,True),('fullscreen-playing',5000,True),('minimized',5000,False)]
def fixture_file(path, count, art):
    tracks=[dict(id=i+1,title=f'Track {i+1:05d}',performer=f'Artist {i%50:02d}',duration=196,mime_type='audio/mpeg',file_size=1048576) for i in range(count)]
    lane=lambda period:base64.b64encode(bytes(220 if i%period<period//3 else 30 for i in range(8000))).decode()
    value=dict(tracks=tracks,playlists=[dict(id=1,kind='liked',track_ids=[1,3,5],updated_at='2026-11-14'),dict(id=2,kind='custom',name='Presentation',track_ids=[1,2,1,4],updated_at='2026-11-14')],user=dict(tg_user_id=1,first_name='SoundGrammy'),player=dict(cursor=4,seconds=83),cached=[],thumbnails={str(i+1):f'/private/tmp/soundgrammy-benchmark-assets/cover-{i%64+1:02d}.png' for i in range(count)} if art else {},bounce_profile=dict(status='ready',algorithmVersion=2,frameMs=25,durationMs=200000,loudnessData=lane(20),onsetData=lane(20),bassData=lane(20),midData=lane(13),trebleData=lane(7)))
    path.write_text(json.dumps(value))
def measure(app,binary,scenario,count,art,index,seconds):
    previous=processes()
    with tempfile.TemporaryDirectory(prefix='soundgrammy-matrix-',dir='/private/tmp') as folder:
        root=Path(folder);fixture=root/'fixture.json';fixture_file(fixture,count,art);ready=root/'ready.json'
        env=dict(os.environ,SOUNDGRAMMY_DATA_DIR=folder,SOUNDGRAMMY_BENCH_FIXTURE='1',SOUNDGRAMMY_BENCH_FIXTURE_PATH=str(fixture),SOUNDGRAMMY_BENCH_SCENARIO=scenario,SOUNDGRAMMY_BENCH_READY_FILE=str(ready),SOUNDGRAMMY_PREVIEW_PERF=str(root/'enabled.json'))
        args=[str(binary)]+(['--preview-fixture',str(fixture)]+(['--preview-compact'] if scenario in ('search','search-filter') else []) if app.startswith('GPUI') else [])
        with (root/'app.log').open('w') as log:
            started=time.monotonic();child=subprocess.Popen(args,env=env,stdout=log,stderr=log,start_new_session=True)
            startup_ms=None
            try:
                while not ready.exists():
                    if startup_ms is None and ready.with_suffix('.startup.json').exists():startup_ms=(time.monotonic()-started)*1000
                    if child.poll() is not None or time.monotonic()-started>30:raise RuntimeError(f'{app} {scenario}: {(root/"app.log").read_text()}')
                    time.sleep(.01)
                metadata=json.loads(ready.read_text())
                if metadata.get('error'):raise RuntimeError(str(metadata))
                if metadata['tracks']!=count:raise RuntimeError('Invalid population')
                if scenario.startswith('fullscreen') and not metadata.get('fullscreen'):raise RuntimeError('Not fullscreen')
                baseline=related(processes(),child.pid,previous,app=='Tauri');tracked=dict(baseline);initial=time.monotonic();samples=[]
                for _ in range(seconds):
                    time.sleep(1)
                    current=related(processes(),child.pid,previous,app=='Tauri');tracked.update(current)
                    samples.append(dict(at=time.monotonic(),rss_mib=sum(p['rss_kib'] for p in current.values())/1024,cpu={str(pid):p['cpu_seconds'] for pid,p in current.items()},process_count=len(current)))
                elapsed=samples[-1]['at']-initial
                cpu=sum(max(0,samples[-1]['cpu'].get(str(pid),p['cpu_seconds'])-p['cpu_seconds']) for pid,p in baseline.items())/elapsed*100
                metricsfile=ready.with_suffix('.frames.json')
                if not app.startswith('GPUI') and scenario in ('scroll','artwork-scroll','search','fullscreen','fullscreen-playing'):
                    deadline=time.monotonic()+4
                    while not metricsfile.exists() and time.monotonic()<deadline:time.sleep(.05)
                metrics=json.loads(metricsfile.read_text()) if metricsfile.exists() else None
                return dict(app=app,scenario=scenario,run=index,tracks=count,artwork=art,startup_detect_ms=startup_ms,startup=json.loads(ready.with_suffix('.startup.json').read_text()) if ready.with_suffix('.startup.json').exists() else None,rss_mib_median=statistics.median(s['rss_mib'] for s in samples),rss_mib_max=max(s['rss_mib'] for s in samples),cpu_percent_one_core=cpu,elapsed_seconds=elapsed,ready=metadata,samples=samples,metrics=metrics,processes=[dict(pid=pid,**p) for pid,p in tracked.items()])
            finally:
                if child.poll() is None:
                    os.killpg(child.pid,signal.SIGTERM)
                    try:child.wait(timeout=5)
                    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
                # WebKit XPC helpers belong to launchd, never globally kill them.
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tauri',type=Path,required=True);parser.add_argument('--gpui',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--runs',type=int,default=3);parser.add_argument('--seconds',type=int,default=8);parser.add_argument('--cases',nargs='*');parser.add_argument('--gpui-label',default='GPUI');parser.add_argument('--baseline-label',default='Tauri');parser.add_argument('--only-app')
    args=parser.parse_args()
    for binary in (args.tauri,args.gpui):
        if not binary.is_absolute() or not str(binary).startswith('/private/tmp/') or '.app/Contents/MacOS/' not in str(binary):parser.error('Only temporary audit bundle executables are accepted')
    result=dict(date_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),system=subprocess.check_output(['sw_vers'],text=True),hardware=subprocess.check_output(['sysctl','-n','hw.model','hw.memsize','hw.logicalcpu'],text=True),seconds=args.seconds,requested_runs=args.runs,binaries={name:dict(bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for name,p in [(args.baseline_label,args.tauri),(args.gpui_label,args.gpui)]},runs=[])
    if args.output.exists():
        old=json.loads(args.output.read_text());result['runs']=old['runs']
        if old['binaries']!=result['binaries']:raise RuntimeError('Refuse to mix different binaries in one result file')
    for index in range(1,args.runs+1):
        cases=[c for c in CASES if not args.cases or c[0] in args.cases]
        if index%2==0:cases.reverse()
        for scenario,count,art in cases:
            order=[(args.baseline_label,args.tauri),(args.gpui_label,args.gpui)]
            if index%2==0:order.reverse()
            for app,binary in order:
                if args.only_app and app!=args.only_app:continue
                if any(r['app']==app and r['scenario']==scenario and r['run']==index for r in result['runs']):continue
                item=measure(app,binary,scenario,count,art,index,args.seconds);result['runs'].append(item)
                args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,indent=2)+'\n')
                print(f'{index} {app} {scenario}: {item["rss_mib_median"]:.1f} MiB, {item["cpu_percent_one_core"]:.2f}% CPU, startup {item["startup_detect_ms"]} ms',flush=True)
if __name__=='__main__':main()
