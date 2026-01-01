import config from './config.json';

export function getProjectName(): string {
  return config.name;
}

export function getVersion(): string {
  return config.version;
}
