export type Reducer<S, A> = (state: S, action: A) => S;
export type Listener<S> = (state: S, previous: S) => void;
export type Unsubscribe = () => void;

export interface Store<S, A> {
  getState(): S;
  dispatch(action: A): void;
  /** Listener is called after a dispatch only if the reducer returned a different state object. */
  subscribe(listener: Listener<S>): Unsubscribe;
  /**
   * Call `onChange(selected, previousSelected)` whenever the selected slice changes
   * (compared with Object.is) after a dispatch. Returns an unsubscribe function.
   */
  observe<T>(selector: (state: S) => T, onChange: (value: T, previous: T) => void): Unsubscribe;
}

/**
 * Create a store.
 *
 *  - `dispatch` runs the reducer synchronously and then notifies listeners in subscription order.
 *  - Listeners are not called when the reducer returns the same state object (Object.is).
 *  - A listener may unsubscribe itself or others while being notified; the current notification
 *    round still uses the listeners that were subscribed when it started.
 *  - Calling `dispatch` from inside a reducer must throw an Error("reducers may not dispatch").
 *  - Unsubscribing twice is harmless.
 */
export function createStore<S, A>(reducer: Reducer<S, A>, initialState: S): Store<S, A> {
  throw new Error("not implemented");
}
