import { onBackButtonPress } from '@tauri-apps/api/app'

type BackListener = { unregister: () => Promise<void> }
type BackAction = { id: symbol, close: () => void, priority: number }

/** One native listener dispatches Back to the most recently opened UI layer. */
export class AndroidBackStack {
  private actions: BackAction[] = []
  private listener: BackListener | null = null
  private pending = Promise.resolve()

  constructor(private readonly listen: (onBack: () => void) => Promise<BackListener>) {}

  add(close: () => void, priority = 0): () => void {
    const action = { id: Symbol('back-action'), close, priority }
    this.actions.push(action)
    this.reconcile()

    return () => {
      this.actions = this.actions.filter(item => item.id !== action.id)
      this.reconcile()
    }
  }

  private dispatch = () => {
    const top = this.actions.reduce<BackAction | null>((selected, action) =>
      !selected || action.priority >= selected.priority ? action : selected, null)
    top?.close()
  }

  private reconcile() {
    this.pending = this.pending.then(async () => {
      if (this.actions.length > 0 && !this.listener) {
        this.listener = await this.listen(this.dispatch)
      }
      if (this.actions.length === 0 && this.listener) {
        const listener = this.listener
        await listener.unregister()
        this.listener = null
      }
    }).catch((error: unknown) => {
      console.error('Could not update Android Back listener', error)
    })
  }

  /** Wait for pending native registration work (used by regression tests). */
  settled(): Promise<void> {
    return this.pending
  }
}

export const androidBackStack = new AndroidBackStack(handler => onBackButtonPress(handler))
