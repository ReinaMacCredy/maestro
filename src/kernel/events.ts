export type Disposer = () => void | Promise<void>;
export type EventListener<T = unknown, R = unknown> = (
  value: T,
  next: (value?: T) => Promise<R>,
) => R | Promise<R>;

export class Events {
  private readonly listeners = new Map<string, EventListener[]>();

  on<T = unknown, R = unknown>(event: string, listener: EventListener<T, R>): Disposer {
    const listeners = this.listeners.get(event) ?? [];
    listeners.push(listener as EventListener);
    this.listeners.set(event, listeners);
    return () => {
      const current = this.listeners.get(event);
      if (!current) return;
      const index = current.indexOf(listener as EventListener);
      if (index >= 0) current.splice(index, 1);
      if (current.length === 0) this.listeners.delete(event);
    };
  }

  async emit<T>(event: string, value: T): Promise<void> {
    for (const listener of [...(this.listeners.get(event) ?? [])]) {
      await listener(value, async () => undefined);
    }
  }

  async parallel<T>(event: string, value: T): Promise<unknown[]> {
    return Promise.all(
      [...(this.listeners.get(event) ?? [])].map((listener) =>
        listener(value, async () => undefined),
      ),
    );
  }

  async serial<T>(event: string, value: T): Promise<unknown[]> {
    const results: unknown[] = [];
    for (const listener of [...(this.listeners.get(event) ?? [])]) {
      results.push(await listener(value, async () => undefined));
    }
    return results;
  }

  async waterfall<T, R>(
    event: string,
    value: T,
    terminal: (value: T) => R | Promise<R>,
  ): Promise<R> {
    const listeners = [...(this.listeners.get(event) ?? [])];
    const dispatch = async (index: number, current: T): Promise<R> => {
      const listener = listeners[index] as EventListener<T, R> | undefined;
      if (!listener) return terminal(current);
      return listener(current, (nextValue = current) => dispatch(index + 1, nextValue));
    };
    return dispatch(0, value);
  }
}
