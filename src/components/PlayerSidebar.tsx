import { SidebarProfile } from '@/components/SidebarProfile'
import { SidebarPlaylists } from '@/components/SidebarPlaylists'
import { Separator } from '@/components/ui/separator'

export interface PlayerSidebarProps {
  onLogout: () => void
}

export function PlayerSidebar(props: PlayerSidebarProps) {
  return (
    <div className="flex h-full flex-col gap-4 pt-4">
      <div className="flex w-full items-center gap-2 px-4 h-10">
        <SidebarProfile onLogout={props.onLogout} />
      </div>

      <Separator />

      <SidebarPlaylists />
    </div>
  )
}
