import { useEffect, useRef, useState } from 'react'
import { isTauri } from '@tauri-apps/api/core'
import { androidBackStack } from '@/lib/android-back'

/** Back is handled by the app only while a dismissible layer is open. */
export function useAndroidBackAction(active: boolean, close: () => void, priority = 0) {
  const closeRef = useRef(close)

  useEffect(() => {
    closeRef.current = close
  })

  useEffect(() => {
    if (!active || !isTauri() || !navigator.userAgent.includes('Android')) return
    return androidBackStack.add(() => closeRef.current(), priority)
  }, [active, priority])
}

/** Track controlled and uncontrolled Base UI roots through the same Back stack. */
export function useAndroidBackOverlay<Actions extends { close: () => void }, Details>({
  open,
  defaultOpen,
  onOpenChange,
  actionsRef,
}: {
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean, details: Details) => void
  actionsRef?: React.RefObject<Actions | null>
}) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen ?? false)
  const internalActionsRef = useRef<Actions | null>(null)
  const effectiveActionsRef = actionsRef ?? internalActionsRef

  useAndroidBackAction(open ?? uncontrolledOpen, () => effectiveActionsRef.current?.close(), 1)

  return {
    actionsRef: effectiveActionsRef,
    onOpenChange: (nextOpen: boolean, details: Details) => {
      setUncontrolledOpen(nextOpen)
      onOpenChange?.(nextOpen, details)
    },
  }
}
