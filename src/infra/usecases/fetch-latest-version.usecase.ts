import { MaestroError } from "@/shared/errors.js";
import {
  getReleasesApiBaseUrl,
  normalizeReleaseTag,
  versionFromTag,
} from "./install-release-binary.usecase.js";

const FETCH_TIMEOUT_MS = 8000;

export interface FetchLatestVersionOptions {
  readonly fetchImpl?: typeof fetch;
  readonly signal?: AbortSignal;
}

export interface LatestRelease {
  readonly version: string;
  readonly tag: string;
}

export async function fetchLatestVersion(
  options: FetchLatestVersionOptions = {},
): Promise<LatestRelease> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const url = `${getReleasesApiBaseUrl()}/latest`;
  const response = await fetchImpl(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "maestro-cli",
    },
    signal: options.signal ?? AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });

  if (!response.ok) {
    throw new MaestroError(`Could not fetch release metadata (${response.status})`, [
      `GitHub API URL: ${url}`,
    ]);
  }

  const payload = await response.json() as { readonly tag_name?: string };
  const rawTag = payload.tag_name?.trim();
  if (!rawTag) {
    throw new MaestroError("GitHub release response was missing tag_name", [
      `GitHub API URL: ${url}`,
    ]);
  }

  const tag = normalizeReleaseTag(rawTag);
  return { version: versionFromTag(tag), tag };
}
