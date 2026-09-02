import { AnimatePresence, motion, useAnimationControls, useReducedMotion } from 'motion/react'
import { useState } from 'react'

const TRANSMISSIONS = [
  'Click to make it slightly less dead',
  'Still dead, now with momentum',
  'Signal sent, nobody complained',
  'One small click for a human',
  'The feature remains in orbit',
  'Houston, we have no content',
  'Make us whole again',
] as const

export function DeadSpace() {
  const [transmissionIndex, setTransmissionIndex] = useState(0)
  const [boostCount, setBoostCount] = useState(0)
  const satelliteControls = useAnimationControls()
  const prefersReducedMotion = useReducedMotion()

  const boostSatellite = () => {
    setTransmissionIndex((current) => {
      const nextOffset = 1 + Math.floor(Math.random() * (TRANSMISSIONS.length - 1))
      return (current + nextOffset) % TRANSMISSIONS.length
    })
    setBoostCount(current => current + 1)

    if (prefersReducedMotion) return

    satelliteControls.stop()
    void satelliteControls.start({
      x: [0, 72, 150, -150, -72, 0],
      y: [0, -3, -5, 5, 3, 0],
      opacity: [1, 1, 0, 0, 1, 1],
      transition: {
        duration: 1.25,
        ease: 'easeInOut',
        times: [0, 0.28, 0.43, 0.57, 0.72, 1],
      },
    })
  }

  return (
    <button
      type="button"
      onClick={boostSatellite}
      className="group relative flex min-h-0 grow flex-col items-center justify-center overflow-hidden bg-sidebar pb-4 text-center outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-sidebar-ring/60"
    >
      <motion.svg
        aria-hidden="true"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        className="pointer-events-none absolute inset-0 size-full text-sidebar-foreground"
        initial={false}
      >
        <motion.g
          animate={prefersReducedMotion ? undefined : { x: [1.5, -1.5, 1.5] }}
          transition={{ duration: 9, ease: 'linear', repeat: Infinity }}
          fill="currentColor"
          stroke="currentColor"
          strokeLinecap="round"
        >
          <motion.path
            d="M4 0v4M2 2h4"
            fill="none"
            strokeWidth="0.55"
            opacity="0.82"
            animate={prefersReducedMotion ? undefined : { opacity: [0.38, 0.9, 0.38] }}
            transition={{ duration: 3.6, repeat: Infinity }}
          />
          <circle cx="20" cy="14" r="0.5" stroke="none" opacity="0.72" />
          <path d="M39 1v3m-1.5-1.5h3" fill="none" strokeWidth="0.45" opacity="0.56" />
          <circle cx="59" cy="8" r="0.4" stroke="none" opacity="0.48" />
          <motion.path
            d="M97 0v5m-2.5-2.5h5"
            fill="none"
            strokeWidth="0.6"
            opacity="0.88"
            animate={prefersReducedMotion ? undefined : { opacity: [0.85, 0.35, 0.85] }}
            transition={{ duration: 4.4, repeat: Infinity }}
          />
          <circle cx="99" cy="29" r="0.6" stroke="none" opacity="0.68" />
          <circle cx="82" cy="51" r="0.4" stroke="none" opacity="0.4" />
          <path d="M96 72v4m-2-2h4" fill="none" strokeWidth="0.45" opacity="0.58" />
          <circle cx="97" cy="98" r="0.5" stroke="none" opacity="0.7" />
          <path d="M55 95v4m-2-2h4" fill="none" strokeWidth="0.5" opacity="0.76" />
          <circle cx="36" cy="78" r="0.45" stroke="none" opacity="0.48" />
          <motion.path
            d="M3 95v5M0.5 97.5h5"
            fill="none"
            strokeWidth="0.6"
            opacity="0.8"
            animate={prefersReducedMotion ? undefined : { opacity: [0.3, 0.85, 0.3] }}
            transition={{ duration: 3.2, repeat: Infinity }}
          />
          <circle cx="1" cy="54" r="0.45" stroke="none" opacity="0.62" />
          <circle cx="15" cy="40" r="0.35" stroke="none" opacity="0.46" />
          <path d="M72 22v3m-1.5-1.5h3" fill="none" strokeWidth="0.4" opacity="0.52" />
          <circle cx="69" cy="69" r="0.5" stroke="none" opacity="0.66" />
          <path d="M16 67v3m-1.5-1.5h3" fill="none" strokeWidth="0.4" opacity="0.48" />
          <circle cx="87" cy="85" r="0.35" stroke="none" opacity="0.52" />
        </motion.g>
      </motion.svg>

      <div className="relative min-h-28 w-full grow overflow-hidden text-sidebar-foreground">
        <motion.svg
          aria-hidden="true"
          viewBox="0 0 280 220"
          className="absolute inset-0 size-full"
          initial={false}
        >

          <AnimatePresence initial={false}>
            {boostCount > 0 && !prefersReducedMotion && (
              <motion.g
                key={boostCount}
                fill="none"
                stroke="currentColor"
                strokeLinecap="round"
                initial={{ opacity: 0, x: 12 }}
                animate={{ opacity: [0, 0.7, 0], x: [12, -8, -28] }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.7 }}
              >
                <path d="M70 87H28" strokeWidth="1.5" />
                <path d="M61 105H17" strokeWidth="1" opacity="0.65" />
                <path d="M72 130H36" strokeWidth="1.5" opacity="0.8" />
                <path d="M83 149H48" strokeWidth="1" opacity="0.5" />
              </motion.g>
            )}
          </AnimatePresence>

          <motion.g
            animate={prefersReducedMotion ? undefined : { x: [-3, 3, -3], y: [1, -1, 1], rotate: [-0.7, 0.7, -0.7] }}
            transition={{ duration: 6.5, ease: 'easeInOut', repeat: Infinity }}
            style={{ transformOrigin: '155px 112px' }}
          >
            <motion.g
              animate={satelliteControls}
              style={{ transformOrigin: '155px 112px' }}
              fill="none"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              {/* All four antennae disappear behind the shell at their mounting points. */}
              <g opacity="0.92">
                <path d="M168 95 35 35" strokeWidth="2" />
                <path d="M168 103 22 76" strokeWidth="2.2" />
                <path d="M168 117 26 160" strokeWidth="2.2" />
                <path d="M169 124 53 198" strokeWidth="2" />

                {/* Offset pencil passes keep the otherwise straight silhouette hand-drawn. */}
                <path d="M167 96 36 37" strokeWidth="0.8" opacity="0.42" />
                <path d="M166 104 24 78" strokeWidth="0.8" opacity="0.36" />
                <path d="M167 118 28 162" strokeWidth="0.8" opacity="0.4" />
                <path d="M170 125 55 199" strokeWidth="0.8" opacity="0.38" />
              </g>

              {/* A plain spherical shell is the defining Sputnik silhouette. */}
              <circle cx="187" cy="110" r="31" fill="var(--sidebar)" strokeWidth="2.2" />

              {/* Sparse pencil marks suggest the reflection on the polished shell. */}
              <path d="M175 87c8-5 17-5 25-2m-28 7 11-4" strokeWidth="1.5" opacity="0.62" />
            </motion.g>
          </motion.g>
        </motion.svg>
      </div>

      <div className="relative mt-1 min-h-15 shrink-0 px-5">
        <p className="text-lg font-semibold tracking-tight text-sidebar-foreground">
          Dead space
        </p>
        <AnimatePresence mode="wait" initial={false}>
          <motion.p
            key={transmissionIndex}
            aria-live="polite"
            className="mt-1 max-w-56 text-xs leading-relaxed text-muted-foreground"
            initial={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: -4 }}
            transition={{ duration: 0.18 }}
          >
            {TRANSMISSIONS[transmissionIndex]}
          </motion.p>
        </AnimatePresence>
      </div>
    </button>
  )
}
