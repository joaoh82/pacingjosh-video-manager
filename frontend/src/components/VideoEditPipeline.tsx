'use client';

import { useState, useEffect, useRef } from 'react';
import { Production, EditJobStatus, ProductionEdit, EditDecisionList } from '@/lib/types';
import {
  startProductionEdit,
  getEditStatus,
  getProductionEdit,
  revealEditOutput,
} from '@/lib/api';

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
  completed: 'Completed',
  failed: 'Failed',
};

export default function VideoEditPipeline({
  isOpen,
  production,
  onClose,
}: VideoEditPipelineProps) {
  const [script, setScript] = useState('');
  const [instructions, setInstructions] = useState('');
  const [jobId, setJobId] = useState<string | null>(null);
  const [status, setStatus] = useState<EditJobStatus | null>(null);
  const [latest, setLatest] = useState<ProductionEdit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const running = status?.status === 'in_progress' || starting;

  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      setError(null);
      setJobId(null);
      setStatus(null);
      setLatest(null);
      if (production) {
        getProductionEdit(production.id)
          .then((e) => {
            setLatest(e);
            if (e?.instructions) setInstructions(e.instructions);
          })
          .catch(() => setLatest(null));
      }
    } else {
      document.body.style.overflow = 'unset';
    }
    return () => {
      document.body.style.overflow = 'unset';
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, production]);

  // Poll job progress until it reaches a terminal state.
  useEffect(() => {
    if (!jobId) return;
    let cancelled = false;

    const tick = async () => {
      try {
        const s = await getEditStatus(jobId);
        if (cancelled) return;
        setStatus(s);
        if (s.status === 'completed' || s.status === 'failed') {
          if (pollRef.current) clearInterval(pollRef.current);
          pollRef.current = null;
          if (s.status === 'completed' && production) {
            getProductionEdit(production.id).then(setLatest).catch(() => {});
          }
        }
      } catch {
        // Transient error — keep polling.
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
  }, [jobId, production]);

  if (!isOpen || !production) return null;

  const handleRun = async () => {
    if (!script.trim()) {
      setError('Paste the script for this video first.');
      return;
    }
    setStarting(true);
    setError(null);
    setStatus(null);
    try {
      const res = await startProductionEdit(production.id, {
        script,
        instructions: instructions.trim() || undefined,
      });
      setJobId(res.job_id);
    } catch (e: any) {
      setError(e.message || 'Failed to start the pipeline');
    } finally {
      setStarting(false);
    }
  };

  const handleReveal = async () => {
    try {
      await revealEditOutput(production.id);
    } catch {
      // ignore
    }
  };

  const pct =
    status && status.total > 0
      ? Math.min(100, Math.round((status.processed / status.total) * 100))
      : 0;

  // The EDL + output to display: prefer the just-finished job, fall back to the
  // last persisted edit.
  const edl: EditDecisionList | null | undefined = status?.edl ?? latest?.edl;
  const outputPath = status?.output_path ?? latest?.output_path;
  const showResult =
    (status?.status === 'completed' || (!status && latest?.status === 'completed')) &&
    !!edl;

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto">
      <div className="fixed inset-0 bg-black bg-opacity-50 transition-opacity" onClick={onClose} />

      <div className="flex min-h-full items-center justify-center p-4">
        <div className="relative w-full max-w-3xl bg-white dark:bg-gray-800 rounded-lg shadow-xl overflow-hidden">
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

          <div className="p-4 space-y-4 max-h-[75vh] overflow-y-auto">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Transcribes every take in this production, asks the LLM to assemble the best cut
              from your script (newest clean takes, re-shoots in timeline order, warm-up
              “Hey&nbsp;…” intros trimmed), writes an edit decision list, then stitches the final
              clip with ffmpeg. Configure API keys and prompts under{' '}
              <span className="font-medium">Settings → AI / LLM</span>.
            </p>

            {/* Script + instructions form */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Script <span className="text-red-500">*</span>
              </label>
              <textarea
                value={script}
                onChange={(e) => setScript(e.target.value)}
                rows={8}
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
                rows={3}
                disabled={running}
                className="input text-sm"
                placeholder="e.g. I warm up by saying “Hey Sarah” — cut that; and I re-shot scene 1 at the very end."
              />
            </div>

            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-gray-500 dark:text-gray-400">
                This can take a few minutes depending on the number and length of takes.
              </p>
              <button onClick={handleRun} disabled={running} className="btn btn-primary text-sm whitespace-nowrap">
                {running ? 'Running…' : showResult ? 'Run again' : 'Run pipeline'}
              </button>
            </div>

            {error && (
              <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
              </div>
            )}

            {/* Live progress */}
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
                      <div
                        className="bg-primary-600 h-2 transition-all"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                      {status.processed} / {status.total}
                    </p>
                  </div>
                )}

                {status.status === 'failed' && status.error && (
                  <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                    <p className="text-sm text-red-600 dark:text-red-400 whitespace-pre-wrap">
                      {status.error}
                    </p>
                  </div>
                )}

                {status.logs && status.logs.length > 0 && (
                  <details className="text-xs">
                    <summary className="cursor-pointer text-gray-500 dark:text-gray-400">
                      Activity log
                    </summary>
                    <div className="mt-2 max-h-40 overflow-y-auto font-mono text-[11px] leading-relaxed text-gray-600 dark:text-gray-300 bg-gray-50 dark:bg-gray-900/40 rounded p-2">
                      {status.logs.map((line, i) => (
                        <div key={i}>{line}</div>
                      ))}
                    </div>
                  </details>
                )}
              </div>
            )}

            {/* Result: the edit decision list + final video */}
            {showResult && edl && (
              <div className="card space-y-3">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="font-semibold text-gray-900 dark:text-white">
                    Final cut · {edl.clips.length} clip{edl.clips.length !== 1 ? 's' : ''}
                  </h3>
                  {outputPath && (
                    <button onClick={handleReveal} className="btn btn-secondary text-xs whitespace-nowrap">
                      📁 Reveal final video
                    </button>
                  )}
                </div>

                {outputPath && (
                  <p className="text-xs text-gray-500 dark:text-gray-400 break-all">{outputPath}</p>
                )}

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
                      {edl.clips.map((c) => (
                        <tr key={c.order} className="border-b dark:border-gray-700/50 align-top">
                          <td className="py-1.5 pr-2 text-gray-900 dark:text-white">{c.order}</td>
                          <td className="py-1.5 pr-2 text-gray-900 dark:text-white break-all">
                            {c.filename}
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

                {edl.text_model && (
                  <p className="text-xs text-gray-400 dark:text-gray-500">
                    Planned by {edl.text_provider}/{edl.text_model} · transcribed with{' '}
                    {edl.transcription_provider}
                  </p>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
