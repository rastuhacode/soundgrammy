import { Volume2, VolumeX } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export interface AudioVolumeProps {
  volume: number;
  onVolumeChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onMuteToggle: () => void;
}

export function AudioVolume(props: AudioVolumeProps) {
  const isMuted = props.volume === 0;

  const VolumeIcon = isMuted ? VolumeX : Volume2;

  return (
    <>
      <Button
        aria-label={isMuted ? "Unmute" : "Mute"}
        onClick={props.onMuteToggle}
        variant="ghost"
        size="icon-sm"
      >
        <VolumeIcon className="size-5 shrink-0" />
      </Button>
      <input
        type="range"
        className={cn(
          "h-0.5 w-16 appearance-none outline-none",
          // track fill (replaces .hifi-range background)
          "[background:linear-gradient(to_right,var(--primary)_0%,var(--primary)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_100%)]",
          // webkit thumb
          "[&::-webkit-slider-thumb]:appearance-none",
          "[&::-webkit-slider-thumb]:size-[8px]",
          "[&::-webkit-slider-thumb]:rounded-full",
          "[&::-webkit-slider-thumb]:bg-primary",
          "[&::-webkit-slider-thumb]:transition-transform",
          "[&::-webkit-slider-thumb]:duration-150",
          "[&::-webkit-slider-thumb:hover]:scale-125",
          // firefox thumb
          "[&::-moz-range-thumb]:size-[8px]",
          "[&::-moz-range-thumb]:border-none",
          "[&::-moz-range-thumb]:rounded-full",
          "[&::-moz-range-thumb]:bg-primary",
        )}
        min={0}
        max={100}
        step={0.01}
        value={props.volume}
        onChange={props.onVolumeChange}
        autoComplete="off"
        aria-label="Volume"
        style={{ "--progress": `${props.volume}%` } as React.CSSProperties}
      />
    </>
  );
}
