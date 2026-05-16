"use client";

import { useState, useRef, useCallback, useEffect } from "react";
import { Row, Col } from "react-bootstrap";
import {
  ChiptunomaticGenerationError,
  getMusicModes,
  runChiptunomaticGeneration,
  type ChiptuneSongInfo,
} from "@/lib/chiptune-wasm";

function formatTime(s: number): string {
  if (!isFinite(s) || s < 0) return "00:00";
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

export default function HomeClient() {
  const [file, setFile] = useState<File | null>(null);
  const [status, setStatus] = useState<
    "idle" | "generating" | "ready" | "error"
  >("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(0.25);
  const [loop, setLoop] = useState(false);
  const [mode, setMode] = useState('chiptune');
  const [modeNames, setModeNames] = useState<string[] | null>(null);
  const [songInfo, setSongInfo] = useState<ChiptuneSongInfo | null>(null);
  const [metaPending, setMetaPending] = useState(false);
  const [metaError, setMetaError] = useState("");

  const audioRef = useRef<HTMLAudioElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const progressTrackRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getMusicModes().then((names) => {
      if (names.length > 0) setModeNames(names);
    });
  }, []);

  useEffect(() => {
    if (audioRef.current) {
      audioRef.current.volume = volume;
      audioRef.current.loop = loop;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [audioUrl]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !audioUrl) return;

    const onTimeUpdate = () => {
      setCurrentTime(audio.currentTime);
      if (audio.duration)
        setProgress((audio.currentTime / audio.duration) * 100);
    };
    const onLoaded = () => setDuration(audio.duration);
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onEnded = () => {
      setPlaying(false);
      setProgress(0);
      if (audio) audio.currentTime = 0;
    };

    audio.addEventListener("timeupdate", onTimeUpdate);
    audio.addEventListener("loadedmetadata", onLoaded);
    audio.addEventListener("play", onPlay);
    audio.addEventListener("pause", onPause);
    audio.addEventListener("ended", onEnded);
    return () => {
      audio.removeEventListener("timeupdate", onTimeUpdate);
      audio.removeEventListener("loadedmetadata", onLoaded);
      audio.removeEventListener("play", onPlay);
      audio.removeEventListener("pause", onPause);
      audio.removeEventListener("ended", onEnded);
    };
  }, [audioUrl]);

  useEffect(() => {
    if (!file) {
      setSongInfo(null);
      setMetaError("");
      setMetaPending(false);
      return;
    }

    setAudioUrl((prev) => {
      if (prev) URL.revokeObjectURL(prev);
      return null;
    });
    setStatus("idle");
    setPlaying(false);
    setProgress(0);
    setCurrentTime(0);
    setDuration(0);

    let cancelled = false;
    const ac = new AbortController();

    setMetaPending(true);
    setSongInfo(null);
    setMetaError("");
    setErrorMsg("");

    void (async () => {
      try {
        const wav = await runChiptunomaticGeneration(
          file,
          (info) => {
            if (cancelled) return;
            setSongInfo(info);
            setMetaPending(false);
            setStatus("generating");
          },
          { signal: ac.signal, mode },
        );
        if (cancelled) return;
        const blob = new Blob([new Uint8Array(wav)], { type: "audio/wav" });
        setAudioUrl(URL.createObjectURL(blob));
        setStatus("ready");
      } catch (err: unknown) {
        if (cancelled) return;
        if (err instanceof DOMException && err.name === "AbortError") return;

        const msg =
          err instanceof Error
            ? err.message
            : "generation failed unknown error";

        if (
          err instanceof ChiptunomaticGenerationError &&
          err.phase === "wav"
        ) {
          setStatus("error");
          setErrorMsg(msg);
          return;
        }

        setSongInfo(null);
        setMetaPending(false);
        setMetaError(msg);
      }
    })();

    return () => {
      cancelled = true;
      ac.abort();
    };
  }, [file, mode]);

  const applyFile = useCallback(
    (f: File) => {
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      setAudioUrl(null);
      setFile(f);
      setSongInfo(null);
      setMetaError("");
      setMetaPending(true);
      setStatus("idle");
      setErrorMsg("");
      setPlaying(false);
      setProgress(0);
      setCurrentTime(0);
      setDuration(0);
    },
    [audioUrl],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const f = e.dataTransfer.files[0];
      if (f) applyFile(f);
    },
    [applyFile],
  );

  const togglePlay = () => {
    const audio = audioRef.current;
    if (!audio) return;
    if (playing) audio.pause();
    else audio.play();
  };

  const seekOnClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const audio = audioRef.current;
    const el = progressTrackRef.current;
    if (!audio || !el || !audio.duration) return;
    const rect = el.getBoundingClientRect();
    audio.currentTime = ((e.clientX - rect.left) / rect.width) * audio.duration;
  };

  const toggleLoop = () => {
    const next = !loop;
    setLoop(next);
    if (audioRef.current) audioRef.current.loop = next;
  };

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = Number(e.target.value);
    setVolume(v);
    if (audioRef.current) audioRef.current.volume = v;
  };

  return (
    <main>
      <Row className="gx-0 justify-content-center align-items-center">
        <Col xs={12}>
          <div className="terminal mx-auto">
            {/* Title bar */}
            <div className="title-bar d-flex align-items-center">
              <span className="title-dots d-flex">
                <span className="title-dot" />
                <span className="title-dot" />
                <span className="title-dot" />
              </span>
              <span>C:\CHIPTUNOMATIC.EXE</span>
            </div>

            <div className="content">
              {/* Logo */}
              <Row className="gx-0 mb-3">
                <Col xs={12}>
                  <header className="header text-center">
                    <h1 className="logo">CHIPTUNOMATIC</h1>
                    <p className="subtitle">► FILE → CHIPTUNE CONVERTER ◄</p>
                    <hr className="ruler" />
                  </header>
                </Col>
              </Row>

              {/* Drop zone */}
              <Row className="gx-0 mb-3">
                <Col xs={12}>
                  <div
                    className={`drop-zone d-flex align-items-center justify-content-center${dragOver ? " drag-over" : ""}`}
                    onDragEnter={(e) => {
                      e.preventDefault();
                      setDragOver(true);
                    }}
                    onDragLeave={() => setDragOver(false)}
                    onDragOver={(e) => e.preventDefault()}
                    onDrop={handleDrop}
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <input
                      ref={fileInputRef}
                      type="file"
                      style={{ display: "none" }}
                      onChange={(e) => {
                        const f = e.target.files?.[0];
                        if (f) applyFile(f);
                      }}
                    />
                    {file ? (
                      <div className="file-info text-center">
                        <span className="file-icon d-block">▪</span>
                        <span className="file-name d-block">{file.name}</span>
                        <span className="file-size d-block">
                          ({(file.size / 1024).toFixed(1)} KB)
                        </span>
                      </div>
                    ) : (
                      <div className="drop-prompt text-center">
                        <span className="drop-icon d-block">▒▒▒</span>
                        <span className="drop-label d-block">
                          DROP ANY FILE HERE
                        </span>
                        <span className="drop-sub d-block">
                          or click to browse
                        </span>
                        <span className="drop-warn d-block">
                          files bigger than 5 KB may fail to generate
                        </span>
                      </div>
                    )}
                  </div>
                </Col>
              </Row>

              {file && (metaPending || songInfo || metaError) && (
                <Row className="gx-0 mb-3">
                  <Col xs={12}>
                    <div className="song-meta-panel">
                      {metaPending && (
                        <p className="song-meta-status">
                          Scanning file header for tune parameters…
                        </p>
                      )}
                      {metaError && !metaPending && (
                        <p className="song-meta-error">
                          Could not derive tune info: {metaError}
                        </p>
                      )}
                      {songInfo && !metaPending && (
                        <dl className="song-meta-dl">
                          <div className="song-meta-row">
                            <dt>Root</dt>
                            <dd>{songInfo.rootNoteName}</dd>
                          </div>
                          <div className="song-meta-row">
                            <dt>Tempo</dt>
                            <dd>{songInfo.bpm} BPM</dd>
                          </div>
                          <div className="song-meta-row">
                            <dt>Chords</dt>
                            <dd>{songInfo.chordDescription}</dd>
                          </div>
                          <div className="song-meta-row">
                            <dt>Length</dt>
                            <dd>
                              {formatTime(songInfo.totalDurationSec)}
                              <span className="song-meta-sub">
                                {" "}
                                ({songInfo.totalBeats} beats)
                              </span>
                            </dd>
                          </div>
                        </dl>
                      )}
                    </div>
                  </Col>
                </Row>
              )}

              {/* Mode selector */}
              {file && modeNames && (
                <Row className="gx-0 mb-3">
                  <Col xs={12}>
                    <div className="mode-select-row d-flex align-items-center">
                      <span className="mode-select-label">MODE</span>
                      <select
                        className="mode-select"
                        value={mode}
                        onChange={(e) => setMode(e.target.value)}
                        aria-label="Music mode"
                      >
                        {modeNames.map((name) => (
                          <option key={name} value={name}>
                            {name.toUpperCase()}
                          </option>
                        ))}
                      </select>
                    </div>
                  </Col>
                </Row>
              )}

              {/* Loading bar */}
              {status === "generating" && (
                <Row className="gx-0 mb-3">
                  <Col xs={12}>
                    <div className="loading-section">
                      <Row className="gx-0 mb-1">
                        <Col xs={12}>
                          <div className="loading-bar">
                            <div className="loading-fill" />
                          </div>
                        </Col>
                      </Row>
                      <Row className="gx-0">
                        <Col xs={12}>
                          <span className="loading-text">
                            PROCESSING BINARY DATA
                            <span className="blink">_</span>
                          </span>
                        </Col>
                      </Row>
                    </div>
                  </Col>
                </Row>
              )}

              {/* Error */}
              {status === "error" && (
                <Row className="gx-0 mb-3">
                  <Col xs={12}>
                    <div className="error-box">✖ ERROR: {errorMsg}</div>
                  </Col>
                </Row>
              )}

              {/* Player */}
              {audioUrl && (
                <Row className="gx-0 mb-3">
                  <Col xs={12}>
                    <div className="player">
                      {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
                      <audio
                        ref={audioRef}
                        src={audioUrl}
                        preload="metadata"
                      />

                      <Row className="gx-0 mb-2">
                        <Col xs={12}>
                          <div className="player-header d-flex align-items-center">
                            <span className="note-icon">♪</span>
                            <span>CHIPTUNE READY</span>
                          </div>
                        </Col>
                      </Row>

                      <Row className="gx-0 mb-2">
                        <Col xs={12}>
                          <div className="player-controls d-flex align-items-center">
                            <button className="btn-play" onClick={togglePlay}>
                              {playing ? "[❙❙]" : "[▶]"}
                            </button>

                            <button
                              className={`btn-loop${loop ? " btn-loop--on" : ""}`}
                              onClick={toggleLoop}
                              aria-label="Toggle loop"
                              aria-pressed={loop}
                            >
                              ↻
                            </button>

                            <div
                              ref={progressTrackRef}
                              className="progress-track"
                              onClick={seekOnClick}
                            >
                              <div
                                className="progress-fill"
                                style={{ width: `${progress}%` }}
                              />
                            </div>

                            <span className="time-display">
                              {formatTime(currentTime)}&nbsp;/&nbsp;
                              {formatTime(duration)}
                            </span>

                            <div className="volume-control d-flex align-items-center flex-shrink-0">
                              <span className="volume-icon">
                                ◄{volume === 0 ? "x" : ")"}
                              </span>
                              <input
                                className="volume-slider"
                                type="range"
                                min={0}
                                max={1}
                                step={0.01}
                                value={volume}
                                onChange={handleVolumeChange}
                                aria-label="Volume"
                                style={
                                  {
                                    "--vol": `${volume * 100}%`,
                                  } as React.CSSProperties
                                }
                              />
                            </div>
                          </div>
                        </Col>
                      </Row>

                      <Row className="gx-0">
                        <Col xs={12}>
                          <a
                            className="btn-download"
                            href={audioUrl}
                            download={`chiptune-${file?.name ?? "output"}.wav`}
                          >
                            ↓ DOWNLOAD WAV
                          </a>
                        </Col>
                      </Row>
                    </div>
                  </Col>
                </Row>
              )}

              <Row className="gx-0">
                <Col xs={12}>
                  <p className="version">
                    v{process.env.NEXT_PUBLIC_CRATE_VERSION}
                  </p>
                </Col>
              </Row>

              <Row className="gx-0 mb-2">
                <Col xs={12}>
                  <p className="privacy-note">
                    No files are uploaded to any server. Your audio is
                    generated entirely in your browser on this device.
                  </p>
                </Col>
              </Row>
            </div>
          </div>
        </Col>
      </Row>
    </main>
  );
}
