'use client';

import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { EditTimeline as Timeline, TimelineSpeech } from '@/lib/types';
import { getThumbnailUrl } from '@/lib/api';

function fmtTime(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

function mergeIntervals(xs: TimelineSpeech[], duration: number) {
  const sorted = xs
    .map((s) => ({ start: Math.max(0, s.start), end: Math.min(duration, s.end) }))
    .filter((s) => s.end > s.start)
    .sort((a, b) => a.start - b.start);
  const out: { start: number; end: number }[] = [];
  for (const s of sorted) {
    const last = out[out.length - 1];
    if (last && s.start <= last.end) last.end = Math.max(last.end, s.end);
    else out.push({ ...s });
  }
  return out;
}

/** Partition [0,duration] into talking (ducked) / not-talking (music on) regions. */
function regionsFrom(duck: TimelineSpeech[], duration: number) {
  const merged = mergeIntervals(duck, duration);
  const regions: { start: number; end: number; talking: boolean }[] = [];
  let cursor = 0;
  for (const s of merged) {
    if (s.start > cursor) regions.push({ start: cursor, end: s.start, talking: false });
    regions.push({ start: s.start, end: s.end, talking: true });
    cursor = s.end;
  }
  if (cursor < duration) regions.push({ start: cursor, end: duration, talking: false });
  return regions;
}

/** Pending timeline edits collected for a re-render. */
export interface TimelineEdits {
  /** Music regions (seconds) to remove. */
  mute: { start: number; end: number }[];
  /** Clip `order`s to apply voice enhancement to. */
  enhanceClips: number[];
}

interface EditTimelineProps {
  timeline: Timeline;
  /** Re-render the run with these timeline edits (undefined disables editing). */
  onRerender?: (edits: TimelineEdits) => void;
  busy?: boolean;
}

export default function EditTimeline({ timeline, onRerender, busy }: EditTimelineProps) {
  const dur = timeline.duration > 0 ? timeline.duration : 1;
  const wrapRef = useRef<HTMLDivElement>(null);
  const [containerW, setContainerW] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [muted, setMuted] = useState<{ start: number; end: number }[]>([]);
  // Clip `order`s the user selected to enhance on the next re-render.
  const [selectedClips, setSelectedClips] = useState<number[]>([]);

  // Reset selections when the underlying run changes.
  useEffect(() => {
    setMuted([]);
    setSelectedClips([]);
  }, [timeline]);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const measure = () => setContainerW(el.clientWidth);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const baseW = containerW > 0 ? containerW : 800;
  const pxPerSec = (baseW / dur) * zoom;
  const innerW = Math.max(baseW, dur * pxPerSec);
  const X = (t: number) => t * pxPerSec;
  const Wd = (a: number, b: number) => Math.max(0, (b - a) * pxPerSec);

  const music = timeline.music;
  const full = Math.max(0, Math.min(1, music.full_volume));
  const duck = Math.max(0, Math.min(full, music.duck_volume));
  const duckFrac = full > 0 ? duck / full : 0;
  const duckIntervals = timeline.duck && timeline.duck.length ? timeline.duck : timeline.speech;
  const regions = regionsFrom(duckIntervals, dur);

  const editable = !!onRerender && !busy;
  const musicEditable = editable && music.present;

  const isMuted = (r: { start: number; end: number }) =>
    muted.some((m) => Math.abs(m.start - r.start) < 0.01 && Math.abs(m.end - r.end) < 0.01);
  const toggleMute = (r: { start: number; end: number }) => {
    if (!musicEditable) return;
    setMuted((prev) =>
      isMuted(r)
        ? prev.filter((m) => !(Math.abs(m.start - r.start) < 0.01 && Math.abs(m.end - r.end) < 0.01))
        : [...prev, { start: r.start, end: r.end }]
    );
  };

  const isSelected = (order: number) => selectedClips.includes(order);
  const toggleSelectClip = (order: number) => {
    if (!editable) return;
    setSelectedClips((prev) =>
      prev.includes(order) ? prev.filter((o) => o !== order) : [...prev, order]
    );
  };

  const ticks = Array.from({ length: 9 }, (_, i) => (dur * i) / 8);
  const Label = ({ h, children }: { h: number; children: ReactNode }) => (
    <div
      style={{ height: h }}
      className="flex items-center justify-end pr-2 text-[10px] font-medium text-gray-500 dark:text-gray-400"
    >
      {children}
    </div>
  );

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-2 bg-gray-50 dark:bg-gray-900/40">
      {/* Toolbar */}
      <div className="flex items-center gap-2 mb-2">
        <span className="text-[10px] text-gray-500 dark:text-gray-400">Zoom</span>
        <button
          type="button"
          onClick={() => setZoom((z) => Math.max(1, +(z - 1).toFixed(1)))}
          className="px-2 leading-none text-sm border border-gray-300 dark:border-gray-600 rounded"
        >
          −
        </button>
        <input
          type="range"
          min={1}
          max={20}
          step={0.5}
          value={zoom}
          onChange={(e) => setZoom(parseFloat(e.target.value))}
          className="w-32"
        />
        <button
          type="button"
          onClick={() => setZoom((z) => Math.min(20, +(z + 1).toFixed(1)))}
          className="px-2 leading-none text-sm border border-gray-300 dark:border-gray-600 rounded"
        >
          +
        </button>
        <span className="text-[10px] text-gray-400 dark:text-gray-500">{zoom.toFixed(1)}×</span>
        <div className="flex-1" />
        {onRerender &&
          (() => {
            const nEnh = selectedClips.length;
            const nMute = muted.length;
            const has = nEnh + nMute > 0;
            const label = busy
              ? 'Re-rendering…'
              : nEnh && nMute
              ? `Re-render · ${nEnh} enhanced, ${nMute} music`
              : nEnh
              ? `✨ Enhance voice — re-render (${nEnh})`
              : nMute
              ? `Re-render without ${nMute} music clip${nMute !== 1 ? 's' : ''}`
              : 'Re-render';
            return (
              <button
                type="button"
                onClick={() => onRerender({ mute: muted, enhanceClips: selectedClips })}
                disabled={busy || !has}
                className="btn btn-primary text-xs whitespace-nowrap disabled:opacity-40"
                title="Re-render a new version with your timeline edits (enhance selected clips / remove selected music)"
              >
                {label}
              </button>
            );
          })()}
      </div>

      <div className="flex gap-2">
        {/* Fixed label gutter */}
        <div className="flex flex-col gap-1 flex-none w-12">
          <div style={{ height: 14 }} />
          <Label h={52}>Video</Label>
          <Label h={16}>Voice</Label>
          <Label h={44}>Music</Label>
        </div>

        {/* Scrollable tracks */}
        <div ref={wrapRef} className="overflow-x-auto flex-1">
          <div className="flex flex-col gap-1" style={{ width: innerW }}>
            {/* Ruler */}
            <div className="relative" style={{ height: 14 }}>
              {ticks.map((t, i) => (
                <span
                  key={i}
                  className="absolute top-0 text-[9px] text-gray-400 dark:text-gray-500 -translate-x-1/2 whitespace-nowrap"
                  style={{ left: X(t) }}
                >
                  {fmtTime(t)}
                </span>
              ))}
            </div>

            {/* Video — click a clip to mark it for voice enhancement */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded" style={{ height: 52 }}>
              {timeline.clips.map((c) => {
                const sel = isSelected(c.order);
                const alreadyEnhanced = !!c.enhanced;
                return (
                  <div
                    key={c.order}
                    onClick={() => toggleSelectClip(c.order)}
                    className={`absolute top-0 bottom-0 overflow-hidden bg-gray-300 dark:bg-gray-700 ${
                      editable ? 'cursor-pointer' : ''
                    } ${
                      sel
                        ? 'ring-2 ring-inset ring-purple-500 z-10'
                        : 'border-r border-white/70 dark:border-black/40'
                    }`}
                    style={{ left: X(c.start), width: Wd(c.start, c.end) }}
                    title={`#${c.order} ${c.filename}\nfinal ${fmtTime(c.start)}–${fmtTime(c.end)}\nsource ${c.source_start.toFixed(2)}s–${c.source_end.toFixed(2)}s${
                      alreadyEnhanced ? '\nvoice enhanced' : ''
                    }${editable ? '\n(click to mark for voice enhancement)' : ''}`}
                  >
                    <img
                      src={getThumbnailUrl(c.video_id, 0)}
                      alt=""
                      className="absolute inset-0 w-full h-full object-cover opacity-90"
                      onError={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')}
                    />
                    {(sel || alreadyEnhanced) && (
                      <span className="absolute top-0 left-0 px-1 text-[9px] leading-tight text-white bg-purple-600/85 rounded-br pointer-events-none">
                        {sel ? '✨' : '🎙'}
                      </span>
                    )}
                    <span className="absolute bottom-0 left-0 right-0 px-1 text-[9px] leading-tight text-white bg-black/50 truncate">
                      {c.filename}
                    </span>
                  </div>
                );
              })}
            </div>

            {/* Voice */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded" style={{ height: 16 }}>
              {timeline.speech.map((s, i) => (
                <div
                  key={i}
                  className="absolute top-0.5 bottom-0.5 rounded-sm bg-teal-500/80"
                  style={{ left: X(s.start), width: Wd(s.start, s.end) }}
                  title={`speech ${fmtTime(s.start)}–${fmtTime(s.end)}`}
                />
              ))}
            </div>

            {/* Music — click the tall (playing) bars to remove them */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded overflow-hidden" style={{ height: 44 }}>
              {regions.map((r, i) => {
                const removed = isMuted(r);
                const playing = !r.talking && !removed;
                return (
                  <div
                    key={i}
                    onClick={() => !r.talking && toggleMute(r)}
                    title={
                      r.talking
                        ? `ducked → ${Math.round(duck * 100)}%`
                        : removed
                        ? `removed (click to keep) · ${fmtTime(r.start)}–${fmtTime(r.end)}`
                        : `music ${Math.round(full * 100)}% · ${fmtTime(r.start)}–${fmtTime(r.end)} (click to remove)`
                    }
                    className={`absolute bottom-0 ${
                      playing
                        ? `bg-emerald-500/80 ${musicEditable ? 'cursor-pointer hover:bg-emerald-400' : ''}`
                        : removed
                        ? 'bg-red-400/50 cursor-pointer'
                        : 'bg-emerald-400/40'
                    }`}
                    style={{
                      left: X(r.start),
                      width: Wd(r.start, r.end),
                      height: `${(r.talking || removed ? duckFrac : 1) * 100}%`,
                    }}
                  />
                );
              })}
              <span className="absolute top-0.5 left-1 text-[9px] text-gray-700 dark:text-gray-200 truncate pointer-events-none">
                {music.name || 'music'} · {Math.round(full * 100)}% → {Math.round(duck * 100)}%
              </span>
            </div>
          </div>
        </div>
      </div>

      <p className="text-[10px] text-gray-400 dark:text-gray-500 mt-1 pl-14">
        Total {fmtTime(dur)} · {timeline.clips.length} clip{timeline.clips.length !== 1 ? 's' : ''}.
        {editable ? (
          <>
            {' '}Click a video clip to mark it for{' '}
            <span className="text-purple-600 dark:text-purple-400">✨ voice enhancement</span>
            {music.present ? ', or click the tall green music bars to remove those bursts' : ''}, then
            re-render. 🎙 = already enhanced.
          </>
        ) : (
          ' The music bar drops to the ducked level wherever the voice track shows speech.'
        )}
      </p>
    </div>
  );
}
