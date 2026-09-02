import { openUrl } from '@tauri-apps/plugin-opener'

interface TauriLinkProps {
  href: string
  children: React.ReactNode
  className?: string
}

/**
 * A link that opens the URL in the default browser following Tauri model
 * @param href - The URL to open
 * @param children - The content to display
 * @param className - The class name to apply to the link
 * @returns A link that opens the URL in the default browser
 */
export function TauriLink({ href, children, className }: TauriLinkProps) {
  const handleClick = (event: React.MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault()
    openUrl(href)
  }

  return (
    <a target="_blank" rel="noopener noreferrer" href={href} className={className} onClick={handleClick}>
      {children}
    </a>
  )
}

// Reminder: you need to add the href to the `capabilities/default.json` file
