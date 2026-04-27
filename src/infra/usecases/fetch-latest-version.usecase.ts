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
  readonly timeoutMs?: number;
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
  const timeoutSignal = AbortSignal.timeout(options.timeoutMs ?? FETCH_TIMEOUT_MS);
  const signal = options.signal
    ? AbortSignal.any([options.signal, timeoutSignal])
    : timeoutSignal;
  const response = await fetchImpl(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "maestro-cli",
    },
    signal,
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
