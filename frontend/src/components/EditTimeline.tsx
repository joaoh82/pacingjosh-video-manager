'use client';

import type { ReactNode } from 'react';
import { EditTimeline as Timeline } from '@/lib/types';
import { getThumbnailUrl } from '@/lib/api';

function fmtTime(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

/** Merge speech intervals and return them sorted, clamped to [0, duration]. */
function mergeSpeech(speech: { start: number; end: number }[], duration: number) {
  const sorted = speech
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

/** Partition [0,duration] into talking / not-talking regions for the music bar. */
function musicRegions(speech: { start: number; end: number }[], duration: number) {
  const merged = mergeSpeech(speech, duration);
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

function makeTicks(duration: number, count = 6): number[] {
  if (duration <= 0) return [0];
  return Array.from({ length: count + 1 }, (_, i) => (duration * i) / count);
}

export default function EditTimeline({ timeline }: { timeline: Timeline }) {
  const dur = timeline.duration > 0 ? timeline.duration : 1;
  const left = (t: number) => `${Math.max(0, Math.min(100, (t / dur) * 100))}%`;
  const width = (a: number, b: number) => `${Math.max(0, Math.min(100, ((b - a) / dur) * 100))}%`;

  const music = timeline.music;
  const full = Math.max(0, Math.min(1, music.full_volume));
  const duck = Math.max(0, Math.min(full, music.duck_volume));
  const duckFrac = full > 0 ? duck / full : 0;
  const regions = musicRegions(timeline.speech, dur);

  const Label = ({ children }: { children: ReactNode }) => (
    <div className="w-14 flex-shrink-0 text-[10px] font-medium text-gray-500 dark:text-gray-400 flex items-center pr-2 justify-end">
      {children}
    </div>
  );

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-2 bg-gray-50 dark:bg-gray-900/40 overflow-hidden">
      {/* Time ruler */}
      <div className="flex">
        <div className="w-14 flex-shrink-0" />
        <div className="relative flex-1 h-4">
          {makeTicks(dur).map((t, i) => (
            <span
              key={i}
              className="absolute top-0 text-[9px] text-gray-400 dark:text-gray-500 -translate-x-1/2 whitespace-nowrap"
              style={{ left: left(t) }}
            >
              {fmtTime(t)}
            </span>
          ))}
        </div>
      </div>

      {/* Video track */}
      <div className="flex items-stretch mt-1">
        <Label>Video</Label>
        <div className="relative flex-1 h-14 bg-gray-200/60 dark:bg-gray-800 rounded">
          {timeline.clips.map((c) => (
            <div
              key={c.order}
              className="absolute top-0 bottom-0 overflow-hidden border-r border-white/70 dark:border-black/40 bg-gray-300 dark:bg-gray-700"
              style={{ left: left(c.start), width: width(c.start, c.end) }}
              title={`#${c.order} ${c.filename}\nfinal ${fmtTime(c.start)}–${fmtTime(c.end)}\nsource ${c.source_start.toFixed(2)}s–${c.source_end.toFixed(2)}s`}
            >
              <img
                src={getThumbnailUrl(c.video_id, 0)}
                alt=""
                className="absolute inset-0 w-full h-full object-cover opacity-90"
                onError={(e) => {
                  (e.currentTarget as HTMLImageElement).style.display = 'none';
                }}
              />
              <span className="absolute bottom-0 left-0 right-0 px-1 text-[9px] leading-tight text-white bg-black/50 truncate">
                {c.filename}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Voice track */}
      <div className="flex items-stretch mt-1">
        <Label>Voice</Label>
        <div className="relative flex-1 h-4 bg-gray-200/60 dark:bg-gray-800 rounded">
          {timeline.speech.map((s, i) => (
            <div
              key={i}
              className="absolute top-0.5 bottom-0.5 rounded-sm bg-teal-500/80"
              style={{ left: left(s.start), width: width(s.start, s.end) }}
              title={`speech ${fmtTime(s.start)}–${fmtTime(s.end)}`}
            />
          ))}
        </div>
      </div>

      {/* Music track with volume envelope */}
      {music.present && (
        <div className="flex items-stretch mt-1">
          <Label>Music</Label>
          <div className="relative flex-1 h-12 bg-gray-200/60 dark:bg-gray-800 rounded overflow-hidden">
            {regions.map((r, i) => (
              <div
                key={i}
                className={`absolute bottom-0 ${r.talking ? 'bg-emerald-400/50' : 'bg-emerald-500/80'}`}
                style={{
                  left: left(r.start),
                  width: width(r.start, r.end),
                  height: `${(r.talking ? duckFrac : 1) * 100}%`,
                }}
                title={
                  r.talking
                    ? `talking → music ${Math.round(duck * 100)}%`
                    : `no talking → music ${Math.round(full * 100)}%`
                }
              />
            ))}
            <span className="absolute top-0.5 left-1 right-1 text-[9px] text-gray-700 dark:text-gray-200 truncate pointer-events-none">
              {music.name || 'music'} · {Math.round(full * 100)}% in pauses → {Math.round(duck * 100)}% while talking
            </span>
          </div>
        </div>
      )}

      <p className="text-[10px] text-gray-400 dark:text-gray-500 mt-1 pl-14">
        Total {fmtTime(dur)} · {timeline.clips.length} clip{timeline.clips.length !== 1 ? 's' : ''}. The
        music bar drops to the ducked level wherever the voice track shows speech.
      </p>
    </div>
  );
}
