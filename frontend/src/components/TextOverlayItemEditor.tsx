'use client';

import { useState } from 'react';
import type { TextOverlaySpec, ThumbnailTextStyle } from '@/lib/types';
import { DEFAULT_TEXT_STYLE } from '@/lib/styledText';
import { generateEditTextStyle } from '@/lib/api';

type Align = 'left' | 'center' | 'right';

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

function fmt(s: number): string {
  const m = Math.floor(s / 60);
  const sec = s - m * 60;
  return `${m}:${sec.toFixed(1).padStart(4, '0')}`;
}

interface TextOverlayItemEditorProps {
  spec: TextOverlaySpec;
  start: number;
  duration: number;
  /** Edited-timeline length (seconds), for clamping the timing. */
  dur: number;
  /** Current playhead time on the edited timeline (for "start here"). */
  playhead: number;
  editId?: number;
  context?: string;
  busy?: boolean;
  selected: boolean;
  onSelect: () => void;
  onSpecChange: (spec: TextOverlaySpec) => void;
  onTimingChange: (start: number, duration: number) => void;
  onRemove: () => void;
}

/** Inline editor for one text overlay: content, style (reusing the thumbnail
 *  text treatment), free X/Y position, and its on-screen timing window. */
export default function TextOverlayItemEditor({
  spec,
  start,
  duration,
  dur,
  playhead,
  editId,
  context,
  busy,
  selected,
  onSelect,
  onSpecChange,
  onTimingChange,
  onRemove,
}: TextOverlayItemEditorProps) {
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiPrompt, setAiPrompt] = useState('');
  const [aiOpen, setAiOpen] = useState(false);

  const style = spec.style;
  const patch = (p: Partial<TextOverlaySpec>) => onSpecChange({ ...spec, ...p });
  const patchStyle = (p: Partial<ThumbnailTextStyle>) =>
    onSpecChange({ ...spec, style: { ...style, ...p } });

  const end = start + duration;

  const runAiStyle = async () => {
    if (!editId) return;
    setAiBusy(true);
    setAiError(null);
    try {
      const s = await generateEditTextStyle(editId, {
        text: spec.text,
        context,
        prompt: aiPrompt || undefined,
      });
      const merged: ThumbnailTextStyle = { ...DEFAULT_TEXT_STYLE, ...s };
      // Keep the "Text" color control and the renderer consistent: a highlight
      // band carries only its bg color; the intended text color becomes fill.
      if (merged.highlight?.textColor) merged.fill = merged.highlight.textColor;
      onSpecChange({ ...spec, style: merged });
    } catch (e: any) {
      setAiError(e?.message || 'AI styling failed');
    } finally {
      setAiBusy(false);
    }
  };

  const AlignBtn = ({ a, label }: { a: Align; label: string }) => (
    <button
      type="button"
      onClick={() => patch({ align: a })}
      disabled={busy}
      className={`px-1.5 py-0.5 rounded text-[10px] border ${
        spec.align === a
          ? 'bg-primary-500 text-white border-primary-500'
          : 'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300'
      }`}
    >
      {label}
    </button>
  );

  return (
    <div
      onMouseDown={onSelect}
      className={`rounded border px-2 py-1.5 ${
        selected
          ? 'border-primary-400 ring-1 ring-primary-300 dark:ring-primary-700'
          : 'border-gray-200 dark:border-gray-700'
      }`}
    >
      {/* Text + remove */}
      <div className="flex items-center gap-2">
        <span className="text-[11px]">🅣</span>
        <input
          value={spec.text}
          onChange={(e) => patch({ text: e.target.value })}
          onFocus={onSelect}
          disabled={busy}
          placeholder="Overlay text"
          className="input text-[11px] py-0.5 px-1 flex-1"
        />
        <button
          type="button"
          onClick={onRemove}
          disabled={busy}
          className="text-[10px] text-gray-500 hover:text-red-600 dark:text-gray-400"
        >
          remove
        </button>
      </div>

      {/* Style row 1: size, outline, caps, align */}
      <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1">
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Size
          <input
            type="range"
            min={32}
            max={220}
            step={2}
            value={spec.fontSize}
            onChange={(e) => patch({ fontSize: parseFloat(e.target.value) })}
            disabled={busy}
            className="w-20"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Edge
          <input
            type="range"
            min={0}
            max={30}
            step={1}
            value={style.outlineWidth}
            onChange={(e) => patchStyle({ outlineWidth: parseFloat(e.target.value) })}
            disabled={busy}
            className="w-16"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          <input
            type="checkbox"
            checked={spec.uppercase}
            onChange={(e) => patch({ uppercase: e.target.checked })}
            disabled={busy}
          />
          CAPS
        </label>
        <div className="flex items-center gap-1">
          <AlignBtn a="left" label="⯇" />
          <AlignBtn a="center" label="≡" />
          <AlignBtn a="right" label="⯈" />
        </div>
      </div>

      {/* Style row 2: colors, band, shadow */}
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Text
          <input
            type="color"
            value={style.fill}
            onChange={(e) => patchStyle({ fill: e.target.value, gradient: null })}
            disabled={busy}
            className="h-5 w-6 rounded border border-gray-300 dark:border-gray-600 bg-transparent"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Edge
          <input
            type="color"
            value={style.outlineColor}
            onChange={(e) => patchStyle({ outlineColor: e.target.value })}
            disabled={busy}
            className="h-5 w-6 rounded border border-gray-300 dark:border-gray-600 bg-transparent"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          <input
            type="checkbox"
            checked={!!style.highlight}
            onChange={(e) =>
              patchStyle({
                highlight: e.target.checked
                  ? { color: '#facc15', textColor: style.fill }
                  : null,
              })
            }
            disabled={busy}
          />
          Band
        </label>
        {style.highlight && (
          <input
            type="color"
            value={style.highlight.color}
            onChange={(e) =>
              patchStyle({ highlight: { ...style.highlight!, color: e.target.value } })
            }
            disabled={busy}
            className="h-5 w-6 rounded border border-gray-300 dark:border-gray-600 bg-transparent"
          />
        )}
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          <input
            type="checkbox"
            checked={style.shadowBlur > 0}
            onChange={(e) =>
              patchStyle(
                e.target.checked
                  ? { shadowBlur: 18, shadowOffsetY: 6 }
                  : { shadowBlur: 0, shadowOffsetY: 0 }
              )
            }
            disabled={busy}
          />
          Shadow
        </label>
      </div>

      {/* Position row: free X/Y */}
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          X
          <input
            type="range"
            min={0.02}
            max={0.98}
            step={0.01}
            value={spec.posX}
            onChange={(e) => patch({ posX: parseFloat(e.target.value) })}
            disabled={busy}
            className="w-20"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Y
          <input
            type="range"
            min={0.03}
            max={0.97}
            step={0.01}
            value={spec.posY}
            onChange={(e) => patch({ posY: parseFloat(e.target.value) })}
            disabled={busy}
            className="w-20"
          />
        </label>
        <span className="text-[9px] text-gray-400 dark:text-gray-500">
          or drag the text on the preview
        </span>
      </div>

      {/* Timing row */}
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          Start
          <input
            type="number"
            min={0}
            max={dur}
            step={0.1}
            value={Number(start.toFixed(2))}
            onChange={(e) => {
              const s = clamp(parseFloat(e.target.value) || 0, 0, Math.max(0, dur - 0.2));
              onTimingChange(s, Math.min(duration, dur - s));
            }}
            disabled={busy}
            className="input text-[10px] py-0.5 px-1 w-16"
          />
        </label>
        <label className="flex items-center gap-1 text-[10px] text-gray-600 dark:text-gray-300">
          End
          <input
            type="number"
            min={0}
            max={dur}
            step={0.1}
            value={Number(end.toFixed(2))}
            onChange={(e) => {
              const en = clamp(parseFloat(e.target.value) || 0, start + 0.2, dur);
              onTimingChange(start, en - start);
            }}
            disabled={busy}
            className="input text-[10px] py-0.5 px-1 w-16"
          />
        </label>
        <span className="text-[9px] text-gray-400 dark:text-gray-500 tabular-nums">
          {fmt(start)}–{fmt(end)}
        </span>
        <button
          type="button"
          onClick={() =>
            onTimingChange(clamp(playhead, 0, Math.max(0, dur - 0.2)), duration)
          }
          disabled={busy}
          className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300 hover:bg-gray-200"
          title="Move the start to the current playhead position"
        >
          ⤓ at playhead
        </button>
      </div>

      {/* AI style */}
      {editId && (
        <div className="mt-1">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setAiOpen((o) => !o)}
              disabled={busy || aiBusy}
              className="text-[10px] px-1.5 py-0.5 rounded bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300 hover:bg-purple-200 disabled:opacity-40"
            >
              ✨ AI style
            </button>
            {aiOpen && (
              <>
                <input
                  value={aiPrompt}
                  onChange={(e) => setAiPrompt(e.target.value)}
                  disabled={busy || aiBusy}
                  placeholder="optional: e.g. bold red with a yellow band"
                  className="input text-[10px] py-0.5 px-1 flex-1"
                />
                <button
                  type="button"
                  onClick={runAiStyle}
                  disabled={busy || aiBusy || !spec.text.trim()}
                  className="text-[10px] px-1.5 py-0.5 rounded btn-primary disabled:opacity-40"
                >
                  {aiBusy ? '…' : 'Go'}
                </button>
              </>
            )}
          </div>
          {aiError && (
            <p className="text-[10px] text-red-600 dark:text-red-400 mt-0.5">{aiError}</p>
          )}
        </div>
      )}
    </div>
  );
}
