import { getProjectName, getVersion } from './api';

describe('API', () => {
  it('should return project name', () => {
    expect(getProjectName()).toBe('orchestrator-stress-test');
  });

  it('should return version', () => {
    expect(getVersion()).toBe('1.0.0');
  });
});
