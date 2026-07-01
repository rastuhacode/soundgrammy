import type { Track } from "@/lib/db";
import { Music } from "lucide-react";
import { useCachedThumbnail } from "@/hooks/use-cached-thumbnail";

export interface AudioTrackDescriptionProps {
  track: Track;
}

export function AudioTrackDescription(props: AudioTrackDescriptionProps) {
  const { url, failed } = useCachedThumbnail(
    props.track.file_unique_id,
    props.track.id,
  );

  return (
    <>
      <div className="relative shrink-0">
        {failed || !url
          ? (
              <div className="flex size-16 items-center justify-center rounded-lg bg-muted text-muted-foreground">
                <Music className="size-5" />
              </div>
            )
          : (
              <img
                src={url}
                alt="Thumbnail"
                className="size-16 rounded-lg object-cover ring-1 ring-border"
              />
            )}
      </div>
      <div className="hidden min-w-0 flex-col sm:flex">
        <span
          className="truncate text-sm font-medium text-foreground"
          title={props.track.title ?? "Unknown Title"}
        >
          {props.track.title ?? "Unknown Title"}
        </span>
        <span
          className="truncate text-xs text-muted-foreground"
          title={props.track.performer ?? "Unknown Artist"}
        >
          {props.track.performer ?? "Unknown Artist"}
        </span>
      </div>
    </>
  );
}
