import { ShuffleButton, type ShuffleButtonProps } from './buttons/ShuffleButton'
import { NextButton, type NextButtonProps } from './buttons/NextButton'
import { PlayPauseButton, type PlayPauseButtonProps } from './buttons/PlayPauseButton'
import { PreviousButton, type PreviousButtonProps } from './buttons/PreviousButton'
import { RepeatButton, type RepeatButtonProps } from './buttons/RepeatButton'

type AudioMainOperationsProps
  = ShuffleButtonProps
    & NextButtonProps
    & PlayPauseButtonProps
    & PreviousButtonProps
    & RepeatButtonProps
    & {}

export function AudioMainOperations(props: AudioMainOperationsProps) {
  return (
    <div className="flex items-center gap-6">
      <div className="flex items-center gap-4">
        <RepeatButton repeatState={props.repeatState} onRepeatToggle={props.onRepeatToggle} />
        <PreviousButton onPreviousTrack={props.onPreviousTrack} />
      </div>

      <PlayPauseButton isPlaying={props.isPlaying} onPlayToggle={props.onPlayToggle} />

      <div className="flex items-center gap-4">
        <NextButton onNextTrack={props.onNextTrack} />
        <ShuffleButton shuffleState={props.shuffleState} onShuffleToggle={props.onShuffleToggle} />
      </div>
    </div>
  )
}
