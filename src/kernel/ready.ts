export interface ReadinessItem {
  id: string;
  state: string;
  blockers: Array<{ id: string; state: string }>;
}

export class Ready {
  project<T extends ReadinessItem>(items: T[]): T[] {
    return items.filter(
      (item) => item.state === "open" && item.blockers.every((blocker) => blocker.state === "done"),
    );
  }
}
