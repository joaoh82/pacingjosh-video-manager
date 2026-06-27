'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import {
  EditTimeline as Timeline,
  TimelineSpeech,
  TimelineClip,
  ClipEdit,
  MusicEdit,
  TimelineAiPlan,
  OverlaySpecPayload,
} from '@/lib/types';
import { getThumbnailUrl, aiEditTimeline, getBuiltinOverlays, browseImage } from '@/lib/api';

/** An overlay snippet (transparent GIF/image) being edited on the timeline
 * (re-render UI state). */
interface OverlayItem {
  path: string;
  label: string;
  scale: number;
  position: string;
  /** Legacy chroma key colour, preserved when re-rendering an older run that
   * used a keyed video overlay. Empty for normal GIF/image overlays. */
  chromaColor: string;
}

const OVERLAY_POSITIONS: { value: string; label: string }[] = [
  { value: 'center', label: 'Center' },
  { value: 'bottom', label: 'Bottom' },
  { value: 'top', label: 'Top' },
  { value: 'bottom_right', label: 'Bottom right' },
  { value: 'bottom_left', label: 'Bottom left' },
  { value: 'top_right', label: 'Top right' },
  { value: 'top_left', label: 'Top left' },
];

function overlayLabelFromPath(p: string): string {
  const base = p.replace(/\\/g, '/').split('/').pop() || p;
  return base.replace(/\.[^.]+$/, '');
}

/** Map an overlay editor item to the re-render payload (auto-placed, no start).
 * chroma_color is only sent when set (preserves a legacy keyed-video overlay). */
function overlayItemToPayload(o: OverlayItem): OverlaySpecPayload {
  return {
    path: o.path,
    label: o.label || undefined,
    scale: o.scale,
    position: o.position,
    ...(o.chromaColor ? { chroma_color: o.chromaColor } : {}),
  };
}

