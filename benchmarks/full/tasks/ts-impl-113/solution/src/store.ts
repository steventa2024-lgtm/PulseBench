export type Reducer<S, A> = (state: S, action: A) => S;
export type Listener<S> = (state: S, previous: S) => void;
export type Unsubscribe = () => void;

export interface Store<S, A> {
  getState(): S;
  dispatch(action: A): void;
  subscribe(listener: Listener<S>): Unsubscribe;
  observe<T>(selector: (state: S) => T, onChange: (value: T, previous: T) => void): Unsubscribe;
}

export function createStore<S, A>(reducer: Reducer<S, A>, initialState: S): Store<S, A> {
  let state = initialState;
  let listeners: Listener<S>[] = [];
  let reducing = false;

  function subscribe(listener: Listener<S>): Unsubscribe {
    const entry: Listener<S> = (s, p) => listener(s, p);
    listeners = [...listeners, entry];
    return () => {
      listeners = listeners.filter((l) => l !== entry);
    };
  }

  return {
    getState: () => state,
    dispatch(action: A): void {
      if (reducing) throw new Error("reducers may not dispatch");
      const previous = state;
      try {
        reducing = true;
        state = reducer(state, action);
      } finally {
        reducing = false;
      }
      if (Object.is(previous, state)) return;
      const snapshot = listeners;
      for (const l of snapshot) l(state, previous);
    },
    subscribe,
    observe<T>(selector: (s: S) => T, onChange: (value: T, previous: T) => void): Unsubscribe {
      return subscribe((s, p) => {
        const next = selector(s);
        const prev = selector(p);
        if (!Object.is(next, prev)) onChange(next, prev);
      });
    },
  };
}
