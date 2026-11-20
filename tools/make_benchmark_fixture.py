#!/usr/bin/env python3
"""Write the synthetic display-only fixture shared by both benchmark apps."""
import argparse
import json
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('output',type=Path)
parser.add_argument('--tracks',type=int,default=5000)
args=parser.parse_args()
if args.tracks<5:parser.error('at least five tracks are required')
tracks=[dict(id=i+1,title=f'Track {i+1:05d}',performer=f'Artist {i%50:02d}',duration=196,mime_type='audio/mpeg',file_size=1048576) for i in range(args.tracks)]
fixture=dict(tracks=tracks,playlists=[dict(id=1,kind='liked',track_ids=[1,3,5],updated_at='2026-11-14'),dict(id=2,kind='custom',name='Presentation',track_ids=[1,2,1,4],updated_at='2026-11-14')],user=dict(tg_user_id=1,first_name='SoundGrammy'),player=dict(cursor=4,seconds=83),cached=[],thumbnails={})
args.output.write_text(json.dumps(fixture)+'\n')
