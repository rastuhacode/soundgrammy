import { useMediaQuery } from '@mantine/hooks'

/** Use input capability rather than viewport width for touch interactions. */
export function useTouchScreen() {
  return useMediaQuery('(hover: none) and (pointer: coarse)', undefined, {
    getInitialValueInEffect: false,
  })
}
