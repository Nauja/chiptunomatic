import HomeClient from "@/components/HomeClient";
import CliSection, { type GitHubRelease } from "@/components/CliSection";

async function fetchReleases(): Promise<GitHubRelease[]> {
  const repoUrl = process.env.NEXT_PUBLIC_GITHUB_REPO_URL?.trim() ?? "";
  const match = repoUrl.match(/github\.com\/([^/?#]+\/[^/?#]+)/);
  if (!match) return [];

  const repo = match[1];
  const token = process.env.GITHUB_TOKEN;
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  try {
    const res = await fetch(
      `https://api.github.com/repos/${repo}/releases`,
      { headers },
    );
    if (!res.ok) return [];
    const data = (await res.json()) as GitHubRelease[];
    return data.filter(
      (r): r is GitHubRelease =>
        typeof r === "object" && r !== null && !("draft" in r && r.draft),
    );
  } catch {
    return [];
  }
}

export default async function Home() {
  const releases = await fetchReleases();
  return (
    <>
      <HomeClient />
      <CliSection releases={releases} />
    </>
  );
}
