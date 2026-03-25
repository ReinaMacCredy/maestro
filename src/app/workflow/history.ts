/**
 * history use case.
 * Shows feature completion history with stats.
 */

import type { FeaturePort } from '../../features/port.ts';
import type { TaskPort } from '../../tasks/port.ts';
import type { FeatureStatusType } from '../../core/types.ts';

export interface HistoryServices {
  featureAdapter: FeaturePort;
  taskPort: TaskPort;
}

export interface FeatureRecord {
  name: string;
  status: FeatureStatusType;
  createdAt: string;
  completedAt?: string;
  durationDays?: number;
  taskStats: { total: number; done: number; blocked: number };
}

export interface HistoryResult {
  features: FeatureRecord[];
  total: number;
}

export interface HistoryOpts {
  limit?: number;
  status?: FeatureStatusType;
}

export async function history(
  services: HistoryServices,
  opts: HistoryOpts = {},
): Promise<HistoryResult> {
  const { limit = 10, status } = opts;

  const names = services.featureAdapter.list();
  const records: FeatureRecord[] = [];

  // Collect feature metadata (sync), filter early
  const infos = names
    .map((name) => services.featureAdapter.get(name))
    .filter((info): info is NonNullable<typeof info> => !!info)
    .filter((info) => !status || info.status === status);

  // Fetch task stats in parallel
  const taskStatsResults = await Promise.all(
    infos.map(async (info) => {
      try {
        const tasks = await services.taskPort.list(info.name);
        return {
          total: tasks.length,
          done: tasks.filter((t) => t.status === 'done').length,
          blocked: tasks.filter((t) => t.status === 'blocked').length,
        };
      } catch {
        return { total: 0, done: 0, blocked: 0 };
      }
    }),
  );

  for (let i = 0; i < infos.length; i++) {
    const info = infos[i];
    let durationDays: number | undefined;
    if (info.completedAt && info.createdAt) {
      const ms = new Date(info.completedAt).getTime() - new Date(info.createdAt).getTime();
      durationDays = Math.round((ms / (1000 * 60 * 60 * 24)) * 10) / 10;
    }

    records.push({
      name: info.name,
      status: info.status,
      createdAt: info.createdAt,
      completedAt: info.completedAt,
      durationDays,
      taskStats: taskStatsResults[i],
    });
  }

  // Sort by createdAt descending (most recent first)
  records.sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());

  const limited = records.slice(0, limit);

  return { features: limited, total: records.length };
}
