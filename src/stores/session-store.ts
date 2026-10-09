import { create } from 'zustand'
import type { SessionPayload } from '@/lib/auth'

interface SessionState {
  generation: number
  session: SessionPayload | null
  setSession: (session: SessionPayload) => void
  clearSession: () => void
}

export const useSessionStore = create<SessionState>(set => ({
  generation: 0,
  session: null,
  setSession: session => set(state => ({ session, generation: state.generation + (state.session?.tgUserId === session.tgUserId ? 0 : 1) })),
  clearSession: () => set(state => ({ session: null, generation: state.generation + 1 })),
}))

export function formatDisplayName(session: SessionPayload): string {
  return [session.firstName, session.lastName].filter(Boolean).join(' ')
}

export function formatInitials(session: SessionPayload): string {
  const first = session.firstName.at(0) ?? ''
  const last = session.lastName?.at(0) ?? ''
  return (first + last).toUpperCase() || '?'
}

/** Invalidate responses and queued operations when the account lifetime changes. */
export function captureSession(): number {
  return useSessionStore.getState().generation
}

export function isSessionCurrent(generation: number): boolean {
  return captureSession() === generation
}

export function assertSession(generation: number): void {
  if (!isSessionCurrent(generation)) throw new Error('The account changed. Please try again.')
}

/** Serial work belongs to one account; failures never poison subsequent work. */
export function createSessionQueue() {
  let generation = -1
  let tail: Promise<void> = Promise.resolve()
  return <T>(operation: (generation: number) => Promise<T>): Promise<T> => {
    const captured = captureSession()
    if (captured !== generation) {
      generation = captured
      tail = Promise.resolve()
    }
    const next = tail.then(() => {
      assertSession(captured)
      return operation(captured)
    })
    tail = next.then(() => {}, () => {})
    return next
  }
}
