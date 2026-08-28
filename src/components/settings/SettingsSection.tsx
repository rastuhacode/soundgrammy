import { useState, type ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'

interface SettingsSectionProps {
  title: ReactNode
  children: ReactNode | ((open: boolean) => ReactNode)
  contentClassName?: string
}

export function SettingsSection({
  title,
  children,
  contentClassName = '',
}: SettingsSectionProps) {
  const [open, setOpen] = useState(false)

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
        {title}
        <ChevronDown
          className={`size-4 text-muted-foreground transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </CollapsibleTrigger>
      <CollapsibleContent
        className={`mt-1 mr-2 ml-4 min-w-0 rounded-md bg-muted/20 px-3 py-3 ${contentClassName}`}
      >
        {typeof children === 'function' ? children(open) : children}
      </CollapsibleContent>
    </Collapsible>
  )
}
