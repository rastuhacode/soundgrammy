import type { ShuffleMode } from './model'

export interface ShuffleModeOption {
  id: ShuffleMode
  label: string
  description: string
}

export const SHUFFLE_MODE_OPTIONS: readonly ShuffleModeOption[] = [
  {
    id: 'random',
    label: 'Random',
    description: 'Gives every track an equal chance using a standard random shuffle.',
  },
  {
    id: 'variety',
    label: 'Variety',
    description: 'Spreads tracks by the same performer apart whenever possible.',
  },
  {
    id: 'rediscover',
    label: 'Rediscover',
    description: 'Prioritizes tracks you have never played or have not heard recently.',
  },
  {
    id: 'smart',
    label: 'Smart Mix',
    description: 'Balances favorites, forgotten tracks, discovery, and your skip history.',
  },
  {
    id: 'fresh',
    label: 'Fresh Mix',
    description: 'Favors recently added tracks while continuing to mix in older music.',
  },
  {
    id: 'duration',
    label: 'Duration Mix',
    description: 'Alternates shorter and longer tracks whenever the playlist allows it.',
  },
]
