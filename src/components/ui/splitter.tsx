import { useId } from 'react'
import { GripVertical } from 'lucide-react'
import {
  Group,
  Panel,
  Separator,
  useDefaultLayout,
  type GroupProps,
  type Layout,
  type PanelProps,
  type SeparatorProps,
} from 'react-resizable-panels'

import { cn } from '@/lib/utils'

const inertStorage = {
  getItem: () => null,
  setItem: () => undefined,
}

export interface SplitterGroupProps extends GroupProps {
  autoSaveId?: string
  panelIds?: string[]
}

function SplitterGroup({
  autoSaveId,
  panelIds,
  className,
  defaultLayout,
  onLayoutChanged,
  ...props
}: SplitterGroupProps) {
  const fallbackId = useId()
  const savedLayout = useDefaultLayout({
    id: autoSaveId ?? fallbackId,
    panelIds,
    storage: autoSaveId && typeof window !== 'undefined'
      ? window.localStorage
      : inertStorage,
    onlySaveAfterUserInteractions: true,
  })

  function handleLayoutChanged(
    layout: Layout,
    meta: Parameters<NonNullable<GroupProps['onLayoutChanged']>>[1],
  ) {
    if (autoSaveId) savedLayout.onLayoutChanged(layout, meta)
    onLayoutChanged?.(layout, meta)
  }

  return (
    <Group
      data-slot="splitter-group"
      className={cn('size-full', className)}
      defaultLayout={autoSaveId
        ? savedLayout.defaultLayout ?? defaultLayout
        : defaultLayout}
      onLayoutChanged={handleLayoutChanged}
      {...props}
    />
  )
}

function SplitterPanel({ className, ...props }: PanelProps) {
  return (
    <Panel
      data-slot="splitter-panel"
      className={cn('min-h-0 min-w-0', className)}
      {...props}
    />
  )
}

export interface SplitterResizeHandleProps extends SeparatorProps {
  withHandle?: boolean
}

function SplitterResizeHandle({
  withHandle = false,
  className,
  ...props
}: SplitterResizeHandleProps) {
  return (
    <Separator
      data-slot="splitter-resize-handle"
      className={cn(
        'group relative z-10 flex w-px items-center justify-center bg-border outline-none transition-colors',
        'after:absolute after:inset-y-0 after:left-1/2 after:w-2 after:-translate-x-1/2',
        'hover:bg-primary/50 focus-visible:bg-primary data-[separator=active]:bg-primary',
        'aria-[orientation=horizontal]:h-px aria-[orientation=horizontal]:w-full',
        'aria-[orientation=horizontal]:after:inset-x-0 aria-[orientation=horizontal]:after:top-1/2 aria-[orientation=horizontal]:after:h-2 aria-[orientation=horizontal]:after:w-full aria-[orientation=horizontal]:after:-translate-y-1/2 aria-[orientation=horizontal]:after:translate-x-0',
        '[&[aria-orientation=horizontal]>div]:rotate-90',
        className,
      )}
      {...props}
    >
      {withHandle && (
        <div className="z-10 flex h-5 w-3 items-center justify-center rounded-sm border border-border bg-background shadow-sm">
          <GripVertical className="size-2.5" />
        </div>
      )}
    </Separator>
  )
}

export {
  SplitterGroup,
  SplitterPanel,
  SplitterResizeHandle,
}
