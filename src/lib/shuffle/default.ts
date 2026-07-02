import type { ShuffleAlgorithm } from './model'

/**
 * Default shuffle algorithm that shuffles the playlist using the Fisher-Yates shuffle algorithm.
 * @param playlist - The playlist to shuffle.
 * @returns The new shuffled playlist.
 */
export const defaultShuffle: ShuffleAlgorithm = (playlist) => {
  const shuffled = [...playlist]
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j]!, shuffled[i]!]
  }
  return shuffled
}
