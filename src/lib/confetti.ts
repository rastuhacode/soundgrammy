import type { Options } from 'canvas-confetti'
import confetti from 'canvas-confetti'

/** Small burst centered on an element — for like / celebrate affordances. */
export function fireSmallConfettiAt(
  element: HTMLElement,
  options?: Options,
) {
  const rect = element.getBoundingClientRect()
  const x = (rect.left + rect.width / 2) / window.innerWidth
  const y = (rect.top + rect.height / 2) / window.innerHeight
  return confetti({
    particleCount: 28,
    spread: 46,
    startVelocity: 16,
    scalar: 0.65,
    ticks: 90,
    gravity: 1.1,
    origin: { x, y },
    colors: ['#ff4d6d', '#ff8fa3', '#ffc2d1', '#c9184a', '#fff0f3'],
    ...options,
  })
}
