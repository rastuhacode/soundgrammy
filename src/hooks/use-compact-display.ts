import { useMediaQuery } from '@mantine/hooks'

const COMPACT_DISPLAY_QUERY = '(max-width: 47.999rem)'

export function useCompactDisplay() {
  const isCompact = useMediaQuery(COMPACT_DISPLAY_QUERY, undefined, {
    getInitialValueInEffect: false,
  })
  return { isCompact }
}
