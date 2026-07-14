import { NextButton } from './buttons/NextButton'
import { PlayPauseButton } from './buttons/PlayPauseButton'
import { PreviousButton } from './buttons/PreviousButton'
import { RepeatButton } from './buttons/RepeatButton'
import { ShuffleButton } from './buttons/ShuffleButton'

export function AudioMainOperations() {
  return (
    <div className="flex items-center gap-6">
      <div className="flex items-center gap-4">
        <RepeatButton />
        <PreviousButton />
      </div>

      <PlayPauseButton />

      <div className="flex items-center gap-4">
        <NextButton />
        <ShuffleButton />
      </div>
    </div>
  )
}
