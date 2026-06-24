'use client';

import { useState, useEffect, useRef, useMemo } from 'react';
import ThumbnailEditor from './ThumbnailEditor';
import { Production, EditJobStatus, ProductionEdit, YoutubeCopy, Video } from '@/lib/types';
import {
  startProductionEdit,
  getEditStatus,
  getProductionEdits,
  getProductionVideos,
  getStreamUrl,
  getThumbnailUrl,
  getEditVideoUrl,
  revealEditOutput,
  revealEditFile,
  deleteEdit,
  rerenderEdit,
  generateEditCopy,
  browseFolder,
  browseFile,
} from '@/lib/api';

/** Recover the chosen output root from a previous run's output path. Outputs are
 * written to `<root>/productions/v<N>/<file>.mp4`, so the root is everything
 * before the `productions` segment. Returns '' if it can't be determined (e.g.
 * an old-format path), so we don't re-nest. */
function rootFromOutputPath(p?: string | null): string {
  if (!p) return '';
  const parts = p.replace(/\\/g, '/').split('/').filter(Boolean);
  const idx = parts.map((s) => s.toLowerCase()).lastIndexOf('productions');
  if (idx > 0) return parts.slice(0, idx).join('/');
  return '';
}
import { format } from 'date-fns';
import EditTimeline, { TimelineEdits } from './EditTimeline';

interface VideoEditPipelineProps {
  isOpen: boolean;
  production: Production | null;
  onClose: () => void;
}

const STAGE_LABELS: Record<string, string> = {
  starting: 'Starting',
  transcribing: 'Transcribing takes',
  planning: 'Planning the edit',
  stitching: 'Stitching the final video',
  mixing: 'Adding background music',
  completed: 'Completed',
  failed: 'Failed',
};

function fmtDate(iso: string): string {
  try {
    return format(new Date(iso), 'MMM d, yyyy HH:mm');
  } catch {
    return iso;
  }
}

