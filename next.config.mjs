import { readFileSync, existsSync } from 'fs';
import { execSync } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const cargoToml = readFileSync(
  resolve(__dirname, 'crates/chiptunomatic/Cargo.toml'),
  'utf8',
);
const crateVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? '0.0.0';

const cliHelpCandidates = [
  process.env.CHIPTUNOMATIC_BIN,
  resolve(__dirname, 'target/release/chiptunomatic'),
].filter(Boolean);

let cliHelpOutput = '';
for (const bin of cliHelpCandidates) {
  if (existsSync(bin)) {
    try {
      cliHelpOutput = execSync(`"${bin}" --help`, { encoding: 'utf8', timeout: 5000 }).trim();
    } catch {
      // binary crashed or timed out
    }
    break;
  }
}

const isGithubPages = process.env.GITHUB_PAGES === 'true';
const repoName = process.env.GITHUB_REPOSITORY?.split('/')[1] ?? '';
const isProjectPages = isGithubPages && repoName && !repoName.endsWith('.github.io');
const basePath = isProjectPages ? `/${repoName}` : '';

/** @type {import('next').NextConfig} */
const nextConfig = {
  output: isGithubPages ? 'export' : undefined,
  images: {
    unoptimized: true,
  },
  basePath,
  assetPrefix: isProjectPages ? `/${repoName}/` : undefined,
  env: {
    NEXT_PUBLIC_BASE_PATH: basePath,
    NEXT_PUBLIC_CRATE_VERSION: crateVersion,
    NEXT_PUBLIC_CLI_HELP: cliHelpOutput,
    NEXT_PUBLIC_CLI_GIF_URL: process.env.NEXT_PUBLIC_CLI_GIF_URL ?? '',
    NEXT_PUBLIC_DOWNLOAD_LINUX_URL: process.env.NEXT_PUBLIC_DOWNLOAD_LINUX_URL ?? '',
    NEXT_PUBLIC_DOWNLOAD_MAC_URL: process.env.NEXT_PUBLIC_DOWNLOAD_MAC_URL ?? '',
    NEXT_PUBLIC_DOWNLOAD_WINDOWS_URL: process.env.NEXT_PUBLIC_DOWNLOAD_WINDOWS_URL ?? '',
  },
};
export default nextConfig;
