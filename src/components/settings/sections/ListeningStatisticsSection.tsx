import { SettingsSection } from '../SettingsSection'
import { ListeningStatisticsForm } from '../forms/ListeningStatisticsForm'

export function ListeningStatisticsSection() {
  return (
    <SettingsSection title="Listening statistics">
      <ListeningStatisticsForm />
    </SettingsSection>
  )
}