/** Format seconds as `m:ss` for the take list. */
function fmtClock(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, '0')}`;
}

export default function VideoEditPipeline({
  isOpen,
  production,
  onClose,
}: VideoEditPipelineProps) {
  // History
  const [history, setHistory] = useState<ProductionEdit[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [view, setView] = useState<'new' | 'detail'>('new');

  // New-run form
  const [script, setScript] = useState('');
  const [instructions, setInstructions] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [outputName, setOutputName] = useState('');
  const [captions, setCaptions] = useState(true);
  const [tighten, setTighten] = useState(false);
  const [tightenGap, setTightenGap] = useState(1.5);
  const [musicPath, setMusicPath] = useState('');
  const [musicVolume, setMusicVolume] = useState(0.3);
  const [musicDuckVolume, setMusicDuckVolume] = useState(0.08);
  const [musicMinGap, setMusicMinGap] = useState(1.5);

  // Enhance voice (per-take noise removal) + intensity
  const [takes, setTakes] = useState<Video[]>([]);
  const [enhanceIds, setEnhanceIds] = useState<number[]>([]);
  const [enhanceIntensity, setEnhanceIntensity] = useState(0.6);
  // Which take (if any) is expanded for an inline video preview.
  const [previewId, setPreviewId] = useState<number | null>(null);

  // Run state
  const [jobId, setJobId] = useState<string | null>(null);
  const [status, setStatus] = useState<EditJobStatus | null>(null);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const running = status?.status === 'in_progress' || starting;

  // video_id → duration (seconds), used to clamp clip trims on the timeline.
  const takeDurations = useMemo(
    () => Object.fromEntries(takes.map((t) => [t.id, t.duration ?? 0])),
    [takes]
  );

  const loadHistory = async (pid: number, selectLatest: boolean) => {
    try {
      const list = await getProductionEdits(pid);
      setHistory(list);
      if (selectLatest && list.length > 0) {
        setSelectedId(list[0].id);
        setView('detail');
      } else if (list.length === 0) {
        setView('new');
      }
      // Seed the form from the most recent run so re-edits are quick.
      if (list.length > 0) {
        setScript((s) => s || list[0].script || '');
        setInstructions((i) => i || list[0].instructions || '');
        const root = rootFromOutputPath(list[0].output_path);
        if (root) setOutputDir((d) => d || root);
      }
    } catch {
      setHistory([]);
    }
  };

  const loadTakes = async (pid: number) => {
    try {
      setTakes(await getProductionVideos(pid));
    } catch {
      setTakes([]);
    }
  };

  useEffect(() => {
    if (isOpen && production) {
      document.body.style.overflow = 'hidden';
      setError(null);
      setJobId(null);
      setStatus(null);
      setScript('');
      setInstructions('');
      setOutputDir('');
      setOutputName('');
      setCaptions(true);
      setTighten(false);
      setTightenGap(1.5);
      setMusicPath('');
      setMusicVolume(0.3);
      setMusicDuckVolume(0.08);
      setMusicMinGap(1.5);
      setTakes([]);
      setEnhanceIds([]);
      setEnhanceIntensity(0.6);
      setPreviewId(null);
      setSelectedId(null);
      setView('new');
      loadHistory(production.id, true);
      loadTakes(production.id);
    } else {
      document.body.style.overflow = 'unset';
    }
    return () => {
      document.body.style.overflow = 'unset';
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, production]);

  // Poll job progress until terminal.
  useEffect(() => {
    if (!jobId || !production) return;
    let cancelled = false;

    const tick = async () => {
      try {
        const s = await getEditStatus(jobId);
        if (cancelled) return;
        setStatus(s);
        if (s.status === 'completed' || s.status === 'failed') {
          if (pollRef.current) clearInterval(pollRef.current);
          pollRef.current = null;
          await loadHistory(production.id, s.status === 'completed');
        }
      } catch {
        // transient — keep polling
      }
    };

    tick();
    pollRef.current = setInterval(tick, 1500);
    return () => {
      cancelled = true;
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobId, production]);

  if (!isOpen || !production) return null;

  const handleRun = async () => {
    if (!script.trim()) {
      setError('Paste the script for this video first.');
      return;
    }
    if (!outputDir.trim()) {
      setError('Choose an output folder for the final video.');
      return;
    }
    setStarting(true);
    setError(null);
    setStatus(null);
    try {
      const res = await startProductionEdit(production.id, {
        script,
        instructions: instructions.trim() || undefined,
        output_dir: outputDir.trim() || undefined,
        output_name: outputName.trim() || undefined,
        captions,
        tighten,
        tighten_gap: tightenGap,
        music_path: musicPath.trim() || undefined,
        music_volume: musicVolume,
        music_duck_volume: musicDuckVolume,
        music_min_gap: musicMinGap,
        enhance_voice: enhanceIds,
        enhance_voice_intensity: enhanceIntensity,
      });
      setJobId(res.job_id);
    } catch (e: any) {
      setError(e.message || 'Failed to start the pipeline');
    } finally {
      setStarting(false);
    }
  };

  const handleBrowseDir = async () => {
    try {
      const r = await browseFolder();
      if (r.success && r.path) setOutputDir(r.path);
    } catch {
      /* ignore */
    }
  };

  const handleBrowseMusic = async () => {
    try {
      const r = await browseFile();
      if (r.success && r.path) setMusicPath(r.path);
    } catch {
      /* ignore */
    }
  };

  const toggleEnhance = (id: number) =>
    setEnhanceIds((ids) => (ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id]));
  const allEnhanced = takes.length > 0 && enhanceIds.length === takes.length;
  const toggleAllEnhance = () => setEnhanceIds(allEnhanced ? [] : takes.map((t) => t.id));

  const handleRerender = async (editId: number, edits: TimelineEdits) => {
    if (running) return;
    setError(null);
    setStatus(null);
    try {
      const res = await rerenderEdit(editId, edits);
      setJobId(res.job_id);
    } catch (e: any) {
      setError(e.message || 'Failed to start re-render');
    }
  };

  const selected = history.find((h) => h.id === selectedId) || null;
  const pct =
    status && status.total > 0
      ? Math.min(100, Math.round((status.processed / status.total) * 100))
      : 0;

  return (
    <div className="fixed inset-0 z-[60] overflow-y-auto">
      <div className="fixed inset-0 bg-black bg-opacity-50 transition-opacity" onClick={onClose} />

      <div className="flex min-h-full items-center justify-center p-4">
        <div className="relative w-[96vw] max-w-[1700px] bg-white dark:bg-gray-800 rounded-lg shadow-xl overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b dark:border-gray-700">
            <div className="min-w-0">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white truncate">
                Edit &amp; Create Video
              </h2>
              <p className="text-sm text-gray-500 dark:text-gray-400 truncate">
                {production.title} · {production.video_count ?? 0} take
                {(production.video_count ?? 0) !== 1 ? 's' : ''}
              </p>
            </div>
            <button
              onClick={onClose}
              className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
            >
              <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 max-h-[88vh]">
            {/* History sidebar */}
            <aside className="md:col-span-1 border-b md:border-b-0 md:border-r dark:border-gray-700 overflow-y-auto max-h-[30vh] md:max-h-[88vh]">
              <div className="p-3">
                <button
                  onClick={() => {
                    setView('new');
                    setSelectedId(null);
                  }}
                  className={`btn w-full text-sm mb-3 ${view === 'new' ? 'btn-primary' : 'btn-secondary'}`}
                  disabled={running}
                >
                  ＋ New edit
                </button>
                <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
                  History
                </div>
                {history.length === 0 ? (
                  <p className="text-sm text-gray-500 dark:text-gray-400 italic">No runs yet.</p>
                ) : (
                  <ul className="space-y-1">
                    {history.map((h) => (
                      <li key={h.id}>
                        <button
                          onClick={() => {
                            setSelectedId(h.id);
                            setView('detail');
                          }}
                          className={`w-full text-left px-2 py-2 rounded text-sm ${
                            selectedId === h.id && view === 'detail'
                              ? 'bg-primary-50 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                              : 'hover:bg-gray-100 dark:hover:bg-gray-700/60 text-gray-700 dark:text-gray-300'
                          }`}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="truncate">{fmtDate(h.created_at)}</span>
                            <span
                              className={`text-[10px] px-1.5 py-0.5 rounded flex-shrink-0 ${
                                h.status === 'completed'
                                  ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-400'
                                  : 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-400'
                              }`}
                            >
                              {h.status}
                            </span>
                          </div>
                          <div className="text-xs text-gray-400 dark:text-gray-500 truncate">
                            {h.edl?.clips?.length
                              ? `${h.edl.clips.length} clip${h.edl.clips.length !== 1 ? 's' : ''}`
                              : h.error
                              ? 'error'
                              : '—'}
                          </div>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </aside>

            {/* Main panel */}
            <main className="md:col-span-2 overflow-y-auto max-h-[55vh] md:max-h-[88vh] p-4 space-y-4">
              {/* Live progress (shown whenever a job is running/finishing) */}
              {status && (
                <div className="card space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium text-gray-900 dark:text-white">
                      {STAGE_LABELS[status.stage] || status.stage}
                    </span>
                    <span
                      className={`text-xs px-2 py-0.5 rounded ${
                        status.status === 'completed'
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-400'
                          : status.status === 'failed'
                          ? 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-400'
                          : 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-400'
                      }`}
                    >
                      {status.status}
                    </span>
                  </div>
                  <p className="text-sm text-gray-600 dark:text-gray-400">{status.message}</p>
                  {status.status === 'in_progress' && status.total > 0 && (
                    <div>
                      <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 overflow-hidden">
                        <div className="bg-primary-600 h-2 transition-all" style={{ width: `${pct}%` }} />
                      </div>
                      <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                        {status.processed} / {status.total}
                      </p>
                    </div>
                  )}
                  {status.status === 'completed' && status.output_path && (
                    <div className="flex items-center gap-3 flex-wrap">
                      <button onClick={() => revealEditOutput(production.id)} className="btn btn-primary text-sm">
                        📂 Open the final video
                      </button>
                      <span className="text-xs text-gray-500 dark:text-gray-400 break-all">
                        {status.output_path}
                      </span>
                    </div>
                  )}
                  {status.logs && status.logs.length > 0 && (
                    <details open={status.status === 'in_progress'} className="text-xs">
                      <summary className="cursor-pointer text-gray-500 dark:text-gray-400">Activity log</summary>
                      <div className="mt-2 max-h-40 overflow-y-auto font-mono text-[11px] leading-relaxed text-gray-600 dark:text-gray-300 bg-gray-50 dark:bg-gray-900/40 rounded p-2">
                        {status.logs.map((line, i) => (
                          <div key={i}>{line}</div>
                        ))}
                      </div>
                    </details>
                  )}
                </div>
              )}

              {error && (
                <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                  <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
                </div>
              )}

              {view === 'new' ? (
                /* ---------- New run form ---------- */
                <div className="space-y-4">
                  <p className="text-sm text-gray-600 dark:text-gray-400">
                    Transcribes every take, assembles the best cut from your script, writes an edit
                    decision list, and stitches the final clip. Configure API keys/prompts under{' '}
                    <span className="font-medium">Settings → AI / LLM</span>.
                  </p>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Script <span className="text-red-500">*</span>
                    </label>
                    <textarea
                      value={script}
                      onChange={(e) => setScript(e.target.value)}
                      rows={7}
                      disabled={running}
                      className="input font-mono text-xs leading-relaxed"
                      placeholder="Paste the script (Markdown is fine). Scene breaks help the editor align takes."
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Extra instructions (optional)
                    </label>
                    <textarea
                      value={instructions}
                      onChange={(e) => setInstructions(e.target.value)}
                      rows={2}
                      disabled={running}
                      className="input text-sm"
                      placeholder="e.g. I warm up by saying “Hey Sarah” — cut that; and I re-shot scene 1 at the very end."
                    />
                  </div>

                  {/* Output location */}
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Output folder <span className="text-red-500">*</span>
                      </label>
                      <div className="flex gap-2">
                        <input
                          type="text"
                          value={outputDir}
                          onChange={(e) => setOutputDir(e.target.value)}
                          disabled={running}
                          className="input flex-1 text-sm"
                          placeholder="Where to save the video"
                        />
                        <button
                          type="button"
                          onClick={handleBrowseDir}
                          disabled={running}
                          className="btn btn-secondary text-sm whitespace-nowrap"
                        >
                          📁
                        </button>
                      </div>
                    </div>
                    <div>
                      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Filename
                      </label>
                      <input
                        type="text"
                        value={outputName}
                        onChange={(e) => setOutputName(e.target.value)}
                        disabled={running}
                        className="input text-sm"
                        placeholder={`${production.title}.mp4`}
                      />
                    </div>
                  </div>
                  <p className="text-xs text-gray-500 dark:text-gray-400 -mt-2">
                    Each run is written to <code>productions/v1</code>, <code>productions/v2</code>, …
                    inside this folder (the final video and its <code>.json</code> edit decision list),
                    so re-edits never overwrite each other. The folder you pick stays fixed — nothing
                    nests and nothing is stored in the app&apos;s data directory.
                  </p>

                  {/* Captions */}
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={captions}
                      onChange={(e) => setCaptions(e.target.checked)}
                      disabled={running}
                      className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                    />
                    <span className="text-sm text-gray-700 dark:text-gray-300">
                      Burn in captions from the spoken words
                    </span>
                  </label>

                  {/* Tighten — remove dead air & filler */}
                  <div>
                    <label className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={tighten}
                        onChange={(e) => setTighten(e.target.checked)}
                        disabled={running}
                        className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                      />
                      <span className="text-sm text-gray-700 dark:text-gray-300">
                        Tighten the cut — remove long pauses &amp; “um/uh” within clips
                      </span>
                    </label>
                    {tighten && (
                      <div className="mt-2 ml-6">
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                            Cut silent/filler gaps longer than
                          </span>
                          <input
                            type="number"
                            min={0.3}
                            max={10}
                            step={0.1}
                            value={tightenGap}
                            onChange={(e) =>
                              setTightenGap(Math.max(0.3, Math.min(10, parseFloat(e.target.value) || 0.3)))
                            }
                            disabled={running}
                            className="input w-16 text-sm py-1"
                          />
                          <span className="text-xs text-gray-500 dark:text-gray-400">s</span>
                        </div>
                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                          A clip with a long pause/filler is split into separate sub-clips that skip
                          it, so the final cut is tighter. Needs word-level timestamps
                          (ElevenLabs/Whisper).
                        </p>
                      </div>
                    )}
                  </div>

                  {/* Enhance voice — per-take noise removal */}
                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                        Enhance voice — remove background noise
                      </label>
                      {takes.length > 0 && (
                        <button
                          type="button"
                          onClick={toggleAllEnhance}
                          disabled={running}
                          className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                        >
                          {allEnhanced ? 'Clear all' : 'Select all'}
                        </button>
                      )}
                    </div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                      Cleans up the spoken audio on the takes you check — rolls off wind/handling
                      rumble, knocks down steady background hiss, and removes mouth clicks/pops.
                    </p>
                    {takes.length === 0 ? (
                      <p className="text-xs text-gray-400 dark:text-gray-500 italic">
                        No takes found for this production.
                      </p>
                    ) : (
                      <>
                        <div className="max-h-72 overflow-y-auto rounded border border-gray-200 dark:border-gray-700 divide-y divide-gray-100 dark:divide-gray-700/60">
                          {takes.map((t) => {
                            const open = previewId === t.id;
                            return (
                              <div key={t.id} className="px-2 py-1.5">
                                <div className="flex items-center gap-2">
                                  <input
                                    type="checkbox"
                                    checked={enhanceIds.includes(t.id)}
                                    onChange={() => toggleEnhance(t.id)}
                                    disabled={running}
                                    className="rounded border-gray-300 text-primary-600 focus:ring-primary-500 flex-shrink-0"
                                  />
                                  <button
                                    type="button"
                                    onClick={() => setPreviewId(open ? null : t.id)}
                                    className="relative w-16 h-9 flex-shrink-0 rounded overflow-hidden bg-gray-200 dark:bg-gray-700 group"
                                    title={open ? 'Hide preview' : 'Preview this take'}
                                  >
                                    {/* eslint-disable-next-line @next/next/no-img-element */}
                                    <img
                                      src={getThumbnailUrl(t.id, 0)}
                                      alt=""
                                      className="absolute inset-0 w-full h-full object-cover"
                                      onError={(e) => ((e.currentTarget as HTMLImageElement).style.visibility = 'hidden')}
                                    />
                                    <span className="absolute inset-0 flex items-center justify-center bg-black/30 text-white text-xs opacity-0 group-hover:opacity-100 transition-opacity">
                                      {open ? '✕' : '▶'}
                                    </span>
                                  </button>
                                  <button
                                    type="button"
                                    onClick={() => toggleEnhance(t.id)}
                                    disabled={running}
                                    className="text-sm text-gray-700 dark:text-gray-300 truncate flex-1 text-left"
                                  >
                                    {t.filename}
                                  </button>
                                  {t.duration ? (
                                    <span className="text-xs text-gray-400 dark:text-gray-500 flex-shrink-0">
                                      {fmtClock(t.duration)}
                                    </span>
                                  ) : null}
                                  <button
                                    type="button"
                                    onClick={() => setPreviewId(open ? null : t.id)}
                                    className="text-xs text-primary-600 dark:text-primary-400 hover:underline flex-shrink-0"
                                  >
                                    {open ? 'Hide' : 'Preview'}
                                  </button>
                                </div>
                                {open && (
                                  <video
                                    key={t.id}
                                    src={getStreamUrl(t.id)}
                                    controls
                                    preload="metadata"
                                    className="mt-2 w-full max-h-64 rounded bg-black"
                                  />
                                )}
                              </div>
                            );
                          })}
                        </div>
                        {enhanceIds.length > 0 && (
                          <div className="mt-3 flex items-center gap-3">
                            <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-44">
                              Intensity: {Math.round(enhanceIntensity * 100)}%
                            </span>
                            <input
                              type="range"
                              min={0}
                              max={1}
                              step={0.05}
                              value={enhanceIntensity}
                              onChange={(e) => setEnhanceIntensity(parseFloat(e.target.value))}
                              disabled={running}
                              className="flex-1"
                            />
                          </div>
                        )}
                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                          Higher intensity removes more noise but can start to sound processed.
                          Applies only to the checked takes; needs no extra API keys.
                        </p>
                      </>
                    )}
                  </div>

                  {/* Music */}
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Background music (optional)
                    </label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={musicPath}
                        onChange={(e) => setMusicPath(e.target.value)}
                        disabled={running}
                        className="input flex-1 text-sm"
                        placeholder="No music"
                      />
                      <button
                        type="button"
                        onClick={handleBrowseMusic}
                        disabled={running}
                        className="btn btn-secondary text-sm whitespace-nowrap"
                      >
                        🎵 Browse
                      </button>
                    </div>
                    {musicPath.trim() && (
                      <div className="mt-3 space-y-2">
                        <div className="flex items-center gap-3">
                          <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-44">
                            When no one&apos;s talking: {Math.round(musicVolume * 100)}%
                          </span>
                          <input
                            type="range"
                            min={0}
                            max={1}
                            step={0.05}
                            value={musicVolume}
                            onChange={(e) => {
                              const v = parseFloat(e.target.value);
                              setMusicVolume(v);
                              if (musicDuckVolume > v) setMusicDuckVolume(v);
                            }}
                            disabled={running}
                            className="flex-1"
                          />
                        </div>
                        <div className="flex items-center gap-3">
                          <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-44">
                            While talking: {Math.round(musicDuckVolume * 100)}%
                          </span>
                          <input
                            type="range"
                            min={0}
                            max={musicVolume}
                            step={0.01}
                            value={musicDuckVolume}
                            onChange={(e) => setMusicDuckVolume(parseFloat(e.target.value))}
                            disabled={running}
                            className="flex-1"
                          />
                          <button
                            type="button"
                            onClick={() => setMusicPath('')}
                            disabled={running}
                            className="text-xs text-gray-500 hover:text-red-600 dark:text-gray-400"
                          >
                            clear
                          </button>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                            Only bring music back in pauses longer than
                          </span>
                          <input
                            type="number"
                            min={0.3}
                            max={10}
                            step={0.1}
                            value={musicMinGap}
                            onChange={(e) =>
                              setMusicMinGap(Math.max(0.3, Math.min(10, parseFloat(e.target.value) || 0.3)))
                            }
                            disabled={running}
                            className="input w-16 text-sm py-1"
                          />
                          <span className="text-xs text-gray-500 dark:text-gray-400">s</span>
                        </div>
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                          The music plays at the first level in pauses and drops to the second level
                          while you&apos;re speaking. Short thinking pauses (under the value above) stay
                          ducked so the music doesn&apos;t pop in mid-sentence.
                        </p>
                      </div>
                    )}
                  </div>

                  <div className="flex items-center justify-between gap-3 pt-1">
                    <p className="text-xs text-gray-500 dark:text-gray-400">
                      Can take a few minutes depending on the number and length of takes.
                    </p>
                    <button onClick={handleRun} disabled={running} className="btn btn-primary text-sm whitespace-nowrap">
                      {running ? 'Running…' : 'Run pipeline'}
                    </button>
                  </div>
                </div>
              ) : (
                /* ---------- History detail ---------- */
                selected && (
                  <EditDetail
                    edit={selected}
                    onDeleted={() => loadHistory(production.id, true)}
                    onRerender={handleRerender}
                    busy={running}
                    takeDurations={takeDurations}
                  />
                )
              )}
            </main>
          </div>
        </div>
      </div>
    </div>
  );
}

function EditDetail({
  edit,
  onDeleted,
  onRerender,
  busy,
  takeDurations,
}: {
  edit: ProductionEdit;
  onDeleted: () => void;
  onRerender: (editId: number, edits: TimelineEdits) => void;
  busy: boolean;
  takeDurations: Record<number, number>;
}) {
  const [scriptOpen, setScriptOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const clips = edit.edl?.clips ?? [];

  // YouTube copy (titles / description / tags / thumbnail text)
  const [copy, setCopy] = useState<YoutubeCopy | null>(edit.copy ?? null);
  const [copyLoading, setCopyLoading] = useState(false);
  const [copyErr, setCopyErr] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  // Thumbnail builder
  const [thumbOpen, setThumbOpen] = useState(false);
  const thumbDuration =
    edit.edl?.timeline?.duration && edit.edl.timeline.duration > 0
      ? edit.edl.timeline.duration
      : 60;

  useEffect(() => {
    setCopy(edit.copy ?? null);
    setCopyErr(null);
    setCopiedKey(null);
  }, [edit.id, edit.copy]);

  const toClipboard = (key: string, text: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopiedKey(key);
        setTimeout(() => setCopiedKey(null), 1500);
      })
      .catch(() => {});
  };

  const handleGenerateCopy = async (regenerate: boolean) => {
    setCopyLoading(true);
    setCopyErr(null);
    try {
      setCopy(await generateEditCopy(edit.id, regenerate));
    } catch (e: any) {
      setCopyErr(e.message || 'Failed to generate copy');
    } finally {
      setCopyLoading(false);
    }
  };

  const handleDelete = async () => {
    if (
      !confirm(
        'Delete this run? This permanently removes its database entry and deletes the video and JSON from disk.'
      )
    )
      return;
    setDeleting(true);
    try {
      await deleteEdit(edit.id);
      onDeleted();
    } catch (e: any) {
      alert(e.message || 'Failed to delete run');
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h3 className="font-semibold text-gray-900 dark:text-white">
            {edit.status === 'completed' ? 'Final cut' : 'Run'} · {fmtDate(edit.created_at)}
          </h3>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {clips.length} clip{clips.length !== 1 ? 's' : ''}
            {edit.edl?.captions ? ' · captions' : ''}
            {edit.edl?.music ? ` · music: ${edit.edl.music}` : ''}
          </p>
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          {edit.status === 'completed' && edit.output_path && (
            <button
              onClick={() => revealEditFile(edit.id).catch(() => {})}
              className="btn btn-secondary text-xs whitespace-nowrap"
            >
              📁 Reveal final video
            </button>
          )}
          <button
            onClick={handleDelete}
            disabled={deleting}
            className="btn btn-secondary text-xs whitespace-nowrap text-red-600 dark:text-red-400"
          >
            {deleting ? 'Deleting…' : '🗑 Delete'}
          </button>
        </div>
      </div>

      {edit.status === 'failed' && edit.error && (
        <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p className="text-sm text-red-600 dark:text-red-400 whitespace-pre-wrap">{edit.error}</p>
        </div>
      )}

      {edit.output_path && (
        <p className="text-xs text-gray-500 dark:text-gray-400 break-all">Output: {edit.output_path}</p>
      )}

      {/* Editor-style timeline preview (interactive for completed runs) */}
      {edit.edl?.timeline && edit.edl.timeline.clips.length > 0 && (
        <EditTimeline
          timeline={edit.edl.timeline}
          videoSrc={
            edit.status === 'completed' && edit.output_path ? getEditVideoUrl(edit.id) : undefined
          }
          onRerender={
            edit.status === 'completed' ? (edits) => onRerender(edit.id, edits) : undefined
          }
          busy={busy}
          takeDurations={takeDurations}
          editId={edit.status === 'completed' ? edit.id : undefined}
        />
      )}

      {/* YouTube copy (completed runs) */}
      {edit.status === 'completed' && (
        <div className="card">
          <div className="flex items-center justify-between gap-3 mb-2">
            <div>
              <h3 className="font-semibold text-gray-900 dark:text-white">YouTube copy</h3>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Title options, description, tags &amp; thumbnail text from the final cut&apos;s transcript.
              </p>
            </div>
            <button
              onClick={() => handleGenerateCopy(!!copy)}
              disabled={copyLoading}
              className="btn btn-primary text-sm whitespace-nowrap"
            >
              {copyLoading ? 'Generating…' : copy ? 'Regenerate' : 'Generate copy'}
            </button>
          </div>

          {copyErr && !copyLoading && (
            <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <p className="text-sm text-red-600 dark:text-red-400">{copyErr}</p>
            </div>
          )}

          {!copy && !copyLoading && !copyErr && (
            <p className="text-sm text-gray-500 dark:text-gray-400 italic">
              No copy yet. Click “Generate copy” for SEO titles, a description, tags and thumbnail
              text ideas.
            </p>
          )}

          {copy && !copyLoading && (
            <div className="space-y-4">
              {copy.titles.length > 0 && (
                <div>
                  <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Title options</span>
                  <ul className="mt-2 space-y-2">
                    {copy.titles.map((t, i) => (
                      <li key={i} className="flex items-center justify-between gap-2 bg-gray-50 dark:bg-gray-700/50 rounded px-3 py-2">
                        <span className="text-sm text-gray-900 dark:text-white">{t}</span>
                        <button
                          onClick={() => toClipboard(`title-${i}`, t)}
                          className="text-xs text-primary-600 dark:text-primary-400 hover:underline whitespace-nowrap"
                        >
                          {copiedKey === `title-${i}` ? 'Copied!' : 'Copy'}
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {copy.description && (
                <div>
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Description</span>
                    <button
                      onClick={() => toClipboard('desc', copy.description)}
                      className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                    >
                      {copiedKey === 'desc' ? 'Copied!' : 'Copy'}
                    </button>
                  </div>
                  <p className="text-sm text-gray-900 dark:text-white whitespace-pre-wrap bg-gray-50 dark:bg-gray-700/50 rounded px-3 py-2">
                    {copy.description}
                  </p>
                </div>
              )}

              {copy.thumbnail_texts.length > 0 && (
                <div>
                  <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Thumbnail text ideas</span>
                  <div className="flex flex-wrap gap-2 mt-2">
                    {copy.thumbnail_texts.map((t, i) => (
                      <button
                        key={i}
                        onClick={() => toClipboard(`thumb-${i}`, t)}
                        className="badge badge-primary text-xs"
                        title="Click to copy"
                      >
                        {copiedKey === `thumb-${i}` ? 'Copied!' : t}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {copy.tags.length > 0 && (
                <div>
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Tags</span>
                    <button
                      onClick={() => toClipboard('tags', copy.tags.join(', '))}
                      className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                    >
                      {copiedKey === 'tags' ? 'Copied!' : 'Copy all'}
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {copy.tags.map((t, i) => (
                      <span key={i} className="badge bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200 text-xs">
                        {t}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Thumbnail builder (completed runs) */}
      {edit.status === 'completed' && edit.output_path && (
        <div className="card">
          <div className="flex items-center justify-between gap-3 mb-2">
            <div>
              <h3 className="font-semibold text-gray-900 dark:text-white">
                Thumbnail
                {edit.thumbnail && (
                  <span className="ml-2 text-xs font-normal text-green-600 dark:text-green-400">
                    ✓ saved
                  </span>
                )}
              </h3>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Grab a real frame from the final video and lay stylized text on top. Drag to position,
                pick alignment, and ✨ AI-style the text or restyle the frame. Saved thumbnails reopen
                ready to edit.
              </p>
            </div>
            <button
              onClick={() => setThumbOpen((o) => !o)}
              className="btn btn-secondary text-sm whitespace-nowrap"
            >
              {thumbOpen ? 'Close' : edit.thumbnail ? 'Edit thumbnail' : 'Make thumbnail'}
            </button>
          </div>
          {thumbOpen && (
            <ThumbnailEditor
              editId={edit.id}
              duration={thumbDuration}
              suggestedTexts={copy?.thumbnail_texts ?? []}
              saved={edit.thumbnail ?? null}
              context={copy?.titles?.[0] ?? edit.script?.slice(0, 200) ?? ''}
            />
          )}
        </div>
      )}

      {/* Script (collapsible) */}
      {edit.script && (
        <div>
          <button
            onClick={() => setScriptOpen((o) => !o)}
            className="text-sm font-medium text-gray-600 dark:text-gray-400 hover:underline"
          >
            {scriptOpen ? '▼' : '▶'} Script
          </button>
          {scriptOpen && (
            <pre className="mt-2 text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap bg-gray-50 dark:bg-gray-900/40 rounded p-3 max-h-48 overflow-y-auto">
              {edit.script}
            </pre>
          )}
        </div>
      )}

      {edit.instructions && (
        <div>
          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Instructions</span>
          <p className="text-sm text-gray-700 dark:text-gray-300 mt-1 whitespace-pre-wrap">{edit.instructions}</p>
        </div>
      )}

      {/* EDL */}
      {clips.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 dark:text-gray-400 border-b dark:border-gray-700">
                <th className="py-1 pr-2 font-medium">#</th>
                <th className="py-1 pr-2 font-medium">Take</th>
                <th className="py-1 pr-2 font-medium">Range</th>
                <th className="py-1 font-medium">Why</th>
              </tr>
            </thead>
            <tbody>
              {clips.map((c) => (
                <tr key={c.order} className="border-b dark:border-gray-700/50 align-top">
                  <td className="py-1.5 pr-2 text-gray-900 dark:text-white">{c.order}</td>
                  <td className="py-1.5 pr-2 text-gray-900 dark:text-white break-all">
                    {c.filename}
                    {c.enhanced && (
                      <span
                        className="ml-1.5 inline-block text-[10px] px-1.5 py-0.5 rounded bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300 align-middle whitespace-nowrap"
                        title="Voice enhanced (background noise removed)"
                      >
                        🎙 enhanced
                      </span>
                    )}
                  </td>
                  <td className="py-1.5 pr-2 text-gray-600 dark:text-gray-300 whitespace-nowrap">
                    {c.start.toFixed(2)}s – {c.end.toFixed(2)}s
                  </td>
                  <td className="py-1.5 text-gray-600 dark:text-gray-400">{c.reason}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Activity log */}
      {edit.logs && edit.logs.length > 0 && (
        <details className="text-xs">
          <summary className="cursor-pointer text-gray-500 dark:text-gray-400">Activity log</summary>
          <div className="mt-2 max-h-48 overflow-y-auto font-mono text-[11px] leading-relaxed text-gray-600 dark:text-gray-300 bg-gray-50 dark:bg-gray-900/40 rounded p-2">
            {edit.logs.map((line, i) => (
              <div key={i}>{line}</div>
            ))}
          </div>
        </details>
      )}

      {edit.text_model && (
        <p className="text-xs text-gray-400 dark:text-gray-500">
          Planned by {edit.text_provider}/{edit.text_model} · transcribed with {edit.transcription_provider}
        </p>
      )}
    </div>
  );
}
