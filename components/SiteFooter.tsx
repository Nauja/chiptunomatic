const repoUrl = process.env.NEXT_PUBLIC_GITHUB_REPO_URL?.trim();

export function SiteFooter() {
  return (
    <footer className="site-footer mt-auto">
      {repoUrl ? (
        <a
          className="site-footer-repo"
          href={repoUrl}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="GitHub repository"
        >
          <i className="bi bi-github" />
        </a>
      ) : null}
    </footer>
  );
}
