import { remark } from "remark";
import remarkHtml from "remark-html";
import { Row, Col } from "react-bootstrap";

export interface GitHubRelease {
  id: number;
  tag_name: string;
  name: string | null;
  body: string | null;
  published_at: string;
  html_url: string;
}

interface Props {
  releases: GitHubRelease[];
}

const MONTHS = [
  "JAN",
  "FEB",
  "MAR",
  "APR",
  "MAY",
  "JUN",
  "JUL",
  "AUG",
  "SEP",
  "OCT",
  "NOV",
  "DEC",
];

function formatDate(iso: string): string {
  const d = new Date(iso);
  return `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}, ${d.getUTCFullYear()}`;
}

async function renderBody(raw: string | null): Promise<string> {
  if (!raw?.trim()) return "";
  const result = await remark()
    .use(remarkHtml, { sanitize: false })
    .process(raw);
  return String(result);
}

export default async function CliSection({ releases }: Props) {
  const renderedBodies = await Promise.all(
    releases.map((r) => renderBody(r.body)),
  );

  const gifUrl = process.env.NEXT_PUBLIC_CLI_GIF_URL;
  const dlLinux = process.env.NEXT_PUBLIC_DOWNLOAD_LINUX_URL;
  const dlWindows = process.env.NEXT_PUBLIC_DOWNLOAD_WINDOWS_URL;
  const repoUrl = process.env.NEXT_PUBLIC_GITHUB_REPO_URL?.trim();

  return (
    <section className="cli-section">
      <div className="cli-section-inner">
        {gifUrl && (
          <Row className="gx-0 mb-3">
            <Col xs={12}>
              <div className="cli-gif-frame">
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img src={gifUrl} alt="Chiptunomatic CLI demo" />
              </div>
            </Col>
          </Row>
        )}

        <Row className="gx-0 mb-2">
          <Col xs={12}>
            <p className="cli-section-heading">CHIPTUNOMATIC CLI</p>
          </Col>
        </Row>

        <Row className="gx-0 mb-3">
          <Col xs={12}>
            <p className="cli-intro">
              Lightweight Rust music player that turns any file into a chiptune
              right in your terminal. Feed it a binary, an executable, a
              document — anything. The file name and bytes are used as a
              deterministic seed: same input always produces the same tune.
              <br />
              Standalone, portable, and no configuration required.
            </p>
            <Row as="ul" xs={1} md={3} className="g-2 mt-2 cli-feature-list">
              <Col as="li">
                <div className="cli-feature-card">
                  Lightweight
                  <br />
                  Built in Rust for a smaller footprint
                </div>
              </Col>
              <Col as="li">
                <div className="cli-feature-card">
                  Portable
                  <br />
                  Single binary with no dependencies
                </div>
              </Col>
              <Col as="li">
                <div className="cli-feature-card">
                  Open Source
                  <br />
                  Review the{" "}
                  <a href={repoUrl} target="_blank" rel="noopener noreferrer">
                    code
                  </a>{" "}
                  and compile yourself
                </div>
              </Col>
              <Col as="li">
                <div className="cli-feature-card">
                  Procedural
                  <br />
                  Generate chiptune musics from your files
                </div>
              </Col>
              <Col as="li">
                <div className="cli-feature-card">
                  Visualize
                  <br />
                  Four stems oscilloscope visualizer
                </div>
              </Col>
              <Col as="li">
                <div className="cli-feature-card">
                  Export
                  <br />
                  Save your preferred chiptune musics as wav
                </div>
              </Col>
            </Row>
          </Col>
        </Row>

        <Row className="gx-0 mb-3">
          <Col xs={12}>
            <p className="cli-features-label">FEATURES COMPARISON</p>
            <table className="compare-table">
              <thead>
                <tr>
                  <th className="compare-feature-col"></th>
                  <th>WEB DEMO</th>
                  <th>CLI</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>File conversion</td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>Music modes (chiptune, rock, metal, ...)</td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>WAV export</td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>No install needed</td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>Max file size</td>
                  <td>
                    <span className="compare-limit">~5 KB</span>
                  </td>
                  <td>
                    <span className="compare-check">Unlimited</span>
                  </td>
                </tr>
                <tr>
                  <td>Stems visualization</td>
                  <td>
                    <span className="compare-cross">✗</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>Stems volume control</td>
                  <td>
                    <span className="compare-cross">✗</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>Stems mute / solo</td>
                  <td>
                    <span className="compare-cross">✗</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
                <tr>
                  <td>Arpeggiator</td>
                  <td>
                    <span className="compare-cross">✗</span>
                  </td>
                  <td>
                    <span className="compare-check">✓</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </Col>
        </Row>

        <Row className="gx-0 mb-3">
          <Col xs={12}>
            <p className="cli-features-label">DOWNLOAD</p>
            {(dlLinux || dlWindows) && (
              <Row className="g-2">
                {dlLinux && (
                  <Col>
                    <a className="btn-dl" href={dlLinux}>
                      ↓ DOWNLOAD LINUX
                    </a>
                  </Col>
                )}
                {dlWindows && (
                  <Col>
                    <a className="btn-dl" href={dlWindows}>
                      ↓ DOWNLOAD WINDOWS
                    </a>
                  </Col>
                )}
              </Row>
            )}
          </Col>
        </Row>

        <Row className="gx-0 mb-3">
          <Col xs={12}>
            <p className="cli-features-label">USAGE</p>
            <div className="cli-shell">
              <div className="cli-shell-bar d-flex align-items-center">
                <span className="title-dots d-flex">
                  <span className="title-dot" />
                  <span className="title-dot" />
                  <span className="title-dot" />
                </span>
                <span>bash</span>
              </div>
              <pre className="cli-shell-body">
                <span className="cli-shell-prompt">$</span>
                {" chiptunomatic --help\n"}
                {process.env.NEXT_PUBLIC_CLI_HELP && (
                  <span className="cli-shell-out">
                    {process.env.NEXT_PUBLIC_CLI_HELP}
                  </span>
                )}
              </pre>
            </div>
          </Col>
        </Row>

        {releases.length > 0 && (
          <Row className="gx-0">
            <Col xs={12}>
              <p className="cli-features-label">WHAT&apos;S NEW</p>
              <ul className="whats-new-list">
                {releases.map((r, i) => (
                  <li key={r.id} className="whats-new-item">
                    <div className="whats-new-header">
                      <span className="whats-new-tag">{r.tag_name}</span>
                      <span className="whats-new-date">
                        {formatDate(r.published_at)}
                      </span>
                    </div>
                    {renderedBodies[i] && (
                      <div
                        className="whats-new-body"
                        dangerouslySetInnerHTML={{ __html: renderedBodies[i] }}
                      />
                    )}
                    <a
                      href={r.html_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="whats-new-link"
                    >
                      VIEW RELEASE ↗
                    </a>
                  </li>
                ))}
              </ul>
            </Col>
          </Row>
        )}
      </div>
    </section>
  );
}