function fmtTime(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

function round2(v: number): number {
  return Math.round(v * 100) / 100;
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

/** Pending per-clip edit state, keyed by the clip's original `order`. */
interface ClipEditState {
  remove?: boolean;
  /** Source range start override (seconds into the take). */
  sourceStart?: number;
  /** Source range end override (seconds into the take). */
  sourceEnd?: number;
  enhance?: boolean;
}

/** Pending timeline edits collected for a re-render. */
export interface TimelineEdits {
  /** Per-clip edits (trim source range / remove / enhance). */
  clips: ClipEdit[];
  /** Music regions (seconds) to remove (duck away). */
  mute: { start: number; end: number }[];
  /** Music regions (seconds) to fade in/out. */
  fade: { start: number; end: number }[];
  /** Overlays to use on the re-render (omitted → keep the run's saved set). */
  overlays?: OverlaySpecPayload[];
}

/** A clip placed on the (edit-adjusted) timeline, with resolved geometry. */
interface LaidClip extends TimelineClip {
  /** Effective source range after pending trims. */
  ss: number;
  se: number;
  /** Position on the edit-adjusted timeline (seconds). */
  ns: number;
  ne: number;
  /** Effective voice-enhancement state. */
  enh: boolean;
}

const MIN_CLIP = 0.2; // shortest a clip can be trimmed to (seconds)

interface EditTimelineProps {
  timeline: Timeline;
  /** Re-render the run with these timeline edits (undefined disables editing). */
  onRerender?: (edits: TimelineEdits) => void;
  /** Streaming URL of the rendered video — enables the synced preview player. */
  videoSrc?: string;
  busy?: boolean;
  /** Map of take video_id → duration (seconds), for clamping clip trims. */
  takeDurations?: Record<number, number>;
  /** Edit run id — enables the AI assistant. AI disabled when absent. */
  editId?: number;
}

export default function EditTimeline({
  timeline,
  onRerender,
  videoSrc,
  busy,
  takeDurations,
  editId,
}: EditTimelineProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [containerW, setContainerW] = useState(0);
  const [zoom, setZoom] = useState(1);

  // Pending edits.
  const [clipEdits, setClipEdits] = useState<Record<number, ClipEditState>>({});
  const [musicActions, setMusicActions] = useState<Record<string, 'remove' | 'fade'>>({});
  // The music actions seeded from this run (its already-applied removes/fades),
  // so we can tell genuine new edits from the sticky baseline.
  const seedRef = useRef<Record<string, 'remove' | 'fade'>>({});
  const [selected, setSelected] = useState<number[]>([]);

  // Overlay snippets for the re-render, seeded from the run's saved overlays so
  // they stay sticky and can be added to / changed / removed.
  const [overlayItems, setOverlayItems] = useState<OverlayItem[]>([]);
  const [addingOverlay, setAddingOverlay] = useState(false);
  const [overlayError, setOverlayError] = useState<string | null>(null);
  // JSON snapshot of the seeded overlays, to detect genuine changes.
  const overlaySeedRef = useRef<string>('[]');

  // Playback position (seconds, original render) mirrored by the playhead.
  const [currentTime, setCurrentTime] = useState(0);
  const [playing, setPlaying] = useState(false);

  // Live trim drag preview (kept out of clipEdits so the rest of the timeline
  // stays frozen until release — only the dragged edge moves, then the cut
  // ripples on commit).
  const [dragPreview, setDragPreview] = useState<
    { order: number; edge: 'L' | 'R'; ss: number; se: number } | null
  >(null);

  // Music region popover menu.
  const [menu, setMenu] = useState<{ key: string; x: number; y: number } | null>(null);

  // AI assistant.
  const [aiOpen, setAiOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState('');
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiNote, setAiNote] = useState<string | null>(null);

  // Reset everything when the underlying run changes — but SEED the music
  // actions from what was persisted on this run (its muted/faded regions) so
  // earlier music edits stay applied across re-renders (sticky), shown as
  // selected and re-sent on the next render.
  useEffect(() => {
    setClipEdits({});
    setSelected([]);
    setDragPreview(null);
    setMenu(null);
    setCurrentTime(0);
    setPlaying(false);
    setAiOpen(false);
    setAiPrompt('');
    setAiError(null);
    setAiNote(null);

    const dur0 = timeline.duration > 0 ? timeline.duration : 1;
    const regions = regionsFrom(timeline.speech, dur0);
    const covers = (s: number, e: number, list?: TimelineSpeech[]) => {
      const mid = (s + e) / 2;
      return !!list?.some((m) => mid >= m.start && mid <= m.end);
    };
    const seed: Record<string, 'remove' | 'fade'> = {};
    for (const r of regions) {
      if (r.talking) continue;
      if (covers(r.start, r.end, timeline.muted)) seed[`${Math.round(r.start * 10)}_${Math.round(r.end * 10)}`] = 'remove';
      else if (covers(r.start, r.end, timeline.fades)) seed[`${Math.round(r.start * 10)}_${Math.round(r.end * 10)}`] = 'fade';
    }
    seedRef.current = seed;
    setMusicActions(seed);

    // Seed the overlay editor from the run's saved overlays (those that carry a
    // path can be re-sent). Missing path → display-only, skipped from the editor.
    const seededOverlays: OverlayItem[] = (timeline.overlays ?? [])
      .filter((o) => !!o.path)
      .map((o) => ({
        path: o.path as string,
        label: o.label || o.filename || overlayLabelFromPath(o.path as string),
        scale: o.scale ?? 1.0,
        position: o.position || 'center',
        chromaColor: o.chroma_color ?? '',
      }));
    overlaySeedRef.current = JSON.stringify(seededOverlays);
    setOverlayItems(seededOverlays);
    setOverlayError(null);
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

  const editable = !!onRerender && !busy;
  const music = timeline.music;
  const full = Math.max(0, Math.min(1, music.full_volume));
  const duck = Math.max(0, Math.min(full, music.duck_volume));
  const duckFrac = full > 0 ? duck / full : 0;
  const musicEditable = editable && music.present;

  // Overlay snippets (e.g. a Subscribe bug) placed in the pauses. Read-only on
  // the timeline — they're re-placed automatically on each re-render.
  const overlays = timeline.overlays ?? [];

  // Committed effective source range for a clip (pending edits, not the live
  // drag — the drag is overlaid only on the dragged clip so the layout holds
  // still until release).
  const effSource = (c: TimelineClip): { ss: number; se: number } => {
    const e = clipEdits[c.order];
    return {
      ss: e?.sourceStart ?? c.source_start,
      se: e?.sourceEnd ?? c.source_end,
    };
  };
  const effEnhance = (c: TimelineClip) => clipEdits[c.order]?.enhance ?? !!c.enhanced;
  const isRemoved = (order: number) => !!clipEdits[order]?.remove;
  const maxSource = (c: TimelineClip) =>
    takeDurations?.[c.video_id] ?? Math.max(c.source_end, effSource(c).se);

  // Lay the kept clips end-to-end after applying pending trims/removals.
  const laid = useMemo<LaidClip[]>(() => {
    const out: LaidClip[] = [];
    let cursor = 0;
    for (const c of timeline.clips) {
      if (clipEdits[c.order]?.remove) continue;
      const { ss, se } = effSource(c);
      const d = Math.max(0, se - ss);
      out.push({ ...c, ss, se, ns: cursor, ne: cursor + d, enh: effEnhance(c) });
      cursor += d;
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [timeline, clipEdits]);

  const editedDuration = laid.length ? laid[laid.length - 1].ne : 0;
  const dur = editedDuration > 0 ? editedDuration : 1;
  const byOrder = useMemo(() => {
    const m = new Map<number, LaidClip>();
    for (const c of laid) m.set(c.order, c);
    return m;
  }, [laid]);

  // Remap an interval list from the ORIGINAL timeline onto the edited one,
  // following each clip's surviving source range. Used for the voice/music
  // tracks so they track trims and removals.
  const remapIntervals = (intervals?: TimelineSpeech[]): { start: number; end: number }[] => {
    if (!intervals) return [];
    const out: { start: number; end: number }[] = [];
    for (const iv of intervals) {
      for (const c of timeline.clips) {
        const lay = byOrder.get(c.order);
        if (!lay) continue; // removed
        const os = Math.max(iv.start, c.start);
        const oe = Math.min(iv.end, c.end);
        if (oe <= os) continue;
        const srcS = c.source_start + (os - c.start);
        const srcE = c.source_start + (oe - c.start);
        const ks = Math.max(srcS, lay.ss);
        const ke = Math.min(srcE, lay.se);
        if (ke <= ks) continue;
        out.push({ start: lay.ns + (ks - lay.ss), end: lay.ns + (ke - lay.ss) });
      }
    }
    out.sort((a, b) => a.start - b.start);
    return out;
  };

  const editedSpeech = useMemo(
    () => remapIntervals(timeline.speech),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [timeline, laid]
  );

  // Map a time on the ORIGINAL render onto the edited timeline (for the playhead).
  const remapPoint = (tOld: number): number => {
    let cursor = 0;
    for (const c of timeline.clips) {
      const lay = byOrder.get(c.order);
      const removed = !lay;
      if (tOld <= c.end) {
        if (removed) return cursor;
        if (tOld <= c.start) return cursor;
        const srcT = c.source_start + (tOld - c.start);
        const clamped = Math.min(Math.max(srcT, lay!.ss), lay!.se);
        return cursor + (clamped - lay!.ss);
      }
      if (lay) cursor += lay.ne - lay.ns;
    }
    return cursor;
  };

  const structuralEdits = useMemo(
    () =>
      Object.values(clipEdits).some(
        (e) => e.remove || e.sourceStart !== undefined || e.sourceEnd !== undefined
      ),
    [clipEdits]
  );

  const baseW = containerW > 0 ? containerW : 800;
  const pxPerSec = (baseW / dur) * zoom;
  const innerW = Math.max(baseW, dur * pxPerSec);
  const X = (t: number) => t * pxPerSec;
  const Wd = (a: number, b: number) => Math.max(0, (b - a) * pxPerSec);

  // --- Music regions & actions -------------------------------------------------
  // Music actions are keyed by the region's position on the SAVED timeline (a
  // stable identity), so they survive clip trims/removals: each region is shown
  // at its remapped (edited) position, but its action persists under the saved
  // key. Regions are partitioned by speech (talking = ducked, not editable) and
  // overlaid with the user's remove/fade choices.
  const mk = (start: number, end: number) =>
    `${Math.round(start * 10)}_${Math.round(end * 10)}`;

  // Non-overlapping talking/not-talking regions on the SAVED timeline.
  const savedRegions = useMemo(
    () => regionsFrom(timeline.speech, timeline.duration > 0 ? timeline.duration : 1),
    [timeline]
  );

  // Each saved region, projected onto the edited timeline (es..ee). Collapsed
  // regions (inside a removed clip) get es≈ee and are skipped when rendering.
  const musicRegions = useMemo(
    () =>
      savedRegions.map((r) => ({
        talking: r.talking,
        key: mk(r.start, r.end),
        es: remapPoint(r.start),
        ee: remapPoint(r.end),
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [savedRegions, laid]
  );

  const setActionForKey = (key: string, action: 'remove' | 'fade' | null) =>
    setMusicActions((prev) => {
      const next = { ...prev };
      if (action === null) delete next[key];
      else next[key] = action;
      return next;
    });

  // --- Clip selection / edits --------------------------------------------------
  const toggleSelect = (order: number) => {
    if (!editable) return;
    setSelected((prev) =>
      prev.includes(order) ? prev.filter((o) => o !== order) : [...prev, order]
    );
  };
  const patchClip = (order: number, patch: ClipEditState) =>
    setClipEdits((prev) => ({ ...prev, [order]: { ...prev[order], ...patch } }));
  const removeSelected = () => {
    setClipEdits((prev) => {
      const next = { ...prev };
      for (const o of selected) next[o] = { ...next[o], remove: true };
      return next;
    });
    setSelected([]);
  };
  const restoreClip = (order: number) =>
    setClipEdits((prev) => {
      const next = { ...prev };
      if (next[order]) {
        const { remove, ...rest } = next[order];
        void remove;
        next[order] = rest;
      }
      return next;
    });
  const setEnhanceSelected = (on: boolean) =>
    setClipEdits((prev) => {
      const next = { ...prev };
      for (const o of selected) next[o] = { ...next[o], enhance: on };
      return next;
    });
  const resetEdits = () => {
    setClipEdits({});
    setMusicActions(seedRef.current); // back to this run's already-applied music
    setSelected([]);
    setDragPreview(null);
    setAiNote(null);
    setOverlayItems(JSON.parse(overlaySeedRef.current)); // back to saved overlays
    setOverlayError(null);
  };

  // --- Overlay editor (add a Subscribe bug / custom snippet on re-render) ------
  const addOverlayItem = (item: OverlayItem) => setOverlayItems((o) => [...o, item]);

  const handleAddSubscribe = async () => {
    setAddingOverlay(true);
    setOverlayError(null);
    try {
      const builtins = await getBuiltinOverlays();
      const sub = builtins.find((b) => b.id === 'subscribe') || builtins[0];
      if (!sub) {
        setOverlayError('No built-in overlay is available.');
        return;
      }
      addOverlayItem({
        path: sub.path,
        label: sub.label,
        scale: 1.0,
        position: 'center',
        chromaColor: '',
      });
    } catch (e: any) {
      setOverlayError(e?.message || 'Could not load the built-in Subscribe overlay.');
    } finally {
      setAddingOverlay(false);
    }
  };

  const handleAddCustomOverlay = async () => {
    try {
      const r = await browseImage();
      if (r.success && r.path) {
        addOverlayItem({
          path: r.path,
          label: overlayLabelFromPath(r.path),
          scale: 1.0,
          position: 'center',
          chromaColor: '',
        });
      }
    } catch {
      /* ignore */
    }
  };

  const updateOverlayItem = (idx: number, patch: Partial<OverlayItem>) =>
    setOverlayItems((list) => list.map((o, i) => (i === idx ? { ...o, ...patch } : o)));
  const removeOverlayItem = (idx: number) =>
    setOverlayItems((list) => list.filter((_, i) => i !== idx));

  const removedClips = timeline.clips.filter((c) => isRemoved(c.order));
  const selectedClips = selected.map((o) => byOrder.get(o)).filter(Boolean) as LaidClip[];
  const singleSel = selectedClips.length === 1 ? selectedClips[0] : null;

  // --- Build the re-render payload ---------------------------------------------
  const buildEdits = (): TimelineEdits => {
    const clips: ClipEdit[] = [];
    for (const c of timeline.clips) {
      const e = clipEdits[c.order];
      if (!e) continue;
      const entry: ClipEdit = { order: c.order };
      let changed = false;
      if (e.remove) {
        entry.remove = true;
        changed = true;
      } else {
        if (e.sourceStart !== undefined && Math.abs(e.sourceStart - c.source_start) > 0.005) {
          entry.source_start = round2(e.sourceStart);
          changed = true;
        }
        if (e.sourceEnd !== undefined && Math.abs(e.sourceEnd - c.source_end) > 0.005) {
          entry.source_end = round2(e.sourceEnd);
          changed = true;
        }
      }
      if (e.enhance) {
        entry.enhance = true;
        changed = true;
      }
      if (changed) clips.push(entry);
    }
    // Music actions are sent in EDITED (final) coordinates — the position each
    // region maps to after the clip edits — so they line up with the cut the
    // backend rebuilds. Regions collapsed by a clip removal are skipped.
    const mute: { start: number; end: number }[] = [];
    const fade: { start: number; end: number }[] = [];
    for (const r of musicRegions) {
      if (r.talking) continue;
      const a = musicActions[r.key];
      if (!a || r.ee - r.es <= 0.05) continue;
      if (a === 'remove') mute.push({ start: round2(r.es), end: round2(r.ee) });
      else fade.push({ start: round2(r.es), end: round2(r.ee) });
    }
    // Only send overlays when the user actually changed them; otherwise the
    // backend keeps the run's saved set (so a clip/music-only re-render doesn't
    // need to re-send overlays).
    const overlays = overlaysChanged ? overlayItems.map(overlayItemToPayload) : undefined;
    return { clips, mute, fade, overlays };
  };

  const overlaysChanged = useMemo(
    () => JSON.stringify(overlayItems) !== overlaySeedRef.current,
    [overlayItems]
  );

  const pendingEdits = buildEdits();
  const nClipEdits = pendingEdits.clips.length;
  const nMute = pendingEdits.mute.length;
  const nFade = pendingEdits.fade.length;
  // Music is "changed" only relative to what this run already had applied (the
  // seeded baseline), so reopening a run with sticky mutes doesn't look like an
  // unsaved edit. The re-render still SENDS all active mutes/fades to keep them.
  const musicChanged = useMemo(() => {
    const seed = seedRef.current;
    const cur = Object.keys(musicActions);
    const base = Object.keys(seed);
    return cur.length !== base.length || cur.some((k) => musicActions[k] !== seed[k]);
  }, [musicActions]);
  const hasEdits = nClipEdits > 0 || musicChanged || overlaysChanged;

  // --- AI assistant ------------------------------------------------------------
  const applyPlan = (plan: TimelineAiPlan) => {
    setClipEdits((prev) => {
      const next = { ...prev };
      for (const c of plan.clips ?? []) {
        const cur: ClipEditState = { ...next[c.order] };
        if (c.remove) cur.remove = true;
        if (c.source_start !== undefined) cur.sourceStart = c.source_start;
        if (c.source_end !== undefined) cur.sourceEnd = c.source_end;
        if (c.enhance) cur.enhance = true;
        next[c.order] = cur;
      }
      return next;
    });
    // Music regions come back in SAVED-timeline coords (the AI reads the saved
    // cut), which is the same frame as our saved-region keys — tag any region
    // whose midpoint the AI's range covers.
    const incoming = (plan.music ?? []) as MusicEdit[];
    if (incoming.length) {
      setMusicActions((prev) => {
        const next = { ...prev };
        for (const m of incoming) {
          for (const r of savedRegions) {
            if (r.talking) continue;
            const mid = (r.start + r.end) / 2;
            if (mid >= m.start && mid <= m.end) next[mk(r.start, r.end)] = m.action;
          }
        }
        return next;
      });
    }
  };

  const runAi = async () => {
    if (!editId || !aiPrompt.trim()) return;
    setAiBusy(true);
    setAiError(null);
    setAiNote(null);
    try {
      const plan = await aiEditTimeline(editId, aiPrompt.trim(), selected);
      applyPlan(plan);
      const n = (plan.clips?.length ?? 0) + (plan.music?.length ?? 0);
      if (n === 0) {
        setAiNote(
          plan.explanation
            ? `${plan.explanation} (no changes applied)`
            : 'The AI returned no changes for that request.'
        );
      } else {
        setAiNote(plan.explanation || `Applied ${n} change${n !== 1 ? 's' : ''}. Review, then re-render.`);
        setAiPrompt('');
      }
    } catch (e) {
      setAiError((e as Error).message || 'AI edit failed');
    } finally {
      setAiBusy(false);
    }
  };

  // --- Preview player ↔ playhead sync -----------------------------------------
  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    let raf = 0;
    const sync = () => {
      const t = v.currentTime;
      setCurrentTime((prev) => (Math.abs(prev - t) > 0.03 ? t : prev));
    };
    const loop = () => {
      sync();
      raf = requestAnimationFrame(loop);
    };
    const onPlay = () => {
      setPlaying(true);
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(loop);
    };
    const onStop = () => {
      setPlaying(false);
      cancelAnimationFrame(raf);
      sync();
    };
    v.addEventListener('play', onPlay);
    v.addEventListener('playing', onPlay);
    v.addEventListener('pause', onStop);
    v.addEventListener('ended', onStop);
    v.addEventListener('seeked', sync);
    v.addEventListener('timeupdate', sync);
    return () => {
      cancelAnimationFrame(raf);
      v.removeEventListener('play', onPlay);
      v.removeEventListener('playing', onPlay);
      v.removeEventListener('pause', onStop);
      v.removeEventListener('ended', onStop);
      v.removeEventListener('seeked', sync);
      v.removeEventListener('timeupdate', sync);
    };
  }, [videoSrc]);

  const seekTo = (t: number) => {
    const clamped = Math.max(0, Math.min(dur, t));
    setCurrentTime(clamped);
    const v = videoRef.current;
    if (v) v.currentTime = clamped;
  };

  // Scrub by clicking or dragging on the ruler.
  const rulerRef = useRef<HTMLDivElement>(null);
  const scrubbingRef = useRef(false);
  const seekFromClientX = (clientX: number) => {
    const el = rulerRef.current;
    if (!el) return;
    const x = clientX - el.getBoundingClientRect().left;
    seekTo(x / pxPerSec);
  };
  useEffect(() => {
    if (!videoSrc) return;
    const move = (e: MouseEvent) => {
      if (scrubbingRef.current) seekFromClientX(e.clientX);
    };
    const up = () => {
      scrubbingRef.current = false;
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [videoSrc, pxPerSec]);

  // Keep the playhead in view while playing on a zoomed-in timeline.
  useEffect(() => {
    if (!playing) return;
    const el = wrapRef.current;
    if (!el) return;
    const x = remapPoint(currentTime) * pxPerSec;
    if (x < el.scrollLeft + 24 || x > el.scrollLeft + el.clientWidth - 24) {
      el.scrollLeft = Math.max(0, x - el.clientWidth / 2);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentTime, playing]);

  // --- Trim drag handles -------------------------------------------------------
  const dragRef = useRef<
    { order: number; edge: 'L' | 'R'; startX: number; ss0: number; se0: number; max: number } | null
  >(null);
  const startTrim = (clip: LaidClip, edge: 'L' | 'R', clientX: number) => {
    if (!editable) return;
    dragRef.current = {
      order: clip.order,
      edge,
      startX: clientX,
      ss0: clip.ss,
      se0: clip.se,
      max: maxSource(clip),
    };
    setDragPreview({ order: clip.order, edge, ss: clip.ss, se: clip.se });
  };
  useEffect(() => {
    const move = (e: MouseEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const delta = (e.clientX - d.startX) / pxPerSec;
      if (d.edge === 'R') {
        const se = Math.min(d.max, Math.max(d.ss0 + MIN_CLIP, d.se0 + delta));
        setDragPreview({ order: d.order, edge: 'R', ss: d.ss0, se });
      } else {
        const ss = Math.max(0, Math.min(d.se0 - MIN_CLIP, d.ss0 + delta));
        setDragPreview({ order: d.order, edge: 'L', ss, se: d.se0 });
      }
    };
    const up = () => {
      const d = dragRef.current;
      if (d) {
        setDragPreview((p) => {
          if (p && p.order === d.order) {
            patchClip(d.order, { sourceStart: p.ss, sourceEnd: p.se });
          }
          return null;
        });
        dragRef.current = null;
      }
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pxPerSec]);

  const ticks = Array.from({ length: 9 }, (_, i) => (dur * i) / 8);
  const Label = ({ h, children }: { h: number; children: ReactNode }) => (
    <div
      style={{ height: h }}
      className="flex items-center justify-end pr-2 text-[10px] font-medium text-gray-500 dark:text-gray-400"
    >
      {children}
    </div>
  );

  const rerenderLabel = busy
    ? 'Re-rendering…'
    : (() => {
        const parts: string[] = [];
        if (nClipEdits) parts.push(`${nClipEdits} clip${nClipEdits !== 1 ? 's' : ''}`);
        if (nMute) parts.push(`${nMute} muted`);
        if (nFade) parts.push(`${nFade} faded`);
        if (overlaysChanged) parts.push('overlays');
        return parts.length ? `Re-render · ${parts.join(', ')}` : 'Re-render';
      })();

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-2 bg-gray-50 dark:bg-gray-900/40">
      {/* Preview player — its position drives the timeline playhead */}
      {videoSrc && (
        <video
          key={videoSrc}
          ref={videoRef}
          src={videoSrc}
          controls
          preload="metadata"
          className="w-full max-h-[360px] bg-black rounded mb-2"
        />
      )}

      {/* Toolbar */}
      <div className="flex items-center gap-2 mb-2 flex-wrap">
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
          className="w-28"
        />
        <button
          type="button"
          onClick={() => setZoom((z) => Math.min(20, +(z + 1).toFixed(1)))}
          className="px-2 leading-none text-sm border border-gray-300 dark:border-gray-600 rounded"
        >
          +
        </button>
        <span className="text-[10px] text-gray-400 dark:text-gray-500">{zoom.toFixed(1)}×</span>
        {videoSrc && (
          <span className="text-[10px] tabular-nums text-gray-500 dark:text-gray-400 ml-2">
            {fmtTime(remapPoint(currentTime))} / {fmtTime(dur)}
          </span>
        )}
        <div className="flex-1" />
        {editable && editId && (
          <button
            type="button"
            onClick={() => setAiOpen((o) => !o)}
            className="btn btn-secondary text-xs whitespace-nowrap"
            title="Ask AI to edit the timeline for you"
          >
            ✨ Ask AI
          </button>
        )}
        {editable && hasEdits && (
          <button
            type="button"
            onClick={resetEdits}
            className="text-xs text-gray-500 hover:text-red-600 dark:text-gray-400 whitespace-nowrap"
          >
            Reset edits
          </button>
        )}
        {onRerender && (
          <button
            type="button"
            onClick={() => onRerender(pendingEdits)}
            disabled={busy || !hasEdits}
            className="btn btn-primary text-xs whitespace-nowrap disabled:opacity-40"
            title="Re-render a new version with your timeline edits"
          >
            {rerenderLabel}
          </button>
        )}
      </div>

      {/* AI assistant panel */}
      {editable && editId && aiOpen && (
        <div className="mb-2 p-2 rounded border border-purple-200 dark:border-purple-900/50 bg-purple-50/60 dark:bg-purple-900/10">
          <div className="flex items-start gap-2">
            <textarea
              value={aiPrompt}
              onChange={(e) => setAiPrompt(e.target.value)}
              rows={2}
              disabled={aiBusy}
              className="input text-xs flex-1"
              placeholder={
                selected.length
                  ? `Tell the AI what to do with the ${selected.length} selected clip${
                      selected.length !== 1 ? 's' : ''
                    } (e.g. "trim the long pause at the end", "drop clip ${selected[0]}", "fade the music out at the end")`
                  : 'e.g. "shorten clip 2 to just the key sentence", "remove the rambling intro", "fade the music in at the start and out at the end"'
              }
            />
            <button
              type="button"
              onClick={runAi}
              disabled={aiBusy || !aiPrompt.trim()}
              className="btn btn-primary text-xs whitespace-nowrap disabled:opacity-40 self-stretch"
            >
              {aiBusy ? 'Thinking…' : 'Apply'}
            </button>
          </div>
          {selected.length > 0 && (
            <p className="text-[10px] text-purple-700 dark:text-purple-300 mt-1">
              Focusing on clip{selected.length !== 1 ? 's' : ''} {selected.join(', ')}.
            </p>
          )}
          {aiError && <p className="text-[11px] text-red-600 dark:text-red-400 mt-1">{aiError}</p>}
          {aiNote && !aiError && (
            <p className="text-[11px] text-gray-600 dark:text-gray-300 mt-1">{aiNote}</p>
          )}
        </div>
      )}

      {/* Selected-clip inspector */}
      {editable && selectedClips.length > 0 && (
        <div className="mb-2 p-2 rounded border border-primary-200 dark:border-primary-900/50 bg-primary-50/60 dark:bg-primary-900/10 text-xs">
          {singleSel ? (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <span className="font-medium text-gray-800 dark:text-gray-100 truncate max-w-[260px]">
                #{singleSel.order} {singleSel.filename}
              </span>
              <label className="flex items-center gap-1 text-gray-600 dark:text-gray-300">
                Start
                <input
                  type="number"
                  step={0.1}
                  min={0}
                  max={singleSel.se - MIN_CLIP}
                  value={round2(singleSel.ss)}
                  onChange={(e) => {
                    const v = Math.max(0, Math.min(singleSel.se - MIN_CLIP, parseFloat(e.target.value) || 0));
                    patchClip(singleSel.order, { sourceStart: v });
                  }}
                  className="input w-20 py-1 text-xs"
                />
                s
              </label>
              <label className="flex items-center gap-1 text-gray-600 dark:text-gray-300">
                End
                <input
                  type="number"
                  step={0.1}
                  min={singleSel.ss + MIN_CLIP}
                  max={maxSource(singleSel)}
                  value={round2(singleSel.se)}
                  onChange={(e) => {
                    const v = Math.min(
                      maxSource(singleSel),
                      Math.max(singleSel.ss + MIN_CLIP, parseFloat(e.target.value) || 0)
                    );
                    patchClip(singleSel.order, { sourceEnd: v });
                  }}
                  className="input w-20 py-1 text-xs"
                />
                s
              </label>
              <span className="text-gray-500 dark:text-gray-400">
                = {fmtTime(Math.max(0, singleSel.se - singleSel.ss))} on the cut
              </span>
            </div>
          ) : (
            <span className="font-medium text-gray-800 dark:text-gray-100">
              {selectedClips.length} clips selected
            </span>
          )}
          <div className="flex flex-wrap items-center gap-2 mt-2">
            <button
              type="button"
              onClick={removeSelected}
              className="btn btn-secondary text-xs text-red-600 dark:text-red-400"
            >
              🗑 Remove from cut
            </button>
            <button
              type="button"
              onClick={() => setEnhanceSelected(!selectedClips.every((c) => c.enh))}
              className="btn btn-secondary text-xs"
            >
              {selectedClips.every((c) => c.enh) ? 'Undo voice enhance' : '✨ Enhance voice'}
            </button>
            {singleSel && (clipEdits[singleSel.order]?.sourceStart !== undefined ||
              clipEdits[singleSel.order]?.sourceEnd !== undefined) && (
              <button
                type="button"
                onClick={() =>
                  patchClip(singleSel.order, { sourceStart: undefined, sourceEnd: undefined })
                }
                className="text-xs text-gray-500 hover:underline"
              >
                Reset trim
              </button>
            )}
            <button
              type="button"
              onClick={() => setSelected([])}
              className="text-xs text-gray-500 hover:underline ml-auto"
            >
              Clear selection
            </button>
          </div>
        </div>
      )}

      <div className="flex gap-2">
        {/* Fixed label gutter */}
        <div className="flex flex-col gap-1 flex-none w-12">
          <div style={{ height: 16 }} />
          <Label h={52}>Video</Label>
          {overlays.length > 0 && <Label h={18}>Overlays</Label>}
          <Label h={16}>Voice</Label>
          <Label h={44}>Music</Label>
        </div>

        {/* Scrollable tracks */}
        <div ref={wrapRef} className="overflow-x-auto flex-1">
          <div className="relative flex flex-col gap-1" style={{ width: innerW }}>
            {/* Playhead */}
            {videoSrc && (
              <div
                className="absolute top-0 bottom-0 z-20 pointer-events-none"
                style={{ left: X(remapPoint(currentTime)) }}
              >
                <div className="absolute inset-y-0 w-px bg-red-500" />
                <div className="absolute -top-0.5 -left-[4px] w-[9px] h-[9px] rounded-sm bg-red-500" />
              </div>
            )}

            {/* Ruler */}
            <div
              ref={rulerRef}
              onMouseDown={
                videoSrc
                  ? (e) => {
                      scrubbingRef.current = true;
                      seekFromClientX(e.clientX);
                    }
                  : undefined
              }
              className={`relative ${videoSrc ? 'cursor-pointer' : ''}`}
              style={{ height: 16 }}
            >
              {ticks.map((t, i) => (
                <span
                  key={i}
                  className="absolute top-0 text-[9px] text-gray-400 dark:text-gray-500 -translate-x-1/2 whitespace-nowrap pointer-events-none"
                  style={{ left: X(t) }}
                >
                  {fmtTime(t)}
                </span>
              ))}
            </div>

            {/* Video — click a clip to select it; drag the edges to trim */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded" style={{ height: 52 }}>
              {laid.map((c) => {
                const sel = selected.includes(c.order);
                // While dragging this clip's edge, move only that edge under the
                // cursor; the laid-out positions (and everything after) hold
                // still until the drag commits and the cut ripples.
                let leftSec = c.ns;
                let widthSec = c.ne - c.ns;
                if (dragPreview && dragPreview.order === c.order) {
                  if (dragPreview.edge === 'R') {
                    widthSec = Math.max(0, dragPreview.se - c.ss);
                  } else {
                    widthSec = Math.max(0, c.se - dragPreview.ss);
                    leftSec = c.ns + (dragPreview.ss - c.ss);
                  }
                }
                const left = X(leftSec);
                const width = X(widthSec);
                return (
                  <div
                    key={c.order}
                    onClick={() => toggleSelect(c.order)}
                    className={`absolute top-0 bottom-0 overflow-hidden bg-gray-300 dark:bg-gray-700 ${
                      editable ? 'cursor-pointer' : ''
                    } ${
                      sel ? 'z-10' : 'border-r border-white/70 dark:border-black/40'
                    }`}
                    style={{ left, width }}
                    title={`#${c.order} ${c.filename}\nsource ${c.ss.toFixed(2)}s–${c.se.toFixed(2)}s${
                      c.enh ? '\nvoice enhanced' : ''
                    }${editable ? '\n(click to select; drag the edges to trim)' : ''}`}
                  >
                    <img
                      src={getThumbnailUrl(c.video_id, 0)}
                      alt=""
                      className="absolute inset-0 w-full h-full object-cover opacity-90"
                      onError={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')}
                    />
                    {c.enh && (
                      <span className="absolute top-0 left-0 px-1 text-[9px] leading-tight text-white bg-purple-600/85 rounded-br pointer-events-none">
                        🎙
                      </span>
                    )}
                    <span className="absolute bottom-0 left-0 right-0 px-1 text-[9px] leading-tight text-white bg-black/50 truncate">
                      {c.filename}
                    </span>
                    {/* Trim handles (single selection) */}
                    {sel && editable && selected.length === 1 && (
                      <>
                        <div
                          onMouseDown={(e) => {
                            e.stopPropagation();
                            startTrim(c, 'L', e.clientX);
                          }}
                          onClick={(e) => e.stopPropagation()}
                          className="absolute left-0 top-0 bottom-0 w-2 bg-primary-500/90 cursor-ew-resize z-20"
                          title="Drag to trim the start"
                        />
                        <div
                          onMouseDown={(e) => {
                            e.stopPropagation();
                            startTrim(c, 'R', e.clientX);
                          }}
                          onClick={(e) => e.stopPropagation()}
                          className="absolute right-0 top-0 bottom-0 w-2 bg-primary-500/90 cursor-ew-resize z-20"
                          title="Drag to trim the end"
                        />
                      </>
                    )}
                    {/* Selection highlight — drawn ABOVE the thumbnail so the
                        blue frame is visible (an inset ring would sit behind the
                        full-bleed <img>). */}
                    {sel && (
                      <span className="absolute inset-0 border-2 border-primary-500 bg-primary-500/20 pointer-events-none z-30" />
                    )}
                  </div>
                );
              })}
              {laid.length === 0 && (
                <span className="absolute inset-0 flex items-center justify-center text-[11px] text-gray-500 dark:text-gray-400">
                  All clips removed — restore one below or reset edits.
                </span>
              )}
            </div>

            {/* Overlays — read-only markers for snippets dropped into the pauses */}
            {overlays.length > 0 && (
              <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded overflow-hidden" style={{ height: 18 }}>
                {overlays.map((o, i) => {
                  const name = o.label || o.filename || 'overlay';
                  const subscribe = /subscrib/i.test(name);
                  return (
                    <div
                      key={i}
                      className="absolute top-0.5 bottom-0.5 rounded-sm bg-indigo-500/80 border border-indigo-300/60 flex items-center px-1 overflow-hidden pointer-events-auto"
                      style={{ left: X(o.start), width: Math.max(8, Wd(o.start, o.end)) }}
                      title={`${name} · ${fmtTime(o.start)}–${fmtTime(o.end)} · ${o.position} (auto-placed in a pause; re-placed on re-render)`}
                    >
                      <span className="text-[9px] leading-none text-white truncate">
                        {subscribe ? '🔔' : '▶'} {name}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}

            {/* Voice */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded" style={{ height: 16 }}>
              {editedSpeech.map((s, i) => (
                <div
                  key={i}
                  className="absolute top-0.5 bottom-0.5 rounded-sm bg-teal-500/80"
                  style={{ left: X(s.start), width: Wd(s.start, s.end) }}
                  title={`speech ${fmtTime(s.start)}–${fmtTime(s.end)}`}
                />
              ))}
            </div>

            {/* Music — click a tall (playing) bar to remove / fade it */}
            <div className="relative bg-gray-200/60 dark:bg-gray-800 rounded overflow-hidden" style={{ height: 44 }}>
              {musicRegions.map((r, i) => {
                if (r.ee - r.es <= 0.01) return null; // collapsed by a clip removal
                const action = r.talking ? undefined : musicActions[r.key];
                const removed = action === 'remove';
                const faded = action === 'fade';
                const playing = !r.talking && !removed;
                const cls = r.talking
                  ? 'bg-emerald-400/40'
                  : removed
                  ? 'bg-red-400/50 cursor-pointer'
                  : faded
                  ? 'bg-amber-400/70 cursor-pointer'
                  : `bg-emerald-500/80 ${musicEditable ? 'cursor-pointer hover:bg-emerald-400' : ''}`;
                return (
                  <div
                    key={i}
                    onClick={(e) => {
                      if (r.talking || !musicEditable) return;
                      setMenu({ key: r.key, x: e.clientX, y: e.clientY });
                    }}
                    title={
                      r.talking
                        ? `ducked → ${Math.round(duck * 100)}%`
                        : removed
                        ? `removed · ${fmtTime(r.es)}–${fmtTime(r.ee)} (click to change)`
                        : faded
                        ? `fade in/out · ${fmtTime(r.es)}–${fmtTime(r.ee)} (click to change)`
                        : `music ${Math.round(full * 100)}% · ${fmtTime(r.es)}–${fmtTime(r.ee)} (click for options)`
                    }
                    className={`absolute bottom-0 ${cls}`}
                    style={{
                      left: X(r.es),
                      width: Wd(r.es, r.ee),
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

      {/* Overlay editor — add a Subscribe bug / custom snippet on re-render */}
      {editable && (
        <div className="mt-3 pl-14">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-[10px] font-medium text-gray-500 dark:text-gray-400">
              Overlays:
            </span>
            <button
              type="button"
              onClick={handleAddSubscribe}
              disabled={busy || addingOverlay}
              className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300 hover:bg-indigo-200 disabled:opacity-40"
            >
              🔔 Add Subscribe
            </button>
            <button
              type="button"
              onClick={handleAddCustomOverlay}
              disabled={busy}
              className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300 hover:bg-gray-200 disabled:opacity-40"
            >
              ➕ Add image/GIF…
            </button>
            <span className="text-[10px] text-gray-400 dark:text-gray-500">
              dropped into the longest pause; re-render to apply
            </span>
          </div>
          {overlayError && (
            <p className="text-[10px] text-red-600 dark:text-red-400 mt-1">{overlayError}</p>
          )}
          {overlayItems.length > 0 && (
            <div className="mt-2 space-y-1.5">
              {overlayItems.map((o, idx) => (
                <div
                  key={idx}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded border border-gray-200 dark:border-gray-700 px-2 py-1.5"
                >
                  <span className="text-[11px] text-gray-700 dark:text-gray-200 truncate max-w-[180px]" title={o.path}>
                    {/subscrib/i.test(o.label) ? '🔔' : '▶'} {o.label || overlayLabelFromPath(o.path)}
                  </span>
                  <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
                    Pos
                    <select
                      value={o.position}
                      onChange={(e) => updateOverlayItem(idx, { position: e.target.value })}
                      disabled={busy}
                      className="input text-[10px] py-0.5 px-1"
                    >
                      {OVERLAY_POSITIONS.map((p) => (
                        <option key={p.value} value={p.value}>
                          {p.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
                    Size {Math.round(o.scale * 100)}%
                    <input
                      type="range"
                      min={0.2}
                      max={1}
                      step={0.05}
                      value={o.scale}
                      onChange={(e) => updateOverlayItem(idx, { scale: parseFloat(e.target.value) })}
                      disabled={busy}
                      className="w-20"
                    />
                  </label>
                  <button
                    type="button"
                    onClick={() => removeOverlayItem(idx)}
                    disabled={busy}
                    className="text-[10px] text-gray-500 hover:text-red-600 dark:text-gray-400 ml-auto"
                  >
                    remove
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Removed clips */}
      {removedClips.length > 0 && (
        <div className="flex items-center gap-2 mt-2 flex-wrap pl-14">
          <span className="text-[10px] text-gray-500 dark:text-gray-400">
            Removed ({removedClips.length}):
          </span>
          {removedClips.map((c) => (
            <button
              key={c.order}
              type="button"
              onClick={() => restoreClip(c.order)}
              className="text-[10px] px-1.5 py-0.5 rounded bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300 line-through hover:no-underline"
              title="Click to restore this clip"
            >
              #{c.order} {c.filename} ↺
            </button>
          ))}
        </div>
      )}

      <p className="text-[10px] text-gray-400 dark:text-gray-500 mt-1 pl-14">
        Total {fmtTime(dur)} · {laid.length} clip{laid.length !== 1 ? 's' : ''}
        {overlays.length > 0
          ? ` · ${overlays.length} overlay${overlays.length !== 1 ? 's' : ''} (auto-placed in pauses)`
          : ''}
        .
        {videoSrc ? ' Play the preview above or click/drag the ruler to scrub.' : ''}
        {editable ? (
          <>
            {' '}Click a clip to select it (then trim its edges, remove it, or enhance it); click the
            green music bars to remove or fade them; or use{' '}
            <span className="text-purple-600 dark:text-purple-400">✨ Ask AI</span>. Then re-render.
            {structuralEdits && videoSrc
              ? ' The preview shows the last render — re-render to see your trims/removals.'
              : ''}
          </>
        ) : (
          ' The music bar drops to the ducked level wherever the voice track shows speech.'
        )}
      </p>

      {/* Music region popover menu */}
      {menu && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setMenu(null)} />
          <div
            className="fixed z-50 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg py-1 text-xs"
            style={{ left: Math.min(menu.x, (typeof window !== 'undefined' ? window.innerWidth : 9999) - 160), top: menu.y + 6 }}
          >
            <button
              type="button"
              className="block w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
              onClick={() => {
                setActionForKey(menu.key, 'remove');
                setMenu(null);
              }}
            >
              🔇 Remove music here
            </button>
            <button
              type="button"
              className="block w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
              onClick={() => {
                setActionForKey(menu.key, 'fade');
                setMenu(null);
              }}
            >
              🎚 Fade in / out
            </button>
            {musicActions[menu.key] && (
              <button
                type="button"
                className="block w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-500"
                onClick={() => {
                  setActionForKey(menu.key, null);
                  setMenu(null);
                }}
              >
                ↺ Keep music as-is
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
