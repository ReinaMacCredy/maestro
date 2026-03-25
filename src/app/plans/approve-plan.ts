import type { PlanPort } from '../../plans/port.ts';
import type { FeaturePort } from '../../features/port.ts';
import { MaestroError } from '../../core/errors.ts';

export interface ApprovePlanServices {
  planAdapter: PlanPort;
  featureAdapter: FeaturePort;
}

export interface ApprovePlanResult {
  feature: string;
  commentCount: number;
}

export async function approvePlan(
  services: ApprovePlanServices,
  featureName: string,
): Promise<ApprovePlanResult> {
  const { planAdapter, featureAdapter } = services;
  featureAdapter.requireActive(featureName);

  const plan = planAdapter.read(featureName);
  if (!plan) throw new MaestroError(`No plan found for feature '${featureName}'`);

  const comments = plan.comments || [];
  if (comments.length > 0) {
    throw new MaestroError(
      `Plan has ${comments.length} unresolved comment(s)`,
      ['Clear comments first: maestro plan-comments-clear --feature ' + featureName]
    );
  }

  planAdapter.approve(featureName);
  featureAdapter.updateStatus(featureName, 'approved');
  return { feature: featureName, commentCount: 0 };
}
